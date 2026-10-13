//! Lowering: the generated action layout, fragment validation, self-transitions, erasure and
//! Core admissibility (lowering contract §3, data-model §6; T016–T019).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, SemanticProfile};
use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::{WField, WType};
use behavior_models::ids::decode;
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{ModelSpec, TransitionFragments, compose};
use common::*;
use serde_json::json;

fn table(
    id: &str,
    f: impl FnMut(
        &mut Builder,
        &behavior_models::TransitionScope,
    ) -> Result<TransitionFragments, behavior_models::BindingError>
    + 'static,
) -> FragmentTable<'static> {
    let mut t: FragmentTable<'static> = BTreeMap::new();
    t.insert(id.into(), Box::new(f));
    t
}

fn with(
    input: behavior_models::ModelInput,
    fragments: FragmentTable<'static>,
) -> ModelSpec<'static> {
    ModelSpec {
        input,
        fragments: Box::new(fragments),
    }
}

// --- T016: layout -----------------------------------------------------------------------------

#[test]
fn approve_has_the_exact_layout() {
    let out = build_reference();
    let a = &out.module.actions()["purchase_request__approve"];

    // `ParamRole` is not named by the facade; its debug form is enough for a test.
    let params: Vec<(String, String)> = a
        .params()
        .iter()
        .map(|p| (p.name().to_string(), format!("{:?}", p.role())))
        .collect();
    assert_eq!(
        params,
        vec![
            ("purchase_request".to_string(), "State".to_string()),
            ("actor".to_string(), "Context".to_string())
        ]
    );

    // pre = [In(s.status, [InReview])] ++ [guard]
    let pre = a.preconditions();
    assert_eq!(pre.len(), 2);
    assert_eq!(pre[0].expr().children().len(), 1, "generated `In`");
    assert_eq!(pre[1].expr().children().len(), 2, "author guard `eq`");

    // effects = [(s, status, Approved)] ++ [(s, approved_by, some(actor))]
    let fields: Vec<(&str, &str)> = a.effects().iter().map(|e| (e.param(), e.field())).collect();
    assert_eq!(
        fields,
        vec![
            ("purchase_request", "status"),
            ("purchase_request", "approved_by")
        ]
    );

    // post = [s.status == Approved]: its literal is the effect's literal.
    let post = a.postconditions();
    assert_eq!(post.len(), 1);
    assert_eq!(
        hash_display(post[0].expr().children()[1].hash()),
        hash_display(a.effects()[0].value().hash())
    );

    assert_eq!(a.command_emissions().len(), 1);
}

#[test]
fn single_source_still_lowers_to_in() {
    let out = build_reference();
    let submit = &out.module.actions()["purchase_request__submit"];
    assert_eq!(submit.preconditions().len(), 1);
    assert_eq!(submit.preconditions()[0].expr().children().len(), 1);
}

#[test]
fn source_states_are_canonically_ordered() {
    let action_hash = |from: &[&str]| {
        let mut m = machine();
        m.transitions[3] = transition("reject", from, "Rejected");
        let out = build(|_| {}, vec![spec(m)]).unwrap();
        hash_display(out.module.actions()["purchase_request__reject"].hash())
    };
    assert_eq!(
        action_hash(&["InReview", "Submitted", "Draft"]),
        action_hash(&["Draft", "InReview", "Submitted"])
    );
    assert_ne!(
        action_hash(&["InReview", "Submitted"]),
        action_hash(&["InReview"])
    );
}

// --- T017: fragment validation ------------------------------------------------------------------

fn lower_errors(fragments: FragmentTable<'static>) -> Vec<&'static str> {
    let errs = build(|_| {}, vec![with(machine(), fragments)]).unwrap_err();
    codes(&errs)
}

#[test]
fn non_boolean_guard() {
    let errs = build(
        |_| {},
        vec![with(
            machine(),
            table("submit", |b, s| {
                let g = b.field(&subject(s), "approver", loc()).unwrap();
                Ok(TransitionFragments {
                    guards: vec![(g, loc())],
                    ..TransitionFragments::default()
                })
            }),
        )],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"]);
    assert_eq!(errs[0].to_json()["core_code"], "NOT_BOOLEAN");
}

#[test]
fn ill_typed_effect() {
    let errs = lower_errors(table("submit", |b, s| {
        let v = b.lit(WType::String, json!("x"), loc()).unwrap();
        Ok(TransitionFragments {
            effects: vec![(subject(s), "approved_by".into(), v, loc())],
            ..TransitionFragments::default()
        })
    }));
    assert_eq!(errs, vec!["FRAGMENT_INVALID"]);
}

#[test]
fn node_from_another_builder() {
    let errs = build(
        |_| {},
        vec![with(
            machine(),
            table("submit", |_, _| {
                let mut other = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
                let g = other.lit(WType::Bool, json!(true), loc()).unwrap();
                Ok(TransitionFragments {
                    guards: vec![(g, loc())],
                    ..TransitionFragments::default()
                })
            }),
        )],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"]);
    assert_eq!(errs[0].to_json()["core_code"], "BUILDER_MISMATCH");
}

#[test]
fn commands_need_the_command_profile() {
    let mut b = Builder::new();
    b.declare_enum(
        "RequestStatus",
        CASES.iter().map(|c| c.to_string()).collect(),
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "PurchaseRequest",
        vec![WField {
            name: "status".into(),
            ty: status_ty(),
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
    let mut m = machine();
    m.transitions.retain(|t| t.transition_id == "submit");
    let errs = compose(
        &mut b,
        vec![with(
            m,
            table("submit", |_, _| {
                Ok(TransitionFragments {
                    commands: vec![CommandEmissionSpec {
                        command: "Notify".into(),
                        when: None,
                        payload: BTreeMap::new(),
                        loc: loc(),
                    }],
                    ..TransitionFragments::default()
                })
            }),
        )],
        None,
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"]);
    assert_eq!(errs[0].to_json()["core_code"], "UNSUPPORTED_IR_VERSION");
}

#[test]
fn extra_effect_on_the_owned_field() {
    let errs = lower_errors(table("submit", |b, s| {
        let v = b.lit(status_ty(), json!("Approved"), loc()).unwrap();
        Ok(TransitionFragments {
            effects: vec![(subject(s), "status".into(), v, loc())],
            ..TransitionFragments::default()
        })
    }));
    assert_eq!(errs, vec!["TRANSITION_WRITES_OWNED_FIELD"]);
}

#[test]
fn inadmissible_module() {
    // A guard over a derived value that is never declared: the builder accepts the untyped node,
    // Core admission rejects the module.
    let errs = lower_errors(table("submit", |b, s| {
        let g = b.derived_ref("ghost", vec![subject(s)], loc()).unwrap();
        Ok(TransitionFragments {
            guards: vec![(g, loc())],
            ..TransitionFragments::default()
        })
    }));
    assert_eq!(errs, vec!["CORE_ADMISSION_FAILED"]);
}

#[test]
fn binding_errors_are_fragment_errors() {
    let errs = build(
        |_| {},
        vec![with(
            machine(),
            table("submit", |_, _| {
                Err(behavior_models::BindingError {
                    code: "TYPE_MISMATCH".into(),
                    message: "bad lambda".into(),
                })
            }),
        )],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"]);
    assert_eq!(errs[0].to_json()["core_code"], "TYPE_MISMATCH");
}

#[test]
fn errors_are_collected_over_all_transitions() {
    let mut t = table("submit", |b, s| {
        let v = b.lit(status_ty(), json!("Approved"), loc()).unwrap();
        Ok(TransitionFragments {
            effects: vec![(subject(s), "status".into(), v, loc())],
            ..TransitionFragments::default()
        })
    });
    t.insert(
        "reject".into(),
        Box::new(|b: &mut Builder, s: &behavior_models::TransitionScope| {
            let v = b.lit(status_ty(), json!("Draft"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![(subject(s), "status".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    assert_eq!(
        lower_errors(t),
        vec![
            "TRANSITION_WRITES_OWNED_FIELD",
            "TRANSITION_WRITES_OWNED_FIELD"
        ]
    );
}

// --- T018: self-transitions ---------------------------------------------------------------------

fn with_touch(
    from: &[&str],
    fragments: FragmentTable<'static>,
) -> Result<behavior_models::BuildOutput, Vec<behavior_models::ModelError>> {
    let mut m = machine();
    m.transitions.push(transition("touch", from, "InReview"));
    build(|_| {}, vec![with(m, fragments)])
}

#[test]
fn pure_self_transition_is_rejected() {
    let errs = with_touch(&["InReview"], BTreeMap::new()).unwrap_err();
    assert_eq!(codes(&errs), vec!["PURE_SELF_TRANSITION"]);
    // A guard alone does not make it meaningful.
    let errs = with_touch(
        &["InReview"],
        table("touch", |b, _| {
            let g = b.lit(WType::Bool, json!(true), loc()).unwrap();
            Ok(TransitionFragments {
                guards: vec![(g, loc())],
                ..TransitionFragments::default()
            })
        }),
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["PURE_SELF_TRANSITION"]);
}

#[test]
fn self_transition_with_an_effect_or_command_is_allowed() {
    with_touch(
        &["InReview"],
        table("touch", |b, s| {
            let v = b.lit(WType::String, json!("touched"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![(subject(s), "note".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    )
    .unwrap();
    with_touch(
        &["InReview"],
        table("touch", |b, s| {
            let a = b.field(&subject(s), "approver", loc()).unwrap();
            Ok(TransitionFragments {
                commands: vec![CommandEmissionSpec {
                    command: "Notify".into(),
                    when: None,
                    payload: BTreeMap::from([("approver".to_string(), a)]),
                    loc: loc(),
                }],
                ..TransitionFragments::default()
            })
        }),
    )
    .unwrap();
}

#[test]
fn multi_source_transition_including_its_target_is_allowed() {
    with_touch(&["Submitted", "InReview"], BTreeMap::new()).unwrap();
}

// --- T019: erasure and admissibility ------------------------------------------------------------

#[test]
fn lowered_module_contains_only_ordinary_items() {
    let out = build_reference();
    let declared = build(|_| {}, vec![]).unwrap();
    let before = item_names(&declared.module);
    let generated: Vec<String> = item_names(&out.module)
        .into_iter()
        .filter(|n| !before.contains(n))
        .collect();
    assert_eq!(
        generated,
        vec![
            "purchase_request__approve",
            "purchase_request__reject",
            "purchase_request__start_review",
            "purchase_request__submit"
        ]
    );
    for name in &generated {
        let (m, _) = decode(name).unwrap();
        assert_eq!(m.as_str(), "purchase_request");
    }
    // Nothing else changed: every declared item keeps its Core hash.
    for ((kind, name), h) in declared.module.name_table() {
        assert_eq!(
            out.module.name_table()[&(*kind, name.clone())],
            *h,
            "{name}"
        );
    }
}
