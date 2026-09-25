#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Canonical serialization of admitted modules (research R17): round trip and idempotence.

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::Value;

fn wires() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> =
        common::files(&common::fixtures().join("wire/valid"), ".json")
            .into_iter()
            .map(|p| (p.display().to_string(), common::read(&p)))
            .collect();
    let vectors = common::json(&common::fixtures().join("hash_vectors.json"));
    for v in vectors["vectors"].as_array().unwrap() {
        out.push((
            v["name"].as_str().unwrap().to_string(),
            v["wire"].to_string(),
        ));
    }
    out
}

#[test]
fn round_trip_keeps_identity() {
    for (name, wire) in wires() {
        let first = admission_report(&wire);
        let module = admit(&wire).unwrap();
        let text = to_wire_json(&module);
        let second = admission_report(&text);
        assert!(
            second.ok,
            "{name}: serialized form not admitted: {:?}",
            second.errors
        );
        assert_eq!(first.behavior_version, second.behavior_version, "{name}");
        assert_eq!(first.items, second.items, "{name}");
        assert_eq!(first.evaluation_order, second.evaluation_order, "{name}");
    }
}

#[test]
fn serialization_is_idempotent_and_canonical() {
    for (name, wire) in wires() {
        let once = to_wire_json(&admit(&wire).unwrap());
        let twice = to_wire_json(&admit(&once).unwrap());
        assert_eq!(once, twice, "{name}");
        let v: Value = serde_json::from_str(&once).unwrap();
        assert_eq!(
            behavior_core::canonical::to_canonical_string(&v).unwrap(),
            once,
            "{name}"
        );
    }
}

#[test]
fn serialized_form_is_explicit_and_sorted() {
    let wire = common::read(&common::fixtures().join("wire/valid/invoice.json"));
    let v: Value = serde_json::from_str(&to_wire_json(&admit(&wire).unwrap())).unwrap();
    let names: Vec<&str> = v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["apply_discount", "approve_invoice"]);
    let approve = &v["actions"][1];
    // approved_by := actor.id is written with the explicit conversion.
    assert_eq!(approve["effects"][1]["value"]["op"], "some");
    // Source locations survive serialization.
    assert_eq!(approve["loc"]["file"], "invoice.py");
    assert_eq!(v["entities"][0]["fields"][0]["loc"]["file"], "invoice.py");
}
