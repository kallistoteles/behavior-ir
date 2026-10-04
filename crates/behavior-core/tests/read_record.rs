#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The read record and the read response (feature 010, research R6/R7): the record identity binds
//! every other field; the response carries only the declared result and that identity.

use behavior_core::read::{ReadRecord, record_id};
use serde_json::{Value, json};

fn record(result: &str) -> Value {
    let mut r = json!({
        "format": "behavior.read_record.v1",
        "behavior_version": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "read": {"name": "open_total", "hash": "sha256:1111111111111111111111111111111111111111111111111111111111111111", "declared": true},
        "data_version": "store:abc@3",
        "state": {"customer": {"id": "c1", "name": "Ada", "credit_limit": 100}},
        "input": {},
        "context": {},
        "result": result,
        "derived": [],
        "observed": [["customer", "id"]],
        "facts": {"fields": [{"entity": "Order", "id": "o1", "field": "amount", "value": 5}]},
    });
    if result == "VALUE" {
        r["value"] = json!(5);
    } else {
        r["reasons"] = json!([{"code": "EVALUATION_ERROR", "message": "division by zero"}]);
    }
    let id = record_id(&r);
    r["record_id"] = json!(id);
    r
}

#[test]
fn a_record_identity_is_checked_on_parse() {
    let r = record("VALUE");
    assert!(r["record_id"].as_str().unwrap().starts_with("read:sha256:"));
    let parsed = ReadRecord::from_json(&r.to_string()).unwrap();
    assert_eq!(parsed.record_id(), r["record_id"].as_str().unwrap());
    assert_eq!(parsed.result(), "VALUE");
    let mut forged = r.clone();
    forged["record_id"] =
        json!("read:sha256:2222222222222222222222222222222222222222222222222222222222222222");
    assert!(ReadRecord::from_json(&forged.to_string()).is_err());
}

#[test]
fn the_identity_changes_with_every_other_field() {
    let r = record("VALUE");
    let id = r["record_id"].clone();
    let mutations: Vec<(&str, Value)> = vec![
        ("behavior_version", json!("sha256:9")),
        (
            "read",
            json!({"name": "open_total", "hash": "sha256:9", "declared": true}),
        ),
        ("data_version", json!("store:abc@4")),
        ("state", json!({"customer": {"id": "c2"}})),
        ("input", json!({"x": 1})),
        ("context", json!({"actor": "a"})),
        ("value", json!(6)),
        ("derived", json!([{"name": "exposure", "value": 5}])),
        ("observed", json!([])),
        ("facts", json!({})),
    ];
    for (key, v) in mutations {
        let mut m = r.clone();
        m[key] = v;
        let new_id = record_id(&m);
        assert_ne!(json!(new_id), id, "{key}");
        // The stored identity no longer matches: parsing refuses the record.
        assert!(ReadRecord::from_json(&m.to_string()).is_err(), "{key}");
    }
    // The identity is computed over everything but itself.
    let mut without = r.clone();
    without.as_object_mut().unwrap().remove("record_id");
    assert_eq!(json!(record_id(&without)), id);
}

#[test]
fn a_response_holds_only_the_declared_result_and_the_record_identity() {
    let ok = ReadRecord::from_json(&record("VALUE").to_string()).unwrap();
    let response: Value = serde_json::from_str(&ok.response().to_json_string()).unwrap();
    let keys: Vec<&str> = response
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["record_id", "result", "value"]);
    assert_eq!(response["value"], 5);
    assert_eq!(response["record_id"], ok.record_id());

    let err = ReadRecord::from_json(&record("EVALUATION_ERROR").to_string()).unwrap();
    let response: Value = serde_json::from_str(&err.response().to_json_string()).unwrap();
    let keys: Vec<&str> = response
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["reasons", "record_id", "result"]);
}

#[test]
fn a_record_serializes_canonically() {
    let r = ReadRecord::from_json(&record("VALUE").to_string()).unwrap();
    let text = r.to_json_string();
    let again = ReadRecord::from_json(&text).unwrap();
    assert_eq!(again.to_json_string(), text);
    assert!(!text.contains(' '), "canonical JSON is compact: {text}");
}
