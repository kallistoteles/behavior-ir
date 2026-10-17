//! Resolution: `WorkQueueInput` → `ResolvedWorkQueue` (data model §1–§3; research R5).
//!
//! The generated names, the work entity's schema, the generated parameters of every capability
//! and the embedded StateMachine input are computed here and nowhere else. The declarations
//! follow from the input alone (law D1); resolution then checks the worker, the time type and
//! the capabilities against the declaration environment.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, Role, WParam, WType};

use crate::approval::resolve::{is_core_identifier, location};
use crate::composition::{Contribution, DeclaredEnum, DeclaredSchema};
use crate::error::ModelError;
use crate::ids::{SemanticId, encode, encode_entity_name};
use crate::state_machine::input::{DisplayMeta, ModelInput, SourceLoc, TransitionInput};
use crate::state_machine::resolve::{ResolvedStateMachine, resolve as resolve_machine, snake_case};
use crate::work_queue::input::WorkQueueInput;

pub const STATUS: &str = "status";
pub const ATTEMPTS: &str = "attempts";
pub const OWNER: &str = "owner";
pub const LEASE_UNTIL: &str = "lease_until";
pub const AVAILABLE_AT: &str = "available_at";
/// Core's implicit identity field, and enqueue's identity input.
pub const ID: &str = "id";
pub const WORKER: &str = "worker";
pub const NOW: &str = "now";
pub const GENERATION: &str = "generation";
pub const EXPECTED_GENERATION: &str = "expected_generation";

/// The status cases, in declaration order (they fix the enum's Core hash).
pub const STATUSES: [&str; 5] = ["READY", "LEASED", "SUCCEEDED", "FAILED", "CANCELLED"];
/// The terminal cases (spec FR-013).
pub const TERMINAL: [&str; 3] = ["SUCCEEDED", "FAILED", "CANCELLED"];

/// The nine capabilities and whether each needs an authored authorization guard (FR-021).
pub const CAPABILITIES: [(&str, bool); 9] = [
    ("enqueue", true),
    ("claim", true),
    ("renew", false),
    ("complete", false),
    ("retry", false),
    ("fail", false),
    ("recover", true),
    ("exhaust", true),
    ("cancel", true),
];

/// The transitions of the embedded machine: `(id, from, to)` (data model §4).
pub const TRANSITIONS: [(&str, &[&str], &str); 8] = [
    ("claim", &["READY"], "LEASED"),
    ("renew", &["LEASED"], "LEASED"),
    ("complete", &["LEASED"], "SUCCEEDED"),
    ("retry", &["LEASED"], "READY"),
    ("fail", &["LEASED"], "FAILED"),
    ("recover", &["LEASED"], "READY"),
    ("exhaust", &["LEASED"], "FAILED"),
    ("cancel", &["READY", "LEASED"], "CANCELLED"),
];

/// Every parameter name the model generates; authored parameters may not use them (FR-022).
pub const RESERVED_PARAMETERS: [&str; 7] = [
    ID,
    WORKER,
    NOW,
    LEASE_UNTIL,
    AVAILABLE_AT,
    GENERATION,
    EXPECTED_GENERATION,
];

/// The field names the model generates; a payload field may not use them.
pub const GENERATED_FIELDS: [&str; 6] = [STATUS, ATTEMPTS, OWNER, LEASE_UNTIL, AVAILABLE_AT, ID];

/// `(enum, entity)` of queue `q`: `E = EncodeEntityName(q)` and `E ++ "Status"`. Injective like
/// the RequestFlow and Approval names; a cross-kind collision is a `DECLARATION_COLLISION`.
pub fn work_names(id: &SemanticId) -> (String, String) {
    let e = encode_entity_name(id);
    (format!("{e}Status"), e)
}

/// A parameter mapping as a list `(name, type)`.
pub type Params = Vec<(String, WType)>;

/// The generated `(inputs, context)` of capability `c` (data model §3), for worker `a` and time
/// `t`. Enqueue's payload inputs are added by the caller.
pub fn generated_params(c: &str, a: &str, t: &WType) -> (Params, Params) {
    let worker = || (WORKER.to_string(), WType::Id(a.to_string()));
    let now = || (NOW.to_string(), t.clone());
    let time = |n: &str| (n.to_string(), t.clone());
    let int = |n: &str| (n.to_string(), WType::Int);
    match c {
        "enqueue" => (vec![time(AVAILABLE_AT)], vec![]),
        "claim" => (
            vec![int(EXPECTED_GENERATION)],
            vec![worker(), now(), time(LEASE_UNTIL)],
        ),
        "renew" => (
            vec![int(GENERATION)],
            vec![worker(), now(), time(LEASE_UNTIL)],
        ),
        "complete" | "fail" => (vec![int(GENERATION)], vec![worker(), now()]),
        "retry" => (
            vec![int(GENERATION)],
            vec![worker(), now(), time(AVAILABLE_AT)],
        ),
        "recover" => (vec![], vec![now(), time(AVAILABLE_AT)]),
        "exhaust" => (vec![], vec![now()]),
        _ => (vec![], vec![]),
    }
}

/// A resolved, validated, canonical work queue.
#[derive(Debug, Clone)]
pub struct ResolvedWorkQueue {
    pub queue_id: SemanticId,
    /// `<E>Status`
    pub enum_name: String,
    /// `E`
    pub entity: String,
    /// `A`
    pub worker: String,
    /// `T`
    pub time: WType,
    /// `M`
    pub max_attempts: i64,
    /// `Π`
    pub payload: BTreeMap<String, WType>,
    /// The work entity's fields, sorted by name.
    pub schema: Vec<(String, WType)>,
    /// The nine capabilities, by id.
    pub capabilities: BTreeMap<String, ResolvedCapability>,
    /// The embedded machine `(E, status, {READY}, 𝒯)`, resolved by the StateMachine resolver.
    pub machine: ResolvedStateMachine,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// A resolved capability: the author's parameters only (the generated ones follow from the id).
#[derive(Debug, Clone)]
pub struct ResolvedCapability {
    pub capability_id: SemanticId,
    pub inputs: BTreeMap<String, WType>,
    pub context: BTreeMap<String, WType>,
    pub state: BTreeMap<String, String>,
    /// `encode(q, capability_id)`
    pub action_name: String,
    pub needs_authorization: bool,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

impl ResolvedWorkQueue {
    pub fn capability_ref(&self, c: &str) -> String {
        format!("{}/capability/{c}", self.queue_id)
    }

    /// The subject parameter of every transition: `snake_case(E)`.
    pub fn subject_param(&self) -> &str {
        &self.machine.subject_param
    }

    /// Enqueue's parameters (data model §3): `id`, then inputs (generated `available_at`, the
    /// payload fields and the authored inputs), state bindings and context, each sorted by name.
    pub fn enqueue_params(&self) -> Vec<WParam> {
        let Some(c) = self.capabilities.get("enqueue") else {
            return Vec::new();
        };
        let mut inputs: BTreeMap<String, WType> = c.inputs.clone();
        inputs.insert(AVAILABLE_AT.into(), self.time.clone());
        inputs.extend(self.payload.iter().map(|(n, t)| (n.clone(), t.clone())));
        let mut ps = vec![WParam {
            name: ID.into(),
            role: Some(Role::Input),
            ty: WType::Id(self.entity.clone()),
        }];
        ps.extend(inputs.into_iter().map(|(name, ty)| WParam {
            name,
            role: Some(Role::Input),
            ty,
        }));
        ps.extend(c.state.iter().map(|(name, e)| WParam {
            name: name.clone(),
            role: Some(Role::State),
            ty: WType::Entity(e.clone()),
        }));
        ps.extend(c.context.iter().map(|(name, ty)| WParam {
            name: name.clone(),
            role: Some(Role::Context),
            ty: ty.clone(),
        }));
        ps
    }
}

fn max_attempts(model_ref: &str, value: i64, errors: &mut Vec<ModelError>) {
    if value < 1 || value == i64::MAX {
        errors.push(ModelError::InvalidMaxAttempts {
            model_ref: model_ref.to_string(),
            value,
        });
    }
}

/// What the input alone determines (law D1): the id, `M`, and the contributions
/// `[Enum <E>Status, Entity E]`.
pub fn input_contributions(input: &WorkQueueInput) -> Result<Vec<Contribution>, Vec<ModelError>> {
    let model_ref = input.queue_id.clone();
    let Some(id) = SemanticId::parse(&input.queue_id) else {
        return Err(vec![ModelError::InvalidSemanticId {
            model_ref: model_ref.clone(),
            id: input.queue_id.clone(),
        }]);
    };
    let mut errors = Vec::new();
    max_attempts(&model_ref, input.max_attempts, &mut errors);
    let (enum_name, entity) = work_names(&id);
    let mut fields: BTreeMap<String, WType> = BTreeMap::new();
    fields.insert(ATTEMPTS.into(), WType::Int);
    fields.insert(AVAILABLE_AT.into(), input.time.clone());
    fields.insert(
        LEASE_UNTIL.into(),
        WType::Option(Box::new(input.time.clone())),
    );
    fields.insert(
        OWNER.into(),
        WType::Option(Box::new(WType::Id(input.worker_entity.clone()))),
    );
    fields.insert(STATUS.into(), WType::Enum(enum_name.clone()));
    let mut seen = BTreeSet::new();
    for (name, ty) in &input.payload {
        if GENERATED_FIELDS.contains(&name.as_str()) {
            errors.push(ModelError::PayloadFieldReserved {
                model_ref: model_ref.clone(),
                name: name.clone(),
            });
        } else if !is_core_identifier(name) || !seen.insert(name.clone()) {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: name.clone(),
            });
        } else {
            fields.insert(name.clone(), ty.clone());
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let loc = location(&input.source, &input.queue_id);
    Ok(vec![
        Contribution::Enum(DeclaredEnum {
            model_id: model_ref.clone(),
            name: enum_name,
            cases: STATUSES.iter().map(|c| c.to_string()).collect(),
            loc: loc.clone(),
        }),
        Contribution::Entity(DeclaredSchema {
            model_id: model_ref,
            entity,
            // Sorted by field name: payload order never reaches the entity hash.
            fields: fields.into_iter().collect(),
            loc,
        }),
    ])
}

/// The contributions, checked against `module`: the worker must be an entity and the time type
/// a declared nominal (its `order` op is checked where the first comparison is built).
pub fn decls(
    input: &WorkQueueInput,
    module: &Module,
) -> Result<Vec<Contribution>, Vec<ModelError>> {
    let mut errors = Vec::new();
    let contributions = input_contributions(input);
    if SemanticId::parse(&input.queue_id).is_some() {
        if !module.entities().contains_key(&input.worker_entity) {
            errors.push(ModelError::UnknownEntity {
                model_ref: input.queue_id.clone(),
                entity: input.worker_entity.clone(),
            });
        }
        let declared =
            matches!(&input.time, WType::Nominal(n) if module.nominals().contains_key(n));
        if !declared {
            errors.push(ModelError::TimeNotOrdered {
                model_ref: input.queue_id.clone(),
                time: type_name(&input.time),
            });
        }
    }
    match contributions {
        Ok(c) if errors.is_empty() => Ok(c),
        Ok(_) => Err(errors),
        Err(es) => {
            errors.extend(es);
            Err(errors)
        }
    }
}

pub(crate) fn type_name(t: &WType) -> String {
    match t {
        WType::Nominal(n) | WType::Enum(n) | WType::Entity(n) => n.clone(),
        other => serde_json::to_value(other)
            .map(|v| v.to_string())
            .unwrap_or_default(),
    }
}

/// Resolves a work queue against `module`, which already holds its declarations (P2).
pub fn resolve(
    input: &WorkQueueInput,
    module: &Module,
) -> Result<ResolvedWorkQueue, Vec<ModelError>> {
    let contributions = decls(input, module)?;
    let model_ref = input.queue_id.clone();
    let queue_id = SemanticId::parse(&input.queue_id).ok_or_else(Vec::new)?;
    let (enum_name, entity) = work_names(&queue_id);
    let schema = contributions
        .iter()
        .find_map(|c| match c {
            Contribution::Entity(s) => Some(s.fields.clone()),
            Contribution::Enum(_) => None,
        })
        .ok_or_else(Vec::new)?;
    let subject_param = snake_case(&entity);
    let payload: BTreeMap<String, WType> = input.payload.iter().cloned().collect();

    let mut errors = Vec::new();
    let mut capabilities = BTreeMap::new();
    for c in &input.capabilities {
        let c_ref = format!("{model_ref}/capability/{}", c.capability_id);
        let known = CAPABILITIES.iter().find(|(id, _)| *id == c.capability_id);
        let (Some((_, needs_authorization)), Some(capability_id)) =
            (known, SemanticId::parse(&c.capability_id))
        else {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: c.capability_id.clone(),
            });
            continue;
        };
        if capabilities.contains_key(&c.capability_id) {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: c.capability_id.clone(),
            });
            continue;
        }
        // Authored names: Core identifiers, distinct, never a generated name, the subject or (for
        // enqueue) a payload field.
        let mut names = BTreeSet::new();
        let mut check = |name: &String, errors: &mut Vec<ModelError>| -> bool {
            let reserved = RESERVED_PARAMETERS.contains(&name.as_str())
                || name == &subject_param
                || (c.capability_id == "enqueue" && payload.contains_key(name));
            if reserved {
                errors.push(ModelError::ReservedParameter {
                    model_ref: c_ref.clone(),
                    name: name.clone(),
                });
                return false;
            }
            if !is_core_identifier(name) || !names.insert(name.clone()) {
                errors.push(ModelError::InvalidParameter {
                    model_ref: c_ref.clone(),
                    name: name.clone(),
                });
                return false;
            }
            true
        };
        let mut inputs = BTreeMap::new();
        for (n, t) in &c.inputs {
            if check(n, &mut errors) {
                inputs.insert(n.clone(), t.clone());
            }
        }
        let mut context = BTreeMap::new();
        for (n, t) in &c.context {
            if check(n, &mut errors) {
                context.insert(n.clone(), t.clone());
            }
        }
        let mut state = BTreeMap::new();
        for (n, e) in &c.state {
            if !check(n, &mut errors) {
                continue;
            }
            if e == &entity {
                errors.push(ModelError::WorkItemBinding {
                    model_ref: c_ref.clone(),
                    name: n.clone(),
                });
            } else if !module.entities().contains_key(e) {
                errors.push(ModelError::UnknownEntity {
                    model_ref: c_ref.clone(),
                    entity: e.clone(),
                });
            } else {
                state.insert(n.clone(), e.clone());
            }
        }
        capabilities.insert(
            c.capability_id.clone(),
            ResolvedCapability {
                action_name: encode(&queue_id, &capability_id),
                capability_id,
                inputs,
                context,
                state,
                needs_authorization: *needs_authorization,
                display: c.display.clone(),
                source: c.source.clone(),
            },
        );
    }
    for (id, _) in CAPABILITIES {
        if !capabilities.contains_key(id)
            && !input.capabilities.iter().any(|c| c.capability_id == id)
        {
            errors.push(ModelError::MissingCapability {
                model_ref: model_ref.clone(),
                capability: id.to_string(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // The embedded machine, resolved by the unchanged StateMachine resolver.
    let transitions = TRANSITIONS
        .iter()
        .filter_map(|(id, from, to)| {
            let c = capabilities.get(*id)?;
            let (mut inputs, mut context) = generated_params(id, &input.worker_entity, &input.time);
            inputs.extend(c.inputs.iter().map(|(n, t)| (n.clone(), t.clone())));
            context.extend(c.context.iter().map(|(n, t)| (n.clone(), t.clone())));
            Some(TransitionInput {
                transition_id: id.to_string(),
                from_states: from.iter().map(|s| s.to_string()).collect(),
                to_state: to.to_string(),
                inputs,
                context,
                state: c
                    .state
                    .iter()
                    .map(|(n, e)| (n.clone(), e.clone()))
                    .collect(),
                display: c.display.clone(),
                source: c.source.clone(),
            })
        })
        .collect();
    let machine = resolve_machine(
        &ModelInput {
            model_id: input.queue_id.clone(),
            subject_entity: entity.clone(),
            state_field: STATUS.into(),
            subject_param: None,
            entry_states: vec!["READY".into()],
            transitions,
            display: input.display.clone(),
            source: input.source.clone(),
        },
        module,
    )?;
    Ok(ResolvedWorkQueue {
        queue_id,
        enum_name,
        entity,
        worker: input.worker_entity.clone(),
        time: input.time.clone(),
        max_attempts: input.max_attempts,
        payload,
        schema,
        capabilities,
        machine,
        display: input.display.clone(),
        source: input.source.clone(),
    })
}

/// The location of capability `c`: its own, else the queue's.
pub(crate) fn capability_loc(q: &ResolvedWorkQueue, c: &ResolvedCapability) -> Loc {
    match c.source.as_ref().or(q.source.as_ref()) {
        Some(s) => Loc {
            file: s.file.clone(),
            line: s.line,
        },
        None => location(&None, q.queue_id.as_str()),
    }
}
