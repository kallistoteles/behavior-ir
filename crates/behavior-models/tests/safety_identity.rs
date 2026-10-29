#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "support/safety.rs"]
mod support;

use behavior_models::safety::Inventory;
use serde_json::json;

#[test]
fn inventory_hash_excludes_its_own_field_and_binds_exact_module() {
    let module = support::fixture();
    let inventory = Inventory::extract(&module).unwrap();
    let mut payload = inventory.to_json();
    let hash = payload
        .as_object_mut()
        .unwrap()
        .remove("inventory_hash")
        .unwrap();
    assert_eq!(
        hash,
        json!(
            behavior_engine::canonical::tagged_hash("behavior.safety_inventory.v1", &payload)
                .unwrap()
        )
    );
    assert_eq!(payload["behavior_hash"], module.behavior_version());
    assert_eq!(
        Inventory::from_json(&inventory.to_json())
            .unwrap()
            .to_json(),
        inventory.to_json()
    );
    let mut b = support::declarations();
    support::edit(&mut b, "other", "target", false);
    assert!(inventory.verify_module(&b.finish(None).unwrap()).is_err());
}

#[test]
fn repeat_encoding_is_canonical_and_type_changes_fail() {
    let module = support::fixture();
    let left = Inventory::extract(&module).unwrap();
    let right = Inventory::extract(&module).unwrap();
    assert_eq!(left.to_json().to_string(), right.to_json().to_string());
    let mut data = left.to_json();
    data["core_release"]["version"] = json!(true);
    support::rehash(&mut data);
    assert!(Inventory::from_json(&data).is_err());
}

#[test]
fn checker_transition_preserves_every_established_reference_semantic_identity() {
    use std::collections::BTreeMap;
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../models/fixtures/safety-compat.json")).unwrap();
    let mut cases = BTreeMap::new();
    let mut historical = false;
    let mut current = false;
    for tuple in manifest["tuples"].as_array().unwrap() {
        historical |= tuple["lowerer_version"] == 2;
        current |= tuple["lowerer_version"] == 3;
        for (name, case) in tuple["cases"].as_object().unwrap() {
            if let Some(old) = cases.insert(name, case) {
                assert_eq!(old, case, "historical case {name} changed");
            }
        }
    }
    assert!(historical && current);
    assert_eq!(behavior_models::safety::FIELD_OWNERSHIP_SEMANTIC_VERSION, 1);
}
