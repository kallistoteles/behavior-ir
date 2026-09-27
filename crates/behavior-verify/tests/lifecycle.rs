#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 006: verification of creation, removal, `exists`, `referenced` and referential
//! integrity. Every counterexample carries the evaluation facts it needs and is confirmed by
//! plain evaluation; nothing is inconclusive (SC-005).

mod common;

use serde_json::{Value, json};

#[test]
fn lifecycle_outcomes_match_expectations() {
    let (a, expected) = common::run("lifecycle");
    let got = common::outcomes(&a);
    let want = common::expected_outcomes(&expected);
    assert_eq!(
        Value::Array(got.clone()),
        Value::Array(want),
        "\n{}",
        serde_json::to_string_pretty(&got).unwrap()
    );
    assert!(
        got.iter().all(|c| c["outcome"] != "inconclusive"),
        "{got:?}"
    );
}

#[test]
fn counterexamples_are_confirmed_with_their_facts() {
    let (a, _, module) = common::run_with_module("lifecycle");
    let findings = a.findings();
    assert!(!findings.is_empty());
    for f in findings {
        let cx = &f["counterexample"];
        let mut request = json!({
            "action": cx["record"]["action"]["name"],
            "data_version": "verification",
            "state": cx["state"], "input": cx["input"], "context": cx["context"],
        });
        if let Some(facts) = cx.get("facts") {
            request["facts"] = facts.clone();
        }
        let record = behavior_core::evaluate(&module, &request.to_string());
        let record = record.as_json();
        assert_eq!(record, &cx["record"], "{f}");
        assert_eq!(record["result"], "DENY", "{f}");
        match f["kind"].as_str().unwrap() {
            "referential_integrity" => {
                assert_eq!(record["reasons"][0]["code"], "DANGLING_REFERENCE", "{f}");
                assert!(cx["facts"]["references"].is_array(), "{f}");
            }
            "preservation" => {
                assert_eq!(record["reasons"][0]["code"], "CONSTRAINT_VIOLATED", "{f}");
                assert!(cx["facts"]["identities"].is_array(), "{f}");
            }
            other => panic!("unexpected finding {other}: {f}"),
        }
    }
}
