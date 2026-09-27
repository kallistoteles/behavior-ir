//! History queries and the two replays (research R10): data replay re-applies each recorded write
//! set and recomputes every state identity; behavior replay re-evaluates each transition under its
//! recorded behavior version. Both report the first divergence. Snapshots are verified, never
//! trusted.

use std::collections::BTreeMap;

use behavior_core::semantic::module::Module;
use serde_json::Value as Json;

use crate::documents::{
    Divergence, EntityKey, EntityVersion, R, ReplayReport, StateRef, StoreError, TAG_REPLAY_REPORT,
    TransitionRecord,
};
use crate::muhash::Accumulator;
use crate::store::{Store, transition_hash};
use crate::{Backend, BackendError};

fn backend_err(e: BackendError) -> StoreError {
    StoreError::Backend(e.0)
}

impl<B: Backend> Store<B> {
    /// The transition records from state `from` to state `to`, in order.
    pub fn transitions(&self, from: &StateRef, to: &StateRef) -> R<Vec<TransitionRecord>> {
        for s in [from, to] {
            if self.state_at(s.position)? != *s {
                return Err(StoreError::EntityNotFound(format!(
                    "{} at position {} is not a state of this store",
                    s.state, s.position
                )));
            }
        }
        let mut out = Vec::new();
        for pos in (from.position + 1)..=to.position {
            let r = self
                .backend()
                .record(pos)
                .map_err(backend_err)?
                .ok_or_else(|| StoreError::Backend(format!("record {pos} missing")))?;
            if r.position != pos {
                return Err(StoreError::Backend(format!(
                    "record {pos} reports position {}",
                    r.position
                )));
            }
            out.push(r);
        }
        Ok(out)
    }
}

struct Report {
    kind: &'static str,
    from: StateRef,
    to: StateRef,
    checked: u64,
}

impl Report {
    fn ok(self) -> ReplayReport {
        ReplayReport {
            format: TAG_REPLAY_REPORT.into(),
            kind: self.kind.into(),
            from: self.from,
            to: self.to,
            checked: self.checked,
            ok: true,
            divergence: None,
        }
    }

    fn diverged(self, position: u64, kind: &str, expected: String, found: String) -> ReplayReport {
        ReplayReport {
            format: TAG_REPLAY_REPORT.into(),
            kind: self.kind.into(),
            from: self.from,
            to: self.to,
            checked: self.checked,
            ok: false,
            divergence: Some(Divergence {
                position,
                kind: kind.into(),
                expected,
                found,
            }),
        }
    }
}

/// The record hash that precedes position `pos + 1` (the genesis hash for position 0).
fn chain_link<B: Backend>(store: &Store<B>, pos: u64) -> R<String> {
    if pos == 0 {
        return store.store_id();
    }
    store
        .backend()
        .record(pos)
        .map_err(backend_err)?
        .ok_or_else(|| StoreError::Backend(format!("record {pos} missing")))?
        .hash()
}

/// The entity versions valid at `pos`, for every entity of the (fixed) universe.
fn content_at<B: Backend>(store: &Store<B>, pos: u64) -> R<BTreeMap<EntityKey, EntityVersion>> {
    let genesis = store.genesis()?;
    let mut out = BTreeMap::new();
    for s in &genesis.seed {
        let key = EntityKey {
            entity: s.entity.clone(),
            id: s.value["id"].as_str().unwrap_or_default().to_string(),
        };
        let v = store
            .backend()
            .version_at(&key, pos)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::EntityNotFound(key.to_string()))?;
        out.insert(key, v);
    }
    Ok(out)
}

/// Data replay from `from` to `to`: re-applies each write set to the previous state, recomputes the
/// new entity versions and the state identity, and checks the record chain, parents, and the
/// `evaluated_against == committed_on` invariant.
pub fn replay_data<B: Backend>(store: &Store<B>, from: &StateRef, to: &StateRef) -> ReplayReport {
    let mut rep = Report {
        kind: "data",
        from: from.clone(),
        to: to.clone(),
        checked: 0,
    };
    let fail =
        |rep: Report, pos: u64, kind: &str, e: String| rep.diverged(pos, kind, String::new(), e);
    let genesis = match store.genesis() {
        Ok(g) => g,
        Err(e) => return fail(rep, from.position, "state", e.to_string()),
    };
    let mut content = match content_at(store, from.position) {
        Ok(c) => c,
        Err(e) => return fail(rep, from.position, "state", e.to_string()),
    };
    // Stored content hashes are recomputed from the stored values, never trusted.
    let mut acc = Accumulator::empty();
    for v in content.values() {
        let decl = genesis
            .entity_declarations
            .get(&v.entity)
            .cloned()
            .unwrap_or_default();
        match v.content(&decl).hash() {
            Ok(h) if h == v.content_hash => {}
            _ => {
                return rep.diverged(
                    from.position,
                    "state",
                    format!("{} content matching its hash", v.key()),
                    "a stored value that does not match its content hash".into(),
                );
            }
        }
        if let Err(e) = acc.insert(&v.content_hash) {
            return fail(rep, from.position, "state", e.to_string());
        }
    }
    match acc.state_id() {
        Ok(s) if s == from.state => {}
        Ok(s) => return rep.diverged(from.position, "state", from.state.clone(), s),
        Err(e) => return fail(rep, from.position, "state", e.to_string()),
    }
    let mut running = from.clone();
    let mut link = match chain_link(store, from.position) {
        Ok(l) => l,
        Err(e) => return fail(rep, from.position, "chain", e.to_string()),
    };
    for pos in (from.position + 1)..=to.position {
        let rec = match store.backend().record(pos) {
            Ok(Some(r)) => r,
            Ok(None) => {
                return rep.diverged(pos, "chain", format!("record {pos}"), "missing".into());
            }
            Err(e) => return fail(rep, pos, "chain", e.0),
        };
        if rec.position != pos || rec.previous_record != link {
            return rep.diverged(pos, "chain", link, rec.previous_record.clone());
        }
        if rec.evaluated_against != rec.committed_on {
            return rep.diverged(
                pos,
                "invariant",
                format!("{:?}", rec.evaluated_against),
                format!("{:?}", rec.committed_on),
            );
        }
        if rec.committed_on != running {
            return rep.diverged(
                pos,
                "parent",
                format!("{running:?}"),
                format!("{:?}", rec.committed_on),
            );
        }
        match (rec.bundle.hash(), transition_hash(&rec.bundle.record)) {
            (Ok(bh), Ok(th)) if bh == rec.bundle_hash && th == rec.bundle.transition_hash => {}
            _ => {
                return rep.diverged(
                    pos,
                    "record",
                    "hashes matching the bundle".into(),
                    "altered bundle or record".into(),
                );
            }
        }
        // Apply the write set.
        let mut new_values: BTreeMap<EntityKey, Json> = BTreeMap::new();
        for w in &rec.bundle.write_set {
            let key = EntityKey {
                entity: w.entity.clone(),
                id: w.id.clone(),
            };
            let Some(old) = content.get(&key) else {
                return rep.diverged(
                    pos,
                    "invariant",
                    "an existing entity".into(),
                    key.to_string(),
                );
            };
            if old.value.get(&w.field) != Some(&w.old) {
                return rep.diverged(
                    pos,
                    "changes",
                    format!("{key}.{} = {}", w.field, old.value[&w.field]),
                    format!("old {}", w.old),
                );
            }
            let v = new_values.entry(key).or_insert_with(|| old.value.clone());
            v[&w.field] = w.new.clone();
        }
        let mut expected_versions = Vec::new();
        for (key, value) in new_values {
            let old = &content[&key];
            if old.value == value {
                continue;
            }
            let decl = genesis
                .entity_declarations
                .get(&key.entity)
                .cloned()
                .unwrap_or_default();
            let mut v = EntityVersion {
                content_hash: String::new(),
                entity: key.entity.clone(),
                id: key.id.clone(),
                revision: old.revision + 1,
                created_at: pos,
                value,
            };
            v.content_hash = match v.content(&decl).hash() {
                Ok(h) => h,
                Err(e) => return fail(rep, pos, "changes", e.to_string()),
            };
            expected_versions.push(v);
        }
        if expected_versions != rec.new_versions {
            return rep.diverged(
                pos,
                "changes",
                "versions from the write set".into(),
                "different versions".into(),
            );
        }
        for v in &expected_versions {
            match store.backend().version(&v.key(), v.revision) {
                Ok(Some(stored)) if stored == *v => {}
                _ => {
                    return rep.diverged(
                        pos,
                        "state",
                        format!("stored {}#{}", v.key(), v.revision),
                        "missing or different".into(),
                    );
                }
            }
            let old = &content[&v.key()];
            if acc
                .remove(&old.content_hash)
                .and_then(|_| acc.insert(&v.content_hash))
                .is_err()
            {
                return fail(rep, pos, "state", "accumulator".into());
            }
            content.insert(v.key(), v.clone());
        }
        let state = match acc.state_id() {
            Ok(s) => s,
            Err(e) => return fail(rep, pos, "state", e.to_string()),
        };
        let expected = StateRef {
            state,
            position: pos,
        };
        if rec.result_state != expected {
            return rep.diverged(
                pos,
                "state",
                format!("{expected:?}"),
                format!("{:?}", rec.result_state),
            );
        }
        link = match rec.hash() {
            Ok(h) => h,
            Err(e) => return fail(rep, pos, "chain", e.to_string()),
        };
        running = expected;
        rep.checked += 1;
    }
    if running != *to {
        return rep.diverged(
            to.position,
            "state",
            format!("{to:?}"),
            format!("{running:?}"),
        );
    }
    if let Ok(Some(head)) = store.backend().head()
        && head.state_ref == *to
        && head.last_record != link
    {
        return rep.diverged(to.position, "chain", link, head.last_record);
    }
    rep.ok()
}

/// Behavior replay from `from` to `to`: re-evaluates each transition under its recorded behavior
/// version (from `modules`, keyed by behavior version) against the parent state, and checks that
/// the decision, the changes and the observed reads are the recorded ones.
pub fn replay_behavior<B: Backend>(
    store: &Store<B>,
    modules: &BTreeMap<String, Module>,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    let mut rep = Report {
        kind: "behavior",
        from: from.clone(),
        to: to.clone(),
        checked: 0,
    };
    let genesis = match store.genesis() {
        Ok(g) => g,
        Err(e) => return rep.diverged(from.position, "state", String::new(), e.to_string()),
    };
    for pos in (from.position + 1)..=to.position {
        let rec = match store.backend().record(pos) {
            Ok(Some(r)) => r,
            _ => return rep.diverged(pos, "chain", format!("record {pos}"), "missing".into()),
        };
        if rec.evaluated_against != rec.committed_on {
            return rep.diverged(
                pos,
                "invariant",
                "evaluated_against == committed_on".into(),
                "differ".into(),
            );
        }
        let b = &rec.bundle;
        let Some(module) = modules.get(&b.behavior_version) else {
            return rep.diverged(
                pos,
                "record",
                b.behavior_version.clone(),
                "no module for this behavior version".into(),
            );
        };
        if b.record["behavior_version"] != b.behavior_version.as_str() {
            return rep.diverged(
                pos,
                "record",
                b.behavior_version.clone(),
                b.record["behavior_version"].to_string(),
            );
        }
        match store.rederive(module, &genesis, &rec.committed_on, &b.record) {
            Ok((_, reads, writes)) => {
                if writes != b.write_set {
                    return rep.diverged(
                        pos,
                        "changes",
                        "the record's changes".into(),
                        "a different write set".into(),
                    );
                }
                if reads != b.read_set {
                    return rep.diverged(
                        pos,
                        "changes",
                        "the observed reads".into(),
                        "a different read set".into(),
                    );
                }
            }
            Err(e) => {
                let kind = if e.to_string().contains("reproduce") {
                    "decision"
                } else {
                    "state"
                };
                return rep.diverged(pos, kind, "a reproducible transition".into(), e.to_string());
            }
        }
        rep.checked += 1;
    }
    rep.ok()
}

/// Verifies a snapshot of state `at` (a cache): its content must hash to `at.state`, and `at` must be
/// a state of the store's history. The store is never changed by a snapshot.
pub fn verify_snapshot<B: Backend>(
    store: &Store<B>,
    at: &StateRef,
    entities: &[EntityVersion],
) -> ReplayReport {
    let rep = Report {
        kind: "snapshot",
        from: at.clone(),
        to: at.clone(),
        checked: 0,
    };
    let genesis = match store.genesis() {
        Ok(g) => g,
        Err(e) => return rep.diverged(at.position, "state", String::new(), e.to_string()),
    };
    match store.state_at(at.position) {
        Ok(s) if s == *at => {}
        _ => {
            return rep.diverged(
                at.position,
                "state",
                format!("{at:?}"),
                "not a state of this store".into(),
            );
        }
    }
    let mut acc = Accumulator::empty();
    for v in entities {
        let decl = genesis
            .entity_declarations
            .get(&v.entity)
            .cloned()
            .unwrap_or_default();
        let ok = v
            .content(&decl)
            .hash()
            .map(|h| h == v.content_hash)
            .unwrap_or(false);
        if !ok || acc.insert(&v.content_hash).is_err() {
            return rep.diverged(
                at.position,
                "state",
                "consistent entity content".into(),
                v.key().to_string(),
            );
        }
    }
    match acc.state_id() {
        Ok(s) if s == at.state => rep.ok(),
        Ok(s) => rep.diverged(at.position, "state", at.state.clone(), s),
        Err(e) => rep.diverged(at.position, "state", String::new(), e.to_string()),
    }
}
