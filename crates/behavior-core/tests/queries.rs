#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Relational evaluation with supplied facts (feature 007): set semantics, exact aggregates,
//! canonical order for short-circuiting, observed query and field facts, `UNKNOWN_FACT`,
//! `INCONSISTENT_FACTS`, record 0.6 and replay.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admit, evaluate, replay};
use proptest::prelude::*;
use serde_json::{Value, json};

fn orders() -> Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/orders.json"),
    ))
    .unwrap()
}

fn eval(m: &Module, request: &Value) -> (Value, String) {
    let text = evaluate(m, &request.to_string()).to_json_string();
    (serde_json::from_str(&text).unwrap(), text)
}

fn c1() -> Value {
    json!({"id": "c1", "name": "Ada", "credit_limit": "100.00", "region": "north"})
}

fn order(id: &str, customer: &str, amount: &str, status: &str) -> Value {
    json!({"id": id, "customer": customer, "amount": amount, "status": status, "region": "north"})
}

fn request(action: &str, members: Vec<Value>) -> Value {
    json!({"action": action, "data_version": "1", "input": {}, "context": {},
           "state": {"customer": c1()},
           "facts": {"universe": [{"entity": "Order", "members": members}]}})
}

#[test]
fn records_meet_007_expectations_and_replay() {
    let m = orders();
    let exp = common::json(&common::fixtures().join("requests/007/expectations.json"));
    let mut failures = Vec::new();
    for (name, e) in exp.as_object().unwrap() {
        let request = common::read(&common::fixtures().join(format!("requests/007/{name}.json")));
        let text = evaluate(&m, &request).to_json_string();
        let rec: Value = serde_json::from_str(&text).unwrap();
        let mut out = common::subset_problems(e, &rec);
        let r = replay(&m, &text);
        if !r.matches {
            out.push(format!("replay mismatch: {:?}", r.diff));
        }
        if !out.is_empty() {
            failures.push(format!(
                "{name}:\n    {}\n    record: {rec}",
                out.join("\n    ")
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The orders module plus an action whose preconditions pin exact aggregate values.
fn with_probe(pre: Vec<Value>) -> Module {
    let mut w = common::json(&common::fixtures().join("wire/valid/orders.json"));
    w["actions"].as_array_mut().unwrap().push(json!({
        "name": "probe", "loc": {"file": "t.py", "line": 1}, "effects": [], "postconditions": [],
        "params": [{"name": "customer", "type": {"t": "entity", "name": "Customer"}, "role": "state"}],
        "preconditions": pre.into_iter().map(|e| json!({"expr": e, "loc": {"file": "t.py", "line": 1}})).collect::<Vec<_>>(),
    }));
    admit(&w.to_string()).unwrap()
}

fn l() -> Value {
    json!({"file": "t.py", "line": 1})
}

fn orders_of_c() -> Value {
    json!({"op": "where", "param": "o", "loc": l(), "args": [{"op": "select", "entity": "Order", "loc": l()}],
           "body": {"op": "eq", "loc": l(), "args": [{"op": "field", "param": "o", "field": "customer", "loc": l()},
                                                     {"op": "field", "param": "customer", "field": "id", "loc": l()}]}})
}

fn money(v: &str) -> Value {
    json!({"op": "lit", "type": {"t": "nominal", "name": "Money"}, "value": v, "loc": l()})
}

fn agg(op: &str) -> Value {
    json!({"op": op, "args": [orders_of_c()], "param": "o", "loc": l(),
           "body": {"op": "field", "param": "o", "field": "amount", "loc": l()}})
}

#[test]
fn aggregates_are_exact_and_empty_sets_have_their_values() {
    let m = with_probe(vec![
        json!({"op": "eq", "args": [agg("sum"), money("30.01")], "loc": l()}),
        json!({"op": "eq", "args": [agg("min"), {"op": "some", "args": [money("0.01")], "loc": l()}], "loc": l()}),
        json!({"op": "eq", "args": [agg("max"), {"op": "some", "args": [money("20.00")], "loc": l()}], "loc": l()}),
        json!({"op": "eq", "args": [{"op": "count", "args": [orders_of_c()], "loc": l()},
                                    {"op": "lit", "type": {"t": "int"}, "value": 3, "loc": l()}], "loc": l()}),
    ]);
    let members = vec![
        order("o1", "c1", "10.00", "open"),
        order("o2", "c1", "20.00", "open"),
        order("o3", "c1", "0.01", "closed"),
        order("o4", "c2", "50.00", "open"),
    ];
    let (rec, _) = eval(&m, &request("probe", members));
    assert_eq!(rec["result"], "ALLOW", "{rec}");

    let empty = with_probe(vec![
        json!({"op": "eq", "args": [agg("sum"), money("0")], "loc": l()}),
        json!({"op": "is_none", "args": [agg("min")], "loc": l()}),
        json!({"op": "is_none", "args": [agg("max")], "loc": l()}),
        json!({"op": "all", "args": [orders_of_c()], "param": "o", "loc": l(),
               "body": {"op": "lit", "type": {"t": "bool"}, "value": false, "loc": l()}}),
        json!({"op": "not", "loc": l(), "args": [{"op": "any", "args": [orders_of_c()], "param": "o", "loc": l(),
               "body": {"op": "lit", "type": {"t": "bool"}, "value": true, "loc": l()}}]}),
    ]);
    let (rec, _) = eval(&empty, &request("probe", vec![]));
    assert_eq!(rec["result"], "ALLOW", "{rec}");
}

#[test]
fn short_circuiting_uses_canonical_order_and_records_only_what_it_read() {
    let m = orders();
    // o1 is blocked, so `not any(..blocked..)` stops at o1: o2's status is never read.
    let members = vec![
        order("o2", "c1", "20.00", "open"),
        order("o1", "c1", "10.00", "blocked"),
    ];
    let (rec, text) = eval(&m, &request("check_orders", members.clone()));
    assert_eq!(rec["result"], "DENY", "{rec}");
    let fields = rec["facts"]["fields"].as_array().unwrap();
    assert!(
        fields
            .iter()
            .any(|f| f["id"] == "o1" && f["field"] == "status"),
        "{rec}"
    );
    assert!(
        !fields
            .iter()
            .any(|f| f["id"] == "o2" && f["field"] == "status"),
        "{rec}"
    );
    // Any permutation of the supplied universe gives the same bytes (SC-008 unit case).
    let mut reversed = members;
    reversed.reverse();
    assert_eq!(eval(&m, &request("check_orders", reversed)).1, text);
}

#[test]
fn a_missing_query_fact_is_unknown_fact() {
    let m = orders();
    let mut r = request("close_customer", vec![]);
    r["facts"] = json!({});
    let (rec, _) = eval(&m, &r);
    assert_eq!(rec["result"], "ERROR");
    assert_eq!(rec["reasons"][0]["code"], "UNKNOWN_FACT");
}

#[test]
fn inconsistent_universes_are_refused() {
    let m = orders();
    let cases = [
        json!({"universe": [{"entity": "Order", "members": [order("o1", "c1", "10.00", "open")]}],
               "existence": [{"entity": "Order", "id": "o1", "exists": false}]}),
        json!({"universe": [{"entity": "Order", "members": [order("o1", "c1", "10.00", "open"),
                                                            order("o1", "c1", "11.00", "open")]}]}),
        json!({"universe": [{"entity": "Order", "members": []}, {"entity": "Order", "members": []}]}),
        json!({"universe": [{"entity": "Order", "members": [order("o1", "c1", "-1.00", "open")]}]}),
    ];
    for facts in cases {
        let mut r = request("close_customer", vec![]);
        r["facts"] = facts.clone();
        let (rec, _) = eval(&m, &r);
        assert_eq!(rec["result"], "INVALID_INPUT", "{facts}: {rec}");
        assert_eq!(rec["reasons"][0]["code"], "INCONSISTENT_FACTS", "{facts}");
    }
}

fn with_redundant_query_fact(members: Vec<Value>) -> (Value, Value) {
    let m = orders();
    let mut request = request("close_customer", members);
    let (rec, _) = eval(&m, &request);
    let query = rec["facts"]["queries"][0].clone();
    request["facts"]["queries"] = json!([query.clone()]);
    (request, query)
}

fn contradict_query_fact(mut query: Value) -> Value {
    let members = query["members"].as_array_mut().unwrap();
    if members.is_empty() {
        members.push(json!({"id": "not_in_the_universe"}));
    } else {
        members.clear();
    }
    query
}

#[test]
fn a_complete_universe_rejects_contradictory_redundant_query_facts() {
    let m = orders();
    let members = vec![order("o1", "c1", "10.00", "open")];
    let (agreeing, query) = with_redundant_query_fact(members.clone());
    let (rec, _) = eval(&m, &agreeing);
    assert_eq!(rec["result"], "DENY", "{rec}");

    let mut contradicting = request("close_customer", members);
    contradicting["facts"]["queries"] = json!([contradict_query_fact(query)]);
    let (rec, _) = eval(&m, &contradicting);
    assert_eq!(rec["result"], "INVALID_INPUT", "{rec}");
    assert_eq!(rec["reasons"][0]["code"], "INCONSISTENT_FACTS");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn complete_universe_and_query_fact_overlap_must_agree(open_statuses in proptest::collection::vec(any::<bool>(), 0..6)) {
        let m = orders();
        let members: Vec<Value> = open_statuses
            .iter()
            .enumerate()
            .map(|(i, open)| {
                order(
                    &format!("o{i}"),
                    "c1",
                    "10.00",
                    if *open { "open" } else { "closed" },
                )
            })
            .collect();
        let (agreeing, query) = with_redundant_query_fact(members.clone());
        let (rec, _) = eval(&m, &agreeing);
        prop_assert_ne!(rec["result"].clone(), json!("INVALID_INPUT"));

        let mut contradicting = request("close_customer", members);
        contradicting["facts"]["queries"] = json!([contradict_query_fact(query)]);
        let (rec, _) = eval(&m, &contradicting);
        prop_assert_eq!(rec["result"].clone(), json!("INVALID_INPUT"));
        prop_assert_eq!(rec["reasons"][0]["code"].clone(), json!("INCONSISTENT_FACTS"));
    }
}

#[test]
fn recorded_facts_replay_and_plain_records_are_unchanged() {
    let m = orders();
    let (rec, text) = eval(
        &m,
        &request("close_customer", vec![order("o3", "c1", "7.00", "open")]),
    );
    assert_eq!(rec["record_version"], "0.6");
    // The record holds observed query facts (with hashes), not the supplied universe.
    assert!(rec["facts"].get("universe").is_none(), "{rec}");
    assert!(
        rec["facts"]["queries"][0]["instance"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(replay(&m, &text).matches);
    // An action without queries writes a 0.4 record.
    let (plain, _) = eval(
        &m,
        &json!({"action": "renumber", "data_version": "1", "context": {},
        "state": {"employee": {"id": "e1", "personnel_number": "N1"}}, "input": {"number": "N2"},
        "facts": {"universe": [{"entity": "Employee", "members": [{"id": "e1", "personnel_number": "N1"}]}]}}),
    );
    assert_eq!(plain["result"], "ALLOW", "{plain}");
}
