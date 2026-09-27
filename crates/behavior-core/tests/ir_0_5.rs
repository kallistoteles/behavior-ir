#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Wire IR 0.5 (feature 006): lifecycle forms need 0.5, 0.4 stays valid without them, and every
//! module without a 006 form keeps its identity and item hashes (SC-006).

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::{Value, json};

fn accounts() -> Value {
    common::json(&common::fixtures().join("wire/valid/accounts.json"))
}

fn codes(doc: &Value) -> Vec<String> {
    admission_report(&doc.to_string())
        .errors
        .iter()
        .map(|e| e.code.clone())
        .collect()
}

#[test]
fn modules_without_006_forms_keep_identity_and_item_hashes() {
    let frozen = common::json(&common::fixtures().join("frozen_versions_006.json"));
    let mut failures = Vec::new();
    for (file, expected) in frozen["modules"].as_object().unwrap() {
        let r = admission_report(&common::read(&common::fixtures().join(file)));
        if r.behavior_version.as_deref() != expected["behavior_version"].as_str() {
            failures.push(format!("{file}: behavior version {:?}", r.behavior_version));
        }
        let items: Value = serde_json::to_value(&r.items).unwrap();
        if items != expected["items"] {
            failures.push(format!("{file}: item hashes changed"));
        }
        let m = admit(&common::read(&common::fixtures().join(file))).unwrap();
        let v: Value = serde_json::from_str(&to_wire_json(&m)).unwrap();
        if v["ir_version"] != "0.4" {
            failures.push(format!("{file}: serialized as {}", v["ir_version"]));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn accounts_is_admitted_and_round_trips_as_0_5() {
    let text = common::read(&common::fixtures().join("wire/valid/accounts.json"));
    let r = admission_report(&text);
    assert!(r.ok, "{:?}", r.errors);
    let out = to_wire_json(&admit(&text).unwrap());
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ir_version"], "0.5");
    assert_eq!(admission_report(&out).behavior_version, r.behavior_version);
    // Synthesized reference constraints are not serialized: the `ref` type carries them.
    assert_eq!(v["constraints"].as_array().unwrap().len(), 1);
}

#[test]
fn each_006_form_needs_0_5() {
    let l = json!({"file": "t.py", "line": 1});
    let base = || {
        json!({
            "ir_version": "0.4", "enums": [], "nominals": [], "derived": [], "invariants": [],
            "constraints": [],
            "entities": [
                {"name": "C", "loc": l, "fields": []},
                {"name": "A", "loc": l, "fields": [{"name": "c", "type": {"t": "id", "entity": "C"}, "loc": l}]},
            ],
            "actions": [{"name": "act", "loc": l, "preconditions": [], "effects": [], "postconditions": [],
                         "params": [{"name": "a", "type": {"t": "entity", "name": "A"}, "role": "state"},
                                    {"name": "i", "type": {"t": "id", "entity": "A"}, "role": "input"}]}],
        })
    };
    let field = json!({"op": "field", "param": "a", "field": "c", "loc": l});
    let mut forms: Vec<(&str, Value)> = Vec::new();
    let mut d = base();
    d["entities"][1]["fields"][0]["type"] = json!({"t": "ref", "entity": "C"});
    forms.push(("ref", d));
    let mut d = base();
    d["actions"][0]["effects"] = json!([{"create": "A", "loc": l,
        "id": {"op": "param", "param": "i", "loc": l}, "fields": {"c": field}}]);
    forms.push(("create", d));
    let mut d = base();
    d["actions"][0]["effects"] = json!([{"remove": "a", "loc": l}]);
    forms.push(("remove", d));
    for op in ["exists", "referenced"] {
        let mut d = base();
        d["actions"][0]["preconditions"] =
            json!([{"loc": l, "expr": {"op": op, "args": [field.clone()], "loc": l}}]);
        forms.push((op, d));
    }
    for (name, doc) in forms {
        let r = admission_report(&doc.to_string());
        assert_eq!(codes(&doc), ["UNSUPPORTED_IR_VERSION"], "{name} in 0.4");
        assert!(
            r.errors[0].message.contains(name),
            "{name}: {}",
            r.errors[0].message
        );
        let mut doc5 = doc.clone();
        doc5["ir_version"] = json!("0.5");
        let r5 = admission_report(&doc5.to_string());
        assert!(r5.ok, "{name} in 0.5: {:?}", r5.errors);
        let out = to_wire_json(&admit(&doc5.to_string()).unwrap());
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ir_version"], "0.5", "{name}");
    }
    // Without any 006 form, 0.5 is accepted and serializes back as 0.4.
    let mut plain = base();
    plain["ir_version"] = json!("0.5");
    let m = admit(&plain.to_string()).unwrap();
    let v: Value = serde_json::from_str(&to_wire_json(&m)).unwrap();
    assert_eq!(v["ir_version"], "0.4");
    let mut v4 = base();
    v4["ir_version"] = json!("0.4");
    assert_eq!(
        admission_report(&plain.to_string()).behavior_version,
        admission_report(&v4.to_string()).behavior_version
    );
}

#[test]
fn the_ref_flag_and_new_nodes_change_hashes_and_renames_keep_them() {
    let base = accounts();
    let version = |d: &Value| admission_report(&d.to_string()).behavior_version.unwrap();
    let items = |d: &Value| admission_report(&d.to_string()).items;
    let v0 = version(&base);

    // `Ref<Customer>` versus a plain `Id<Customer>`: different entity hash.
    let mut plain = base.clone();
    plain["entities"][1]["fields"][0]["type"] = json!({"t": "id", "entity": "Customer"});
    assert_ne!(version(&plain), v0);
    assert_ne!(
        items(&plain)["entity:Account"],
        items(&base)["entity:Account"]
    );

    // `exists` versus `referenced` on the same argument: different expression hashes.
    let mut swapped = base.clone();
    let check = swapped["actions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["name"] == "check_exists")
        .unwrap();
    check["preconditions"][0]["expr"]["op"] = json!("referenced");
    assert_ne!(
        items(&swapped)["action:check_exists"],
        items(&base)["action:check_exists"]
    );

    // A creation versus a removal-free action: the lifecycle effect is hashed.
    let mut no_post = base.clone();
    let close = no_post["actions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["name"] == "close_account")
        .unwrap();
    close["effects"] = json!([]);
    assert_ne!(
        items(&no_post)["action:close_account"],
        items(&base)["action:close_account"]
    );

    // Renaming an action keeps every item hash (names are outside item hashes).
    let mut renamed = base.clone();
    for a in renamed["actions"].as_array_mut().unwrap() {
        if a["name"] == "deposit" {
            a["name"] = json!("pay_in");
        }
    }
    assert_eq!(
        items(&renamed)["action:pay_in"],
        items(&base)["action:deposit"]
    );
    assert_eq!(
        items(&renamed)["entity:Account"],
        items(&base)["entity:Account"]
    );
}

#[test]
fn ref_is_a_field_type_only() {
    let mut d = accounts();
    let open = d["actions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["name"] == "register_customer")
        .unwrap();
    open["params"][0]["type"] = json!({"t": "ref", "entity": "Customer"});
    assert_eq!(codes(&d), ["TYPE_MISMATCH"]);
}
