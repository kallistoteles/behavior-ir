#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Entity constraints (wire IR 0.2) and the stability of feature 001's identities.

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::{Value, json};

fn constraints() -> Value {
    common::json(&common::fixtures().join("wire/valid/constraints.json"))
}

#[test]
fn constraints_are_admitted_and_listed() {
    let r = admission_report(&constraints().to_string());
    assert!(r.ok, "{:?}", r.errors);
    assert!(r.items.contains_key("constraint:non_negative_limit"));
    assert!(r.items.contains_key("constraint:non_negative_balance"));
}

#[test]
fn renaming_a_constraint_keeps_its_item_hash() {
    let doc = constraints();
    let mut renamed = doc.clone();
    renamed["constraints"][0]["name"] = json!("limit_not_negative");
    let before = admission_report(&doc.to_string());
    let after = admission_report(&renamed.to_string());
    assert_eq!(
        before.items["constraint:non_negative_limit"],
        after.items["constraint:limit_not_negative"]
    );
    assert_ne!(before.behavior_version, after.behavior_version);
}

#[test]
fn feature_001_identities_are_unchanged() {
    // Frozen hash vectors still hold (they are asserted in hash_vectors.rs as well).
    let vectors = common::json(&common::fixtures().join("hash_vectors.json"));
    for v in vectors["vectors"].as_array().unwrap() {
        let r = admission_report(&v["wire"].to_string());
        assert_eq!(
            json!(r.behavior_version),
            v["expect"]["behavior_version"],
            "{}",
            v["name"]
        );
    }
    let versions = common::json(&common::fixtures().join("versions.json"));
    for (name, file) in [
        ("invoice", "wire/python/invoice.json"),
        ("project_margin", "wire/python/project_margin.json"),
    ] {
        let r = admission_report(&common::read(&common::fixtures().join(file)));
        assert_eq!(
            json!(r.behavior_version),
            versions[name]["behavior_version"],
            "{name}"
        );
    }
}

#[test]
fn serialization_writes_0_2_only_with_constraints() {
    let m = admit(&constraints().to_string()).unwrap();
    let text = to_wire_json(&m);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["ir_version"], "0.2");
    assert_eq!(v["constraints"].as_array().unwrap().len(), 2);
    let again = admission_report(&text);
    assert_eq!(
        again.behavior_version,
        admission_report(&constraints().to_string()).behavior_version
    );

    let inv = common::read(&common::fixtures().join("wire/valid/invoice.json"));
    let v: Value = serde_json::from_str(&to_wire_json(&admit(&inv).unwrap())).unwrap();
    assert_eq!(v["ir_version"], "0.1");
    assert!(v.get("constraints").is_none());
}
