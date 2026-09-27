//! The in-memory reference backend: deterministic `BTreeMap` storage with an all-or-nothing
//! commit. For tests and as an executable definition of the backend contract, not for production.

use std::collections::BTreeMap;

use crate::documents::{EntityKey, EntityVersion, Genesis, Head, RefChange, TransitionRecord};
use crate::{Backend, BackendError, CasOutcome, RefEdge};

/// One edge event of the derived reverse-reference index (feature 006): `dropped_at` is set at
/// most once, by the commit that drops the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexEdge {
    source: EntityKey,
    field: String,
    added_at: u64,
    dropped_at: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryBackend {
    genesis: Option<Genesis>,
    head: Option<Head>,
    /// Versions per entity, in revision order.
    versions: BTreeMap<EntityKey, Vec<EntityVersion>>,
    /// Records by position (index = position - 1).
    records: Vec<TransitionRecord>,
    /// Removal positions (feature 006).
    removed: BTreeMap<EntityKey, u64>,
    /// Index edge events per target (feature 006).
    edges: BTreeMap<EntityKey, Vec<IndexEdge>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend::default()
    }

    fn apply_refs(&mut self, changes: &[RefChange], position: u64) {
        for c in changes {
            let edges = self.edges.entry(c.target.clone()).or_default();
            if c.op == "add" {
                edges.push(IndexEdge {
                    source: c.source.clone(),
                    field: c.field.clone(),
                    added_at: position,
                    dropped_at: None,
                });
            } else if let Some(e) = edges
                .iter_mut()
                .find(|e| e.source == c.source && e.field == c.field && e.dropped_at.is_none())
            {
                e.dropped_at = Some(position);
            }
        }
    }
}

impl Backend for InMemoryBackend {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        Ok(self.genesis.clone())
    }

    fn head(&self) -> Result<Option<Head>, BackendError> {
        Ok(self.head.clone())
    }

    fn create(
        &mut self,
        genesis: &Genesis,
        head: &Head,
        seed: &[EntityVersion],
        seed_refs: &[RefChange],
    ) -> Result<(), BackendError> {
        if self.genesis.is_some() {
            return Err(BackendError("store already created".into()));
        }
        let mut versions: BTreeMap<EntityKey, Vec<EntityVersion>> = BTreeMap::new();
        for v in seed {
            versions.entry(v.key()).or_default().push(v.clone());
        }
        self.genesis = Some(genesis.clone());
        self.head = Some(head.clone());
        self.versions = versions;
        self.apply_refs(seed_refs, 0);
        Ok(())
    }

    fn version_at(
        &self,
        key: &EntityKey,
        position: u64,
    ) -> Result<Option<EntityVersion>, BackendError> {
        Ok(self
            .versions
            .get(key)
            .and_then(|vs| vs.iter().rev().find(|v| v.created_at <= position))
            .cloned())
    }

    fn version(
        &self,
        key: &EntityKey,
        revision: u64,
    ) -> Result<Option<EntityVersion>, BackendError> {
        Ok(self
            .versions
            .get(key)
            .and_then(|vs| vs.iter().find(|v| v.revision == revision))
            .cloned())
    }

    fn record(&self, position: u64) -> Result<Option<TransitionRecord>, BackendError> {
        let Some(i) = position.checked_sub(1) else {
            return Ok(None);
        };
        Ok(usize::try_from(i)
            .ok()
            .and_then(|i| self.records.get(i))
            .cloned())
    }

    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError> {
        Ok(self.removed.get(key).copied())
    }

    fn incoming_at(&self, target: &EntityKey, position: u64) -> Result<Vec<RefEdge>, BackendError> {
        let mut out: Vec<RefEdge> = self
            .edges
            .get(target)
            .into_iter()
            .flatten()
            .filter(|e| e.added_at <= position && e.dropped_at.is_none_or(|d| d > position))
            .map(|e| RefEdge {
                entity: e.source.entity.clone(),
                id: e.source.id.clone(),
                field: e.field.clone(),
            })
            .collect();
        out.sort();
        Ok(out)
    }

    fn commit(
        &mut self,
        expected_last_record: &str,
        versions: &[EntityVersion],
        removals: &[EntityKey],
        ref_changes: &[RefChange],
        record: &TransitionRecord,
        new_head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        let Some(head) = &self.head else {
            return Err(BackendError("store not created".into()));
        };
        if head.last_record != expected_last_record {
            return Ok(CasOutcome::HeadMoved);
        }
        // All-or-nothing: nothing below can fail.
        let position = new_head.state_ref.position;
        for v in versions {
            self.versions.entry(v.key()).or_default().push(v.clone());
        }
        for k in removals {
            self.removed.entry(k.clone()).or_insert(position);
        }
        self.apply_refs(ref_changes, position);
        self.records.push(record.clone());
        self.head = Some(new_head.clone());
        Ok(CasOutcome::Applied)
    }
}
