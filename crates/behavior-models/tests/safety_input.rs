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
