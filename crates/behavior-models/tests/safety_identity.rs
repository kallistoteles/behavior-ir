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

// Inventory identity includes the exact Core release. Its equality is meaningful
// within that dependency context; Model/policy/behavior identity crosses contexts.
fn assert_established_cases(manifest: &serde_json::Value) {
    use std::collections::BTreeMap;
    let mut semantics = BTreeMap::new();
    let mut inventories = BTreeMap::new();
    for tuple in manifest["tuples"].as_array().unwrap() {
        let core = tuple["core_release"].to_string();
        for (name, case) in tuple["cases"].as_object().unwrap() {
            let mut semantic = case.clone();
            semantic
                .as_object_mut()
                .unwrap()
                .remove("inventory_hash")
                .unwrap();
            if let Some(old) = semantics.insert(name.clone(), semantic.clone()) {
                assert_eq!(old, semantic, "historical semantics for {name} changed");
            }
            if let Some(old) = inventories.insert((core.clone(), name.clone()), case.clone()) {
                assert_eq!(old, *case, "same-Core inventory for {name} changed");
            }
        }
    }
}

#[test]
fn checker_transition_preserves_every_established_reference_semantic_identity() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../models/fixtures/safety-compat.json")).unwrap();
    assert_established_cases(&manifest);
    let tuples = manifest["tuples"].as_array().unwrap();
    assert!(tuples.iter().any(|t| t["lowerer_version"] == 2));
    assert!(tuples.iter().any(|t| t["lowerer_version"] == 3));
    assert_eq!(behavior_models::safety::FIELD_OWNERSHIP_SEMANTIC_VERSION, 1);
}

#[test]
fn dependency_change_allows_only_inventory_identity_drift() {
    let case = json!({"behavior_hash":"same", "policy_set_hash":"same", "models":["same"],
                      "generating_models":[], "inventory_hash":"old"});
    let original =
        json!({"core_release":{"version":"old","commit":"old"}, "cases":{"reference":case}});
    let mut incoming = original.clone();
    incoming["core_release"] = json!({"version":"new","commit":"new"});
    incoming["cases"]["reference"]["inventory_hash"] = json!("new");
    let accepted = json!({"tuples":[original, incoming]});
    assert_established_cases(&accepted);
    for field in [
        "behavior_hash",
        "policy_set_hash",
        "models",
        "generating_models",
    ] {
        let mut changed = accepted.clone();
        changed["tuples"][1]["cases"]["reference"][field] = json!("changed");
        assert!(
            std::panic::catch_unwind(|| assert_established_cases(&changed)).is_err(),
            "{field}"
        );
    }
    let mut same_core = accepted.clone();
    same_core["tuples"][1]["core_release"] = same_core["tuples"][0]["core_release"].clone();
    assert!(std::panic::catch_unwind(|| assert_established_cases(&same_core)).is_err());
    let mut missing_hash = accepted;
    missing_hash["tuples"][1]["cases"]["reference"]
        .as_object_mut()
        .unwrap()
        .remove("inventory_hash");
    assert!(std::panic::catch_unwind(|| assert_established_cases(&missing_hash)).is_err());
}
