#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Module-level invariants (feature 007): closed state expressions, checked on S' of every
//! transition that can affect them (a sound dependency analysis), `unique` by a delta rule.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admission_report, admit, evaluate, replay};
use proptest::prelude::*;
use serde_json::{Value, json};

fn orders() -> Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/orders.json"),
    ))
    .unwrap()
}

fn eval(m: &Module, request: &Value) -> Value {
    let text = evaluate(m, &request.to_string()).to_json_string();
    let r = replay(m, &text);
    assert!(r.matches, "{:?}", r.diff);
    serde_json::from_str(&text).unwrap()
}

fn employees(members: Value) -> Value {
    json!({"universe": [{"entity": "Employee", "members": members},
                        {"entity": "Order", "members": []}],
           "identities": [{"entity": "Employee", "id": "e9", "used": false}]})
}

fn global_steps(rec: &Value) -> Vec<Value> {
    rec["trace"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["phase"] == "invariant_global")
        .cloned()
        .collect()
}

fn l() -> Value {
    json!({"file": "t.py", "line": 1})
}

fn personnel_unique_expr() -> Value {
    json!({"op": "unique", "args": [{"op": "select", "entity": "Employee", "loc": l()}],
           "param": "e", "loc": l(),
           "body": {"op": "field", "param": "e", "field": "personnel_number", "loc": l()}})
}

fn orders_with_derived_invariant_chain(chain_len: usize) -> Module {
    let mut w = common::json(&common::fixtures().join("wire/valid/orders.json"));
    let derived = w["derived"].as_array_mut().unwrap();
    derived.push(
        json!({"name": "employee_numbers_unique_0", "kind": "derived", "loc": l(),
                        "params": [], "body": personnel_unique_expr()}),
    );
    for i in 1..chain_len {
        derived.push(json!({"name": format!("employee_numbers_unique_{i}"), "kind": "derived",
                            "loc": l(), "params": [],
                            "body": {"op": "derived", "name": format!("employee_numbers_unique_{}", i - 1),
                                     "args": [], "loc": l()}}));
    }
    w["invariants"][0]["body"] = json!({"op": "derived",
                                        "name": format!("employee_numbers_unique_{}", chain_len - 1),
                                        "args": [], "loc": l()});
    admit(&w.to_string()).unwrap()
}

#[test]
fn a_module_invariant_is_a_closed_state_expression() {
    let text =
        common::read(&common::fixtures().join("wire/invalid/module_invariant_reads_input.json"));
    let codes: Vec<String> = admission_report(&text)
        .errors
        .iter()
        .map(|e| e.code.clone())
        .collect();
    assert_eq!(codes, ["UNKNOWN_PARAM"]);
    let m = orders();
    assert_eq!(m.global_invariants().len(), 1);
    let sig = m.global_invariants()["personnel_numbers_unique"].signature();
    assert_eq!(sig.types.len(), 1);
    assert_eq!(
        sig.types["Employee"]
            .as_ref()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        ["personnel_number"]
    );
}

#[test]
fn derived_helper_invariant_signature_triggers_related_transitions() {
    let m = orders_with_derived_invariant_chain(2);
    let sig = m.global_invariants()["personnel_numbers_unique"].signature();
    assert_eq!(
        sig.types["Employee"]
            .as_ref()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        ["personnel_number"]
    );

    let create = json!({"action": "hire_unchecked", "data_version": "1", "state": {}, "context": {},
        "input": {"employee_id": "e9", "number": "N1"},
        "facts": employees(json!([{"id": "e1", "personnel_number": "N1"}]))});
    let rec = eval(&m, &create);
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert_eq!(rec["reasons"][0]["code"], "INVARIANT_VIOLATED");
    assert_eq!(global_steps(&rec).len(), 1);

    let update = json!({"action": "renumber", "data_version": "1", "context": {},
        "state": {"employee": {"id": "e1", "personnel_number": "N1"}}, "input": {"number": "N2"},
        "facts": {"universe": [{"entity": "Employee", "members": [
            {"id": "e1", "personnel_number": "N1"}, {"id": "e2", "personnel_number": "N2"}]}]}});
    let rec = eval(&m, &update);
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert_eq!(global_steps(&rec).len(), 1);

    let unrelated = json!({"action": "close_customer", "data_version": "1", "input": {}, "context": {},
        "state": {"customer": {"id": "c1", "name": "Ada", "credit_limit": "1.00", "region": "north"}},
        "facts": {"universe": [{"entity": "Order", "members": []}]}});
    let rec = eval(&m, &unrelated);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    assert!(global_steps(&rec).is_empty(), "{rec}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn derived_invariant_signatures_expand_chains(chain_len in 1usize..8) {
        let m = orders_with_derived_invariant_chain(chain_len);
        let sig = m.global_invariants()["personnel_numbers_unique"].signature();
        prop_assert!(sig.types.contains_key("Employee"));

        let request = json!({"action": "hire_unchecked", "data_version": "1", "state": {}, "context": {},
            "input": {"employee_id": "e9", "number": "N1"},
            "facts": employees(json!([{"id": "e1", "personnel_number": "N1"}]))});
        let rec = eval(&m, &request);
        prop_assert_eq!(rec["result"].clone(), json!("DENY"));
    }
}

#[test]
fn a_duplicate_key_on_s_prime_is_invariant_violated() {
    let m = orders();
    let request = json!({"action": "hire_unchecked", "data_version": "1", "state": {}, "context": {},
        "input": {"employee_id": "e9", "number": "N1"},
        "facts": employees(json!([{"id": "e1", "personnel_number": "N1"}]))});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert_eq!(rec["reasons"][0]["code"], "INVARIANT_VIOLATED");
    let steps = global_steps(&rec);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["outcome"], false);
}

#[test]
fn uniqueness_is_decided_by_the_delta_rule() {
    let m = orders();
    // Many employees, one new number: the record holds the derived "same key" instance, not the
    // full membership, and reads the key of no other employee.
    let many: Vec<Value> = (0..50)
        .map(|i| json!({"id": format!("e{i:02}"), "personnel_number": format!("N{i}")}))
        .collect();
    let request = json!({"action": "hire_unchecked", "data_version": "1", "state": {}, "context": {},
        "input": {"employee_id": "e9", "number": "N999"}, "facts": employees(json!(many))});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    let queries = rec["facts"]["queries"].as_array().unwrap();
    assert!(
        queries
            .iter()
            .all(|q| q["members"].as_array().unwrap().is_empty()),
        "{queries:?}"
    );
    assert!(rec["facts"].get("fields").is_none(), "{rec}");
}

#[test]
fn unrelated_transitions_skip_the_invariant_and_related_ones_check_it() {
    let m = orders();
    // `deposit`-like field changes of another type: close_customer removes a Customer, which the
    // invariant does not query.
    let request = json!({"action": "close_customer", "data_version": "1", "input": {}, "context": {},
        "state": {"customer": {"id": "c1", "name": "Ada", "credit_limit": "1.00", "region": "north"}},
        "facts": {"universe": [{"entity": "Order", "members": []}]}});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    assert!(global_steps(&rec).is_empty());
    // `renumber` changes the queried field: checked.
    let request = json!({"action": "renumber", "data_version": "1", "context": {},
        "state": {"employee": {"id": "e1", "personnel_number": "N1"}}, "input": {"number": "N2"},
        "facts": {"universe": [{"entity": "Employee", "members": [
            {"id": "e1", "personnel_number": "N1"}, {"id": "e2", "personnel_number": "N2"}]}]}});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert_eq!(global_steps(&rec).len(), 1);
}

/// The orders module with the uniqueness invariant rewritten by `wrap` (code review: the delta
/// rule is sound only for a `unique` known to hold on S).
fn orders_with(wrap: impl Fn(Value) -> Value) -> Module {
    let mut w: Value = serde_json::from_str(&common::read(
        &common::fixtures().join("wire/valid/orders.json"),
    ))
    .unwrap();
    let inv = w["invariants"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|i| i["name"] == "personnel_numbers_unique")
        .unwrap();
    inv["body"] = wrap(inv["body"].clone());
    admit(&w.to_string()).unwrap()
}

fn duplicates(extra: Value) -> Value {
    let mut members = vec![
        json!({"id": "e1", "personnel_number": "N1"}),
        json!({"id": "e2", "personnel_number": "N1"}),
    ];
    if !extra.is_null() {
        members.push(extra);
    }
    json!({"universe": [{"entity": "Employee", "members": members},
                        {"entity": "Order", "members": []}],
           "identities": [{"entity": "Employee", "id": "e9", "used": false}]})
}

#[test]
fn a_unique_under_negation_is_evaluated_in_full() {
    let loc = json!({"file": "orders.py", "line": 34});
    let m = orders_with(|u| json!({"op": "not", "args": [u], "loc": loc}));
    // S has a duplicate (so `not unique` holds); renumbering an unrelated employee keeps it.
    let request = json!({"action": "renumber", "data_version": "1", "context": {},
        "state": {"employee": {"id": "e3", "personnel_number": "N3"}}, "input": {"number": "N4"},
        "facts": duplicates(json!({"id": "e3", "personnel_number": "N3"}))});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
}

#[test]
fn a_unique_under_disjunction_is_evaluated_in_full() {
    let loc = json!({"file": "orders.py", "line": 34});
    let few = json!({"op": "lt", "loc": loc, "args": [
        {"op": "count", "loc": loc, "args": [{"op": "select", "entity": "Employee", "loc": loc}]},
        {"op": "lit", "type": {"t": "int"}, "value": 3, "loc": loc}]});
    let m = orders_with(|u| json!({"op": "or", "args": [few.clone(), u], "loc": loc}));
    // S holds through the count (2 < 3) despite the duplicate; a third employee breaks it.
    let request = json!({"action": "hire_unchecked", "data_version": "1", "state": {}, "context": {},
        "input": {"employee_id": "e9", "number": "N9"}, "facts": duplicates(Value::Null)});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert_eq!(rec["reasons"][0]["code"], "INVARIANT_VIOLATED");
}
