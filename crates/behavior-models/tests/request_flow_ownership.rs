//! The ownership matrix (research R7; T025).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::wire::{Role, WLifecycle, WParam, WType};
use behavior_models::{CreationScope, ModelError};
use common::rf::*;
use common::{codes, loc};
use serde_json::json;

fn request_param() -> WParam {
    WParam {
        name: "r".into(),
        role: Some(Role::State),
        ty: WType::Entity("PurchaseRequest".into()),
    }
}

fn hand_set(b: &mut Builder, name: &str, field: &str, ty: WType, value: serde_json::Value) {
    b.push_scope(ScopeSite::Action, vec![request_param()], loc())
        .unwrap();
    let v = b.lit(ty, value, loc()).unwrap();
    b.add_action(
        name,
        vec![request_param()],
        vec![],
        vec![("r".into(), field.into(), v, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

#[test]
fn hand_written_creation_of_the_request_is_rejected() {
    let errs = build_with(
        |b| {
            let ps = vec![
                WParam {
                    name: "id".into(),
                    role: Some(Role::Input),
                    ty: WType::Id("PurchaseRequest".into()),
                },
                WParam {
                    name: "u".into(),
                    role: Some(Role::Input),
                    ty: WType::Id("User".into()),
                },
            ];
            b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
            let id = b.param("id", loc()).unwrap().into_wire();
            let u = b.param("u", loc()).unwrap();
            let none = b
                .lit(
                    WType::Option(Box::new(WType::Id("User".into()))),
                    json!(null),
                    loc(),
                )
                .unwrap();
            let st = b
                .lit(
                    WType::Enum("PurchaseRequestStatus".into()),
                    json!("Draft"),
                    loc(),
                )
                .unwrap();
            let t = b.lit(WType::String, json!("x"), loc()).unwrap();
            let a = b.lit(WType::Int, json!(1), loc()).unwrap();
            let create = WLifecycle::Create {
                entity: "PurchaseRequest".into(),
                id,
                fields: vec![
                    ("amount".into(), a.into_wire()),
                    ("assignee".into(), none.into_wire()),
                    ("requester".into(), u.into_wire()),
                    ("status".into(), st.into_wire()),
                    ("title".into(), t.into_wire()),
                ],
                loc: loc(),
            };
            b.add_action(
                "sneaky_create",
                ps,
                vec![],
                vec![],
                vec![create],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(flow(), source())],
    )
    .unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::UnauthorizedRequestCreation {
            action: "sneaky_create".into(),
            entity: "PurchaseRequest".into(),
            owner: "purchase_request".into(),
        }]
    );
}

#[test]
fn hand_written_status_and_requester_writes_are_rejected() {
    let errs = build_with(
        |b| {
            hand_set(
                b,
                "force",
                "status",
                WType::Enum("PurchaseRequestStatus".into()),
                json!("Approved"),
            )
        },
        vec![spec(flow(), source())],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_OWNED_FIELD_WRITE"]);
    let errs = build_with(
        |b| {
            b.push_scope(
                ScopeSite::Action,
                vec![
                    request_param(),
                    WParam {
                        name: "u".into(),
                        role: Some(Role::Input),
                        ty: WType::Id("User".into()),
                    },
                ],
                loc(),
            )
            .unwrap();
            let u = b.param("u", loc()).unwrap();
            b.add_action(
                "steal",
                vec![
                    request_param(),
                    WParam {
                        name: "u".into(),
                        role: Some(Role::Input),
                        ty: WType::Id("User".into()),
                    },
                ],
                vec![],
                vec![("r".into(), "requester".into(), u, loc())],
                vec![],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(flow(), source())],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_REQUESTER_WRITE"]);
}

#[test]
fn assignee_and_payload_writes_are_ordinary() {
    build_with(
        |b| {
            hand_set(b, "retitle", "title", WType::String, json!("new"));
            hand_set(
                b,
                "unassign",
                "assignee",
                WType::Option(Box::new(WType::Id("User".into()))),
                json!(null),
            );
        },
        vec![spec(flow(), source())],
    )
    .unwrap();
}

#[test]
fn a_transition_writing_the_requester_is_rejected() {
    let mut s = source();
    s.transitions.insert(
        "approve".into(),
        Box::new(|b: &mut Builder, sc: &behavior_models::TransitionScope| {
            let subject = sc.params[0].name.clone();
            let other = b.field(&subject, "requester", loc()).unwrap();
            Ok(behavior_models::TransitionFragments {
                effects: vec![(subject, "requester".into(), other, loc())],
                ..Default::default()
            })
        }),
    );
    let errs = build_with(|_| {}, vec![spec(flow(), s)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_REQUESTER_WRITE"]);
}

/// Creation effects over a state binding of the request type stay under the ownership pass.
#[test]
fn creation_effects_over_a_request_binding_cannot_write_status_or_requester() {
    for (field, ty, value, code) in [
        (
            "status",
            WType::Enum("PurchaseRequestStatus".into()),
            json!("Approved"),
            "UNAUTHORIZED_OWNED_FIELD_WRITE",
        ),
        ("title", WType::String, json!("ok"), ""),
    ] {
        let mut f = flow();
        f.creations[0].state = vec![("previous".into(), "PurchaseRequest".into())];
        let mut s = source();
        let ty2 = ty.clone();
        let v2 = value.clone();
        s.creations.insert(
            "create".into(),
            Box::new(move |b: &mut Builder, sc: &CreationScope| {
                let mut fr = standard(b, sc)?;
                let v = b.lit(ty2.clone(), v2.clone(), loc()).unwrap();
                fr.effects.push(("previous".into(), field.into(), v, loc()));
                Ok(fr)
            }),
        );
        let result = build_with(|_| {}, vec![spec(f, s)]);
        if code.is_empty() {
            result.unwrap();
        } else {
            assert_eq!(codes(&result.unwrap_err()), vec![code]);
        }
    }
}

fn transition_effect(
    field: &'static str,
    ty: WType,
    value: serde_json::Value,
) -> Vec<&'static str> {
    let mut s = source();
    s.transitions.insert(
        "approve".into(),
        Box::new(
            move |b: &mut Builder, sc: &behavior_models::TransitionScope| {
                let subject = sc.params[0].name.clone();
                let v = b.lit(ty.clone(), value.clone(), loc()).unwrap();
                Ok(behavior_models::TransitionFragments {
                    effects: vec![(subject, field.into(), v, loc())],
                    ..Default::default()
                })
            },
        ),
    );
    match build_with(|_| {}, vec![spec(flow(), s)]) {
        Ok(_) => vec![],
        Err(e) => codes(&e),
    }
}

#[test]
fn transition_effects_on_status_are_rejected_and_on_assignee_accepted() {
    assert_eq!(
        transition_effect(
            "status",
            WType::Enum("PurchaseRequestStatus".into()),
            json!("Rejected")
        ),
        vec!["TRANSITION_WRITES_OWNED_FIELD"]
    );
    assert!(
        transition_effect(
            "assignee",
            WType::Option(Box::new(WType::Id("User".into()))),
            json!(null)
        )
        .is_empty()
    );
}

#[test]
fn creation_effects_over_a_request_binding_cannot_write_the_requester() {
    let mut f = flow();
    f.creations[0].state = vec![("previous".into(), "PurchaseRequest".into())];
    let mut s = source();
    s.creations.insert(
        "create".into(),
        Box::new(|b: &mut Builder, sc: &CreationScope| {
            let mut fr = standard(b, sc)?;
            let actor = b.param("actor", loc()).unwrap();
            fr.effects
                .push(("previous".into(), "requester".into(), actor, loc()));
            Ok(fr)
        }),
    );
    let errs = build_with(|_| {}, vec![spec(f, s)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_REQUESTER_WRITE"]);
}
