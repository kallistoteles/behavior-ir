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
    /// Field index events (feature 007): `(entity type, field, canonical value)` → keys with the
    /// positions from which and until which they held that value.
    field_index: BTreeMap<(String, String, String), Vec<FieldEdge>>,
    /// The open field-index edge of each entity field: its value and its place in
    /// `field_index`, so a change finds it directly (a migration rewrites many entities that
    /// share a value in one commit, feature 009).
    open_fields: BTreeMap<(EntityKey, String), (String, usize)>,
}

/// One event of the field index: `key` held the value from `added_at` until `dropped_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldEdge {
    key: EntityKey,
    added_at: u64,
    dropped_at: Option<u64>,
}

fn value_key(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => None,
        other => Some(other.to_string()),
    }
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend::default()
    }

    /// Updates the field index for a new version of an entity (its previous version, if any, is
    /// dropped from the values that changed).
    fn index_version(&mut self, v: &EntityVersion, position: u64) {
        let key = v.key();
        let previous = self
            .versions
            .get(&key)
            .and_then(|vs| vs.last())
            .map(|p| p.value.clone());
        let Some(fields) = v.value.as_object() else {
            return;
        };
        // A field the new version no longer has (a migration removed it, feature 009) leaves
        // the index.
        if let Some(serde_json::Value::Object(old)) = &previous {
            for (field, value) in old {
                if !fields.contains_key(field)
                    && let Some(old) = value_key(value)
                {
                    self.drop_field(&key, field, &old, position);
                }
            }
        }
        for (field, value) in fields {
            let old = previous.as_ref().and_then(|p| p.get(field)).cloned();
            if old.as_ref() == Some(value) {
                continue;
            }
            if let Some(old) = old.as_ref().and_then(value_key) {
                self.drop_field(&key, field, &old, position);
            }
            if let Some(new) = value_key(value) {
                let edges = self
                    .field_index
                    .entry((key.entity.clone(), field.clone(), new.clone()))
                    .or_default();
                edges.push(FieldEdge {
                    key: key.clone(),
                    added_at: position,
                    dropped_at: None,
                });
                self.open_fields
                    .insert((key.clone(), field.clone()), (new, edges.len() - 1));
            }
        }
    }

    fn drop_field(&mut self, key: &EntityKey, field: &str, value: &str, position: u64) {
        let slot = (key.clone(), field.to_string());
        let Some((open, i)) = self.open_fields.get(&slot).cloned() else {
            return;
        };
        if open != value {
            return;
        }
        if let Some(e) = self
            .field_index
            .get_mut(&(key.entity.clone(), field.to_string(), open))
            .and_then(|edges| edges.get_mut(i))
        {
            e.dropped_at = Some(position);
        }
        self.open_fields.remove(&slot);
    }

    fn index_removal(&mut self, key: &EntityKey, position: u64) {
        let last = self
            .versions
            .get(key)
            .and_then(|vs| vs.last())
            .map(|v| v.value.clone());
        if let Some(serde_json::Value::Object(fields)) = last {
            for (field, value) in fields {
                if let Some(v) = value_key(&value) {
                    self.drop_field(key, &field, &v, position);
                }
            }
        }
    }

    fn exists_at(&self, key: &EntityKey, position: u64) -> bool {
        self.versions
            .get(key)
            .is_some_and(|vs| vs.iter().any(|v| v.created_at <= position))
            && self.removed.get(key).is_none_or(|r| *r > position)
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
        self.genesis = Some(genesis.clone());
        self.head = Some(head.clone());
        for v in seed {
            self.index_version(v, 0);
            self.versions.entry(v.key()).or_default().push(v.clone());
        }
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

    fn keys_at(&self, entity_type: &str, position: u64) -> Result<Vec<EntityKey>, BackendError> {
        Ok(self
            .versions
            .keys()
            .filter(|k| k.entity == entity_type && self.exists_at(k, position))
            .cloned()
            .collect())
    }

    fn keys_by_field_at(
        &self,
        entity_type: &str,
        field: &str,
        value: &serde_json::Value,
        position: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        let Some(v) = value_key(value) else {
            return Ok(None);
        };
        let mut out: Vec<EntityKey> = self
            .field_index
            .get(&(entity_type.to_string(), field.to_string(), v))
            .into_iter()
            .flatten()
            .filter(|e| e.added_at <= position && e.dropped_at.is_none_or(|d| d > position))
            .map(|e| e.key.clone())
            .filter(|k| self.exists_at(k, position))
            .collect();
        out.sort();
        out.dedup();
        Ok(Some(out))
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
            self.index_version(v, position);
            self.versions.entry(v.key()).or_default().push(v.clone());
        }
        for k in removals {
            self.index_removal(k, position);
            self.removed.entry(k.clone()).or_insert(position);
        }
        self.apply_refs(ref_changes, position);
        self.records.push(record.clone());
        self.head = Some(new_head.clone());
        Ok(CasOutcome::Applied)
    }
}
