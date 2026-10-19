#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "support/safety.rs"]
mod support;

use behavior_models::safety::Inventory;
use serde_json::json;

#[test]
fn inventories_all_actions_bindings_and_distinct_mutation_kinds() {
    let module = support::fixture();
    let inventory = Inventory::extract(&module).unwrap();
    assert_eq!(inventory.actions().len(), 4);
    let data = inventory.to_json();
    assert_eq!(
        support::action(&data, "edit")["operations"][0]["entity"],
        "Item"
    );
    assert_eq!(
        support::action(&data, "edit")["operations"][0]["param"],
        "renamed_subject"
    );
    assert_eq!(
        support::action(&data, "copy")["params"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        support::action(&data, "create")["operations"][0]["kind"],
        "create"
    );
    assert_eq!(
        support::action(&data, "remove")["operations"][0]["kind"],
        "remove"
    );
    assert_eq!(
        support::action(&data, "create")["operations"][0]["fields"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    inventory.verify_module(&module).unwrap();
}

#[test]
fn independent_module_comparison_detects_an_omitted_action_or_effect() {
    let module = support::fixture();
    let inventory = Inventory::extract(&module).unwrap();
    let mut data = inventory.to_json();
    data["actions"].as_array_mut().unwrap().pop();
    support::rehash(&mut data);
    let incomplete = Inventory::from_json(&data).unwrap();
    assert_eq!(
        incomplete.verify_module(&module).unwrap_err().code(),
        "SAFETY_INVENTORY_MISMATCH"
    );
    let mut data = inventory.to_json();
    data["actions"][0]["operations"]
        .as_array_mut()
        .unwrap()
        .clear();
    support::rehash(&mut data);
    assert!(
        Inventory::from_json(&data)
            .unwrap()
            .verify_module(&module)
            .is_err()
    );
}

#[test]
fn changing_a_resolved_entity_cannot_redirect_a_write() {
    let module = support::fixture();
    let mut data = Inventory::extract(&module).unwrap().to_json();
    data["actions"][0]["operations"][0]["entity"] = json!("Missing");
    support::rehash(&mut data);
    assert!(Inventory::from_json(&data).is_err());
}

#[test]
fn core_rejects_successful_state_aliasing_but_accepts_distinct_bindings() {
    use behavior_engine::store::{InMemoryBackend, Store, documents::SeedEntity};
    let module = support::fixture();
    let policy = behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#,
    ).unwrap();
    let seed = ["i1", "i2"]
        .into_iter()
        .map(|id| SeedEntity {
            entity: "Item".into(),
            value: json!({"id":id,"phase":"Idle","note":id}),
        })
        .collect();
    let genesis = behavior_engine::store::store::genesis_v2_for(&module, policy, seed).unwrap();
    let store = Store::create(InMemoryBackend::default(), &module, genesis).unwrap();
    for (right, allowed) in [("i2", true), ("i1", false)] {
        let document = json!({"format":"behavior.invocation.v1","capability":"copy",
            "bindings":{"left":{"entity":"Item","id":"i1"},"right":{"entity":"Item","id":right}},
            "input":{},"context":{}});
        let (requested, errors) =
            behavior_engine::invocation::RequestedInvocation::decode(&document.to_string())
                .unwrap();
        let requested = requested.unwrap_or_else(|| panic!("{errors:?}"));
        let result = store.invoke(&module, &requested, "2026-10-18T00:00:00Z", None, None);
        if allowed {
            assert!(result.unwrap().bundle.is_some());
        } else {
            assert!(format!("{result:?}").contains("STATE_ALIAS_NOT_ALLOWED"));
        }
    }
}
