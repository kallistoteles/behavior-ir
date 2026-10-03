//! History queries and the two replays (research R10): data replay re-applies each recorded write
//! set and recomputes every state identity; behavior replay re-evaluates each transition under its
//! recorded behavior version. Both report the first divergence. Snapshots are verified, never
//! trusted.

use std::collections::{BTreeMap, BTreeSet};

use behavior_core::migration::{Migration, SourceEntity, apply_migration};
use behavior_core::semantic::module::Module;
use serde_json::Value as Json;

use crate::documents::{
    Divergence, EntityKey, EntityVersion, R, RefChange, RemovedEntity, ReplayReport, SchemaRef,
    StateRef, StoreError, TAG_REPLAY_REPORT, TransitionRecord,
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

/// The schemas in force up to `from`: from the genesis, the genesis schema alone; otherwise the
/// store's schema history up to `from`. A replay extends it with the migration records it walks,
/// so it never trusts the head or a later record about earlier schemas.
fn initial_history<B: Backend>(store: &Store<B>, from: &StateRef) -> R<Vec<SchemaRef>> {
    if from.position == 0 {
        return Ok(vec![store.genesis()?.schema()?]);
    }
    Ok(store
        .schema_history()?
        .into_iter()
        .filter(|s| s.since <= from.position)
        .collect())
}

/// The declaration of `entity` under the schema in force at `position` (feature 009): a version is
/// content-hashed under the schema of the position that created it.
fn declaration_at(history: &[SchemaRef], position: u64, entity: &str) -> String {
    history
        .iter()
        .rev()
        .find(|s| s.since <= position)
        .and_then(|s| s.declarations.get(entity).cloned())
        .unwrap_or_default()
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
    // The schema chain is walked from `from`, never taken from the head (feature 009).
    let mut history = match initial_history(store, from) {
        Ok(h) => h,
        Err(e) => return fail(rep, from.position, "state", e.to_string()),
    };
    let (mut content, mut used) = match content_at(store, from.position) {
        Ok(c) => c,
        Err(e) => return fail(rep, from.position, "state", e.to_string()),
    };
    // Stored content hashes are recomputed from the stored values, never trusted.
    let mut acc = Accumulator::empty();
    for v in content.values() {
        let decl = declaration_at(&history, v.created_at, &v.entity);
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
        if let Err(e) = rec.check_kind() {
            return rep.diverged(pos, "record", "a well-formed record".into(), e.to_string());
        }
        if rec.is_migration() {
            if let Err((kind, expected, found)) =
                data_migration(store, &rec, pos, &mut content, &mut acc, &mut history)
            {
                return rep.diverged(pos, kind, expected, found);
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
            continue;
        }
        let Some(bundle) = rec.bundle.as_ref() else {
            return rep.diverged(
                pos,
                "record",
                "an action record".into(),
                "no commit bundle".into(),
            );
        };
        match (bundle.hash(), transition_hash(&bundle.record)) {
            (Ok(bh), Ok(th)) if bh == rec.bundle_hash && th == bundle.transition_hash => {}
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
        for w in &bundle.write_set {
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
            let decl = declaration_at(&history, pos, &key.entity);
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
        for l in bundle.record["lifecycle"].as_array().into_iter().flatten() {
            let key = EntityKey {
                entity: l["entity"].as_str().unwrap_or_default().to_string(),
                id: l["id"].as_str().unwrap_or_default().to_string(),
            };
            let decl = declaration_at(&history, pos, &key.entity);
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
    {
        if head.last_record != link {
            return rep.diverged(to.position, "chain", link, head.last_record);
        }
        // The head names the schema the chain arrives at (absent: the genesis schema).
        let walked = history.last().filter(|s| s.since > 0).cloned();
        if head.schema != walked {
            return rep.diverged(
                to.position,
                "schema",
                format!("{:?}", walked.map(|s| s.hash)),
                format!("{:?}", head.schema.map(|s| s.hash)),
            );
        }
    }
    rep.ok()
}

type Divergent = (&'static str, String, String);

/// Data replay of a migration record (feature 009): its bundle is intact; it continues the schema
/// chain (its previous schema is the one in force, its source that schema, its target the hash of
/// its target declarations); exactly the entities of the changed types have new versions, each
/// the next revision at this position, content-hashed under the target declarations and stored as
/// recorded; retired types are empty. The state accumulator and the chain move on.
fn data_migration<B: Backend>(
    store: &Store<B>,
    rec: &TransitionRecord,
    pos: u64,
    content: &mut BTreeMap<EntityKey, EntityVersion>,
    acc: &mut Accumulator,
    history: &mut Vec<SchemaRef>,
) -> Result<(), Divergent> {
    let mb = rec.migration.as_ref().ok_or((
        "record",
        "a migration bundle".to_string(),
        "none".to_string(),
    ))?;
    match mb.hash() {
        Ok(h) if h == rec.bundle_hash => {}
        _ => {
            return Err((
                "record",
                "a migration bundle matching its hash".into(),
                "an altered migration bundle".into(),
            ));
        }
    }
    let current = history.last().cloned().ok_or((
        "schema",
        "a schema in force".to_string(),
        "none".to_string(),
    ))?;
    let previous = (current.since > 0).then(|| current.clone());
    if mb.previous_schema != previous || mb.source != current.hash {
        return Err((
            "schema",
            format!("the previous schema {}", current.hash),
            format!(
                "{:?} (source {})",
                mb.previous_schema.as_ref().map(|s| &s.hash),
                mb.source
            ),
        ));
    }
    let target = behavior_core::StoreSchema::of(mb.target_declarations.clone());
    if target.hash != mb.target {
        return Err((
            "schema",
            mb.target.clone(),
            format!("target declarations hashing to {}", target.hash),
        ));
    }
    if !rec.created.is_empty() || !rec.removed.is_empty() {
        return Err((
            "lifecycle",
            "no creations or removals".into(),
            "a migration that creates or removes".into(),
        ));
    }
    let retired: Vec<&String> = current
        .declarations
        .keys()
        .filter(|t| !mb.target_declarations.contains_key(*t))
        .collect();
    if let Some(k) = content.keys().find(|k| retired.contains(&&k.entity)) {
        return Err((
            "state",
            "retired types without entities".into(),
            k.to_string(),
        ));
    }
    let changed = |t: &str| {
        mb.target_declarations
            .get(t)
            .is_some_and(|d| current.declarations.get(t) != Some(d))
    };
    let expected: BTreeSet<EntityKey> = content
        .keys()
        .filter(|k| changed(&k.entity))
        .cloned()
        .collect();
    let written: BTreeSet<EntityKey> = rec.new_versions.iter().map(|v| v.key()).collect();
    if expected != written {
        return Err((
            "changes",
            format!(
                "new versions of the {} entities of changed types",
                expected.len()
            ),
            format!("{} new versions", written.len()),
        ));
    }
    for v in &rec.new_versions {
        let key = v.key();
        let old =
            content
                .get(&key)
                .cloned()
                .ok_or(("changes", key.to_string(), "absent".to_string()))?;
        let decl = mb
            .target_declarations
            .get(&v.entity)
            .cloned()
            .unwrap_or_default();
        let hash_ok = v.content(&decl).hash().is_ok_and(|h| h == v.content_hash);
        if v.revision != old.revision + 1 || v.created_at != pos || !hash_ok {
            return Err((
                "state",
                format!(
                    "{key} at revision {} under the target schema",
                    old.revision + 1
                ),
                "a different version".into(),
            ));
        }
        match store.backend().version(&key, v.revision) {
            Ok(Some(stored)) if stored == *v => {}
            _ => {
                return Err((
                    "state",
                    format!("stored {key}#{}", v.revision),
                    "missing or different".into(),
                ));
            }
        }
        if acc
            .remove(&old.content_hash)
            .and_then(|_| acc.insert(&v.content_hash))
            .is_err()
        {
            return Err(("state", "the accumulator".into(), "an error".into()));
        }
        content.insert(key, v.clone());
    }
    history.push(SchemaRef {
        hash: mb.target.clone(),
        declarations: mb.target_declarations.clone(),
        since: pos,
        migration_record: rec
            .hash()
            .map_err(|e| ("chain", String::new(), e.to_string()))?,
    });
    Ok(())
}

/// Behavior replay from `from` to `to`: re-evaluates each transition under its recorded behavior
/// version (from `modules`, keyed by behavior version) against the parent state, and checks that
/// the decision, the changes and the observed reads are the recorded ones. A history with
/// migrations needs [`replay_behavior_with`].
pub fn replay_behavior<B: Backend>(
    store: &Store<B>,
    modules: &BTreeMap<String, Module>,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    replay_behavior_with(store, modules, &BTreeMap::new(), from, to)
}

/// The migrations a behavior replay may meet, by migration hash, with their source and target
/// modules (feature 009).
pub type Migrations = BTreeMap<String, (Migration, Module, Module)>;

/// Behavior replay across schema generations (feature 009, FR-017): actions as in
/// [`replay_behavior`], and each migration record re-run with its migration (from `migrations`)
/// on the parent state: its requirement outcomes and new versions must be the recorded ones.
pub fn replay_behavior_with<B: Backend>(
    store: &Store<B>,
    modules: &BTreeMap<String, Module>,
    migrations: &Migrations,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    let mut rep = Report {
        kind: "behavior",
        from: from.clone(),
        to: to.clone(),
        checked: 0,
    };
    let mut history = match initial_history(store, from) {
        Ok(h) => h,
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
        let Some(at_schema) = history
            .iter()
            .rev()
            .find(|s| s.since <= rec.committed_on.position)
        else {
            return rep.diverged(pos, "state", "a schema".into(), "none".into());
        };
        if rec.is_migration() {
            if let Err((kind, expected, found)) = behavior_migration(store, &rec, migrations) {
                return rep.diverged(pos, kind, expected, found);
            }
            if let (Some(mb), Ok(h)) = (&rec.migration, rec.hash()) {
                history.push(SchemaRef {
                    hash: mb.target.clone(),
                    declarations: mb.target_declarations.clone(),
                    since: pos,
                    migration_record: h,
                });
            }
            rep.checked += 1;
            continue;
        }
        let Some(b) = rec.bundle.as_ref() else {
            return rep.diverged(
                pos,
                "record",
                "an action record".into(),
                "no commit bundle".into(),
            );
        };
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
        match store.rederive(
            module,
            &at_schema.declarations,
            &rec.committed_on,
            &b.record,
        ) {
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

/// Behavior replay of a migration record: the migration it names, re-run on the parent state,
/// gives the recorded requirement outcomes and exactly the recorded new values.
fn behavior_migration<B: Backend>(
    store: &Store<B>,
    rec: &TransitionRecord,
    migrations: &Migrations,
) -> Result<(), Divergent> {
    let mb = rec.migration.as_ref().ok_or((
        "record",
        "a migration bundle".to_string(),
        "none".to_string(),
    ))?;
    let Some((m, source, target)) = migrations.get(&mb.migration_hash) else {
        return Err((
            "record",
            mb.migration_hash.clone(),
            "no migration for this hash".into(),
        ));
    };
    if m.hash() != mb.migration_hash {
        return Err(("record", mb.migration_hash.clone(), m.hash()));
    }
    let (content, _) = content_at(store, rec.committed_on.position)
        .map_err(|e| ("state", "the parent state".to_string(), e.to_string()))?;
    let entities: Vec<SourceEntity> = content
        .values()
        .map(|v| SourceEntity {
            entity: v.entity.clone(),
            value: v.value.clone(),
        })
        .collect();
    let out = apply_migration(m, source, target, &entities).map_err(|r| {
        (
            "decision",
            "the recorded migration".to_string(),
            r.to_string(),
        )
    })?;
    let outcomes: Vec<(String, bool)> = out
        .requirements
        .iter()
        .map(|r| (r.name.clone(), r.held))
        .collect();
    let recorded: Vec<(String, bool)> = mb
        .requirements
        .iter()
        .map(|r| (r.name.clone(), r.held))
        .collect();
    if outcomes != recorded {
        return Err((
            "decision",
            format!("requirements {outcomes:?}"),
            format!("{recorded:?}"),
        ));
    }
    let migrated: BTreeMap<EntityKey, &Json> = out
        .entities
        .iter()
        .filter(|e| e.migrated)
        .map(|e| {
            (
                EntityKey {
                    entity: e.entity.clone(),
                    id: e.id.clone(),
                },
                &e.value,
            )
        })
        .collect();
    let written: BTreeMap<EntityKey, &Json> = rec
        .new_versions
        .iter()
        .map(|v| (v.key(), &v.value))
        .collect();
    if migrated != written {
        return Err((
            "changes",
            "the values the migration produces".into(),
            "different recorded values".into(),
        ));
    }
    Ok(())
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
/// One module is valid for one schema: a history that crosses a migration needs
/// [`replay_index_with`].
pub fn replay_index<B: Backend>(
    store: &Store<B>,
    module: &Module,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    let modules = BTreeMap::from([(behavior_core::schema(module).hash, module.clone())]);
    replay_index_with(store, &modules, from, to)
}

/// Reference replay across schema generations (feature 009): `modules` gives the module of each
/// schema (by SchemaHash), which names the `Ref` fields of the positions under it. At a migration
/// record the old values are read under the source schema and the new ones under the target.
pub fn replay_index_with<B: Backend>(
    store: &Store<B>,
    modules: &BTreeMap<String, Module>,
    from: &StateRef,
    to: &StateRef,
) -> ReplayReport {
    let mut rep = Report {
        kind: "references",
        from: from.clone(),
        to: to.clone(),
        checked: 0,
    };
    let schema = match initial_history(store, from) {
        Ok(h) => h.last().map(|s| s.hash.clone()).unwrap_or_default(),
        Err(e) => return rep.diverged(from.position, "state", String::new(), e.to_string()),
    };
    let Some(mut module) = modules.get(&schema) else {
        return rep.diverged(
            from.position,
            "schema",
            schema,
            "no module for this schema".into(),
        );
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
        // A migration record reads its new values under its target schema.
        let after_module = match rec.migration.as_ref().filter(|_| rec.is_migration()) {
            Some(mb) => match modules.get(&mb.target) {
                Some(m) => m,
                None => {
                    return rep.diverged(
                        pos,
                        "schema",
                        mb.target.clone(),
                        "no module for this schema".into(),
                    );
                }
            },
            None => module,
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
            let after = references_of(after_module, &v.entity, &v.value);
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
        module = after_module;
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
    let history = match store.schema_history() {
        Ok(h) => h,
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
        let decl = declaration_at(&history, v.created_at, &v.entity);
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
