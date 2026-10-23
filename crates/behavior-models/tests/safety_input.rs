#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "support/safety.rs"]
mod support;

use behavior_models::safety::{Inventory, UncertifiedSafetyAnalysis};
use serde_json::json;

#[test]
fn inventory_is_typed_and_cannot_claim_certification() {
    let module = support::fixture();
    let analysis = UncertifiedSafetyAnalysis::new(&module).unwrap();
    assert_eq!(analysis.status(), "Lowered");
    assert!(!analysis.is_certified());
    let mut data = analysis.inventory().to_json();
    data["certified"] = json!(true);
    support::rehash(&mut data);
    assert_eq!(
        Inventory::from_json(&data).unwrap_err().code(),
        "SAFETY_INVALID_DATA"
    );
}

#[test]
fn malformed_binding_and_duplicate_action_are_rejected() {
    let inventory = Inventory::extract(&support::fixture()).unwrap();
    let mut data = inventory.to_json();
    data["actions"][0]["params"][0]["name"] = json!(false);
    support::rehash(&mut data);
    assert!(Inventory::from_json(&data).is_err());
    let mut data = inventory.to_json();
    let duplicate = data["actions"][0].clone();
    data["actions"].as_array_mut().unwrap().push(duplicate);
    support::rehash(&mut data);
    assert!(Inventory::from_json(&data).is_err());
}

#[test]
fn operation_occurrences_are_not_boolean_numbers() {
    let mut data = Inventory::extract(&support::fixture()).unwrap().to_json();
    data["actions"][0]["operations"][0]["index"] = json!(false);
    support::rehash(&mut data);
    assert!(Inventory::from_json(&data).is_err());
}

#[test]
fn scalable_encoding_preserves_full_keys_and_exact_payload_at_large_sizes() {
    use behavior_models::safety::{FieldOwnershipInput, analyze, certify::proof_input};
    let mut builder = support::declarations();
    let names = (0..512).map(|i| format!("edit_{i}")).collect::<Vec<_>>();
    for name in &names {
        use behavior_engine::{builder::ScopeSite, wire::WType};
        let params = vec![support::state("aliased")];
        builder
            .push_scope(ScopeSite::Action, params.clone(), support::loc())
            .unwrap();
        let value = builder
            .lit(WType::String, json!(name), support::loc())
            .unwrap();
        builder
            .add_action(
                name,
                params,
                vec![],
                vec![("aliased".into(), "note".into(), value, support::loc())],
                vec![],
                vec![],
                support::loc(),
            )
            .unwrap();
        builder.pop_scope();
    }
    let module = builder.finish(None).unwrap();
    let policy = FieldOwnershipInput::from_json(
        &json!({"id":"all","entity":"Item","writers":{"note":names}}),
    )
    .unwrap();
    let analysis = analyze(&module, &[policy]).unwrap();
    let source = proof_input(&module, &analysis).unwrap();
    assert!(source.contains("noncomputable def data : Safety.ModuleData"));
    assert!(source.contains("Safety.Candidate.compactPayload"));
    assert!(!source.contains("def inventory0 : List Nat"));
    assert!(source.contains("Safety.Candidate.checked_from_parts"));
    assert!(source.contains("Safety.Candidate.traceChecked_sound"));
    for (index, action) in analysis.inventory().actions().values().enumerate() {
        let bytes = action
            .hash
            .bytes()
            .map(|v| format!("(nat_lit {v})"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(source.contains(&format!("def actionHash{index} : List Nat := [{bytes}]")));
    }
    assert_eq!(analysis.inventory().actions().len(), 512);
}
