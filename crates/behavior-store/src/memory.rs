//! The in-memory reference backend: deterministic `BTreeMap` storage with an all-or-nothing
//! commit. For tests and as an executable definition of the backend contract, not for production.

use std::collections::BTreeMap;

use crate::documents::{EntityKey, EntityVersion, Genesis, Head, TransitionRecord};
use crate::{Backend, BackendError, CasOutcome};

#[derive(Debug, Clone, Default)]
pub struct InMemoryBackend {
    genesis: Option<Genesis>,
    head: Option<Head>,
    /// Versions per entity, in revision order.
    versions: BTreeMap<EntityKey, Vec<EntityVersion>>,
    /// Records by position (index = position - 1).
    records: Vec<TransitionRecord>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend::default()
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

    fn commit(
        &mut self,
        expected_last_record: &str,
        versions: &[EntityVersion],
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
        for v in versions {
            self.versions.entry(v.key()).or_default().push(v.clone());
        }
        self.records.push(record.clone());
        self.head = Some(new_head.clone());
        Ok(CasOutcome::Applied)
    }
}
