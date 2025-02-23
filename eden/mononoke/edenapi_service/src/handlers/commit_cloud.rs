/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use async_trait::async_trait;
use edenapi_types::cloud::WorkspaceData;
use edenapi_types::CloudWorkspaceRequest;
use futures::stream;
use futures::FutureExt;
use futures::StreamExt;
use mononoke_api_hg::HgRepoContext;

use super::handler::EdenApiContext;
use super::EdenApiHandler;
use super::EdenApiMethod;
use super::HandlerResult;
use crate::errors::ErrorKind;
pub struct CommitCloudWorkspace;

#[async_trait]
impl EdenApiHandler for CommitCloudWorkspace {
    type Request = CloudWorkspaceRequest;
    type Response = WorkspaceData;

    const HTTP_METHOD: hyper::Method = hyper::Method::POST;
    const API_METHOD: EdenApiMethod = EdenApiMethod::CloudWorkspace;
    const ENDPOINT: &'static str = "/cloud/workspace";

    async fn handler(
        ectx: EdenApiContext<Self::PathExtractor, Self::QueryStringExtractor>,
        request: Self::Request,
    ) -> HandlerResult<'async_trait, Self::Response> {
        let repo = ectx.repo();
        let res = get_workspace(request, repo).boxed();
        Ok(stream::once(res).boxed())
    }
}

async fn get_workspace(
    request: CloudWorkspaceRequest,
    repo: HgRepoContext,
) -> anyhow::Result<WorkspaceData> {
    let version = repo
        .cloud_workspace(&request.workspace, &request.reponame)
        .await?;
    if !version.is_empty() {
        let data = WorkspaceData {
            name: request.workspace,
            reponame: request.reponame,
            version: version[0].version,
            archived: version[0].archived,
            timestamp: version[0].timestamp.timestamp_nanos(),
        };
        return Ok(data);
    }
    Err(anyhow::anyhow!(ErrorKind::CloudWorkspaceNotFound(
        request.workspace
    )))
}
