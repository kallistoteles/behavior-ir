//! Ownership: the final composition pass rejects every unauthorized write of an owned field and
//! every creation that does not start in a literal entry state (composition contract §4–§6;
//! T034–T036).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::wire::{WLifecycle, WType};
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{ModelError, ModelSpec, TransitionFragments, compose};
use common::*;
use serde_json::json;

fn force_approve(b: &mut Builder) {
    hand_written_set(b, "force_approve", "status", json!("Approved"), status_ty());
}

// --- T034: writes -----------------------------------------------------------------------------

#[test]
fn a_hand_written_writer_of_the_owned_field_is_rejected() {
    let errs = build(force_approve, vec![spec(machine())]).unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::UnauthorizedOwnedFieldWrite {
            action: "force_approve".into(),
            entity: "PurchaseRequest".into(),
            field: "status".into(),
            owner: "purchase_request".into(),
        }]
    );
}

#[test]
fn the_same_writer_is_fine_without_the_machine() {
    build(force_approve, vec![]).unwrap();
}

#[test]
fn writing_other_fields_reading_and_removing_are_not_violations() {
    build(
        |b| {
            hand_written_set(b, "edit_note", "note", json!("hello"), WType::String);
            // Reads the owned field in a precondition.
            let p = state_param("request", "PurchaseRequest");
            b.push_scope(ScopeSite::Action, vec![p.clone()], loc())
                .unwrap();
            let st = b.field("request", "status", loc()).unwrap();
            let draft = b.lit(status_ty(), json!("Draft"), loc()).unwrap();
            let is_draft = b.op("eq", vec![st, draft], loc()).unwrap();
            let v = b.lit(WType::String, json!("x"), loc()).unwrap();
            b.add_action(
                "annotate_draft",
                vec![p.clone()],
                vec![(is_draft, loc())],
                vec![("request".into(), "note".into(), v, loc())],
                vec![],
                vec![],
                loc(),
            )
            .unwrap();
            // Removes the entity.
            b.add_action(
                "discard",
                vec![p],
                vec![],
                vec![],
                vec![WLifecycle::Remove {
                    param: "request".into(),
                    loc: loc(),
                }],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(machine())],
    )
    .unwrap();
}

/// Machine `review_flow` owns `PurchaseRequest.review`.
fn review_machine() -> behavior_models::ModelInput {
    let mut m = machine();
    m.model_id = "review_flow".into();
    m.state_field = "review".into();
    m.transitions = vec![transition("open", &["Draft"], "InReview")];
    m
}

#[test]
fn a_machine_writing_a_field_owned_by_another_machine_is_rejected() {
    let mut fragments: FragmentTable<'static> = BTreeMap::new();
    fragments.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, s: &behavior_models::TransitionScope| {
            let v = b.lit(status_ty(), json!("Approved"), loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![(subject(s), "review".into(), v, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    let errs = build(
        |_| {},
        vec![
            ModelSpec {
                input: machine(),
                fragments: Box::new(fragments),
            },
            spec_without_fragments(review_machine()),
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

// --- T035: creations ----------------------------------------------------------------------------

fn create_with(
    status: impl FnOnce(&mut Builder) -> behavior_engine::builder::Node + 'static,
) -> Result<behavior_models::BuildOutput, Vec<ModelError>> {
    build(
        move |b| hand_written_create(b, "open_request", status),
        vec![spec(machine())],
    )
}

#[test]
fn creation_in_a_literal_entry_state_is_allowed() {
    create_with(|b| b.lit(status_ty(), json!("Draft"), loc()).unwrap()).unwrap();
}

#[test]
fn creation_in_a_literal_non_entry_state_is_rejected() {
    let errs = create_with(|b| b.lit(status_ty(), json!("Approved"), loc()).unwrap()).unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::EntryStateNotAllowed {
            action: "open_request".into(),
            entity: "PurchaseRequest".into(),
            field: "status".into(),
            state: "Approved".into(),
        }]
    );
}

#[test]
fn creation_from_an_input_is_not_static() {
    let errs = create_with(|b| b.param("initial", loc()).unwrap()).unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::EntryStateNotStatic {
            action: "open_request".into(),
            entity: "PurchaseRequest".into(),
            field: "status".into(),
        }]
    );
}

#[test]
fn creation_from_a_computation_is_not_static_even_if_always_an_entry_state() {
    // `value_or(some(Draft), Draft)` always yields Draft, but v1 accepts literals only.
    let errs = create_with(|b| {
        let d = b.lit(status_ty(), json!("Draft"), loc()).unwrap();
        let some = b.op("some", vec![d.clone()], loc()).unwrap();
        b.op("value_or", vec![some, d], loc()).unwrap()
    })
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["ENTRY_STATE_NOT_STATIC"]);
}

#[test]
fn no_creation_at_all_is_fine() {
    build_reference();
}

// --- T036: no escape hatch ----------------------------------------------------------------------

#[test]
fn every_public_way_to_compose_checks_ownership() {
    // `compose` takes the builder, the models and the location root: nothing else. Every root
    // option still rejects the bypassing writer.
    for root in [None, Some("."), Some("/")] {
        let mut b = declarations();
        force_approve(&mut b);
        let errs = compose(&mut b, vec![spec(machine())], root).unwrap_err();
        assert_eq!(codes(&errs), vec!["UNAUTHORIZED_OWNED_FIELD_WRITE"]);
    }
}
