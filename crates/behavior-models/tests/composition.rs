//! Composition of several models and hand-written behavior (composition contract §2–§3, §6;
//! T059, T063, T067).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use behavior_models::{ModelError, compose};
use common::*;
use serde_json::json;

#[test]
fn independent_machines_reuse_local_transition_ids() {
    let out = build(
        with_invoice,
        vec![spec(machine()), spec_without_fragments(invoice_machine())],
    )
    .unwrap();
    let actions: Vec<&String> = out.module.actions().keys().collect();
    assert!(actions.contains(&&"invoice__submit".to_string()));
    assert!(actions.contains(&&"purchase_request__submit".to_string()));
    assert_eq!(out.receipts.len(), 2);
    // Receipts in model-id order.
    assert_eq!(out.receipts[0].0["model"]["model_id"], "invoice");
}

#[test]
fn two_machines_on_different_fields_of_one_entity() {
    let mut review = machine();
    review.model_id = "review_flow".into();
    review.state_field = "review".into();
    review.transitions = vec![transition("open", &["Draft"], "InReview")];
    build(
        |_| {},
        vec![spec(machine()), spec_without_fragments(review)],
    )
    .unwrap();
}

#[test]
fn two_machines_on_the_same_field_conflict() {
    let mut other = machine();
    other.model_id = "another".into();
    for order in [
        vec![spec(machine()), spec(other.clone())],
        vec![spec(other.clone()), spec(machine())],
    ] {
        let errs = build(|_| {}, order).unwrap_err();
        assert_eq!(
            errs,
            vec![ModelError::ConflictingOwnership {
                entity: "PurchaseRequest".into(),
                field: "status".into(),
                models: vec!["another".into(), "purchase_request".into()],
            }]
        );
    }
}

#[test]
fn duplicate_model_ids() {
    let errs = build(|_| {}, vec![spec(machine()), spec(machine())]).unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::DuplicateModelId {
            model_id: "purchase_request".into()
        }]
    );
}

#[test]
fn a_hand_written_item_taking_a_generated_name_is_a_collision_never_renamed() {
    let errs = build(
        |b| {
            hand_written_set(
                b,
                "purchase_request__reject",
                "note",
                json!("x"),
                WType::String,
            )
        },
        vec![spec(machine())],
    )
    .unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::GeneratedNameCollision {
            model_ref: "purchase_request/transition/reject".into(),
            name: "purchase_request__reject".into(),
        }]
    );
}

/// T063: StateMachine contributes actions only; it never declares entities, enums or fields.
#[test]
fn state_machine_contributes_only_actions() {
    let before = build(|_| {}, vec![]).unwrap().module;
    let after = build_reference().module;
    for (kind, name) in after.name_table().keys() {
        if !before.name_table().keys().any(|(_, n)| n == name) {
            assert_eq!(kind.as_str(), "action", "{name}");
        }
    }
}

/// T067 (US5, FR-032), scope fixed in feature 502 (T002) and extended in feature 503 (T002):
/// StateMachine and the shared model infrastructure stay free of request and approval concepts,
/// so future models can reuse them. Kind-specific code (`request_flow/`, `approval/`), the kind
/// registry (`kinds.rs`, `lib.rs`) and the shared error catalogue (`error.rs`) are outside the
/// scan. Comments and rustdoc may use the reference examples.
#[test]
fn the_model_crate_is_domain_neutral() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack: Vec<std::path::PathBuf> = [
        "state_machine",
        "ids.rs",
        "composition.rs",
        "identity.rs",
        "source_map.rs",
    ]
    .iter()
    .map(|p| src.join(p))
    .collect();
    let mut hits = Vec::new();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
            continue;
        }
        {
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("").to_lowercase();
                for word in [
                    "request",
                    "approv",
                    "assignee",
                    "decision",
                    "reject",
                    "threshold",
                ] {
                    if code.contains(word) {
                        hits.push(format!("{}:{}: {line}", path.display(), n + 1));
                    }
                }
            }
        }
    }
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn compose_rejects_inadmissible_ordinary_behavior_before_models() {
    let mut b = declarations();
    b.push_scope(behavior_engine::builder::ScopeSite::Closed, vec![], loc())
        .unwrap();
    let ghost = b.derived_ref("ghost", vec![], loc()).unwrap();
    b.add_global_invariant("broken", ghost, loc()).unwrap();
    b.pop_scope();
    let errs = compose(&mut b, vec![spec(machine())], None).unwrap_err();
    assert_eq!(codes(&errs), vec!["CORE_ADMISSION_FAILED"]);
}
