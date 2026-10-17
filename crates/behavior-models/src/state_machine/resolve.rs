//! Resolution: `ModelInput` → `ResolvedStateMachine` (data-model §3).
//!
//! This is where model input gains meaning, and the only place: ids are validated, sets are
//! sets (sorted, deduplicated), parameters are mappings, the subject parameter gets its default,
//! and every state is checked against the owned field's enum. The state domain
//! `Q := Cases(T_f)` is never stored; it is read from the enum's declaration when needed.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::wire::WType;

use crate::error::ModelError;
use crate::ids::{SemanticId, encode};
use crate::state_machine::input::{DisplayMeta, ModelInput, SourceLoc};

/// A resolved, validated, canonical state machine `(E, f, Q_entry, 𝒯)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStateMachine {
    pub model_id: SemanticId,
    /// `E`
    pub subject: String,
    /// `f`
    pub field: String,
    /// `T_f`; `Q` is its cases and is deliberately not stored.
    pub enum_type: String,
    pub subject_param: String,
    /// `Q_entry ⊆ Q`, non-empty.
    pub entry_states: BTreeSet<String>,
    pub transitions: BTreeMap<SemanticId, ResolvedTransition>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// A resolved transition: its structure `(F_t, q_t)` and parameter mappings. Its open parts
/// (`G_t, E_t, P_t, K_t`) are Core fragments traced at lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTransition {
    pub transition_id: SemanticId,
    /// `F_t ⊆ Q`, non-empty.
    pub from_states: BTreeSet<String>,
    /// `q_t ∈ Q`
    pub to_state: String,
    /// `ParameterId → Type`
    pub inputs: BTreeMap<String, WType>,
    /// `ParameterId → Type`
    pub context: BTreeMap<String, WType>,
    /// State bindings `ParameterId → entity` (feature 507, D1(a)); empty at language level 2.
    pub state: BTreeMap<String, String>,
    /// `encode(model_id, transition_id)`
    pub action_name: String,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

impl ResolvedStateMachine {
    /// `model_id/transition/transition_id`
    pub fn transition_ref(&self, t: &SemanticId) -> String {
        format!("{}/transition/{t}", self.model_id)
    }

    /// The language level this machine needs (feature 507, research R13): 3 when any transition
    /// binds further entities, otherwise the established level 2, whose canonical document and
    /// mappings are unchanged.
    pub fn semantic_version(&self) -> u32 {
        if self.transitions.values().any(|t| !t.state.is_empty()) {
            crate::STATE_MACHINE_BINDINGS_SEMANTIC_VERSION
        } else {
            crate::STATE_MACHINE_SEMANTIC_VERSION
        }
    }
}

/// The default subject parameter: `snake_case(E)` (`PurchaseRequest` → `purchase_request`,
/// `HTTPRequest` → `http_request`).
pub fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Core's identifier grammar `[A-Za-z_][A-Za-z0-9_]*`.
fn is_core_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Resolves `input` against the declarations of `module` (the admitted ordinary behavior).
/// All resolution errors are reported together.
pub fn resolve(
    input: &ModelInput,
    module: &Module,
) -> Result<ResolvedStateMachine, Vec<ModelError>> {
    let mut errors = Vec::new();
    let model_ref = input.model_id.clone();
    let model_id = SemanticId::parse(&input.model_id);
    if model_id.is_none() {
        errors.push(ModelError::InvalidSemanticId {
            model_ref: model_ref.clone(),
            id: input.model_id.clone(),
        });
    }

    let Some(entity) = module.entities().get(&input.subject_entity) else {
        errors.push(ModelError::UnknownEntity {
            model_ref,
            entity: input.subject_entity.clone(),
        });
        return Err(errors);
    };
    let enum_type = match entity
        .field_type(&input.state_field)
        .map(|t| t.to_wire_type())
    {
        Some(WType::Enum(name)) => name,
        _ => {
            errors.push(ModelError::StateFieldNotEnum {
                model_ref,
                entity: input.subject_entity.clone(),
                field: input.state_field.clone(),
            });
            return Err(errors);
        }
    };
    let cases: BTreeSet<String> = module
        .enums()
        .get(&enum_type)
        .map(|e| e.values.iter().cloned().collect())
        .unwrap_or_default();
    let states = |names: &[String], at: &str, errors: &mut Vec<ModelError>| {
        let mut set = BTreeSet::new();
        for n in names {
            if cases.contains(n) {
                set.insert(n.clone());
            } else {
                errors.push(ModelError::UnknownState {
                    model_ref: at.to_string(),
                    enum_name: enum_type.clone(),
                    state: n.clone(),
                });
            }
        }
        set
    };

    let entry_states = states(&input.entry_states, &model_ref, &mut errors);
    if input.entry_states.is_empty() {
        errors.push(ModelError::EmptyEntryStates {
            model_ref: model_ref.clone(),
        });
    }

    let subject_param = input
        .subject_param
        .clone()
        .unwrap_or_else(|| snake_case(&input.subject_entity));
    if !is_core_identifier(&subject_param) {
        errors.push(ModelError::InvalidParameter {
            model_ref: model_ref.clone(),
            name: subject_param.clone(),
        });
    }

    let mut transitions = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for t in &input.transitions {
        let t_ref = format!("{model_ref}/transition/{}", t.transition_id);
        let Some(transition_id) = SemanticId::parse(&t.transition_id) else {
            errors.push(ModelError::InvalidSemanticId {
                model_ref: t_ref,
                id: t.transition_id.clone(),
            });
            continue;
        };
        if !seen.insert(transition_id.clone()) {
            errors.push(ModelError::DuplicateTransitionId { model_ref: t_ref });
            continue;
        }
        if t.from_states.is_empty() {
            errors.push(ModelError::EmptySourceStates {
                model_ref: t_ref.clone(),
            });
        }
        let from_states = states(&t.from_states, &t_ref, &mut errors);
        let to = states(std::slice::from_ref(&t.to_state), &t_ref, &mut errors);

        // Parameters are mappings, with names distinct from each other and from the subject.
        let mut names = BTreeSet::from([subject_param.clone()]);
        let mut mapping = |decls: &[(String, WType)], errors: &mut Vec<ModelError>| {
            let mut m = BTreeMap::new();
            for (name, ty) in decls {
                if !is_core_identifier(name) || !names.insert(name.clone()) {
                    errors.push(ModelError::InvalidParameter {
                        model_ref: t_ref.clone(),
                        name: name.clone(),
                    });
                    continue;
                }
                m.insert(name.clone(), ty.clone());
            }
            m
        };
        let bindings: Vec<(String, WType)> = t
            .state
            .iter()
            .map(|(n, e)| (n.clone(), WType::Entity(e.clone())))
            .collect();
        let state: BTreeMap<String, String> = mapping(&bindings, &mut errors)
            .into_iter()
            .filter_map(|(n, ty)| match ty {
                WType::Entity(e) => Some((n, e)),
                _ => None,
            })
            .collect();
        let inputs = mapping(&t.inputs, &mut errors);
        let context = mapping(&t.context, &mut errors);
        for entity in state.values() {
            if !module.entities().contains_key(entity) {
                errors.push(ModelError::UnknownEntity {
                    model_ref: t_ref.clone(),
                    entity: entity.clone(),
                });
            }
        }

        if let (Some(m), Some(to_state)) = (&model_id, to.into_iter().next()) {
            transitions.insert(
                transition_id.clone(),
                ResolvedTransition {
                    action_name: encode(m, &transition_id),
                    transition_id,
                    from_states,
                    to_state,
                    inputs,
                    context,
                    state,
                    display: t.display.clone(),
                    source: t.source.clone(),
                },
            );
        }
    }

    match model_id {
        Some(model_id) if errors.is_empty() => Ok(ResolvedStateMachine {
            model_id,
            subject: input.subject_entity.clone(),
            field: input.state_field.clone(),
            enum_type,
            subject_param,
            entry_states,
            transitions,
            display: input.display.clone(),
            source: input.source.clone(),
        }),
        _ => Err(errors),
    }
}
