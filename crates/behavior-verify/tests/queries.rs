#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 007: verification of relational queries (SC-005). Proofs come from symbolic
//! summaries with exact deltas; counterexamples are concretized with explicit unknown members and
//! confirmed by plain evaluation with their query and field facts; what the model cannot decide
//! (a changed capture, an extremum after a removal) is inconclusive, never proven.

mod common;

use serde_json::{Value, json};

#[test]
fn query_outcomes_match_expectations() {
    let (a, expected) = common::run("queries");
    let got = common::outcomes(&a);
    let want = common::expected_outcomes(&expected);
    assert_eq!(
        Value::Array(got.clone()),
        Value::Array(want),
        "\n{}",
        serde_json::to_string_pretty(&got).unwrap()
    );
}

#[test]
fn counterexamples_are_confirmed_with_their_query_facts() {
    let (a, _, module) = common::run_with_module("queries");
    let findings: Vec<Value> = a
        .findings()
        .into_iter()
        .filter(|f| f["kind"] != "inconclusive")
        .collect();
    let actions: Vec<&str> = findings
        .iter()
        .map(|f| {
            f["counterexample"]["record"]["action"]["name"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert!(actions.contains(&"place_order_unchecked") && actions.contains(&"hire_unchecked"));
    for f in &findings {
        let cx = &f["counterexample"];
        let request = json!({
            "action": cx["record"]["action"]["name"],
            "data_version": "verification",
            "state": cx["state"], "input": cx["input"], "context": cx["context"],
            "facts": cx["facts"],
        });
        assert!(cx["facts"]["queries"].is_array(), "{f}");
        assert!(cx["facts"].get("universe").is_none(), "{f}");
        let record = behavior_core::evaluate(&module, &request.to_string());
        let record = record.as_json();
        assert_eq!(record, &cx["record"], "{f}");
        assert_eq!(record["result"], "DENY", "{f}");
    }
}

#[test]
fn nothing_is_proven_outside_the_model() {
    let (a, _) = common::run("queries");
    for c in common::outcomes(&a) {
        let action = c["action"].as_str().unwrap();
        if ["raise_limit", "remove_cheapest"].contains(&action) {
            assert_eq!(c["outcome"], "inconclusive", "{c}");
        }
    }
}

/// Code review: the `unique` delta rule applies only to a `unique` the invariant guarantees on S
/// (a top-level conjunct). Under `or`, S may hold through the other disjunct with duplicates, so
/// `hire` (which only checks the new number) can break the invariant.
#[test]
fn a_unique_under_disjunction_is_not_proven_by_the_delta_rule() {
    let wire = common::read(&common::fixtures().join("wire/valid/orders.json"));
    let mut w: Value = serde_json::from_str(&wire).unwrap();
    let loc = json!({"file": "orders.py", "line": 34});
    let inv = w["invariants"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|i| i["name"] == "personnel_numbers_unique")
        .unwrap();
    let few = json!({"op": "lt", "loc": loc, "args": [
        {"op": "count", "loc": loc, "args": [{"op": "select", "entity": "Employee", "loc": loc}]},
        {"op": "lit", "type": {"t": "int"}, "value": 3, "loc": loc}]});
    inv["body"] = json!({"op": "or", "args": [few, inv["body"].clone()], "loc": loc});
    let module = behavior_core::admit(&w.to_string()).unwrap();
    let profile = behavior_verify::Profile {
        checks: vec![behavior_verify::CheckKind::Preservation],
        ..behavior_verify::Profile::default()
    };
    let solver = behavior_verify::solver::Z3Process::from_env().unwrap();
    let a = behavior_verify::verify(&module, &profile, None, &solver);
    let hire = common::outcomes(&a)
        .into_iter()
        .find(|c| c["action"] == "hire" && c["subject"] == "personnel_numbers_unique")
        .unwrap();
    assert_eq!(hire["outcome"], "counterexample", "{hire}");
}
