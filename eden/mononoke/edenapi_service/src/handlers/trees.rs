/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use anyhow::Context;
use anyhow::Error;
use async_trait::async_trait;
use bytes::Bytes;
use context::PerfCounterType;
use edenapi_types::wire::WireTreeRequest;
use edenapi_types::AnyId;
use edenapi_types::Batch;
use edenapi_types::EdenApiServerError;
use edenapi_types::FileAuxData;
use edenapi_types::TreeAttributes;
use edenapi_types::TreeChildEntry;
use edenapi_types::TreeEntry;
use edenapi_types::TreeRequest;
use edenapi_types::UploadToken;
use edenapi_types::UploadTreeRequest;
use edenapi_types::UploadTreeResponse;
use futures::stream;
use futures::Future;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use gotham::state::FromState;
use gotham::state::State;
use gotham_derive::StateData;
use gotham_derive::StaticResponseExtender;
use gotham_ext::error::HttpError;
use gotham_ext::middleware::request_context::RequestContext;
use gotham_ext::middleware::scuba::ScubaMiddlewareState;
use gotham_ext::response::TryIntoResponse;
use manifest::Entry;
use manifest::Manifest;
use mercurial_types::HgFileNodeId;
use mercurial_types::HgManifestId;
use mercurial_types::HgNodeHash;
use mononoke_api_hg::HgDataContext;
use mononoke_api_hg::HgDataId;
use mononoke_api_hg::HgRepoContext;
use mononoke_api_hg::HgTreeContext;
use rate_limiting::Metric;
use serde::Deserialize;
use types::Key;
use types::RepoPathBuf;

use super::handler::EdenApiContext;
use super::EdenApiHandler;
use super::EdenApiMethod;
use super::HandlerInfo;
use super::HandlerResult;
use crate::context::ServerContext;
use crate::errors::ErrorKind;
use crate::middleware::request_dumper::RequestDumper;
use crate::utils::custom_cbor_stream;
use crate::utils::get_repo;
use crate::utils::parse_wire_request;

// The size is optimized for the batching settings in EdenFs.
const MAX_CONCURRENT_TREE_FETCHES_PER_REQUEST: usize = 128;
const MAX_CONCURRENT_METADATA_FETCHES_PER_TREE_FETCH: usize = 100;
const MAX_CONCURRENT_UPLOAD_TREES_PER_REQUEST: usize = 100;
const LARGE_TREE_METADATA_LIMIT: usize = 25000;

#[derive(Debug, Deserialize, StateData, StaticResponseExtender)]
pub struct TreeParams {
    repo: String,
}

/// Fetch the tree nodes requested by the client.
pub async fn trees(state: &mut State) -> Result<impl TryIntoResponse, HttpError> {
    let params = TreeParams::take_from(state);

    state.put(HandlerInfo::new(&params.repo, EdenApiMethod::Trees));

    let rctx = RequestContext::borrow_from(state).clone();
    let sctx = ServerContext::borrow_from(state);

    let repo = get_repo(sctx, &rctx, &params.repo, Metric::TotalManifests).await?;
    let request = parse_wire_request::<WireTreeRequest>(state).await?;
    if let Some(rd) = RequestDumper::try_borrow_mut_from(state) {
        rd.add_request(&request);
    };

    ScubaMiddlewareState::try_set_sampling_rate(state, nonzero_ext::nonzero!(256_u64));

    Ok(custom_cbor_stream(
        super::monitor_request(state, fetch_all_trees(repo, request)),
        |tree_entry| tree_entry.as_ref().err(),
    ))
}

/// Fetch trees for all of the requested keys concurrently.
fn fetch_all_trees(
    repo: HgRepoContext,
    request: TreeRequest,
) -> impl Stream<Item = Result<TreeEntry, EdenApiServerError>> {
    let ctx = repo.ctx().clone();

    let fetches = request.keys.into_iter().map(move |key| {
        fetch_tree(repo.clone(), key.clone(), request.attributes)
            .map(|r| r.map_err(|e| EdenApiServerError::with_key(key, e)))
    });

    stream::iter(fetches)
        .buffer_unordered(MAX_CONCURRENT_TREE_FETCHES_PER_REQUEST)
        .inspect_ok(move |_| {
            ctx.session().bump_load(Metric::TotalManifests, 1.0);
        })
}

/// Fetch requested tree for a single key.
/// Note that this function consumes the repo context in order
/// to construct a tree context for the requested blob.
async fn fetch_tree(
    repo: HgRepoContext,
    key: Key,
    attributes: TreeAttributes,
) -> Result<TreeEntry, Error> {
    let mut entry = TreeEntry::new(key.clone());
    let id = HgManifestId::from_node_hash(HgNodeHash::from(key.hgid));

    let ctx = id
        .context(repo.clone())
        .await
        .with_context(|| ErrorKind::TreeFetchFailed(key.clone()))?
        .with_context(|| ErrorKind::KeyDoesNotExist(key.clone()))?;

    if attributes.manifest_blob {
        repo.ctx()
            .perf_counters()
            .increment_counter(PerfCounterType::EdenapiTrees);

        let (data, _) = ctx
            .content()
            .await
            .with_context(|| ErrorKind::TreeFetchFailed(key.clone()))?;

        entry.with_data(Some(data));
    }

    if attributes.parents {
        entry.with_parents(Some(ctx.hg_parents().into()));
    }

    if attributes.child_metadata {
        repo.ctx()
            .perf_counters()
            .increment_counter(PerfCounterType::EdenapiTreesAuxData);

        if let Some(entries) = fetch_child_metadata_entries(&repo, &ctx).await? {
            let children: Vec<Result<TreeChildEntry, EdenApiServerError>> = entries
                .buffer_unordered(MAX_CONCURRENT_METADATA_FETCHES_PER_TREE_FETCH)
                .map(|r| r.map_err(|e| EdenApiServerError::with_key(key.clone(), e)))
                .collect()
                .await;

            entry.with_children(Some(children));
        }
    }

    Ok(entry)
}

async fn fetch_child_metadata_entries<'a>(
    repo: &'a HgRepoContext,
    ctx: &'a HgTreeContext,
) -> Result<
    Option<impl Stream<Item = impl Future<Output = Result<TreeChildEntry, Error>> + 'a> + 'a>,
    Error,
> {
    let manifest = ctx.clone().into_blob_manifest()?;
    if manifest.content().files.len() > LARGE_TREE_METADATA_LIMIT {
        return Ok(None);
    }
    let entries = manifest.list().collect::<Vec<_>>();

    Ok(Some(
        stream::iter(entries)
            // .entries iterator is not `Send`
            .map({
                move |(name, entry)| async move {
                    let name = RepoPathBuf::from_string(name.to_string())?;
                    Ok(match entry {
                        Entry::Leaf((_, child_id)) => {
                            let child_key = Key::new(name, child_id.into_nodehash().into());
                            fetch_child_file_metadata(repo, child_key.clone()).await?
                        }
                        Entry::Tree(child_id) => TreeChildEntry::new_directory_entry(Key::new(
                            name,
                            child_id.into_nodehash().into(),
                        )),
                    })
                }
            }),
    ))
}

async fn fetch_child_file_metadata(
    repo: &HgRepoContext,
    child_key: Key,
) -> Result<TreeChildEntry, Error> {
    let metadata = repo
        .file(HgFileNodeId::new(child_key.hgid.into()))
        .await?
        .ok_or_else(|| ErrorKind::FileFetchFailed(child_key.clone()))?
        .content_metadata()
        .await?;
    Ok(TreeChildEntry::new_file_entry(
        child_key,
        FileAuxData {
            total_size: metadata.total_size,
            sha1: metadata.sha1.into(),
            blake3: metadata.seeded_blake3.into(),
        }
        .into(),
    ))
}

/// Store the content of a single tree
async fn store_tree(
    repo: HgRepoContext,
    item: UploadTreeRequest,
) -> Result<UploadTreeResponse, Error> {
    let upload_node_id = HgNodeHash::from(item.entry.node_id);
    let contents = item.entry.data;
    let p1 = item.entry.parents.p1().cloned().map(HgNodeHash::from);
    let p2 = item.entry.parents.p2().cloned().map(HgNodeHash::from);
    repo.store_tree(upload_node_id, p1, p2, Bytes::from(contents))
        .await?;
    Ok(UploadTreeResponse {
        token: UploadToken::new_fake_token(AnyId::HgTreeId(item.entry.node_id), None),
    })
}

/// Upload list of trees requested by the client (batch request).
pub struct UploadTreesHandler;

#[async_trait]
impl EdenApiHandler for UploadTreesHandler {
    type Request = Batch<UploadTreeRequest>;
    type Response = UploadTreeResponse;

    const HTTP_METHOD: hyper::Method = hyper::Method::POST;
    const API_METHOD: EdenApiMethod = EdenApiMethod::UploadTrees;
    const ENDPOINT: &'static str = "/upload/trees";

    async fn handler(
        ectx: EdenApiContext<Self::PathExtractor, Self::QueryStringExtractor>,
        request: Self::Request,
    ) -> HandlerResult<'async_trait, Self::Response> {
        let repo = ectx.repo();
        let tokens = request
            .batch
            .into_iter()
            .map(move |item| store_tree(repo.clone(), item));

        Ok(stream::iter(tokens)
            .buffer_unordered(MAX_CONCURRENT_UPLOAD_TREES_PER_REQUEST)
            .boxed())
    }
}
