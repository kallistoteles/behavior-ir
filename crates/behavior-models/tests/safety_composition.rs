#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use behavior_models::safety::{FieldOwnershipInput, analyze};
use serde_json::json;

#[test]
fn every_generated_model_is_subject_to_additional_restrictions() {
    for output in [
        common::build_reference(),
        common::rf::build_reference(),
        common::ap::build(2, Some(2)),
        common::wq::build_reference(),
    ] {
        let before = output.module.behavior_version();
        let receipts = output
            .receipts
            .iter()
            .map(|r| r.0.clone())
            .collect::<Vec<_>>();
        let inventory = behavior_models::safety::Inventory::extract(&output.module).unwrap();
        let (action, op) = inventory
            .actions()
            .values()
            .find_map(|a| a.operations.first().map(|o| (a, o)))
            .unwrap();
        let entity = op.entity();
        let writers = match op {
            behavior_models::safety::inventory::Operation::Assign { field, .. } => {
                json!({field:[]})
            }
            _ => json!({}),
        };
        let forbidden =
            FieldOwnershipInput::from_json(&json!({"id":"restriction", "entity":entity,
            "writers":writers}))
            .unwrap();
        let e = analyze(&output.module, &[forbidden]).unwrap_err();
        assert!(e.code().starts_with("SAFETY_UNAUTHORIZED_"));
        let writers = match op {
            behavior_models::safety::inventory::Operation::Assign { field, .. } => {
                json!({field:inventory.actions().keys().collect::<Vec<_>>()})
            }
            _ => json!({}),
        };
        let allowed = FieldOwnershipInput::from_json(&json!({"id":"restriction", "entity":entity,
            "writers":writers,
            "creators":inventory.actions().keys().collect::<Vec<_>>(),
            "removers":inventory.actions().keys().collect::<Vec<_>>()}))
        .unwrap();
        analyze(&output.module, &[allowed]).unwrap();
        assert_eq!(output.module.behavior_version(), before);
        assert_eq!(
            output
                .receipts
                .iter()
                .map(|r| r.0.clone())
                .collect::<Vec<_>>(),
            receipts
        );
        assert!(!action.name.is_empty());
    }
}

#[test]
fn safety_aware_composition_preserves_old_output_and_cannot_override_model_ownership() {
    let mut b = common::declarations();
    let plain = common::build_reference();
    let protected = behavior_models::compose_models_with_safety(
        &mut b,
        vec![common::spec(common::machine()).into()],
        None,
        &[],
    )
    .unwrap();
    assert_eq!(
        plain.module.behavior_version(),
        protected.behavior.module.behavior_version()
    );
    assert_eq!(plain.receipts, protected.behavior.receipts);
    let mut b = common::declarations();
    common::hand_written_set(
        &mut b,
        "force_approve",
        "status",
        json!("Approved"),
        common::status_ty(),
    );
    let policy = FieldOwnershipInput::from_json(&json!({"id":"grant", "entity":"PurchaseRequest",
        "writers":{"status":["force_approve"]}}))
    .unwrap();
    let e = behavior_models::compose_models_with_safety(
        &mut b,
        vec![common::spec(common::machine()).into()],
        None,
        &[policy],
    )
    .unwrap_err();
    assert!(matches!(e, behavior_models::SafetyBuildError::Models(_)));
}

#[test]
fn generated_selector_resolves_to_same_authority_as_exact_name() {
    let output = common::build_reference();
    let names = output.module.actions().keys().cloned().collect::<Vec<_>>();
    let generated = names
        .iter()
        .map(|n| {
            let (model, local) = n.split_once("__").unwrap();
            json!({"model_id":model,"local_action_id":local})
        })
        .collect::<Vec<_>>();
    let policy = |writers| {
        FieldOwnershipInput::from_json(&json!({"id":"status",
        "entity":"PurchaseRequest","writers":{"status":writers}}))
        .unwrap()
    };
    assert_eq!(
        analyze(&output.module, &[policy(json!(names))])
            .unwrap()
            .policy_set_hash(),
        analyze(&output.module, &[policy(json!(generated))])
            .unwrap()
            .policy_set_hash()
    );
}
