#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Evaluation facts (feature 006, FR-010c, FR-010f, FR-010g): existence, identity and reference
//! facts are read lazily through the request's `facts` section, only observed facts are recorded,
//! and supplied facts must form a valid snapshot.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admit, evaluate, evaluate_observed, replay};
use serde_json::{Value, json};

fn accounts() -> Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/accounts.json"),
    ))
    .unwrap()
}

fn eval(m: &Module, request: &Value) -> Value {
    serde_json::from_str(&evaluate(m, &request.to_string()).to_json_string()).unwrap()
}

fn note_request(facts: Option<Value>) -> Value {
    let mut r = json!({
        "action": "check_exists", "data_version": "1", "input": {}, "context": {},
        "state": {"note": {"id": "n1", "about": "c1", "text": "hi"}},
    });
    if let Some(f) = facts {
        r["facts"] = f;
    }
    r
}

#[test]
fn records_meet_006_expectations_and_replay() {
    let m = accounts();
    let exp = common::json(&common::fixtures().join("requests/006/expectations.json"));
    let mut failures = Vec::new();
    for (name, e) in exp.as_object().unwrap() {
        let request = common::read(&common::fixtures().join(format!("requests/006/{name}.json")));
        let text = evaluate(&m, &request).to_json_string();
        let rec: Value = serde_json::from_str(&text).unwrap();
        let mut out = common::subset_problems(e, &rec);
        if !replay(&m, &text).matches {
            out.push(format!("replay mismatch: {:?}", replay(&m, &text).diff));
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

#[test]
fn check_exists_records_exactly_one_existence_read() {
    let m = accounts();
    let facts = json!({
        "existence": [
            {"entity": "Customer", "id": "c1", "exists": true},
            {"entity": "Customer", "id": "c9", "exists": false},
        ],
        "identities": [{"entity": "Customer", "id": "c9", "used": true}],
    });
    let rec = eval(&m, &note_request(Some(facts)));
    assert_eq!(rec["result"], "ALLOW");
    assert_eq!(rec["record_version"], "0.5");
    assert_eq!(
        rec["facts"],
        json!({"existence": [{"entity": "Customer", "id": "c1", "exists": true}]})
    );
}

#[test]
fn short_circuited_exists_is_not_observed() {
    // `is_none(x) or exists(x)`: a synthesized optional-reference shape. Here: a precondition
    // that is false before `exists` is reached records no fact and needs none.
    let mut w = common::json(&common::fixtures().join("wire/valid/accounts.json"));
    let l = json!({"file": "t.py", "line": 1});
    let check = w["actions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["name"] == "check_exists")
        .unwrap();
    let exists = check["preconditions"][0]["expr"].clone();
    check["preconditions"][0]["expr"] = json!({"op": "and", "loc": l, "args": [
        {"op": "eq", "loc": l, "args": [
            {"op": "field", "param": "note", "field": "text", "loc": l},
            {"op": "lit", "type": {"t": "string"}, "value": "audit", "loc": l}]},
        exists]});
    let m = admit(&w.to_string()).unwrap();
    let rec = eval(&m, &note_request(None));
    assert_eq!(rec["result"], "DENY", "{rec}");
    assert!(rec.get("facts").is_none(), "{rec}");
    assert_eq!(rec["record_version"], "0.4");
}

#[test]
fn a_missing_fact_is_unknown_fact() {
    let m = accounts();
    let rec = eval(&m, &note_request(Some(json!({}))));
    assert_eq!(rec["result"], "ERROR");
    assert_eq!(rec["reasons"][0]["code"], "UNKNOWN_FACT");
    assert!(
        rec["reasons"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Customer c1"),
        "{rec}"
    );
}

#[test]
fn inconsistent_facts_are_refused() {
    let m = accounts();
    let edge = json!({"entity": "Account", "id": "a1", "field": "owner"});
    let cases = [
        (
            "exists without used",
            json!({"existence": [{"entity": "Customer", "id": "c1", "exists": true}],
                   "identities": [{"entity": "Customer", "id": "c1", "used": false}]}),
        ),
        (
            "incoming to a non-existing target",
            json!({"existence": [{"entity": "Customer", "id": "c1", "exists": false}],
                   "references": [{"entity": "Customer", "id": "c1", "incoming": [edge]}]}),
        ),
        (
            "a bound entity marked absent",
            json!({"existence": [{"entity": "AuditNote", "id": "n1", "exists": false}]}),
        ),
        (
            "a duplicated fact",
            json!({"existence": [{"entity": "Customer", "id": "c1", "exists": true},
                                 {"entity": "Customer", "id": "c1", "exists": true}]}),
        ),
        (
            "an incoming edge from an absent source",
            json!({"existence": [{"entity": "Account", "id": "a1", "exists": false}],
                   "references": [{"entity": "Customer", "id": "c1", "incoming": [edge]}]}),
        ),
        (
            "an edge over a plain identity field",
            json!({"references": [{"entity": "Customer", "id": "c1",
                   "incoming": [{"entity": "AuditNote", "id": "n2", "field": "about"}]}]}),
        ),
    ];
    for (what, facts) in cases {
        let rec = eval(&m, &note_request(Some(facts)));
        assert_eq!(rec["result"], "INVALID_INPUT", "{what}: {rec}");
        assert_eq!(rec["reasons"][0]["code"], "INCONSISTENT_FACTS", "{what}");
    }
}

#[test]
fn a_bound_referrer_must_match_the_incoming_fact() {
    let m = accounts();
    // `switch_and_remove` binds account a1 with owner c1; an incoming(c1) fact without a1 lies.
    let request = json!({
        "action": "switch_and_remove", "data_version": "1", "input": {}, "context": {},
        "state": {"account": {"id": "a1", "owner": "c1", "balance": "0.00"},
                  "old": {"id": "c1", "name": "Ada"}, "new": {"id": "c2", "name": "Bo"}},
        "facts": {"references": [{"entity": "Customer", "id": "c1", "incoming": []}]},
    });
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "INVALID_INPUT", "{rec}");
    assert_eq!(rec["reasons"][0]["code"], "INCONSISTENT_FACTS");
}

#[test]
fn referenced_is_the_projection_of_incoming() {
    let m = accounts();
    let request = |incoming: Value| {
        json!({
            "action": "remove_customer", "data_version": "1", "input": {}, "context": {},
            "state": {"customer": {"id": "c1", "name": "Ada"}},
            "facts": {"references": [{"entity": "Customer", "id": "c1", "incoming": incoming}]},
        })
    };
    let referenced = eval(
        &m,
        &request(json!([{"entity": "Account", "id": "a1", "field": "owner"}])),
    );
    assert_eq!(referenced["result"], "DENY");
    let free = eval(&m, &request(json!([])));
    assert_eq!(free["result"], "ALLOW", "{free}");
    // The removal's integrity check reuses the observed reference fact: one entry.
    assert_eq!(
        free["facts"],
        json!({"references": [{"entity": "Customer", "id": "c1", "incoming": []}]})
    );
}

#[test]
fn records_without_facts_are_unchanged() {
    // Every pre-006 golden record replays byte for byte (the golden replay tests cover them);
    // here: a 0.5 module's action that observes no fact writes a 0.4 record without new keys.
    let m = accounts();
    let request = json!({
        "action": "deposit", "data_version": "1", "context": {},
        "state": {"account": {"id": "a1", "owner": "c1", "balance": "0.00"}},
        "input": {"amount": "5.00"},
    });
    let (rec, reads) = evaluate_observed(&m, &request.to_string());
    let rec = rec.as_json().clone();
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    assert_eq!(rec["record_version"], "0.4");
    assert!(rec.get("facts").is_none() && rec.get("lifecycle").is_none());
    assert!(reads.contains(&("account".to_string(), "balance".to_string())));
}

#[test]
fn a_bound_entitys_reference_target_cannot_be_absent() {
    // The store keeps referential integrity (and the verifier assumes it): a snapshot where bound
    // account a1 refers to a customer marked absent or never used cannot exist.
    let m = accounts();
    for fact in [
        json!({"existence": [{"entity": "Customer", "id": "c1", "exists": false}]}),
        json!({"identities": [{"entity": "Customer", "id": "c1", "used": false}]}),
    ] {
        let request = json!({
            "action": "deposit", "data_version": "1", "context": {},
            "state": {"account": {"id": "a1", "owner": "c1", "balance": "0.00"}},
            "input": {"amount": "5.00"},
            "facts": fact,
        });
        let rec = eval(&m, &request);
        assert_eq!(rec["result"], "INVALID_INPUT", "{rec}");
        assert_eq!(rec["reasons"][0]["code"], "INCONSISTENT_FACTS");
    }
}
