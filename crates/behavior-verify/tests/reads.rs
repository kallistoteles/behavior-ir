#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Declared reads are verified (feature 010, FR-017): every evaluation error a read can hit on a
//! valid state is found, with a counterexample confirmed by evaluating the read.

mod common;

use serde_json::Value;

#[test]
fn read_outcomes_match_expectations() {
    let (a, expected) = common::run("reads");
    let got = common::outcomes(&a);
    let want = common::expected_outcomes(&expected);
    assert_eq!(
        Value::Array(got),
        Value::Array(want),
        "\n{}",
        a.to_json_string()
    );
}

#[test]
fn read_counterexamples_reproduce_the_error() {
    let (a, _, module) = common::run_with_module("reads");
    let findings = a.findings();
    assert_eq!(findings.len(), 4, "{}", a.to_json_string());
    for f in findings {
        assert_eq!(f["kind"], "evaluation_error", "{f}");
        let record = &f["counterexample"]["record"];
        assert_eq!(record["format"], "behavior.read_record.v1", "{f}");
        assert_eq!(record["result"], "EVALUATION_ERROR", "{f}");
        // The confirmed record replays: it is a real read of a real state.
        let r = behavior_core::read::replay_read(&module, &record.to_string());
        assert!(r.matches, "{:?}", r.diff);
    }
}

/// A query projection whose read also binds an entity of the projected type: the counterexample
/// state holds both the bound entity and the member, so it reproduces (code review, finding 4).
#[test]
fn a_projection_with_a_bound_entity_of_its_type_is_confirmed() {
    use behavior_verify::solver::Z3Process;
    use behavior_verify::{CheckKind, Profile, verify};
    use serde_json::json;
    let l = json!({"file": "o.py", "line": 1});
    let f = |p: &str, n: &str| json!({"op": "field", "param": p, "field": n, "loc": l});
    let int = json!({"t": "int"});
    let wire = json!({
        "ir_version": "0.7", "enums": [], "nominals": [], "invariants": [], "actions": [],
        "entities": [{"name": "Culture", "loc": l, "fields": [
            {"name": "measurements", "type": int, "loc": l},
            {"name": "ph_total", "type": int, "loc": l}]}],
        "constraints": [{"name": "counted", "entity": "Culture", "param": "c", "loc": l,
            "body": {"op": "ge", "loc": l, "args": [f("c", "measurements"),
                     {"op": "lit", "type": int, "value": 0, "loc": l}]}}],
        "derived": [{"name": "avg", "kind": "derived", "loc": l,
            "params": [{"name": "c", "type": {"t": "entity", "name": "Culture"}}],
            "body": {"op": "div", "args": [f("c", "ph_total"), f("c", "measurements")],
                     "loc": l}}],
        "reads": [{"name": "others", "loc": l,
            "params": [{"name": "culture", "role": "state",
                        "type": {"t": "entity", "name": "Culture"}}],
            // Every other culture: the member is never the bound one.
            "body": {"project": {"param": "c", "items": [{"derived": "avg"}], "over": {
                "op": "where", "param": "c", "loc": l,
                "args": [{"op": "select", "entity": "Culture", "loc": l}],
                "body": {"op": "ne", "loc": l, "args": [f("c", "id"), f("culture", "id")]}}}}}],
    });
    let module = behavior_core::admit(&wire.to_string()).unwrap();
    let profile = Profile {
        checks: vec![CheckKind::EvaluationError],
        ..Profile::default()
    };
    let a = verify(&module, &profile, None, &Z3Process::from_env().unwrap());
    let outcomes: Vec<(String, String)> = a.value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["action"]["name"] == "read:others")
        .map(|c| {
            (
                c["subject"]["name"].as_str().unwrap().to_string(),
                c["outcome"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        outcomes
            .iter()
            .any(|(s, o)| s.contains("division by zero") && o == "counterexample"),
        "{outcomes:?}\n{}",
        a.to_json_string()
    );
}
