//! Transition state bindings (feature 507, D1(a); research R3 requirements 1–8, R13).
//!
//! A transition may bind further entities and change them atomically with the status, but a
//! binding never grants authority: owned fields stay owned. A machine without bindings is the
//! established language level 2, byte for byte; a machine with bindings is level 3.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::BTreeMap;

use behavior_engine::builder::Builder;
use behavior_engine::wire::WType;
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{ModelError, ModelSpec, TransitionFragments, TransitionScope};
use common::*;
use serde_json::{Value, json};

/// The reference machine with `submit` binding `reviewer: Actor` (and, when `extra`, also
/// `auditor: Actor`, listed first), whose effect sets `reviewer.name := "busy"`.
fn bound_machine(extra: bool) -> behavior_models::ModelInput {
    let mut m = machine();
    let submit = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.state = if extra {
        vec![
            ("reviewer".into(), "Actor".into()),
            ("auditor".into(), "Actor".into()),
        ]
    } else {
        vec![("reviewer".into(), "Actor".into())]
    };
    m
}

fn bound_spec(extra: bool) -> ModelSpec<'static> {
    let mut f: FragmentTable<'static> = BTreeMap::new();
    f.insert("approve".into(), Box::new(approve_fragments));
    f.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, _: &TransitionScope| {
            let v = b.lit(WType::String, json!("busy"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![("reviewer".into(), "name".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    ModelSpec {
        input: bound_machine(extra),
        fragments: Box::new(f),
    }
}

fn receipt(out: &behavior_models::BuildOutput) -> &Value {
    &out.receipts[0].0
}

#[test]
fn bindings_are_state_parameters_after_the_subject_sorted_by_name() {
    let out = build(|_| {}, vec![bound_spec(true)]).unwrap_or_else(|e| panic!("{e:#?}"));
    let action = out
        .module
        .actions()
        .get("purchase_request__submit")
        .unwrap();
    let params: Vec<(String, String)> = action
        .params()
        .iter()
        .map(|p| (p.name().to_string(), format!("{:?}", p.role())))
        .collect();
    let state = |n: &str| (n.to_string(), "State".to_string());
    assert_eq!(
        params,
        vec![
            state("purchase_request"),
            state("auditor"),
            state("reviewer")
        ]
    );
    // The authored effect on the binding follows the generated status effect.
    let effects: Vec<(String, String)> = action
        .effects()
        .iter()
        .map(|e| (e.param().to_string(), e.field().to_string()))
        .collect();
    assert_eq!(
        effects,
        vec![
            ("purchase_request".to_string(), "status".to_string()),
            ("reviewer".to_string(), "name".to_string()),
        ]
    );
}

#[test]
fn a_machine_with_bindings_is_language_level_3_with_identified_bindings() {
    let out = build(|_| {}, vec![bound_spec(true)]).unwrap_or_else(|e| panic!("{e:#?}"));
    let r = receipt(&out);
    assert_eq!(r["model_semantic_version"], json!(3));
    let doc = &r["model"];
    assert_eq!(doc["format"], json!("behavior.model.state_machine.v3"));
    assert_eq!(doc["model_semantic_version"], json!(3));
    let actor_hash = behavior_engine::schema::declarations(&out.module)
        .remove("Actor")
        .unwrap();
    let transitions = doc["transitions"].as_array().unwrap();
    let submit = transitions.iter().find(|t| t["id"] == "submit").unwrap();
    assert_eq!(
        submit["state"],
        json!([
            {"name": "auditor", "entity": "Actor", "entity_hash": actor_hash},
            {"name": "reviewer", "entity": "Actor", "entity_hash": actor_hash},
        ])
    );
    // Every transition of a level-3 machine lists its bindings, possibly none.
    for t in transitions {
        assert!(t.get("state").is_some(), "{t}");
    }
}

#[test]
fn authoring_order_of_bindings_does_not_change_the_model_hash() {
    let a = build(|_| {}, vec![bound_spec(true)]).unwrap();
    let mut reversed = bound_spec(true);
    let submit = reversed
        .input
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.state.reverse();
    let b = build(|_| {}, vec![reversed]).unwrap();
    assert_eq!(receipt(&a)["model_hash"], receipt(&b)["model_hash"]);
    assert_eq!(a.module.behavior_version(), b.module.behavior_version());
    // A different binding set is a different model.
    let c = build(|_| {}, vec![bound_spec(false)]).unwrap();
    assert_ne!(receipt(&a)["model_hash"], receipt(&c)["model_hash"]);
}

#[test]
fn a_machine_without_bindings_stays_language_level_2() {
    let out = build_reference();
    let r = receipt(&out);
    assert_eq!(r["model_semantic_version"], json!(2));
    assert_eq!(
        r["model"]["format"],
        json!("behavior.model.state_machine.v2")
    );
    for t in r["model"]["transitions"].as_array().unwrap() {
        assert!(t.get("state").is_none(), "{t}");
    }
}

#[test]
fn a_binding_of_the_subject_type_cannot_write_the_owned_field() {
    let mut m = machine();
    let submit = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.state = vec![("other".into(), "PurchaseRequest".into())];
    let mut f: FragmentTable<'static> = BTreeMap::new();
    f.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, _: &TransitionScope| {
            let v = b.lit(status_ty(), json!("Approved"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![("other".into(), "status".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    let errs = build(
        |_| {},
        vec![ModelSpec {
            input: m,
            fragments: Box::new(f),
        }],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["TRANSITION_WRITES_OWNED_FIELD"]);
}

#[test]
fn a_binding_never_grants_another_machines_owned_field() {
    // `review_flow` owns `PurchaseRequest.review`; `purchase_request__submit` binds a second
    // request and writes its `review` field: the final pass rejects it whatever the binding.
    let mut m = machine();
    let submit = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.state = vec![("other".into(), "PurchaseRequest".into())];
    let mut f: FragmentTable<'static> = BTreeMap::new();
    f.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, _: &TransitionScope| {
            let v = b.lit(status_ty(), json!("Approved"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![("other".into(), "review".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    let mut review = machine();
    review.model_id = "review_flow".into();
    review.state_field = "review".into();
    review.transitions = vec![transition("open", &["Draft"], "InReview")];
    let errs = build(
        |_| {},
        vec![
            ModelSpec {
                input: m,
                fragments: Box::new(f),
            },
            spec_without_fragments(review),
        ],
    )
    .unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::UnauthorizedOwnedFieldWrite {
            action: "purchase_request__submit".into(),
            entity: "PurchaseRequest".into(),
            field: "review".into(),
            owner: "review_flow".into(),
        }]
    );
}

#[test]
fn an_unknown_binding_entity_and_a_colliding_binding_name_are_rejected() {
    let mut m = machine();
    let submit = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.state = vec![("x".into(), "Nope".into())];
    let errs = build(|_| {}, vec![spec_without_fragments(m)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["UNKNOWN_ENTITY"]);

    let mut m = machine();
    let approve = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "approve")
        .unwrap();
    // `actor` is already the context parameter; `purchase_request` is the subject.
    approve.state = vec![
        ("actor".into(), "Actor".into()),
        ("purchase_request".into(), "Actor".into()),
    ];
    let errs = build(|_| {}, vec![spec_without_fragments(m)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["INVALID_PARAMETER", "INVALID_PARAMETER"]);
}
