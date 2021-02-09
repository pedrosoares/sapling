/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

#![deny(warnings)]

///! Segmented Changelog
///!
///! This represents an implementation for the core commit graph that we have
///! in a given repository. It provides algorithms over the commit graph.
use anyhow::{format_err, Result};
use async_trait::async_trait;
use auto_impl::auto_impl;
use context::CoreContext;
use futures::stream::BoxStream;
use mononoke_types::ChangesetId;

mod builder;
mod bundle;
mod dag;
mod iddag;
mod idmap;
mod logging;
mod manager;
mod on_demand;
mod seeder;
mod sql_types;
mod tailer;
mod types;

#[cfg(test)]
mod tests;

pub use ::dag::{CloneData, FlatSegment, Id as Vertex, Location, PreparedFlatSegments};

pub use crate::builder::SegmentedChangelogBuilder;

// TODO(T74420661): use `thiserror` to represent error case

pub struct StreamCloneData<T> {
    pub head_id: Vertex,
    pub flat_segments: PreparedFlatSegments,
    pub idmap_stream: BoxStream<'static, Result<(Vertex, T)>>,
}

#[async_trait]
#[auto_impl(Arc)]
pub trait SegmentedChangelog: Send + Sync {
    /// Get the identifier of a commit given it's commit graph location.
    ///
    /// The client using segmented changelog will have only a set of identifiers for the commits in
    /// the graph. To retrieve the identifier of an commit that is now known they will provide a
    /// known descendant and the distance from the known commit to the commit we inquire about.
    async fn location_to_changeset_id(
        &self,
        ctx: &CoreContext,
        location: Location<ChangesetId>,
    ) -> Result<ChangesetId> {
        let mut ids = self
            .location_to_many_changeset_ids(ctx, location, 1)
            .await?;
        if ids.len() == 1 {
            if let Some(id) = ids.pop() {
                return Ok(id);
            }
        }
        Err(format_err!(
            "unexpected result from location_to_many_changeset_ids"
        ))
    }

    /// Get identifiers of a continuous set of commit given their commit graph location.
    ///
    /// Similar to `location_to_changeset_id` but instead of returning the ancestor that is
    /// `distance` away from the `known` commit, it returns `count` ancestors following the parents.
    /// It is expected that all but the last ancestor will have a single parent.
    async fn location_to_many_changeset_ids(
        &self,
        ctx: &CoreContext,
        location: Location<ChangesetId>,
        count: u64,
    ) -> Result<Vec<ChangesetId>>;

    /// Returns data necessary for SegmentedChangelog to be initialized by a client.
    ///
    /// Note that the heads that are sent over in a clone can vary. Strictly speaking the client
    /// only needs one head.
    async fn clone_data(&self, ctx: &CoreContext) -> Result<CloneData<ChangesetId>>;

    /// An intermediate step in the quest for Segmented Changelog clones requires the server to
    /// send over the full idmap. For every commit (in master) we send the id that it corresponds
    /// to in the iddag.
    async fn full_idmap_clone_data(
        &self,
        ctx: &CoreContext,
    ) -> Result<StreamCloneData<ChangesetId>>;
}

pub struct DisabledSegmentedChangelog;

impl DisabledSegmentedChangelog {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl SegmentedChangelog for DisabledSegmentedChangelog {
    async fn location_to_many_changeset_ids(
        &self,
        _ctx: &CoreContext,
        _location: Location<ChangesetId>,
        _count: u64,
    ) -> Result<Vec<ChangesetId>> {
        // TODO(T74420661): use `thiserror` to represent error case
        Err(format_err!(
            "Segmented Changelog is not enabled for this repo",
        ))
    }

    async fn clone_data(&self, _ctx: &CoreContext) -> Result<CloneData<ChangesetId>> {
        Err(format_err!(
            "Segmented Changelog is not enabled for this repo",
        ))
    }

    async fn full_idmap_clone_data(
        &self,
        _ctx: &CoreContext,
    ) -> Result<StreamCloneData<ChangesetId>> {
        Err(format_err!(
            "Segmented Changelog is not enabled for this repo",
        ))
    }
}
