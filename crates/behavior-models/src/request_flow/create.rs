//! Creation lowering (data model §5, lowerer version 1). For creation `c` of flow `f`:
//!
//! ```text
//! action   encode(f, c)
//! params   request_id: Input Id(E), inputs, state bindings, context (each sorted by name)
//! pre      G_c
//! create   E { id: request_id, status: lit(T_s, q_c), <V_c> }      exactly one
//! effects  E_c (over the state bindings)
//! post     P_c
//! commands K_c
//! ```
//!
//! The status value is generated from `q_c` only; the initializer can never set it. This is the
//! only file of the crate that may call `into_wire`: the builder's creation input type
//! (`WLifecycle::Create`) takes wire expressions, so the Core nodes built here (in the creation's
//! scope, by Core's own checks) are handed over in that form (research R4).

use std::collections::BTreeSet;

use behavior_engine::builder::{BuildError, Builder, ScopeSite};
use behavior_engine::wire::{Loc, WLifecycle, WType};
use serde_json::json;

use crate::composition::admission_failed;
use crate::error::ModelError;
use crate::request_flow::input::{CreationFragments, CreationScope, RequestFlowSource};
use crate::request_flow::resolve::{REQUEST_ID, ResolvedCreation, ResolvedRequestFlow, STATUS};

fn loc_of(f: &ResolvedRequestFlow, c: &ResolvedCreation) -> Loc {
    match c.source.as_ref().or(f.source.as_ref()) {
        Some(s) => Loc {
            file: s.file.clone(),
            line: s.line,
        },
        None => Loc {
            file: format!("{}.model", f.flow_id),
            line: 1,
        },
    }
}

fn fragment_error(c_ref: &str, e: BuildError) -> ModelError {
    ModelError::FragmentInvalid {
        model_ref: c_ref.to_string(),
        core_code: e.code,
        core_message: e.message,
    }
}

/// The static initializer checks (data model §5): every field once, nothing unknown, no status.
fn check_initializer(
    f: &ResolvedRequestFlow,
    c_ref: &str,
    fragments: &CreationFragments,
) -> Vec<ModelError> {
    let required = f.initialized_fields();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for (field, _) in &fragments.initializer {
        if field == STATUS {
            errors.push(ModelError::InitializerSetsStatus {
                model_ref: c_ref.to_string(),
            });
        } else if !required.contains(field) {
            errors.push(ModelError::InitializerUnknownField {
                model_ref: c_ref.to_string(),
                field: field.clone(),
            });
        } else if !seen.insert(field.clone()) {
            errors.push(ModelError::InitializerDuplicateField {
                model_ref: c_ref.to_string(),
                field: field.clone(),
            });
        }
    }
    for field in required.difference(&seen) {
        errors.push(ModelError::InitializerMissingField {
            model_ref: c_ref.to_string(),
            field: field.clone(),
        });
    }
    errors
}

/// Lowers creation `c` of `f` into `builder`. The scope is always popped again.
pub fn lower_creation(
    builder: &mut Builder,
    f: &ResolvedRequestFlow,
    c: &ResolvedCreation,
    source: &mut dyn RequestFlowSource,
) -> Result<(), Vec<ModelError>> {
    let c_ref = f.creation_ref(&c.creation_id);
    let params = f.creation_params(c);
    let loc = loc_of(f, c);
    builder
        .push_scope(ScopeSite::Action, params.clone(), loc.clone())
        .map_err(|e| vec![fragment_error(&c_ref, e)])?;
    let result = lower_in_scope(builder, f, c, &c_ref, params, loc, source);
    builder.pop_scope();
    result
}

fn lower_in_scope(
    builder: &mut Builder,
    f: &ResolvedRequestFlow,
    c: &ResolvedCreation,
    c_ref: &str,
    params: Vec<behavior_engine::wire::WParam>,
    loc: Loc,
    source: &mut dyn RequestFlowSource,
) -> Result<(), Vec<ModelError>> {
    let scope = CreationScope {
        flow_id: f.flow_id.to_string(),
        creation_id: c.creation_id.to_string(),
        params: params.clone(),
    };
    let fragments = source.trace_creation(builder, &scope).map_err(|e| {
        vec![ModelError::FragmentInvalid {
            model_ref: c_ref.to_string(),
            core_code: e.code,
            core_message: e.message,
        }]
    })?;
    let mut errors = check_initializer(f, c_ref, &fragments);
    for (node, _) in fragments.guards.iter().chain(&fragments.postconditions) {
        if let Err(e) = builder.check_condition(node, "a creation condition") {
            errors.push(fragment_error(c_ref, e));
        }
    }
    for (param, field, node, _) in &fragments.effects {
        if let Err(e) = builder.check_effect(param, field, node) {
            errors.push(fragment_error(c_ref, e));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let CreationFragments {
        initializer,
        guards,
        effects,
        postconditions,
        commands,
    } = fragments;
    let err = |e| vec![fragment_error(c_ref, e)];
    let id = builder.param(REQUEST_ID, loc.clone()).map_err(err)?;
    let status = builder
        .lit(
            WType::Enum(f.status_enum.clone()),
            json!(c.to_state),
            loc.clone(),
        )
        .map_err(err)?;
    // Field values in the entity's canonical (sorted) field order.
    let mut values: Vec<(String, behavior_engine::builder::Node)> = initializer;
    values.push((STATUS.to_string(), status));
    values.sort_by(|a, b| a.0.cmp(&b.0));
    let create = WLifecycle::Create {
        entity: f.entity.clone(),
        id: id.into_wire(),
        fields: values
            .into_iter()
            .map(|(n, v)| (n, v.into_wire()))
            .collect(),
        loc: loc.clone(),
    };
    let lifecycle = vec![create];
    debug_assert_eq!(
        lifecycle.len(),
        1,
        "a creation action creates exactly one request"
    );
    let added = if commands.is_empty() {
        builder.add_action(
            &c.action_name,
            params,
            guards,
            effects,
            lifecycle,
            postconditions,
            loc,
        )
    } else {
        builder.add_action_with_commands(
            &c.action_name,
            params,
            guards,
            effects,
            lifecycle,
            commands,
            postconditions,
            loc,
        )
    };
    added.map_err(|e| {
        vec![if e.code == "DUPLICATE_NAME" {
            ModelError::GeneratedNameCollision {
                model_ref: c_ref.to_string(),
                name: c.action_name.clone(),
            }
        } else {
            fragment_error(c_ref, e)
        }]
    })?;
    // The creation's values reach the builder as wire expressions, which `add_action` does not
    // type-check; admit now so an ill-typed initializer is reported against this creation.
    builder.finish(None).map(|_| ()).map_err(|r| {
        let e = admission_failed(r);
        vec![match e {
            ModelError::CoreAdmissionFailed { errors } => {
                let (code, message) = errors.into_iter().next().unwrap_or_default();
                ModelError::FragmentInvalid {
                    model_ref: c_ref.to_string(),
                    core_code: code,
                    core_message: message,
                }
            }
            other => other,
        }]
    })
}
