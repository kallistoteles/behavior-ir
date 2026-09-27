//! The store semantics over a host backend (research R2, R6–R9): genesis, consistent-snapshot
//! evaluation, and guarded, atomic, idempotent commits with whole-state optimistic concurrency.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json, json};

use behavior_core::semantic::module::{Kind, Module, ParamRole};
use behavior_core::semantic::types::{Type, hash_display};
use behavior_core::{
    EvaluationFacts, FactError, RefEdge, check_entity, decode_entity, evaluate_with,
};
use behavior_verify::hashing::{TAG_TRANSITION, document_hash};

use crate::documents::{
    CommitBundle, EntityKey, EntityVersion, Evidence, Genesis, Head, LifecycleWrite, R, ReadEntry,
    RefChange, RemovedEntity, Require, StateRef, StoreError, TAG_COMMIT_BUNDLE, TAG_GENESIS,
    TAG_TRANSITION_RECORD, TransitionRecord, WriteEntry, data_version, valid_timestamp,
};
use crate::muhash::Accumulator;
use crate::{Backend, BackendError, CasOutcome};

/// The result of an evaluation against the store: the decision record, and a commit bundle only
/// for allowed decisions.
#[derive(Debug, Clone)]
pub struct Evaluation {
    pub record: Json,
    pub bundle: Option<CommitBundle>,
}

/// A successful commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub record_id: String,
    pub result_state: StateRef,
    /// The same transition had already been committed (FR-009).
    pub already: bool,
    /// `Some("structural")` when a commit authorization was bound: a structural guarantee within
    /// the current trust boundary, not proof of who issued it (FR-023).
    pub evidence_trust: Option<&'static str>,
}

/// Engine-owned store semantics over any [`Backend`]. The genesis and the store identity never
/// change, so they are read and hashed once (a commit's cost must not grow with the seed).
pub struct Store<B: Backend> {
    backend: B,
    genesis: std::sync::Arc<Genesis>,
    store_id: String,
}

fn backend_err(e: BackendError) -> StoreError {
    StoreError::Backend(e.0)
}

fn invalid(msg: impl Into<String>) -> StoreError {
    StoreError::BundleInvalid(msg.into())
}

/// The semantic declaration hash of every entity of `module`.
pub fn declarations(module: &Module) -> BTreeMap<String, String> {
    module
        .name_table()
        .iter()
        .filter(|((k, _), _)| *k == Kind::Entity)
        .map(|((_, n), h)| (n.clone(), hash_display(h)))
        .collect()
}

/// A genesis for `module`'s entity declarations with the given evidence policy and seed.
pub fn genesis_for(
    module: &Module,
    evidence_policy: crate::documents::EvidencePolicy,
    seed: Vec<crate::documents::SeedEntity>,
) -> Genesis {
    Genesis {
        format: TAG_GENESIS.into(),
        evidence_policy,
        entity_declarations: declarations(module),
        seed,
    }
}

/// Whether `key` exists at `position`: some version at or before it, and not removed at or
/// before it (feature 006, research R9).
pub fn exists_at<B: Backend + ?Sized>(
    backend: &B,
    key: &EntityKey,
    position: u64,
) -> Result<bool, BackendError> {
    Ok(backend.version_at(key, position)?.is_some()
        && backend.removed_at(key)?.is_none_or(|r| r > position))
}

/// Evaluation facts answered by the backend as of one position: the same snapshot as the bound
/// entities' `version_at` reads (feature 006, research R3).
pub struct StoreFacts<'a, B: Backend + ?Sized> {
    pub backend: &'a B,
    pub position: u64,
}

fn key(entity: &str, id: &str) -> EntityKey {
    EntityKey {
        entity: entity.to_string(),
        id: id.to_string(),
    }
}

impl<B: Backend + ?Sized> EvaluationFacts for StoreFacts<'_, B> {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        exists_at(self.backend, &key(entity, id), self.position).map_err(|e| FactError(e.0))
    }
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        self.backend
            .used_at(&key(entity, id), self.position)
            .map_err(|e| FactError(e.0))
    }
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError> {
        self.backend
            .incoming_at(&key(entity, id), self.position)
            .map_err(|e| FactError(e.0))
    }
}

/// Evaluation facts of a genesis: the seed is the whole universe and the whole history.
struct SeedFacts {
    keys: BTreeSet<EntityKey>,
    incoming: BTreeMap<EntityKey, Vec<RefEdge>>,
}

impl SeedFacts {
    fn new(keys: &BTreeSet<EntityKey>, refs: &[RefChange]) -> Self {
        let mut incoming: BTreeMap<EntityKey, Vec<RefEdge>> = BTreeMap::new();
        for r in refs {
            incoming.entry(r.target.clone()).or_default().push(RefEdge {
                entity: r.source.entity.clone(),
                id: r.source.id.clone(),
                field: r.field.clone(),
            });
        }
        SeedFacts {
            keys: keys.clone(),
            incoming,
        }
    }
}

impl EvaluationFacts for SeedFacts {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        Ok(self.keys.contains(&key(entity, id)))
    }
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        Ok(self.keys.contains(&key(entity, id)))
    }
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError> {
        Ok(self
            .incoming
            .get(&key(entity, id))
            .cloned()
            .unwrap_or_default())
    }
}

/// The reference-field values of an entity value: `(field, target type, target id)` for every
/// `Ref` field that holds an identity.
pub fn references_of(module: &Module, entity: &str, value: &Json) -> Vec<(String, EntityKey)> {
    module
        .entity(entity)
        .map(|item| {
            item.reference_fields()
                .filter_map(|(f, target)| {
                    value[f].as_str().map(|id| (f.to_string(), key(target, id)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The entity types an action touches: its state entities and the types it creates.
fn touched_types(
    module: &Module,
    action: &behavior_core::semantic::module::ActionItem,
) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = action
        .params()
        .iter()
        .filter(|p| matches!(p.role(), ParamRole::State | ParamRole::Read))
        .filter_map(|p| match p.ty() {
            Type::Entity(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    out.extend(action.creates().iter().map(|c| c.entity().to_string()));
    let _ = module;
    out
}

/// The lifecycle writes of a decision record (feature 006), in record order.
fn lifecycle_writes(record: &Json) -> R<Vec<LifecycleWrite>> {
    let mut out = Vec::new();
    for l in record["lifecycle"].as_array().into_iter().flatten() {
        let field = |k: &str| {
            l[k].as_str()
                .map(str::to_string)
                .ok_or_else(|| invalid(format!("lifecycle entry without `{k}`")))
        };
        out.push(LifecycleWrite {
            op: field("op")?,
            entity: field("entity")?,
            id: field("id")?,
        });
    }
    Ok(out)
}

fn id_of(value: &Json) -> R<String> {
    value["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| invalid("an entity needs a string `id`"))
}

impl<B: Backend> Store<B> {
    /// Creates a store from `genesis`, validating the seed against `module` (field set and types,
    /// fixed-scale grid and range, entity constraints, unique keys) and its declarations.
    pub fn create(mut backend: B, module: &Module, genesis: Genesis) -> R<Self> {
        let bad = |m: String| StoreError::GenesisInvalid(m);
        if genesis.format != TAG_GENESIS {
            return Err(bad(format!("genesis format `{}`", genesis.format)));
        }
        genesis.evidence_policy.validate()?;
        let decls = declarations(module);
        for (name, h) in &genesis.entity_declarations {
            if decls.get(name) != Some(h) {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{name}` is declared differently in the behavior than in the genesis"
                )));
            }
        }
        let mut seen = BTreeSet::new();
        let mut versions = Vec::new();
        let mut acc = Accumulator::empty();
        for s in &genesis.seed {
            let decl = genesis
                .entity_declarations
                .get(&s.entity)
                .ok_or_else(|| bad(format!("no declaration for seed entity `{}`", s.entity)))?;
            let value = decode_entity(module, &s.entity, &s.value)
                .map_err(|p| bad(format!("seed `{}`: {}", s.entity, p.join("; "))))?;
            let id = id_of(&value)?;
            let key = EntityKey {
                entity: s.entity.clone(),
                id: id.clone(),
            };
            if !seen.insert(key.clone()) {
                return Err(bad(format!("{key} appears twice in the seed")));
            }
            let mut v = EntityVersion {
                content_hash: String::new(),
                entity: s.entity.clone(),
                id,
                revision: 1,
                created_at: 0,
                value,
            };
            v.content_hash = v.content(decl).hash()?;
            acc.insert(&v.content_hash)?;
            versions.push(v);
        }
        // Seed references must point at seed entities; the initial index is derived from the seed.
        let mut seed_refs = Vec::new();
        for v in &versions {
            for (field, target) in references_of(module, &v.entity, &v.value) {
                if !seen.contains(&target) {
                    return Err(bad(format!(
                        "{}.{field} refers to {target}, which is not a seed entity",
                        v.key()
                    )));
                }
                seed_refs.push(RefChange {
                    target,
                    source: v.key(),
                    field,
                    op: "add".into(),
                });
            }
        }
        seed_refs.sort();
        // Entity constraints, with `exists` and `referenced` answered by the seed itself.
        let facts = SeedFacts::new(&seen, &seed_refs);
        for v in &versions {
            check_entity(module, &v.entity, &v.value, &facts)
                .map_err(|p| bad(format!("seed {}: {}", v.key(), p.join("; "))))?;
        }
        let state = acc.state_id()?;
        let (n, d) = acc.normalize()?.to_hex();
        let head = Head {
            state_ref: StateRef { state, position: 0 },
            acc_num: n,
            acc_den: d,
            last_record: genesis.hash()?,
        };
        backend
            .create(&genesis, &head, &versions, &seed_refs)
            .map_err(backend_err)?;
        let store_id = genesis.hash()?;
        Ok(Store {
            backend,
            genesis: std::sync::Arc::new(genesis),
            store_id,
        })
    }

    /// Opens an existing store.
    pub fn open(backend: B) -> R<Self> {
        let genesis = backend
            .genesis()
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::Backend("the store has not been created".into()))?;
        let store_id = genesis.hash()?;
        Ok(Store {
            backend,
            genesis: std::sync::Arc::new(genesis),
            store_id,
        })
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    pub fn genesis(&self) -> R<Genesis> {
        Ok((*self.genesis).clone())
    }

    /// The store identity (the genesis hash).
    pub fn store_id(&self) -> R<String> {
        Ok(self.store_id.clone())
    }

    fn head(&self) -> R<Head> {
        self.backend
            .head()
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::Backend("no head".into()))
    }

    /// The current state.
    pub fn current(&self) -> R<StateRef> {
        Ok(self.head()?.state_ref)
    }

    /// The state at `position` of this store's history.
    pub fn state_at(&self, position: u64) -> R<StateRef> {
        let head = self.head()?;
        if position == head.state_ref.position {
            return Ok(head.state_ref);
        }
        if position > head.state_ref.position {
            return Err(StoreError::EntityNotFound(format!(
                "position {position} is beyond the head"
            )));
        }
        let rec = self
            .backend
            .record(position + 1)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::Backend(format!("record {} missing", position + 1)))?;
        Ok(rec.committed_on)
    }

    /// The version of `key` valid at state `at` of this store.
    pub fn load(&self, key: &EntityKey, at: &StateRef) -> R<EntityVersion> {
        if self.state_at(at.position)? != *at {
            return Err(StoreError::EntityNotFound(format!(
                "{} at position {} is not a state of this store",
                at.state, at.position
            )));
        }
        if !exists_at(&self.backend, key, at.position).map_err(backend_err)? {
            return Err(StoreError::EntityNotFound(key.to_string()));
        }
        self.backend
            .version_at(key, at.position)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::EntityNotFound(key.to_string()))
    }

    /// Evaluates `action` against one consistent snapshot: the head is read once and every bound
    /// state entity as of that position (FR-018). `bindings` maps each state parameter to an
    /// entity id; input and context are host-supplied. Allowed decisions get a commit bundle.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
        &self,
        module: &Module,
        action: &str,
        bindings: &BTreeMap<String, String>,
        input: &Json,
        context: &Json,
        commit_time: &str,
        evidence: Option<Evidence>,
    ) -> R<Evaluation> {
        if !valid_timestamp(commit_time) {
            return Err(invalid(format!(
                "commit time `{commit_time}` is not RFC 3339 UTC"
            )));
        }
        let head = self.head()?;
        let at = head.state_ref.clone();
        let genesis = std::sync::Arc::clone(&self.genesis);
        let store = self.store_id.clone();
        let a = module
            .action(action)
            .ok_or_else(|| invalid(format!("unknown action `{action}`")))?;
        let decls = declarations(module);
        let mut state = Map::new();
        let mut loaded: BTreeMap<String, EntityVersion> = BTreeMap::new();
        for p in a.params() {
            if !matches!(p.role(), ParamRole::State | ParamRole::Read) {
                continue;
            }
            let Type::Entity(entity) = p.ty() else {
                continue;
            };
            if decls.get(entity) != genesis.entity_declarations.get(entity) {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{entity}` is declared differently in behavior {} than in the store",
                    module.behavior_version()
                )));
            }
            let id = bindings.get(p.name()).ok_or_else(|| {
                StoreError::EntityNotFound(format!("no binding for state parameter `{}`", p.name()))
            })?;
            let key = EntityKey {
                entity: entity.clone(),
                id: id.clone(),
            };
            if !exists_at(&self.backend, &key, at.position).map_err(backend_err)? {
                return Err(StoreError::EntityNotFound(key.to_string()));
            }
            let v = self
                .backend
                .version_at(&key, at.position)
                .map_err(backend_err)?
                .ok_or_else(|| StoreError::EntityNotFound(key.to_string()))?;
            state.insert(p.name().to_string(), v.value.clone());
            loaded.insert(p.name().to_string(), v);
        }
        for b in bindings.keys() {
            if !loaded.contains_key(b) {
                return Err(invalid(format!(
                    "`{b}` is not a state parameter of `{action}`"
                )));
            }
        }
        let request = json!({
            "action": action,
            "data_version": data_version(&store, &at),
            "state": state,
            "input": input,
            "context": context,
        });
        let facts = StoreFacts {
            backend: &self.backend,
            position: at.position,
        };
        let (record, observed) = evaluate_with(module, &request.to_string(), &facts);
        let record = record.as_json().clone();
        if record["result"] != "ALLOW" {
            return Ok(Evaluation {
                record,
                bundle: None,
            });
        }
        let mut touched = BTreeMap::new();
        for entity in touched_types(module, a) {
            if decls.get(&entity) != genesis.entity_declarations.get(&entity) {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{entity}` is declared differently in behavior {} than in the store",
                    module.behavior_version()
                )));
            }
            if let Some(d) = genesis.entity_declarations.get(&entity) {
                touched.insert(entity, d.clone());
            }
        }
        let bundle = CommitBundle {
            format: TAG_COMMIT_BUNDLE.into(),
            evaluated_state: at,
            behavior_version: module.behavior_version(),
            store,
            transition_hash: transition_hash(&record)?,
            entity_declarations: touched,
            read_set: read_set(&observed.fields, &loaded),
            write_set: write_set(&record)?,
            read_facts: facts_json(&observed.facts),
            write_lifecycle: lifecycle_writes(&record)?,
            record,
            commit_time: commit_time.into(),
            evidence,
        };
        Ok(Evaluation {
            record: bundle.record.clone(),
            bundle: Some(bundle),
        })
    }

    /// Commits `bundle` on `expected_parent` (research R8): idempotency, whole-state conflict,
    /// validation with engine-derived dependencies, the evidence policy, then the backend's atomic
    /// compare-and-set.
    pub fn commit(
        &mut self,
        module: &Module,
        expected_parent: &StateRef,
        bundle: &CommitBundle,
    ) -> R<Committed> {
        let genesis = std::sync::Arc::clone(&self.genesis);
        let store = self.store_id.clone();
        if bundle.format != TAG_COMMIT_BUNDLE {
            return Err(invalid(format!("bundle format `{}`", bundle.format)));
        }
        if bundle.store != store {
            return Err(invalid("the bundle was evaluated against another store"));
        }
        if bundle.record["result"] != "ALLOW" {
            return Err(StoreError::NothingToCommit(format!(
                "the decision is {}",
                bundle.record["result"]
            )));
        }
        if bundle.transition_hash != transition_hash(&bundle.record)? {
            return Err(invalid("the transition hash does not match the record"));
        }
        // 0. The module must be the exact content-addressed behavior version that produced the
        //    decision: hash(module) == bundle.behavior_version == record.behavior_version, checked
        //    before the module is used for anything.
        let version = module.behavior_version();
        if version != bundle.behavior_version
            || bundle.record["behavior_version"] != bundle.behavior_version.as_str()
        {
            return Err(invalid(format!(
                "the module is behavior {version}; the bundle and its record were made by {} and {}",
                bundle.behavior_version, bundle.record["behavior_version"]
            )));
        }
        let parent = &bundle.evaluated_state;
        // 1. Idempotency: a transition evaluated at p can only have become record p + 1.
        if let Some(r) = self
            .backend
            .record(parent.position + 1)
            .map_err(backend_err)?
            && r.committed_on == *parent
            && r.bundle.transition_hash == bundle.transition_hash
        {
            return Ok(Committed {
                record_id: r.hash()?,
                result_state: r.result_state.clone(),
                already: true,
                evidence_trust: r.authorization.as_ref().map(|_| "structural"),
            });
        }
        // 2. Consistency and whole-state conflict.
        if expected_parent != parent {
            return Err(invalid(
                "the expected parent is not the state the bundle was evaluated against",
            ));
        }
        let head = self.head()?;
        if head.state_ref != *parent {
            return Err(self.conflict(&head, parent));
        }
        // 3. Validation.
        if bundle.record["data_version"] != data_version(&store, parent).as_str() {
            return Err(invalid("the record is not bound to this store and state"));
        }
        if !valid_timestamp(&bundle.commit_time) {
            return Err(invalid("commit time is not RFC 3339 UTC"));
        }
        // Declarations: derived from the action's entity parameters, never taken from the bundle.
        // Every touched entity must be declared identically by the module and the genesis, and the
        // bundle's list must be exactly that set.
        let decls = declarations(module);
        let action_name = bundle.record["action"]["name"].as_str().unwrap_or_default();
        let action = module
            .action(action_name)
            .ok_or_else(|| invalid(format!("unknown action `{action_name}`")))?;
        let mut touched = BTreeMap::new();
        for entity in touched_types(module, action) {
            let entity = &entity;
            let in_store = genesis.entity_declarations.get(entity);
            if in_store.is_none() || decls.get(entity) != in_store {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{entity}` is declared differently in behavior {} than in the store",
                    module.behavior_version()
                )));
            }
            if let Some(d) = in_store {
                touched.insert(entity.clone(), d.clone());
            }
        }
        if bundle.entity_declarations != touched {
            return Err(invalid(
                "the bundle's entity declarations are not the action's",
            ));
        }
        let (parent_versions, derived_reads, derived_writes, derived_facts) =
            self.rederive(module, &genesis, parent, &bundle.record)?;
        if derived_facts != bundle.read_facts {
            return Err(invalid(
                "the read facts are not the ones the evaluation observed at the parent",
            ));
        }
        if lifecycle_writes(&bundle.record)? != bundle.write_lifecycle {
            return Err(invalid("the lifecycle writes are not the evaluation's"));
        }
        if derived_reads != bundle.read_set {
            return Err(invalid(
                "the read set is not the one the evaluation observed",
            ));
        }
        if derived_writes != bundle.write_set {
            return Err(invalid("the write set is not the evaluation's changes"));
        }
        for r in &bundle.read_set {
            let key = EntityKey {
                entity: r.entity.clone(),
                id: r.id.clone(),
            };
            match parent_versions.get(&key) {
                Some(v) if v.revision == r.revision => {}
                _ => {
                    return Err(invalid(format!(
                        "{key}: revision does not match the parent"
                    )));
                }
            }
        }
        let evidence_policy = genesis.evidence_policy.hash()?;
        let authorization = crate::store::check_evidence(&genesis, bundle)?;
        // 4. Build the new versions, state and record.
        let position = parent.position + 1;
        let mut new_values: BTreeMap<EntityKey, Json> = BTreeMap::new();
        for w in &bundle.write_set {
            let key = EntityKey {
                entity: w.entity.clone(),
                id: w.id.clone(),
            };
            let Some(old) = parent_versions.get(&key) else {
                return Err(StoreError::EntityUniverseChanged(format!(
                    "{key} is not in the parent state"
                )));
            };
            if old.value.get(&w.field) != Some(&w.old) {
                return Err(invalid(format!(
                    "{key}.{}: old value does not match",
                    w.field
                )));
            }
            let value = new_values.entry(key).or_insert_with(|| old.value.clone());
            value[&w.field] = w.new.clone();
        }
        let mut acc = Accumulator::from_hex(&head.acc_num, &head.acc_den)?;
        let mut new_versions = Vec::new();
        let mut ref_changes: Vec<RefChange> = Vec::new();
        let edge = |target: EntityKey, source: &EntityKey, field: &str, op: &str| RefChange {
            target,
            source: source.clone(),
            field: field.to_string(),
            op: op.to_string(),
        };
        for (key, value) in new_values {
            let old = &parent_versions[&key];
            if old.value == value {
                continue;
            }
            // Retargeted references change the derived index.
            let before = references_of(module, &key.entity, &old.value);
            let after = references_of(module, &key.entity, &value);
            for (f, t) in &before {
                if !after.contains(&(f.clone(), t.clone())) {
                    ref_changes.push(edge(t.clone(), &key, f, "drop"));
                }
            }
            for (f, t) in &after {
                if !before.contains(&(f.clone(), t.clone())) {
                    ref_changes.push(edge(t.clone(), &key, f, "add"));
                }
            }
            let decl = &genesis.entity_declarations[&key.entity];
            let mut v = EntityVersion {
                content_hash: String::new(),
                entity: key.entity.clone(),
                id: key.id.clone(),
                revision: old.revision + 1,
                created_at: position,
                value,
            };
            v.content_hash = v.content(decl).hash()?;
            acc.remove(&old.content_hash)?;
            acc.insert(&v.content_hash)?;
            new_versions.push(v);
        }
        // Lifecycle (feature 006): creations and removals, in record order.
        let mut created = Vec::new();
        let mut removed = Vec::new();
        let mut removals = Vec::new();
        for l in bundle.record["lifecycle"].as_array().into_iter().flatten() {
            let (Some(op), Some(entity), Some(id)) =
                (l["op"].as_str(), l["entity"].as_str(), l["id"].as_str())
            else {
                return Err(invalid("malformed lifecycle entry"));
            };
            let k = key(entity, id);
            let decl = genesis
                .entity_declarations
                .get(entity)
                .ok_or_else(|| StoreError::EntityDeclarationMismatch(format!("`{entity}`")))?;
            match op {
                "create" => {
                    if self
                        .backend
                        .used_at(&k, parent.position)
                        .map_err(backend_err)?
                    {
                        return Err(StoreError::EntityIdAlreadyUsed(format!(
                            "{k} was already used; an identity names one lifetime"
                        )));
                    }
                    let mut v = EntityVersion {
                        content_hash: String::new(),
                        entity: entity.to_string(),
                        id: id.to_string(),
                        revision: 1,
                        created_at: position,
                        value: l["value"].clone(),
                    };
                    v.content_hash = v.content(decl).hash()?;
                    acc.insert(&v.content_hash)?;
                    for (f, t) in references_of(module, entity, &v.value) {
                        ref_changes.push(edge(t, &k, &f, "add"));
                    }
                    created.push(v);
                }
                "remove" => {
                    if !exists_at(&self.backend, &k, parent.position).map_err(backend_err)? {
                        return Err(StoreError::EntityNotFound(format!(
                            "{k} does not exist at the parent"
                        )));
                    }
                    let old = parent_versions
                        .get(&k)
                        .cloned()
                        .ok_or_else(|| invalid(format!("{k} is not bound by the action")))?;
                    if old.value != l["value"] {
                        return Err(invalid(format!("{k}: removed value does not match")));
                    }
                    acc.remove(&old.content_hash)?;
                    for (f, t) in references_of(module, entity, &old.value) {
                        ref_changes.push(edge(t, &k, &f, "drop"));
                    }
                    removed.push(RemovedEntity {
                        entity: entity.to_string(),
                        id: id.to_string(),
                        last_revision: old.revision,
                        last_content_hash: old.content_hash.clone(),
                    });
                    removals.push(k);
                }
                other => return Err(invalid(format!("unknown lifecycle op `{other}`"))),
            }
        }
        ref_changes.sort();
        self.check_integrity(parent.position, &removals, &created, &ref_changes)?;
        let result_state = StateRef {
            state: acc.state_id()?,
            position,
        };
        let record = TransitionRecord {
            format: TAG_TRANSITION_RECORD.into(),
            position,
            previous_record: head.last_record.clone(),
            bundle: bundle.clone(),
            bundle_hash: bundle.hash()?,
            evaluated_against: parent.clone(),
            committed_on: parent.clone(),
            result_state: result_state.clone(),
            new_versions: new_versions.clone(),
            evidence_policy,
            authorization,
            created: created.clone(),
            removed,
            ref_changes: ref_changes.clone(),
        };
        let record_id = record.hash()?;
        let (n, d) = acc.normalize()?.to_hex();
        let new_head = Head {
            state_ref: result_state.clone(),
            acc_num: n,
            acc_den: d,
            last_record: record_id.clone(),
        };
        // 5. The atomic compare-and-set.
        let mut versions = new_versions;
        versions.extend(created);
        match self
            .backend
            .commit(
                &head.last_record,
                &versions,
                &removals,
                &ref_changes,
                &record,
                &new_head,
            )
            .map_err(backend_err)?
        {
            CasOutcome::Applied => Ok(Committed {
                record_id,
                result_state,
                already: false,
                evidence_trust: record.authorization.as_ref().map(|_| "structural"),
            }),
            CasOutcome::HeadMoved => {
                let now = self.head()?;
                Err(self.conflict(&now, parent))
            }
        }
    }

    /// Referential integrity on S' (feature 006, research R6): no surviving reference to a removed
    /// identity, and every added reference points at an entity that exists in S'.
    fn check_integrity(
        &self,
        parent: u64,
        removals: &[EntityKey],
        created: &[EntityVersion],
        changes: &[RefChange],
    ) -> R<()> {
        let removed: BTreeSet<&EntityKey> = removals.iter().collect();
        for t in removals {
            let mut incoming: BTreeSet<(String, String, String)> = self
                .backend
                .incoming_at(t, parent)
                .map_err(backend_err)?
                .into_iter()
                .map(|e| (e.entity, e.id, e.field))
                .collect();
            for c in changes.iter().filter(|c| c.target == *t) {
                let e = (
                    c.source.entity.clone(),
                    c.source.id.clone(),
                    c.field.clone(),
                );
                if c.op == "add" {
                    incoming.insert(e);
                } else {
                    incoming.remove(&e);
                }
            }
            if !incoming.is_empty() {
                let list: Vec<String> = incoming
                    .iter()
                    .map(|(e, id, f)| format!("{e}#{id}.{f}"))
                    .collect();
                return Err(StoreError::DanglingReference(format!(
                    "removing {t} leaves references to it: {}",
                    list.join(", ")
                )));
            }
        }
        for c in changes.iter().filter(|c| c.op == "add") {
            let alive = created.iter().any(|v| v.key() == c.target)
                || (!removed.contains(&c.target)
                    && exists_at(&self.backend, &c.target, parent).map_err(backend_err)?);
            if !alive {
                return Err(StoreError::DanglingReference(format!(
                    "{}.{} refers to {}, which does not exist",
                    c.source, c.field, c.target
                )));
            }
        }
        Ok(())
    }

    /// `STATE_CONFLICT` with the entities written since `parent`.
    fn conflict(&self, head: &Head, parent: &StateRef) -> StoreError {
        let mut changed = BTreeSet::new();
        for pos in (parent.position + 1)..=head.state_ref.position {
            if let Ok(Some(r)) = self.backend.record(pos) {
                for w in &r.bundle.write_set {
                    changed.insert(EntityKey {
                        entity: w.entity.clone(),
                        id: w.id.clone(),
                    });
                }
                for l in &r.bundle.write_lifecycle {
                    changed.insert(key(&l.entity, &l.id));
                }
            }
        }
        StoreError::StateConflict {
            current: head.state_ref.clone(),
            changed: changed.into_iter().collect(),
        }
    }

    /// Re-evaluates a decision record against the parent state: checks that its state section is
    /// the parent's content and that it reproduces byte for byte, and derives the read and write
    /// sets from the evaluation (FR-020).
    #[allow(clippy::type_complexity)]
    pub(crate) fn rederive(
        &self,
        module: &Module,
        genesis: &Genesis,
        parent: &StateRef,
        record: &Json,
    ) -> R<(
        BTreeMap<EntityKey, EntityVersion>,
        Vec<ReadEntry>,
        Vec<WriteEntry>,
        Json,
    )> {
        let action_name = record["action"]["name"]
            .as_str()
            .ok_or_else(|| invalid("the record has no action"))?;
        let a = module
            .action(action_name)
            .ok_or_else(|| invalid(format!("unknown action `{action_name}`")))?;
        let mut parent_versions = BTreeMap::new();
        let mut loaded = BTreeMap::new();
        for p in a.params() {
            if !matches!(p.role(), ParamRole::State | ParamRole::Read) {
                continue;
            }
            let Type::Entity(entity) = p.ty() else {
                continue;
            };
            if !genesis.entity_declarations.contains_key(entity) {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{entity}` is not an entity of this store"
                )));
            }
            let value = &record["state"][p.name()];
            let key = EntityKey {
                entity: entity.clone(),
                id: id_of(value)?,
            };
            if !exists_at(&self.backend, &key, parent.position).map_err(backend_err)? {
                return Err(StoreError::EntityUniverseChanged(format!(
                    "{key} is not in the parent state"
                )));
            }
            let v = self
                .backend
                .version_at(&key, parent.position)
                .map_err(backend_err)?
                .ok_or_else(|| {
                    StoreError::EntityUniverseChanged(format!("{key} is not in the parent state"))
                })?;
            if v.value != *value {
                return Err(invalid(format!(
                    "{key}: the record's state is not the parent's"
                )));
            }
            loaded.insert(p.name().to_string(), v.clone());
            parent_versions.insert(key, v);
        }
        let mut request = Map::new();
        for f in ["data_version", "state", "input", "context"] {
            request.insert(f.into(), record[f].clone());
        }
        request.insert("action".into(), json!(action_name));
        if let Some(g) = record.get("git_revision") {
            request.insert("git_revision".into(), g.clone());
        }
        // Facts are re-derived at the parent: a record whose facts differ does not reproduce.
        let facts = StoreFacts {
            backend: &self.backend,
            position: parent.position,
        };
        let (again, observed) = evaluate_with(module, &Json::Object(request).to_string(), &facts);
        if again.as_json() != record {
            return Err(invalid(
                "the record does not reproduce under its behavior version",
            ));
        }
        Ok((
            parent_versions,
            read_set(&observed.fields, &loaded),
            write_set(record)?,
            facts_json(&observed.facts),
        ))
    }
}

/// The transition's semantic identity: the feature-002 transition hash of the decision record
/// (excludes commit time and evidence).
pub fn transition_hash(record: &Json) -> R<String> {
    document_hash(TAG_TRANSITION, record).map_err(|e| invalid(e.to_string()))
}

/// The observed facts as a bundle's `read_facts` (absent when empty).
fn facts_json(f: &behavior_core::Facts) -> Json {
    if f.is_empty() {
        Json::Null
    } else {
        f.to_json()
    }
}

/// Observed reads of state parameters, grouped per entity with its revision at the evaluated state.
fn read_set(
    observed: &BTreeSet<(String, String)>,
    loaded: &BTreeMap<String, EntityVersion>,
) -> Vec<ReadEntry> {
    let mut per: BTreeMap<EntityKey, (u64, BTreeSet<String>)> = BTreeMap::new();
    for (param, field) in observed {
        if let Some(v) = loaded.get(param) {
            per.entry(v.key())
                .or_insert_with(|| (v.revision, BTreeSet::new()))
                .1
                .insert(field.clone());
        }
    }
    per.into_iter()
        .map(|(k, (revision, fields))| ReadEntry {
            entity: k.entity,
            id: k.id,
            revision,
            fields: fields.into_iter().collect(),
        })
        .collect()
}

/// The written cells of a decision record's changes (parameter names dropped).
fn write_set(record: &Json) -> R<Vec<WriteEntry>> {
    let mut out = Vec::new();
    for c in record["changes"].as_array().into_iter().flatten() {
        out.push(WriteEntry {
            entity: c["entity"]
                .as_str()
                .ok_or_else(|| invalid("change without entity"))?
                .into(),
            id: c["id"]
                .as_str()
                .ok_or_else(|| invalid("change without string id"))?
                .into(),
            field: c["field"]
                .as_str()
                .ok_or_else(|| invalid("change without field"))?
                .into(),
            old: c["old"].clone(),
            new: c["new"].clone(),
        });
    }
    out.sort_by(|a, b| (&a.entity, &a.id, &a.field).cmp(&(&b.entity, &b.id, &b.field)));
    Ok(out)
}

/// Enforces the store's evidence policy and returns the bound authorization's hash (research R9).
pub(crate) fn check_evidence(genesis: &Genesis, bundle: &CommitBundle) -> R<Option<String>> {
    match (&bundle.evidence, genesis.evidence_policy.require) {
        (None, Require::None) => Ok(None),
        (None, Require::CommitAuthorization) => Err(StoreError::EvidenceRequired(format!(
            "evidence policy {} requires a commit authorization",
            genesis.evidence_policy.hash()?
        ))),
        (Some(e), _) => crate::store::evidence::check(genesis, bundle, e).map(Some),
    }
}

pub(crate) mod evidence {
    use super::*;
    use behavior_verify::governance::{decode_policy, waiver_hash};
    use behavior_verify::hashing::{TAG_AUTHORIZATION, TAG_VERIFICATION};

    fn mismatch(msg: impl Into<String>) -> StoreError {
        StoreError::EvidenceMismatch(msg.into())
    }

    fn text(v: &Json) -> String {
        behavior_core::canonical::to_canonical_string(v).unwrap_or_default()
    }

    /// Checks supplied evidence against the bundle and the store's policy (research R9) and
    /// returns the authorization's hash. The authorization must be well formed and match this
    /// exact transition (which binds the store and the evaluated state through `data_version`);
    /// cited documents, when supplied, must hash to what it cites. This is a structural guarantee
    /// within the current trust boundary: who issued the authorization is not proven (FR-023).
    pub(crate) fn check(genesis: &Genesis, bundle: &CommitBundle, e: &Evidence) -> R<String> {
        let auth = &e.authorization;
        let claimed = auth["hash"].as_str().unwrap_or_default();
        let actual = document_hash(TAG_AUTHORIZATION, auth).map_err(|x| mismatch(x.to_string()))?;
        if claimed != actual {
            return Err(mismatch(
                "the authorization's hash does not match its content",
            ));
        }
        if auth["decision"] != "allow" {
            return Err(mismatch(format!(
                "the authorization's decision is {}",
                auth["decision"]
            )));
        }
        if auth["transition_hash"] != bundle.transition_hash.as_str() {
            return Err(mismatch("the authorization is for another transition"));
        }
        if auth["behavior_version"] != bundle.behavior_version.as_str() {
            return Err(mismatch(
                "the authorization is for another behavior version",
            ));
        }
        let policy_hash = auth["policy_hash"].as_str().unwrap_or_default();
        if let Some(allowed) = &genesis.evidence_policy.trusted_execution_policies
            && !allowed.iter().any(|p| p == policy_hash)
        {
            return Err(mismatch(format!(
                "execution policy {policy_hash} is not trusted by this store"
            )));
        }
        if let Some(p) = &e.execution_policy {
            let decoded = decode_policy(&text(p)).map_err(|x| mismatch(x.to_string()))?;
            if decoded.hash != policy_hash {
                return Err(mismatch(
                    "the supplied execution policy is not the one cited",
                ));
            }
        }
        if let Some(a) = &e.attestation {
            let mut stripped = a.clone();
            if let Some(Json::Array(checks)) = stripped.get_mut("checks") {
                for c in checks {
                    if let Json::Object(cm) = c {
                        cm.remove("cached");
                    }
                }
            }
            let h =
                document_hash(TAG_VERIFICATION, &stripped).map_err(|x| mismatch(x.to_string()))?;
            if a["hash"] != h.as_str() || auth["verification"]["attestation_hash"] != h.as_str() {
                return Err(mismatch("the supplied attestation is not the one cited"));
            }
        }
        if !e.waivers.is_empty() {
            let mut supplied = BTreeSet::new();
            for w in &e.waivers {
                supplied.insert(waiver_hash(&text(w)).map_err(|x| mismatch(x.to_string()))?);
            }
            let cited: BTreeSet<String> = auth["waivers_used"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|u| u["waiver_hash"].as_str().map(str::to_string))
                .collect();
            if supplied != cited {
                return Err(mismatch("the supplied waivers are not the ones cited"));
            }
        }
        Ok(actual)
    }
}
