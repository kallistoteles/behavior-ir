//! History queries and the two replays (research R10): data replay re-applies each recorded write
//! set and recomputes every state identity; behavior replay re-evaluates each transition under its
//! recorded behavior version. Both report the first divergence. Snapshots are verified, never
//! trusted.

use std::collections::{BTreeMap, BTreeSet};

use behavior_core::semantic::module::Module;
use serde_json::Value as Json;

use crate::documents::{
    Divergence, EntityKey, EntityVersion, R, RefChange, RemovedEntity, ReplayReport, StateRef,
    StoreError, TAG_REPLAY_REPORT, TransitionRecord,
};
use crate::muhash::Accumulator;
use crate::store::{Store, references_of, transition_hash};
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

/// The universe at `pos`, rebuilt from the history (feature 006): the seed plus the recorded
/// creations minus the recorded removals up to `pos`, with each entity's version at `pos`; and
/// the registry, every identity used up to `pos`.
type Universe = (BTreeMap<EntityKey, EntityVersion>, BTreeSet<EntityKey>);

pub(crate) fn content_at<B: Backend>(store: &Store<B>, pos: u64) -> R<Universe> {
    let genesis = store.genesis()?;
    let mut keys: BTreeSet<EntityKey> = genesis
        .seed
        .iter()
        .map(|s| EntityKey {
            entity: s.entity.clone(),
            id: s.value["id"].as_str().unwrap_or_default().to_string(),
        })
        .collect();
    let mut used = keys.clone();
    for p in 1..=pos {
        let r = store
            .backend()
            .record(p)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::Backend(format!("record {p} missing")))?;
        for v in &r.created {
            keys.insert(v.key());
            used.insert(v.key());
        }
        for x in &r.removed {
            keys.remove(&removed_key(x));
        }
    }
    let mut out = BTreeMap::new();
    for key in keys {
        let v = store
            .backend()
            .version_at(&key, pos)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::EntityNotFound(key.to_string()))?;
        out.insert(key, v);
    }
    Ok((out, used))
}

fn removed_key(x: &RemovedEntity) -> EntityKey {
    EntityKey {
        entity: x.entity.clone(),
        id: x.id.clone(),
    }
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
    let (mut content, mut used) = match content_at(store, from.position) {
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
        // Lifecycle (feature 006): creations need a never-used identity and land at revision 1;
        // removals need an existing entity, remove its last content, and keep its versions.
        let mut expected_created = Vec::new();
        let mut expected_removed = Vec::new();
        for l in rec.bundle.record["lifecycle"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let key = EntityKey {
                entity: l["entity"].as_str().unwrap_or_default().to_string(),
                id: l["id"].as_str().unwrap_or_default().to_string(),
            };
            let decl = genesis
                .entity_declarations
                .get(&key.entity)
                .cloned()
                .unwrap_or_default();
            if l["op"] == "create" {
                if !used.insert(key.clone()) {
                    return rep.diverged(
                        pos,
                        "lifecycle",
                        format!("a never-used identity for {key}"),
                        "an identity created twice".into(),
                    );
                }
                let mut v = EntityVersion {
                    content_hash: String::new(),
                    entity: key.entity.clone(),
                    id: key.id.clone(),
                    revision: 1,
                    created_at: pos,
                    value: l["value"].clone(),
                };
                v.content_hash = match v.content(&decl).hash() {
                    Ok(h) => h,
                    Err(e) => return fail(rep, pos, "lifecycle", e.to_string()),
                };
                if acc.insert(&v.content_hash).is_err() {
                    return fail(rep, pos, "state", "accumulator".into());
                }
                content.insert(key, v.clone());
                expected_created.push(v);
            } else {
                let Some(old) = content.remove(&key) else {
                    return rep.diverged(
                        pos,
                        "lifecycle",
                        format!("{key} existing before its removal"),
                        "an absent entity".into(),
                    );
                };
                if acc.remove(&old.content_hash).is_err() {
                    return fail(rep, pos, "state", "accumulator".into());
                }
                match store.backend().removed_at(&key) {
                    Ok(Some(p)) if p == pos => {}
                    other => {
                        return rep.diverged(
                            pos,
                            "lifecycle",
                            format!("{key} removed at {pos}"),
                            format!("{other:?}"),
                        );
                    }
                }
                expected_removed.push(RemovedEntity {
                    entity: key.entity.clone(),
                    id: key.id.clone(),
                    last_revision: old.revision,
                    last_content_hash: old.content_hash,
                });
            }
        }
        if expected_created != rec.created || expected_removed != rec.removed {
            return rep.diverged(
                pos,
                "lifecycle",
                "creations and removals from the decision".into(),
                "different lifecycle entries".into(),
            );
        }
        for v in &expected_created {
            match store.backend().version(&v.key(), 1) {
                Ok(Some(stored)) if stored == *v => {}
                _ => {
                    return rep.diverged(
                        pos,
                        "lifecycle",
                        format!("stored {}#1", v.key()),
                        "missing or different".into(),
                    );
                }
            }
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
            Ok((_, reads, writes, facts)) => {
                if facts != b.read_facts {
                    return rep.diverged(
                        pos,
                        "decision",
                        "the facts observed at the parent".into(),
                        "different read facts".into(),
                    );
                }
                // The record also replays on its own recorded facts (no live store).
                let own = behavior_core::replay(module, &b.record.to_string());
                if !own.matches {
                    return rep.diverged(
                        pos,
                        "decision",
                        "a record that replays on its recorded facts".into(),
                        own.diff.unwrap_or_default(),
                    );
                }
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

type Index = BTreeMap<EntityKey, BTreeSet<RefEdgeKey>>;
type RefEdgeKey = (String, String, String);

fn index_of(module: &Module, content: &BTreeMap<EntityKey, EntityVersion>) -> Index {
    let mut index = Index::new();
    for v in content.values() {
        for (field, target) in references_of(module, &v.entity, &v.value) {
            index
                .entry(target)
                .or_default()
                .insert((v.entity.clone(), v.id.clone(), field));
        }
    }
    index
}

fn index_matches<B: Backend>(
    store: &Store<B>,
    index: &Index,
    target: &EntityKey,
    pos: u64,
) -> Result<(), String> {
    let stored: BTreeSet<RefEdgeKey> = store
        .backend()
        .incoming_at(target, pos)
        .map_err(|e| e.0)?
        .into_iter()
        .map(|e| (e.entity, e.id, e.field))
        .collect();
    let rebuilt = index.get(target).cloned().unwrap_or_default();
    if stored == rebuilt {
        Ok(())
    } else {
        Err(format!("{target}: stored {stored:?}, rebuilt {rebuilt:?}"))
    }
}

/// Reference replay (feature 006, research R9, R11): the derived reverse-reference index is
/// rebuilt from entity content (`module` names the `Ref` fields) and must equal the backend's
/// `incoming_at` at `from`, after every transition for the targets it changes, and at `to` for
/// every entity; each record's `ref_changes` must be the ones its content changes imply.
pub fn replay_index<B: Backend>(
    store: &Store<B>,
    module: &Module,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    let mut rep = Report {
        kind: "references",
        from: from.clone(),
        to: to.clone(),
        checked: 0,
    };
    let (mut content, _) = match content_at(store, from.position) {
        Ok(c) => c,
        Err(e) => return rep.diverged(from.position, "state", String::new(), e.to_string()),
    };
    let mut index = index_of(module, &content);
    // Every entity and every indexed target: the first mismatch, if any.
    let check_all = |index: &Index, content: &BTreeMap<EntityKey, EntityVersion>, pos: u64| {
        let targets: BTreeSet<&EntityKey> = content.keys().chain(index.keys()).collect();
        targets
            .into_iter()
            .find_map(|t| index_matches(store, index, t, pos).err())
    };
    if let Some(e) = check_all(&index, &content, from.position) {
        return rep.diverged(from.position, "references", "the rebuilt index".into(), e);
    }
    for pos in (from.position + 1)..=to.position {
        let rec = match store.backend().record(pos) {
            Ok(Some(r)) => r,
            _ => return rep.diverged(pos, "chain", format!("record {pos}"), "missing".into()),
        };
        let mut expected: Vec<RefChange> = Vec::new();
        let change = |t: EntityKey, s: EntityKey, f: String, op: &str| RefChange {
            target: t,
            source: s,
            field: f,
            op: op.into(),
        };
        for v in &rec.new_versions {
            let Some(old) = content.get(&v.key()) else {
                return rep.diverged(
                    pos,
                    "references",
                    format!("{} existing", v.key()),
                    "absent".into(),
                );
            };
            let before = references_of(module, &v.entity, &old.value);
            let after = references_of(module, &v.entity, &v.value);
            for (f, t) in &before {
                if !after.contains(&(f.clone(), t.clone())) {
                    expected.push(change(t.clone(), v.key(), f.clone(), "drop"));
                }
            }
            for (f, t) in &after {
                if !before.contains(&(f.clone(), t.clone())) {
                    expected.push(change(t.clone(), v.key(), f.clone(), "add"));
                }
            }
            content.insert(v.key(), v.clone());
        }
        for v in &rec.created {
            for (f, t) in references_of(module, &v.entity, &v.value) {
                expected.push(change(t, v.key(), f, "add"));
            }
            content.insert(v.key(), v.clone());
        }
        for x in &rec.removed {
            let k = removed_key(x);
            if let Some(old) = content.remove(&k) {
                for (f, t) in references_of(module, &old.entity, &old.value) {
                    expected.push(change(t, k.clone(), f, "drop"));
                }
            }
        }
        expected.sort();
        if expected != rec.ref_changes {
            return rep.diverged(
                pos,
                "references",
                format!("{expected:?}"),
                format!("{:?}", rec.ref_changes),
            );
        }
        for c in &expected {
            let edge = (
                c.source.entity.clone(),
                c.source.id.clone(),
                c.field.clone(),
            );
            let slot = index.entry(c.target.clone()).or_default();
            if c.op == "add" {
                slot.insert(edge);
            } else {
                slot.remove(&edge);
            }
        }
        let targets: BTreeSet<&EntityKey> = expected.iter().map(|c| &c.target).collect();
        for t in targets {
            if let Err(e) = index_matches(store, &index, t, pos) {
                return rep.diverged(pos, "references", "the rebuilt index".into(), e);
            }
        }
        rep.checked += 1;
    }
    match check_all(&index, &content, to.position) {
        None => rep.ok(),
        Some(e) => rep.diverged(to.position, "references", "the rebuilt index".into(), e),
    }
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
