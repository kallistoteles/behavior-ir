//! Declaration staging laws (declaration-staging contract §2; T006, Rust part). The 501 hash
//! regression over the pinned reference cases is in python/tests/test_model_staging.py.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::{WField, WType};
use behavior_models::{AnyModel, compose_models, declare_models};
use common::rf::*;
use common::{codes, loc, machine, spec as sm_spec};
use serde_json::json;

/// D2: a StateMachine-only set leaves the builder untouched.
#[test]
fn declaration_free_models_do_not_touch_the_builder() {
    let mut b = common::declarations();
    let before = b.finish(None).unwrap();
    let models: Vec<AnyModel<'static>> = vec![sm_spec(machine()).into()];
    assert!(declare_models(&mut b, &models).unwrap().is_empty());
    let after = b.finish(None).unwrap();
    assert_eq!(before.behavior_version(), after.behavior_version());
    assert_eq!(before.name_table(), after.name_table());
}

/// D1, D5: the request entity exists after P2, before any hand-written behavior.
#[test]
fn the_request_entity_is_declared_in_p2() {
    let mut b = authored();
    let models = vec![spec(flow(), source())];
    let declared = declare_models(&mut b, &models).unwrap();
    assert_eq!(declared[0].entity, "PurchaseRequest");
    let names: Vec<&str> = declared[0].fields.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["amount", "assignee", "requester", "status", "title"]
    );
    assert!(
        b.finish(None)
            .unwrap()
            .entities()
            .contains_key("PurchaseRequest")
    );
}

/// D3: the order of authored declarations does not reach any hash.
#[test]
fn authored_declaration_order_is_not_semantic() {
    let build = |reverse: bool| {
        let mut b = behavior_engine::builder::Builder::with_semantic_profile(
            behavior_engine::builder::SemanticProfile::CommandIntents,
        );
        b.declare_enum(
            "PurchaseRequestStatus",
            STATUSES.iter().map(|s| s.to_string()).collect(),
            loc(),
        )
        .unwrap();
        let user = || WField {
            name: "name".into(),
            ty: WType::String,
            loc: loc(),
        };
        let budget = || WField {
            name: "reserved".into(),
            ty: WType::Int,
            loc: loc(),
        };
        if reverse {
            b.declare_entity("Budget", vec![budget()], loc()).unwrap();
            b.declare_entity("User", vec![user()], loc()).unwrap();
        } else {
            b.declare_entity("User", vec![user()], loc()).unwrap();
            b.declare_entity("Budget", vec![budget()], loc()).unwrap();
        }
        let models = vec![spec(flow(), source())];
        declare_models(&mut b, &models).unwrap();
        commands(&mut b);
        compose_models(&mut b, models, None)
            .unwrap()
            .module
            .behavior_version()
    };
    assert_eq!(build(false), build(true));
}

/// D6: an authored declaration with the generated name is an error unless identical.
#[test]
fn a_conflicting_authored_declaration_is_a_collision() {
    let mut b = authored();
    b.declare_entity(
        "PurchaseRequest",
        vec![WField {
            name: "other".into(),
            ty: WType::Int,
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
    let errs = declare_models(&mut b, &[spec(flow(), source())]).unwrap_err();
    assert_eq!(codes(&errs), vec!["DECLARATION_COLLISION"]);
}

#[test]
fn an_identical_authored_declaration_is_accepted() {
    let mut b = authored();
    let mut probe = authored();
    let schema = declare_models(&mut probe, &[spec(flow(), source())])
        .unwrap()
        .remove(0);
    let fields = schema
        .fields
        .iter()
        .map(|(n, t)| WField {
            name: n.clone(),
            ty: t.clone(),
            loc: loc(),
        })
        .collect();
    b.declare_entity("PurchaseRequest", fields, loc()).unwrap();
    let models = vec![spec(flow(), source())];
    declare_models(&mut b, &models).unwrap();
    commands(&mut b);
    compose_models(&mut b, models, None).unwrap();
}

/// D4: composing without the contributed declaration in place is a mismatch.
#[test]
fn skipping_p2_is_a_declaration_mismatch() {
    let mut b = authored();
    let errs = compose_models(&mut b, vec![spec(flow(), source())], None).unwrap_err();
    assert_eq!(codes(&errs), vec!["DECLARATION_MISMATCH"]);
}

fn comment_on_request(b: &mut behavior_engine::builder::Builder) {
    b.declare_entity(
        "Comment",
        vec![WField {
            name: "on".into(),
            ty: WType::Id("PurchaseRequest".into()),
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
}

/// D5: an authored entity may name the generated entity; Core checks declarations together.
#[test]
fn authored_declarations_may_reference_the_request_entity() {
    let mut b = authored();
    comment_on_request(&mut b);
    let models = vec![spec(flow(), source())];
    declare_models(&mut b, &models).unwrap();
    commands(&mut b);
    let out = compose_models(&mut b, models, None).unwrap();
    assert!(out.module.entities().contains_key("Comment"));
}

/// On the forward-reference path a missing actor or status enum makes the declarations
/// inadmissible as a whole, and Core's public API names the missing type only in its message
/// (data model §3, note). It is therefore Core's admission failure carrying Core's own code,
/// rather than `UNKNOWN_ENTITY` / `UNKNOWN_ENUM`; a collision stays a model error.
#[test]
fn forward_references_keep_model_errors() {
    let core_errors = |f: behavior_models::request_flow::input::RequestFlowInput| {
        let mut b = authored();
        comment_on_request(&mut b);
        let errs = declare_models(&mut b, &[spec(f, source())]).unwrap_err();
        assert_eq!(codes(&errs), vec!["CORE_ADMISSION_FAILED"]);
        errs[0].to_json()["errors"].clone()
    };
    let mut f = flow();
    f.actor_entity = "Ghost".into();
    assert_eq!(
        core_errors(f),
        json!([{"code": "UNKNOWN_ENTITY", "message": "unknown entity `Ghost`"},
               {"code": "UNKNOWN_ENTITY", "message": "unknown entity `Ghost`"}])
    );
    let mut f = flow();
    f.status_enum = "GhostStatus".into();
    assert_eq!(
        core_errors(f),
        json!([{"code": "UNKNOWN_TYPE", "message": "unknown enum `GhostStatus`"}])
    );

    let mut b = authored();
    comment_on_request(&mut b);
    b.declare_entity(
        "PurchaseRequest",
        vec![WField {
            name: "other".into(),
            ty: WType::Int,
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
    let errs = declare_models(&mut b, &[spec(flow(), source())]).unwrap_err();
    assert_eq!(codes(&errs), vec!["DECLARATION_COLLISION"]);
}
