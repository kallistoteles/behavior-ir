#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Observed reads (feature 005, research R7): the entity fields an evaluation actually read,
//! named by the action's parameters, including reads inside derived values (also on memo hits),
//! rules, invariants and constraints, and excluding operands skipped by short-circuiting.

mod common;

use std::collections::BTreeSet;

use behavior_core::{admit, evaluate, evaluate_observed};
use serde_json::json;

fn ledger() -> behavior_core::semantic::module::Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/ledger.json"),
    ))
    .unwrap()
}

fn transfer(active: bool) -> String {
    json!({"action": "transfer", "data_version": "1", "context": {},
           "input": {"amount": "10.00"},
           "state": {"from_": {"id": "a1", "active": active, "balance": "100.00"},
                     "to": {"id": "a2", "active": true, "balance": "5.00"}}})
    .to_string()
}

fn set(xs: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    xs.iter()
        .map(|(p, f)| (p.to_string(), f.to_string()))
        .collect()
}

#[test]
fn short_circuit_skips_reads_and_derived_reads_are_attributed() {
    let m = ledger();
    // Denied: `from_.active` is false, so `available(from_)` (reading `balance`) is never
    // evaluated in the precondition. The incoming constraint still reads both balances.
    let (rec, observed) = evaluate_observed(&m, &transfer(false));
    assert_eq!(rec.result(), "DENY");
    assert!(
        observed.contains(&("from_".into(), "active".into())),
        "{observed:?}"
    );
    assert!(
        !observed.contains(&("to".into(), "active".into())),
        "{observed:?}"
    );
    // Allowed: the derived value's read of `from_.balance` is attributed to `from_`, and the
    // effects read both balances.
    let (rec, observed) = evaluate_observed(&m, &transfer(true));
    assert_eq!(rec.result(), "ALLOW");
    assert_eq!(
        observed,
        set(&[("from_", "active"), ("from_", "balance"), ("to", "balance")])
    );
}

#[test]
fn derived_reads_count_without_rules_touching_the_field() {
    // Only through the derived value is `balance` read in the precondition; the constraint on
    // `balance` also reads it. Remove the constraint to isolate the derived read.
    let mut doc: serde_json::Value = serde_json::from_str(&common::read(
        &common::fixtures().join("wire/valid/ledger.json"),
    ))
    .unwrap();
    doc["constraints"] = json!([]);
    // Keep only the precondition that reads through `available`, and no effects.
    doc["actions"][0]["effects"] = json!([]);
    let m = admit(&doc.to_string()).unwrap();
    let (_, observed) = evaluate_observed(&m, &transfer(true));
    assert_eq!(observed, set(&[("from_", "active"), ("from_", "balance")]));
    let (_, observed) = evaluate_observed(&m, &transfer(false));
    assert_eq!(observed, set(&[("from_", "active")]));
}

#[test]
fn records_are_byte_identical_with_and_without_the_collector() {
    let m = ledger();
    for active in [true, false] {
        let plain = evaluate(&m, &transfer(active)).to_json_string();
        let (observed, _) = evaluate_observed(&m, &transfer(active));
        assert_eq!(plain, observed.to_json_string());
    }
}
