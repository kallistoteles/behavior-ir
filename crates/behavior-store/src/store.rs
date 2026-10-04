//! The store semantics over a host backend (research R2, R6–R9): genesis, consistent-snapshot
//! evaluation, and guarded, atomic, idempotent commits with whole-state optimistic concurrency.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json, json};

use behavior_core::intent::{IntentError, IntentRejection};
use behavior_core::read::{
    ReadExecution, ReadSource, check_read_intent, evaluate_read_with, record_source, resolve,
};
use behavior_core::record::{ReplayResult, compare};
use behavior_core::semantic::module::{Module, ParamRole};
use behavior_core::semantic::types::Type;
use behavior_core::{
    EvaluationFacts, FactError, IndexPlan, QueryRequest, RefEdge, StoreSchema, check_entity,
    check_global_invariants, decode_entity, evaluate_with, index_hint, schema,
};
use behavior_verify::hashing::{TAG_TRANSITION, document_hash};

use crate::documents::{
    CommitBundle, DeclarationDiff, EntityKey, EntityVersion, Evidence, Genesis, Head,
    KIND_MIGRATION, LifecycleWrite, MigrationBundle, R, ReadEntry, RefChange, RemovedEntity,
    Require, RequirementOutcome, SchemaRef, StateRef, StoreError, TAG_COMMIT_BUNDLE, TAG_GENESIS,
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
    /// The genesis schema (feature 009), computed once like the store identity.
    genesis_schema: SchemaRef,
}

fn backend_err(e: BackendError) -> StoreError {
    StoreError::Backend(e.0)
}

fn invalid(msg: impl Into<String>) -> StoreError {
    StoreError::BundleInvalid(msg.into())
}

/// The semantic declaration hash of every entity of `module` (its store schema, feature 009).
pub fn declarations(module: &Module) -> BTreeMap<String, String> {
    behavior_core::schema::declarations(module)
}

/// The genesis schema of a store whose identity is `store_id` (the genesis hash).
fn genesis_schema_of(genesis: &Genesis, store_id: &str) -> SchemaRef {
    let s = StoreSchema::of(genesis.entity_declarations.clone());
    SchemaRef {
        hash: s.hash,
        declarations: s.declarations,
        since: 0,
        migration_record: store_id.to_string(),
    }
}

/// `SCHEMA_MISMATCH` between the store's schema at a position and a module's (FR-005).
pub fn schema_mismatch(store: &SchemaRef, module: &StoreSchema) -> StoreError {
    StoreError::SchemaMismatch {
        store: store.hash.clone(),
        module: module.hash.clone(),
        differing: DeclarationDiff::between(&store.declarations, &module.declarations),
    }
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

impl<B: Backend + ?Sized> StoreFacts<'_, B> {
    /// The candidates an index plan names, or `None` if the backend lacks an index it needs.
    fn candidates(&self, t: &str, plan: &IndexPlan) -> Result<Option<Vec<EntityKey>>, FactError> {
        Ok(match plan {
            IndexPlan::Eq { field, value } => self
                .backend
                .keys_by_field_at(t, field, value, self.position)
                .map_err(|e| FactError(e.0))?,
            IndexPlan::Union(a, b) => match (self.candidates(t, a)?, self.candidates(t, b)?) {
                (Some(mut x), Some(y)) => {
                    x.extend(y);
                    x.sort();
                    x.dedup();
                    Some(x)
                }
                _ => None,
            },
        })
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
    /// Evaluates the query over the entities of its type as of the position (feature 007): an
    /// indexable equality narrows the candidates, and every candidate is re-checked.
    fn query(&self, q: &QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        let t = q.node.entity();
        let hinted = match index_hint(q.node, q.env) {
            Some(plan) => self.candidates(t, &plan)?,
            None => None,
        };
        let candidates = match hinted {
            Some(keys) => keys,
            None => self
                .backend
                .keys_at(t, self.position)
                .map_err(|e| FactError(e.0))?,
        };
        let mut out = Vec::new();
        for k in candidates {
            let Some(v) = self
                .backend
                .version_at(&k, self.position)
                .map_err(|e| FactError(e.0))?
            else {
                continue;
            };
            if q.matches(&v.value)? {
                out.push(k.id);
            }
        }
        Ok(out)
    }
    /// One version read for the whole entity (feature 010, research R8).
    fn entity(&self, entity: &str, id: &str) -> Result<Json, FactError> {
        self.backend
            .version_at(&key(entity, id), self.position)
            .map_err(|e| FactError(e.0))?
            .map(|v| v.value)
            .ok_or_else(|| FactError(format!("{entity}#{id} is not in the state")))
    }
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        self.backend
            .version_at(&key(entity, id), self.position)
            .map_err(|e| FactError(e.0))?
            .and_then(|v| v.value.get(field).cloned())
            .ok_or_else(|| FactError(format!("{entity}#{id}.{field} is not in the state")))
    }
}

/// Evaluation facts of a genesis: the seed is the whole universe and the whole history.
struct SeedFacts {
    keys: BTreeSet<EntityKey>,
    incoming: BTreeMap<EntityKey, Vec<RefEdge>>,
    values: BTreeMap<EntityKey, Json>,
}

impl SeedFacts {
    fn new(versions: &[EntityVersion], keys: &BTreeSet<EntityKey>, refs: &[RefChange]) -> Self {
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
            values: versions
                .iter()
                .map(|v| (v.key(), v.value.clone()))
                .collect(),
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
    fn query(&self, q: &QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        let mut out = Vec::new();
        for (k, v) in self
            .values
            .iter()
            .filter(|(k, _)| k.entity == q.node.entity())
        {
            if q.matches(v)? {
                out.push(k.id.clone());
            }
        }
        Ok(out)
    }
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        self.values
            .get(&key(entity, id))
            .and_then(|v| v.get(field))
            .cloned()
            .ok_or_else(|| FactError(format!("{entity}#{id}.{field} is not in the seed")))
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
    // Queried types are read too (feature 007): their declarations must match the store's.
    out.extend(behavior_core::queried_types(module, action));
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
        // The genesis schema is exactly the module's store schema (feature 009, FR-005).
        let genesis_schema = genesis.schema()?;
        let module_schema = schema(module);
        if genesis_schema.hash != module_schema.hash {
            return Err(schema_mismatch(&genesis_schema, &module_schema));
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
        let facts = SeedFacts::new(&versions, &seen, &seed_refs);
        for v in &versions {
            check_entity(module, &v.entity, &v.value, &facts)
                .map_err(|p| bad(format!("seed {}: {}", v.key(), p.join("; "))))?;
        }
        // Module invariants hold at genesis (feature 007).
        check_global_invariants(module, &facts).map_err(|p| bad(p.join("; ")))?;
        let state = acc.state_id()?;
        let (n, d) = acc.normalize()?.to_hex();
        let head = Head {
            state_ref: StateRef { state, position: 0 },
            acc_num: n,
            acc_den: d,
            last_record: genesis.hash()?,
            schema: None,
        };
        backend
            .create(&genesis, &head, &versions, &seed_refs)
            .map_err(backend_err)?;
        let store_id = genesis.hash()?;
        Ok(Store {
            backend,
            genesis_schema: genesis_schema_of(&genesis, &store_id),
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
            genesis_schema: genesis_schema_of(&genesis, &store_id),
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

    /// The schema in force at the head: the last migration's, or the genesis schema.
    fn head_schema(&self, head: &Head) -> R<SchemaRef> {
        match &head.schema {
            Some(s) => Ok(s.clone()),
            None => Ok(self.genesis_schema.clone()),
        }
    }

    /// The schema in force at `position` (FR-002): found by walking back from the head through
    /// migration records only, never by scanning ordinary records (research R3).
    pub(crate) fn schema_at_position(&self, position: u64) -> R<SchemaRef> {
        let head = self.head()?;
        let mut s = self.head_schema(&head)?;
        while s.since > position {
            s = self.previous_schema(&s)?;
        }
        Ok(s)
    }

    /// The schema in force before the migration that introduced `s` (the migration record at
    /// `s.since` names it; absent there means the genesis schema).
    fn previous_schema(&self, s: &SchemaRef) -> R<SchemaRef> {
        let rec = self
            .backend
            .record(s.since)
            .map_err(backend_err)?
            .ok_or_else(|| StoreError::Backend(format!("record {} missing", s.since)))?;
        match &rec.migration {
            Some(m) if rec.is_migration() => match &m.previous_schema {
                Some(p) => Ok(p.clone()),
                None => Ok(self.genesis_schema.clone()),
            },
            _ => Err(StoreError::Backend(format!(
                "the record at position {} does not introduce a schema",
                s.since
            ))),
        }
    }

    /// The schema under which state `at` of this store is valid (FR-001, FR-002).
    pub fn schema_at(&self, at: &StateRef) -> R<SchemaRef> {
        if self.state_at(at.position)? != *at {
            return Err(StoreError::EntityNotFound(format!(
                "{} at position {} is not a state of this store",
                at.state, at.position
            )));
        }
        self.schema_at_position(at.position)
    }

    /// Every schema this store has had, oldest first, each with the position it applies from.
    pub fn schema_history(&self) -> R<Vec<SchemaRef>> {
        let head = self.head()?;
        let mut s = self.head_schema(&head)?;
        let mut out = vec![s.clone()];
        while s.since > 0 {
            s = self.previous_schema(&s)?;
            out.push(s.clone());
        }
        out.reverse();
        Ok(out)
    }

    /// The store's schema at `position`, which `module` must declare exactly (FR-005).
    fn bind_schema(&self, module: &Module, position: u64) -> R<SchemaRef> {
        let at = self.schema_at_position(position)?;
        let m = schema(module);
        if m.hash != at.hash {
            return Err(schema_mismatch(&at, &m));
        }
        Ok(at)
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

    /// Reads (feature 010): evaluates `source` against one exact state of this store, `at` or the
    /// head (read once), and returns its record. A read never writes: it takes `&self`, and every
    /// write of the backend needs `&mut` (FR-012). `bindings` maps each `state` parameter to an
    /// entity id; an id that does not exist at the position gives an `INVALID_BINDING` record.
    /// A schema mismatch, or a position that is not a state of this store, is an error with no
    /// record (FR-009, FR-013).
    pub fn read(
        &self,
        module: &Module,
        source: &ReadSource,
        bindings: &BTreeMap<String, String>,
        input: &Json,
        context: &Json,
        at: Option<&StateRef>,
    ) -> R<ReadExecution> {
        let at = match at {
            None => self.head()?.state_ref,
            Some(a) => {
                if self.state_at(a.position)? != *a {
                    return Err(StoreError::EntityNotFound(format!(
                        "{} at position {} is not a state of this store",
                        a.state, a.position
                    )));
                }
                a.clone()
            }
        };
        self.bind_schema(module, at.position)?;
        let mut state = Map::new();
        if let Some(read) = resolve(module, source) {
            for b in bindings.keys() {
                let is_state = read
                    .params()
                    .iter()
                    .any(|p| p.name() == b && p.role() == ParamRole::State);
                if !is_state {
                    return Err(invalid(format!(
                        "`{b}` is not a state parameter of read `{}`",
                        read.name()
                    )));
                }
            }
            for p in read.params() {
                let (ParamRole::State, Type::Entity(entity)) = (p.role(), p.ty()) else {
                    continue;
                };
                let Some(id) = bindings.get(p.name()) else {
                    continue;
                };
                let key = EntityKey {
                    entity: entity.clone(),
                    id: id.clone(),
                };
                let value = if exists_at(&self.backend, &key, at.position).map_err(backend_err)? {
                    self.backend
                        .version_at(&key, at.position)
                        .map_err(backend_err)?
                        .map(|v| v.value)
                } else {
                    None
                };
                // An identity that does not exist is bound by id alone; evaluation refuses it.
                state.insert(
                    p.name().to_string(),
                    value.unwrap_or_else(|| json!({"id": id})),
                );
            }
        }
        let request = json!({
            "data_version": data_version(&self.store_id, &at),
            "state": state,
            "input": input,
            "context": context,
        });
        let facts = StoreFacts {
            backend: &self.backend,
            position: at.position,
        };
        Ok(evaluate_read_with(
            module,
            source,
            &request.to_string(),
            &facts,
        ))
    }

    /// A read intent (feature 010) against this store, at `at` or the head: the capability
    /// boundary for untrusted callers. The intent is validated against the declared read, and
    /// every target must exist at the position (`UNKNOWN_TARGET`); all problems are listed
    /// before anything is evaluated. Context comes from the host.
    pub fn read_intent(
        &self,
        module: &Module,
        intent: &str,
        context: &Json,
        at: Option<&StateRef>,
    ) -> R<Result<ReadExecution, IntentRejection>> {
        let at = match at {
            None => self.head()?.state_ref,
            Some(a) => {
                if self.state_at(a.position)? != *a {
                    return Err(StoreError::EntityNotFound(format!(
                        "{} at position {} is not a state of this store",
                        a.state, a.position
                    )));
                }
                a.clone()
            }
        };
        let (parsed, mut errors) = check_read_intent(module, intent, None);
        if let Some(i) = &parsed
            && let Some(read) = module.read(&i.capability)
        {
            for (param, id) in &i.targets {
                let Some(Type::Entity(entity)) = read
                    .params()
                    .iter()
                    .find(|p| p.name() == param && p.role() == ParamRole::State)
                    .map(|p| p.ty())
                else {
                    continue;
                };
                let key = EntityKey {
                    entity: entity.clone(),
                    id: id.clone(),
                };
                if !exists_at(&self.backend, &key, at.position).map_err(backend_err)? {
                    errors.push(IntentError {
                        code: "UNKNOWN_TARGET".into(),
                        message: format!("{key} does not exist at the read's state"),
                        path: format!("targets.{param}"),
                    });
                }
            }
        }
        let intent = match parsed {
            Some(i) if errors.is_empty() => i,
            _ => return Ok(Err(IntentRejection::of(errors))),
        };
        self.read(
            module,
            &ReadSource::Declared(intent.capability),
            &intent.targets,
            &intent.input,
            context,
            Some(&at),
        )
        .map(Ok)
    }

    /// Replays a read record against this store (FR-011): the record's `data_version` must name
    /// a state of this store; the read is evaluated again at that position, with facts derived
    /// from the store, and compared byte for byte. Differences, including a foreign store or
    /// state, are reported in the result; only backend failures are errors.
    pub fn replay_read(&self, module: &Module, record: &str) -> R<ReplayResult> {
        let stored: Json = match serde_json::from_str(record) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ReplayResult::mismatch(format!(
                    "record is not valid JSON: {e}"
                )));
            }
        };
        let dv = stored["data_version"].as_str().unwrap_or_default();
        let Some(at) = self.state_of_data_version(dv)? else {
            return Ok(ReplayResult::mismatch(format!(
                "data_version: \"{dv}\" is not a state of this store"
            )));
        };
        let source = match record_source(module, &stored) {
            Ok(s) => s,
            Err(diff) => return Ok(ReplayResult::mismatch(diff)),
        };
        let mut bindings = BTreeMap::new();
        if let Some(state) = stored["state"].as_object() {
            for (param, value) in state {
                if let Some(id) = value.get("id").and_then(Json::as_str) {
                    bindings.insert(param.clone(), id.to_string());
                }
            }
        }
        let empty = json!({});
        match self.read(
            module,
            &source,
            &bindings,
            stored.get("input").unwrap_or(&empty),
            stored.get("context").unwrap_or(&empty),
            Some(&at),
        ) {
            Ok(x) => Ok(compare(&stored, x.record.as_json())),
            Err(StoreError::Backend(m)) => Err(StoreError::Backend(m)),
            Err(e) => Ok(ReplayResult::mismatch(format!("the read was refused: {e}"))),
        }
    }

    /// The state a `data_version` names, if it is a state of this store.
    fn state_of_data_version(&self, dv: &str) -> R<Option<StateRef>> {
        let parts: Vec<&str> = dv.split(';').collect();
        let [store, state, position] = parts.as_slice() else {
            return Ok(None);
        };
        let (Some(store), Some(state), Some(position)) = (
            store.strip_prefix("store:"),
            state.strip_prefix("state:"),
            position
                .strip_prefix("position:")
                .and_then(|p| p.parse::<u64>().ok()),
        ) else {
            return Ok(None);
        };
        if store != self.store_id || position > self.head()?.state_ref.position {
            return Ok(None);
        }
        let at = self.state_at(position)?;
        Ok((at.state == state).then_some(at))
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
        // Exact store-schema binding, before anything is evaluated (FR-005).
        let at_schema = self.bind_schema(module, at.position)?;
        let store = self.store_id.clone();
        let a = module
            .action(action)
            .ok_or_else(|| invalid(format!("unknown action `{action}`")))?;
        let mut state = Map::new();
        let mut loaded: BTreeMap<String, EntityVersion> = BTreeMap::new();
        for p in a.params() {
            if !matches!(p.role(), ParamRole::State | ParamRole::Read) {
                continue;
            }
            let Type::Entity(entity) = p.ty() else {
                continue;
            };
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
            if let Some(d) = at_schema.declarations.get(&entity) {
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
        // Exact store-schema binding at the evaluated position, before the module is used
        // (FR-005).
        let at_schema = self.bind_schema(module, bundle.evaluated_state.position)?;
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
            && r.bundle
                .as_ref()
                .is_some_and(|b| b.transition_hash == bundle.transition_hash)
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
        // Every touched entity must be declared by the store's schema at the parent (which the
        // module declares exactly), and the bundle's list must be exactly that set.
        let action_name = bundle.record["action"]["name"].as_str().unwrap_or_default();
        let action = module
            .action(action_name)
            .ok_or_else(|| invalid(format!("unknown action `{action_name}`")))?;
        let mut touched = BTreeMap::new();
        for entity in touched_types(module, action) {
            let Some(d) = at_schema.declarations.get(&entity) else {
                return Err(StoreError::EntityDeclarationMismatch(format!(
                    "`{entity}` is not an entity type of the store's schema"
                )));
            };
            touched.insert(entity, d.clone());
        }
        if bundle.entity_declarations != touched {
            return Err(invalid(
                "the bundle's entity declarations are not the action's",
            ));
        }
        let (parent_versions, derived_reads, derived_writes, derived_facts) =
            self.rederive(module, &at_schema.declarations, parent, &bundle.record)?;
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
            let decl = at_schema.declarations.get(&key.entity).ok_or_else(|| {
                StoreError::EntityDeclarationMismatch(format!("`{}`", key.entity))
            })?;
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
            let decl = at_schema
                .declarations
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
            kind: None,
            bundle: Some(bundle.clone()),
            migration: None,
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
            schema: head.schema.clone(),
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

    /// Applies `migration` from `source` to `target` as one atomic transition at the next position
    /// (feature 009, FR-011, research R6). The store's current schema must be the migration's
    /// source; every entity at the head is read, the migration is applied to that complete,
    /// immutable universe (source validation, requirements, entity-local transforms, target
    /// validation, none of which any policy can skip), and the new versions, reference-index
    /// changes, migration record and a head naming the new schema are committed with one
    /// compare-and-set.
    pub fn migrate(
        &mut self,
        migration: &behavior_core::migration::Migration,
        source: &Module,
        target: &Module,
        commit_time: &str,
        evidence: Option<Evidence>,
    ) -> R<Committed> {
        use behavior_core::migration::{MigrationRefusal, SourceEntity, apply_migration};
        if !valid_timestamp(commit_time) {
            return Err(invalid(format!(
                "commit time `{commit_time}` is not RFC 3339 UTC"
            )));
        }
        let head = self.head()?;
        let parent = head.state_ref.clone();
        let current = self.head_schema(&head)?;
        if current.hash != migration.source_schema() {
            return Err(StoreError::Migration(MigrationRefusal {
                code: "MIGRATION_SCHEMA_MISMATCH",
                message: format!(
                    "the store's current schema is {}; migration {} applies to {}",
                    current.hash,
                    migration.hash(),
                    migration.source_schema()
                ),
                rule: None,
                entities: Vec::new(),
                count: 0,
            }));
        }
        // The complete source universe at the head.
        let mut parent_versions: BTreeMap<EntityKey, EntityVersion> = BTreeMap::new();
        let mut entities = Vec::new();
        for t in current.declarations.keys() {
            let mut keys = self
                .backend
                .keys_at(t, parent.position)
                .map_err(backend_err)?;
            keys.sort();
            for k in keys {
                let v = self
                    .backend
                    .version_at(&k, parent.position)
                    .map_err(backend_err)?
                    .ok_or_else(|| StoreError::EntityNotFound(k.to_string()))?;
                entities.push(SourceEntity {
                    entity: k.entity.clone(),
                    value: v.value.clone(),
                });
                parent_versions.insert(k, v);
            }
        }
        let out =
            apply_migration(migration, source, target, &entities).map_err(StoreError::Migration)?;
        let (_, target_schema) = migration.schemas();
        let evidence_policy = self.genesis.evidence_policy.hash()?;
        let authorization = self.migration_evidence(evidence.as_ref(), migration, &parent)?;
        let verification = evidence
            .as_ref()
            .map(|e| e.authorization["verification"].clone())
            .filter(|v| !v.is_null());
        // New versions under the target declarations, and the reference-index changes.
        let position = parent.position + 1;
        let mut acc = Accumulator::from_hex(&head.acc_num, &head.acc_den)?;
        let mut new_versions = Vec::new();
        let mut ref_changes: Vec<RefChange> = Vec::new();
        for m in out.entities.iter().filter(|m| m.migrated) {
            let k = key(&m.entity, &m.id);
            let old = parent_versions
                .get(&k)
                .ok_or_else(|| StoreError::EntityNotFound(k.to_string()))?;
            let decl = target_schema
                .declarations
                .get(&m.entity)
                .ok_or_else(|| StoreError::EntityDeclarationMismatch(format!("`{}`", m.entity)))?;
            let mut v = EntityVersion {
                content_hash: String::new(),
                entity: m.entity.clone(),
                id: m.id.clone(),
                revision: old.revision + 1,
                created_at: position,
                value: m.value.clone(),
            };
            v.content_hash = v.content(decl).hash()?;
            acc.remove(&old.content_hash)?;
            acc.insert(&v.content_hash)?;
            let before = references_of(source, &k.entity, &old.value);
            let after = references_of(target, &k.entity, &v.value);
            for (f, t) in &before {
                if !after.contains(&(f.clone(), t.clone())) {
                    ref_changes.push(RefChange {
                        target: t.clone(),
                        source: k.clone(),
                        field: f.clone(),
                        op: "drop".into(),
                    });
                }
            }
            for (f, t) in &after {
                if !before.contains(&(f.clone(), t.clone())) {
                    ref_changes.push(RefChange {
                        target: t.clone(),
                        source: k.clone(),
                        field: f.clone(),
                        op: "add".into(),
                    });
                }
            }
            new_versions.push(v);
        }
        ref_changes.sort();
        let result_state = StateRef {
            state: acc.state_id()?,
            position,
        };
        let bundle = MigrationBundle {
            migration_hash: migration.hash(),
            source: current.hash.clone(),
            target: target_schema.hash.clone(),
            target_declarations: target_schema.declarations.clone(),
            previous_schema: head.schema.clone(),
            requirements: out
                .requirements
                .iter()
                .map(|r| RequirementOutcome {
                    name: r.name.clone(),
                    held: r.held,
                })
                .collect(),
            report: out.report.clone(),
            source_validated: true,
            target_validated: true,
            verification,
            commit_time: commit_time.to_string(),
        };
        let record = TransitionRecord {
            format: TAG_TRANSITION_RECORD.into(),
            position,
            previous_record: head.last_record.clone(),
            kind: Some(KIND_MIGRATION.into()),
            bundle: None,
            bundle_hash: bundle.hash()?,
            migration: Some(bundle),
            evaluated_against: parent.clone(),
            committed_on: parent.clone(),
            result_state: result_state.clone(),
            new_versions: new_versions.clone(),
            evidence_policy,
            authorization,
            created: Vec::new(),
            removed: Vec::new(),
            ref_changes: ref_changes.clone(),
        };
        let record_id = record.hash()?;
        let (n, d) = acc.normalize()?.to_hex();
        let new_head = Head {
            state_ref: result_state.clone(),
            acc_num: n,
            acc_den: d,
            last_record: record_id.clone(),
            schema: Some(SchemaRef {
                hash: target_schema.hash.clone(),
                declarations: target_schema.declarations.clone(),
                since: position,
                migration_record: record_id.clone(),
            }),
        };
        match self
            .backend
            .commit(
                &head.last_record,
                &new_versions,
                &[],
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
                Err(self.conflict(&now, &parent))
            }
        }
    }

    /// The governance evidence a migration needs under the store's evidence policy (FR-019b):
    /// its `migration` section, or what actions need. Returns the bound authorization's hash.
    fn migration_evidence(
        &self,
        evidence: Option<&Evidence>,
        migration: &behavior_core::migration::Migration,
        parent: &StateRef,
    ) -> R<Option<String>> {
        let (require, trusted) = self.genesis.evidence_policy.for_migration();
        match (evidence, require) {
            (None, Require::None) => Ok(None),
            (None, Require::CommitAuthorization) => Err(StoreError::EvidenceRequired(format!(
                "evidence policy {} requires a commit authorization for migrations",
                self.genesis.evidence_policy.hash()?
            ))),
            (Some(e), _) => evidence::check_migration(
                &data_version(&self.store_id, parent),
                migration,
                trusted,
                e,
            )
            .map(Some),
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
                if let Some(b) = &r.bundle {
                    for w in &b.write_set {
                        changed.insert(key(&w.entity, &w.id));
                    }
                    for l in &b.write_lifecycle {
                        changed.insert(key(&l.entity, &l.id));
                    }
                }
                // A migration rewrites every entity of its changed types (feature 009).
                for v in &r.new_versions {
                    changed.insert(v.key());
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
        declarations: &BTreeMap<String, String>,
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
            if !declarations.contains_key(entity) {
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
        return Json::Null;
    }
    let mut j = f.to_json();
    // Each query instance carries the hash of its members (feature 007, FR-014/FR-015).
    if let Some(Json::Array(queries)) = j.get_mut("queries") {
        for q in queries {
            let members = q["members"].clone();
            let h = document_hash(TAG_QUERY_RESULT, &members).unwrap_or_default();
            if let Json::Object(m) = q {
                m.insert("result_hash".into(), json!(h));
            }
        }
    }
    j
}

/// The domain tag of a query result hash.
pub const TAG_QUERY_RESULT: &str = "behavior.query_result.v1";

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
        let actual = sealed(auth)?;
        if auth.get("kind").is_some() {
            return Err(mismatch("the authorization is not for an action"));
        }
        if auth["transition_hash"] != bundle.transition_hash.as_str() {
            return Err(mismatch("the authorization is for another transition"));
        }
        if auth["behavior_version"] != bundle.behavior_version.as_str() {
            return Err(mismatch(
                "the authorization is for another behavior version",
            ));
        }
        cited(
            genesis.evidence_policy.trusted_execution_policies.as_ref(),
            e,
        )?;
        Ok(actual)
    }

    /// Checks migration evidence (feature 009): the authorization must be for this migration on
    /// this store state (`data_version`), from a trusted execution policy, citing the supplied
    /// documents.
    pub(crate) fn check_migration(
        data_version: &str,
        migration: &behavior_core::migration::Migration,
        trusted: Option<&Vec<String>>,
        e: &Evidence,
    ) -> R<String> {
        let auth = &e.authorization;
        let actual = sealed(auth)?;
        if auth["kind"] != "migration" {
            return Err(mismatch("the authorization is not for a migration"));
        }
        if auth["migration_hash"] != migration.hash().as_str() {
            return Err(mismatch("the authorization is for another migration"));
        }
        if auth["data_version"] != data_version {
            return Err(mismatch("the authorization is for another store or state"));
        }
        cited(trusted, e)?;
        Ok(actual)
    }

    /// The authorization's hash, if it matches its content and allows the commit.
    fn sealed(auth: &Json) -> R<String> {
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
        Ok(actual)
    }

    /// The execution policy is trusted, and every supplied document is the one cited.
    fn cited(trusted: Option<&Vec<String>>, e: &Evidence) -> R<()> {
        let auth = &e.authorization;
        let policy_hash = auth["policy_hash"].as_str().unwrap_or_default();
        if let Some(allowed) = trusted
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
        Ok(())
    }
}
