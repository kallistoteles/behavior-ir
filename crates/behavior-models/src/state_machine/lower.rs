//! Lowering of resolved transitions into the shared Core builder (lowering contract §3).
//!
//! For transition `t = (F_t, q_t, G_t, E_t, P_t, K_t)` of machine `(E, f, Q_entry, 𝒯)` with
//! subject parameter `s`, the generated action is exactly:
//!
//! ```text
//! action   encode(m, t)
//! params   (s: state E) ++ inputs sorted by name ++ context sorted by name
//! pre      [ In(s.f, sorted F_t) ] ++ G_t
//! effects  [ (s, f, lit(T_f, q_t)) ] ++ E_t
//! post     [ s.f == lit(T_f, q_t) ] ++ P_t
//! commands K_t
//! ```
//!
//! Only the three generated parts come from the machine; the rest are the author's Core
//! fragments, placed unchanged. Fragments are traced by the binding inside the scope opened
//! here, with the parameter names chosen here.

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::wire::{Loc, Role, WParam, WType};
use serde_json::json;

use crate::error::ModelError;
use crate::state_machine::input::{
    FragmentSource, SourceLoc, TransitionFragments, TransitionScope,
};
use crate::state_machine::resolve::{ResolvedStateMachine, ResolvedTransition};

/// The location generated nodes carry: the transition's, else the machine's. Locations are
/// source metadata and never part of any Core hash.
fn loc_of(m: &ResolvedStateMachine, t: &ResolvedTransition) -> Loc {
    let src = t.source.as_ref().or(m.source.as_ref());
    match src {
        Some(SourceLoc { file, line }) => Loc {
            file: file.clone(),
            line: *line,
        },
        None => Loc {
            file: format!("{}.model", m.model_id),
            line: 1,
        },
    }
}

/// The generated action's parameters (data-model §4).
pub fn params(m: &ResolvedStateMachine, t: &ResolvedTransition) -> Vec<WParam> {
    let mut ps = vec![WParam {
        name: m.subject_param.clone(),
        role: Some(Role::State),
        ty: WType::Entity(m.subject.clone()),
    }];
    ps.extend(t.inputs.iter().map(|(name, ty)| WParam {
        name: name.clone(),
        role: Some(Role::Input),
        ty: ty.clone(),
    }));
    ps.extend(t.context.iter().map(|(name, ty)| WParam {
        name: name.clone(),
        role: Some(Role::Context),
        ty: ty.clone(),
    }));
    ps
}

fn fragment_error(t_ref: &str, e: BuildError) -> ModelError {
    ModelError::FragmentInvalid {
        model_ref: t_ref.to_string(),
        core_code: e.code,
        core_message: e.message,
    }
}

/// Lowers transition `t` of `m` into `builder`. The scope is always popped again.
pub fn lower_transition(
    builder: &mut Builder,
    m: &ResolvedStateMachine,
    t: &ResolvedTransition,
    source: &mut dyn FragmentSource,
) -> Result<(), Vec<ModelError>> {
    let t_ref = m.transition_ref(&t.transition_id);
    let ps = params(m, t);
    let loc = loc_of(m, t);
    builder
        .push_scope(ScopeSite::Action, ps.clone(), loc.clone())
        .map_err(|e| vec![fragment_error(&t_ref, e)])?;
    let result = lower_in_scope(builder, m, t, &t_ref, ps, loc, source);
    builder.pop_scope();
    result
}

fn lower_in_scope(
    builder: &mut Builder,
    m: &ResolvedStateMachine,
    t: &ResolvedTransition,
    t_ref: &str,
    ps: Vec<WParam>,
    loc: Loc,
    source: &mut dyn FragmentSource,
) -> Result<(), Vec<ModelError>> {
    let scope = TransitionScope {
        model_id: m.model_id.to_string(),
        transition_id: t.transition_id.to_string(),
        params: ps.clone(),
        subject_param: m.subject_param.clone(),
        owned_field: m.field.clone(),
    };
    let fragments = source.trace(builder, &scope).map_err(|e| {
        vec![ModelError::FragmentInvalid {
            model_ref: t_ref.to_string(),
            core_code: e.code,
            core_message: e.message,
        }]
    })?;
    validate(builder, m, t, t_ref, &ps, &fragments)?;

    let TransitionFragments {
        guards,
        effects,
        postconditions,
        commands,
    } = fragments;
    let err = |e| vec![fragment_error(t_ref, e)];
    let state: Node = builder
        .field(&m.subject_param, &m.field, loc.clone())
        .map_err(err)?;
    let sources = t.from_states.iter().map(|s| json!(s)).collect();
    let requirement = builder
        .in_(state.clone(), sources, loc.clone())
        .map_err(err)?;
    let target = builder
        .lit(
            WType::Enum(m.enum_type.clone()),
            json!(t.to_state),
            loc.clone(),
        )
        .map_err(err)?;
    let ensured = builder
        .op("eq", vec![state, target.clone()], loc.clone())
        .map_err(err)?;

    let pre = std::iter::once((requirement, loc.clone()))
        .chain(guards)
        .collect();
    let eff = std::iter::once((
        m.subject_param.clone(),
        m.field.clone(),
        target,
        loc.clone(),
    ))
    .chain(effects)
    .collect();
    let post = std::iter::once((ensured, loc.clone()))
        .chain(postconditions)
        .collect();
    let added = if commands.is_empty() {
        builder.add_action(&t.action_name, ps, pre, eff, vec![], post, loc)
    } else {
        builder.add_action_with_commands(&t.action_name, ps, pre, eff, vec![], commands, post, loc)
    };
    added.map_err(|e| {
        vec![if e.code == "DUPLICATE_NAME" {
            ModelError::GeneratedNameCollision {
                model_ref: t_ref.to_string(),
                name: t.action_name.clone(),
            }
        } else {
            fragment_error(t_ref, e)
        }]
    })
}

/// Two-layer validation of the author's fragments (data-model §6). Core validity first, then
/// StateMachine admissibility. All findings are reported.
fn validate(
    builder: &mut Builder,
    m: &ResolvedStateMachine,
    t: &ResolvedTransition,
    t_ref: &str,
    ps: &[WParam],
    f: &TransitionFragments,
) -> Result<(), Vec<ModelError>> {
    let mut errors = Vec::new();
    for (node, _) in f.guards.iter().chain(&f.postconditions) {
        if let Err(e) = builder.check_condition(node, "a transition condition") {
            errors.push(fragment_error(t_ref, e));
        }
    }
    let subject_type = WType::Entity(m.subject.clone());
    for (param, field, node, _) in &f.effects {
        let owned =
            field == &m.field && ps.iter().any(|p| &p.name == param && p.ty == subject_type);
        if owned {
            errors.push(ModelError::TransitionWritesOwnedField {
                model_ref: t_ref.to_string(),
            });
        } else if let Err(e) = builder.check_effect(param, field, node) {
            errors.push(fragment_error(t_ref, e));
        }
    }
    let pure_self = t.from_states.len() == 1
        && t.from_states.contains(&t.to_state)
        && f.effects.is_empty()
        && f.commands.is_empty();
    if pure_self {
        errors.push(ModelError::PureSelfTransition {
            model_ref: t_ref.to_string(),
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
