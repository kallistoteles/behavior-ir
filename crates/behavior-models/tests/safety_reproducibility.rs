#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/safety.rs"]
mod support;
use behavior_models::safety::certify::{ProofLimits, ProofToolchain, certify};
use behavior_models::safety::{FieldOwnershipInput, analyze};
use serde_json::json;

fn policy(label: &str, extra_permission: bool) -> FieldOwnershipInput {
    FieldOwnershipInput::from_json(&json!({"id":label,"entity":"Item",
        "writers":{"note":if extra_permission{vec!["edit","copy","create"]}else{vec!["copy","edit"]}},
        "creators":["create"],"removers":["remove"]})).unwrap()
}
#[test]
fn policy_identity_and_data_ignore_display_labels_and_input_order() {
    let m = support::fixture();
    let a = analyze(&m, &[policy("first_label", false)]).unwrap();
    let b = analyze(&m, &[policy("another_label", false)]).unwrap();
    assert_eq!(a.policy_set_hash(), b.policy_set_hash());
    assert_eq!(a.to_json(), b.to_json());
    let different = analyze(&m, &[policy("third_label", true)]).unwrap();
    assert_ne!(a.policy_set_hash(), different.policy_set_hash());
    assert_eq!(
        a.inventory().behavior_hash(),
        different.inventory().behavior_hash()
    );
}
#[test]
fn successful_records_are_identical_across_output_paths_and_resource_logs() {
    let m = support::fixture();
    let a = analyze(&m, &[policy("notes", false)]).unwrap();
    let tools = ProofToolchain::locked().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/evidence/repro-{}",
        std::process::id()
    ));
    let mut records = vec![];
    for name in ["one", "elsewhere"] {
        let result = certify(&m, &a, &root.join(name), &tools, &ProofLimits::default());
        assert!(result.certificate().is_some(), "{}", result.to_json());
        let record = result.to_json();
        let text = behavior_engine::canonical::to_canonical_string(&record).unwrap();
        assert!(!text.contains(&root.to_string_lossy().to_string()));
        assert!(!text.contains("elapsed_ms"));
        assert!(!text.contains("peak_sampled"));
        assert!(record["dependency_identity"].is_object());
        records.push(text);
    }
    assert_eq!(records[0], records[1]);
}
#[test]
fn untrusted_strings_are_literal_data_or_rejected_before_proof_generation() {
    use behavior_engine::{builder::ScopeSite, wire::WType};
    let mut b = support::declarations();
    let params = vec![support::state("renamed_binding")];
    b.push_scope(ScopeSite::Action, params.clone(), support::loc())
        .unwrap();
    let payload = "\naxiom injected : False\n-- not source";
    let value = b
        .lit(WType::String, json!(payload), support::loc())
        .unwrap();
    let effects = vec![(
        "renamed_binding".into(),
        "note".into(),
        value,
        support::loc(),
    )];
    assert!(
        b.add_action(
            "action\n-- not an identifier",
            params.clone(),
            vec![],
            effects.clone(),
            vec![],
            vec![],
            support::loc()
        )
        .is_err()
    );
    b.add_action(
        "edit",
        params,
        vec![],
        effects,
        vec![],
        vec![],
        support::loc(),
    )
    .unwrap();
    b.pop_scope();
    let m = b.finish(None).unwrap();
    let p = FieldOwnershipInput::from_json(
        &json!({"id":"control","entity":"Item","writers":{"note":["edit"]}}),
    )
    .unwrap();
    let a = analyze(&m, &[p]).unwrap();
    assert_eq!(a.inventory().actions().len(), 1);
    assert_eq!(a.to_json()["is_certified"], false);
    let input = behavior_models::safety::certify::proof_input(&m, &a).unwrap();
    assert!(!input.contains(payload));
    assert!(!input.contains("axiom injected"));
}

#[test]
fn release_recognition_uses_new_checker_identity_without_changing_policy_semantics() {
    assert_eq!(behavior_models::safety::SAFETY_CHECKER_VERSION, 2);
    assert_eq!(behavior_models::safety::FIELD_OWNERSHIP_SEMANTIC_VERSION, 1);
}
