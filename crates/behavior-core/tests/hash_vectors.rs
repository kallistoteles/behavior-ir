#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Frozen hash vectors. Run with `BLESS_HASH_VECTORS=1` once to fill in missing expectations;
//! never re-bless existing ones (a change requires new hash tags).

mod common;

use behavior_core::admission_report;
use serde_json::{Value, json};

#[test]
fn hash_vectors_are_stable() {
    let path = common::fixtures().join("hash_vectors.json");
    let mut doc = common::json(&path);
    let bless = std::env::var("BLESS_HASH_VECTORS").is_ok();
    let mut changed = false;
    let mut failures = Vec::new();
    for v in doc["vectors"].as_array_mut().unwrap() {
        let r = admission_report(&v["wire"].to_string());
        assert!(r.ok, "{}: {:?}", v["name"], r.errors);
        let got = json!({"behavior_version": r.behavior_version, "items": r.items});
        match v.get("expect") {
            Some(expect) if *expect == got => {}
            Some(expect) => failures.push(format!(
                "{}\n  expected {expect}\n  got      {got}",
                v["name"]
            )),
            None if bless => {
                v["expect"] = got;
                changed = true;
            }
            None => failures.push(format!(
                "{}: no expectation (run with BLESS_HASH_VECTORS=1)",
                v["name"]
            )),
        }
    }
    if changed {
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    let _ = Value::Null;
}
