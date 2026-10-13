//! Resolution: `RequestFlowInput` → `ResolvedRequestFlow` (data model §2–§4).
//!
//! The embedded StateMachine is not re-implemented: `sm_input` derives a StateMachine input
//! (with entry states computed here, and only here, from the creation targets), and the existing
//! `state_machine::resolve::resolve` resolves it.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, Role, WParam, WType};

use crate::composition::DeclaredSchema;
use crate::error::ModelError;
use crate::ids::{SemanticId, encode, encode_entity_name};
use crate::request_flow::input::RequestFlowInput;
use crate::state_machine::input::{DisplayMeta, ModelInput, SourceLoc};
use crate::state_machine::resolve::{ResolvedStateMachine, resolve as resolve_machine};

/// The reserved name of a creation's request-identity input (data model §4).
pub const REQUEST_ID: &str = "request_id";
/// The generated status field, owned by the embedded StateMachine.
pub const STATUS: &str = "status";
/// The generated requester field, fixed at creation.
pub const REQUESTER: &str = "requester";
/// The generated assignee field (when enabled), an ordinary field.
pub const ASSIGNEE: &str = "assignee";

/// A resolved, validated, canonical request flow.
#[derive(Debug, Clone)]
pub struct ResolvedRequestFlow {
    pub flow_id: SemanticId,
    /// `E = encode_entity_name(flow_id)`
    pub entity: String,
    pub actor: String,
    /// `T_s`; `Q` is its cases and is not stored.
    pub status_enum: String,
    pub assignee: bool,
    pub payload: BTreeMap<String, WType>,
    /// The generated entity's fields, sorted by name.
    pub schema: Vec<(String, WType)>,
    pub creations: BTreeMap<SemanticId, ResolvedCreation>,
    /// `SM(RF)`, resolved by the StateMachine resolver.
    pub machine: ResolvedStateMachine,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCreation {
    pub creation_id: SemanticId,
    /// `q_c ∈ Q`
    pub to_state: String,
    pub inputs: BTreeMap<String, WType>,
    /// `S_c`: name → entity
    pub state: BTreeMap<String, String>,
    pub context: BTreeMap<String, WType>,
    /// `encode(flow_id, creation_id)`
    pub action_name: String,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

impl ResolvedRequestFlow {
    pub fn creation_ref(&self, c: &SemanticId) -> String {
        format!("{}/creation/{c}", self.flow_id)
    }

    /// The fields an initializer must set: requester, payload and (if enabled) assignee.
    pub fn initialized_fields(&self) -> BTreeSet<String> {
        self.schema
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| n != STATUS)
            .collect()
    }

    /// The creation action's parameters, in canonical order (data model §4).
    pub fn creation_params(&self, c: &ResolvedCreation) -> Vec<WParam> {
        let mut ps = vec![WParam {
            name: REQUEST_ID.into(),
            role: Some(Role::Input),
            ty: WType::Id(self.entity.clone()),
        }];
        ps.extend(c.inputs.iter().map(|(n, t)| WParam {
            name: n.clone(),
            role: Some(Role::Input),
            ty: t.clone(),
        }));
        ps.extend(c.state.iter().map(|(n, e)| WParam {
            name: n.clone(),
            role: Some(Role::State),
            ty: WType::Entity(e.clone()),
        }));
        ps.extend(c.context.iter().map(|(n, t)| WParam {
            name: n.clone(),
            role: Some(Role::Context),
            ty: t.clone(),
        }));
        ps
    }
}

fn is_core_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn location(source: &Option<SourceLoc>, id: &str) -> Loc {
    match source {
        Some(s) => Loc {
            file: s.file.clone(),
            line: s.line,
        },
        None => Loc {
            file: format!("{id}.model"),
            line: 1,
        },
    }
}

/// The generated request entity, derived from the input and the authored declarations only:
/// no lowering, no fragments (law D1). Errors cover what the schema itself needs.
pub fn schema(
    input: &RequestFlowInput,
    module: &Module,
) -> Result<DeclaredSchema, Vec<ModelError>> {
    let mut errors = Vec::new();
    let model_ref = input.flow_id.clone();
    if SemanticId::parse(&input.flow_id).is_some() {
        if !module.entities().contains_key(&input.actor_entity) {
            errors.push(ModelError::UnknownEntity {
                model_ref: model_ref.clone(),
                entity: input.actor_entity.clone(),
            });
        }
        if !module.enums().contains_key(&input.status_enum) {
            errors.push(ModelError::UnknownEnum {
                model_ref: model_ref.clone(),
                name: input.status_enum.clone(),
            });
        }
    }
    match input_schema(input) {
        Ok(s) if errors.is_empty() => Ok(s),
        Ok(_) => Err(errors),
        Err(es) => {
            errors.extend(es);
            Err(errors)
        }
    }
}

/// The schema as far as it follows from the flow input alone: no lookup of the actor or the
/// status enum (those are checked against declarations by [`schema`]).
pub fn input_schema(input: &RequestFlowInput) -> Result<DeclaredSchema, Vec<ModelError>> {
    let mut errors = Vec::new();
    let model_ref = input.flow_id.clone();
    let Some(flow_id) = SemanticId::parse(&input.flow_id) else {
        return Err(vec![ModelError::InvalidSemanticId {
            model_ref: model_ref.clone(),
            id: input.flow_id.clone(),
        }]);
    };
    let actor = WType::Id(input.actor_entity.clone());
    let mut fields: BTreeMap<String, WType> = BTreeMap::new();
    fields.insert(REQUESTER.into(), actor.clone());
    fields.insert(STATUS.into(), WType::Enum(input.status_enum.clone()));
    if input.assignee {
        fields.insert(ASSIGNEE.into(), WType::Option(Box::new(actor)));
    }
    let mut payload_seen = BTreeSet::new();
    for (name, ty) in &input.payload {
        if [REQUESTER, STATUS].contains(&name.as_str()) || (input.assignee && name == ASSIGNEE) {
            errors.push(ModelError::ReservedField {
                model_ref: model_ref.clone(),
                name: name.clone(),
            });
        } else if !is_core_identifier(name) || !payload_seen.insert(name.clone()) {
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
    Ok(DeclaredSchema {
        model_id: input.flow_id.clone(),
        entity: encode_entity_name(&flow_id),
        // Sorted by field name (research R2): payload order never reaches the entity hash.
        fields: fields.into_iter().collect(),
        loc: location(&input.source, &input.flow_id),
    })
}

/// The embedded StateMachine's input. Entry states are computed here and nowhere else:
/// `Q_entry = { q_c | c ∈ 𝒞 }`.
pub fn sm_input(input: &RequestFlowInput, entity: &str) -> ModelInput {
    ModelInput {
        model_id: input.flow_id.clone(),
        subject_entity: entity.to_string(),
        state_field: STATUS.into(),
        subject_param: Some(input.flow_id.clone()),
        entry_states: input.creations.iter().map(|c| c.to_state.clone()).collect(),
        transitions: input.transitions.clone(),
        display: input.display.clone(),
        source: input.source.clone(),
    }
}

/// Resolves a flow against `module`, which already holds the request entity (declared in P2).
pub fn resolve(
    input: &RequestFlowInput,
    module: &Module,
) -> Result<ResolvedRequestFlow, Vec<ModelError>> {
    let declared = schema(input, module)?;
    let mut errors = Vec::new();
    let model_ref = input.flow_id.clone();
    let flow_id = SemanticId::parse(&input.flow_id).ok_or_else(Vec::new)?;
    let cases: BTreeSet<String> = module
        .enums()
        .get(&input.status_enum)
        .map(|e| e.values.iter().cloned().collect())
        .unwrap_or_default();

    if input.creations.is_empty() {
        errors.push(ModelError::NoCreation {
            model_ref: model_ref.clone(),
        });
    }
    // Creation and transition ids share one capability namespace.
    let mut ids = BTreeSet::new();
    for id in input
        .creations
        .iter()
        .map(|c| &c.creation_id)
        .chain(input.transitions.iter().map(|t| &t.transition_id))
    {
        if !ids.insert(id.clone()) {
            errors.push(ModelError::CapabilityIdCollision {
                model_ref: model_ref.clone(),
                id: id.clone(),
            });
        }
    }

    let mut creations = BTreeMap::new();
    for c in &input.creations {
        let c_ref = format!("{model_ref}/creation/{}", c.creation_id);
        let Some(creation_id) = SemanticId::parse(&c.creation_id) else {
            errors.push(ModelError::InvalidSemanticId {
                model_ref: c_ref,
                id: c.creation_id.clone(),
            });
            continue;
        };
        if !cases.contains(&c.to_state) {
            errors.push(ModelError::UnknownState {
                model_ref: c_ref.clone(),
                enum_name: input.status_enum.clone(),
                state: c.to_state.clone(),
            });
        }
        let mut names = BTreeSet::from([REQUEST_ID.to_string()]);
        let mut check = |name: &String, errors: &mut Vec<ModelError>| {
            let ok = is_core_identifier(name) && names.insert(name.clone());
            if !ok {
                errors.push(ModelError::InvalidParameter {
                    model_ref: c_ref.clone(),
                    name: name.clone(),
                });
            }
            ok
        };
        let mut inputs = BTreeMap::new();
        for (n, t) in &c.inputs {
            if check(n, &mut errors) {
                inputs.insert(n.clone(), t.clone());
            }
        }
        let mut state = BTreeMap::new();
        for (n, e) in &c.state {
            if check(n, &mut errors) {
                if module.entities().contains_key(e) || *e == declared.entity {
                    state.insert(n.clone(), e.clone());
                } else {
                    errors.push(ModelError::UnknownEntity {
                        model_ref: c_ref.clone(),
                        entity: e.clone(),
                    });
                }
            }
        }
        let mut context = BTreeMap::new();
        for (n, t) in &c.context {
            if check(n, &mut errors) {
                context.insert(n.clone(), t.clone());
            }
        }
        creations.insert(
            creation_id.clone(),
            ResolvedCreation {
                action_name: encode(&flow_id, &creation_id),
                creation_id,
                to_state: c.to_state.clone(),
                inputs,
                state,
                context,
                display: c.display.clone(),
                source: c.source.clone(),
            },
        );
    }

    let machine = match resolve_machine(&sm_input(input, &declared.entity), module) {
        Ok(m) => Some(m),
        Err(es) => {
            errors.extend(es);
            None
        }
    };
    match machine {
        Some(machine) if errors.is_empty() => Ok(ResolvedRequestFlow {
            flow_id,
            entity: declared.entity,
            actor: input.actor_entity.clone(),
            status_enum: input.status_enum.clone(),
            assignee: input.assignee,
            payload: input.payload.iter().cloned().collect(),
            schema: declared.fields,
            creations,
            machine,
            display: input.display.clone(),
            source: input.source.clone(),
        }),
        _ => Err(errors),
    }
}
