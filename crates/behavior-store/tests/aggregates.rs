#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US2 (feature 007) in the store: every relational operator, and module invariants at genesis
//! and on resulting states.

mod common;

use behavior_store::documents::EvidencePolicy;
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use common::*;
use serde_json::json;

#[test]
fn every_operator_evaluates_in_the_store() {
    let mut seed = orders_seed();
    seed.push(order_seed("o3", "c1", "20.00", "open"));
    seed.push(order_seed("o4", "c1", "0.01", "open"));
    let s = orders_store_with(InMemoryBackend::new(), seed.clone());
    let e = run_orders(&s, "check_orders", &[("customer", "c1")], json!({}));
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    // A blocked order makes `not any(..blocked..)` fail.
    seed.push(order_seed("o5", "c1", "1.00", "blocked"));
    let s = orders_store_with(InMemoryBackend::new(), seed);
    let e = run_orders(&s, "check_orders", &[("customer", "c1")], json!({}));
    assert_eq!(e.record["result"], "DENY", "{}", e.record);
}

#[test]
fn hiring_respects_the_module_invariant() {
    let mut s = orders_store();
    let dup = run_orders(
        &s,
        "hire_unchecked",
        &[],
        json!({"employee_id": "e9", "number": "N1"}),
    );
    assert_eq!(dup.record["result"], "DENY", "{}", dup.record);
    assert_eq!(dup.record["reasons"][0]["code"], "INVARIANT_VIOLATED");
    let guarded = run_orders(
        &s,
        "hire",
        &[],
        json!({"employee_id": "e9", "number": "N1"}),
    );
    assert_eq!(guarded.record["reasons"][0]["code"], "PRECONDITION_FAILED");
    let ok = run_orders(
        &s,
        "hire",
        &[],
        json!({"employee_id": "e9", "number": "N9"}),
    );
    assert_eq!(ok.record["result"], "ALLOW", "{}", ok.record);
    commit_orders(&mut s, &ok);
    let renumber = run_orders(
        &s,
        "renumber",
        &[("employee", "e9")],
        json!({"number": "N7"}),
    );
    assert_eq!(renumber.record["result"], "ALLOW", "{}", renumber.record);
    let clash = run_orders(
        &s,
        "renumber",
        &[("employee", "e9")],
        json!({"number": "N1"}),
    );
    assert_eq!(clash.record["result"], "DENY");
}

#[test]
fn a_genesis_that_breaks_a_module_invariant_is_refused() {
    let m = orders();
    let mut seed = orders_seed();
    seed.push(employee_seed("e2", "N1"));
    let err = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .err()
    .unwrap();
    assert_eq!(err.code(), "GENESIS_INVALID", "{err}");
    assert!(
        err.to_string().contains("personnel_numbers_unique"),
        "{err}"
    );
}
