#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US1: every action preserves the state invariants and entity constraints, or a confirmed
//! counterexample shows how it breaks them.

mod common;

use serde_json::Value;

const FIXTURES: [&str; 4] = [
    "purchase",
    "purchase_fixed",
    "unchanged_entity",
    "constraint_break",
];

#[test]
fn outcomes_match_expectations() {
    let mut failures = Vec::new();
    for name in FIXTURES {
        let (a, expected) = common::run(name);
        let got = common::outcomes(&a);
        let want = common::expected_outcomes(&expected);
        if got != want {
            failures.push(format!(
                "{name}\n  expected {}\n  got      {}",
                Value::Array(want),
                Value::Array(got)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn counterexamples_reproduce_and_respect_runtime_guarantees() {
    for name in FIXTURES {
        let (a, _, module) = common::run_with_module(name);
        for f in a.value["findings"].as_array().unwrap() {
            let cx = &f["counterexample"];
            if f["kind"] == "inconclusive" {
                continue;
            }
            let record = &cx["record"];
            assert_eq!(record["result"], "DENY", "{name}: {f}");
            let failing = record["trace"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["outcome"] == Value::Bool(false))
                .unwrap();
            assert!(
                failing["phase"] == "invariant_post" || failing["phase"] == "constraint_post",
                "{name}: {failing}"
            );
            // Entity constraints hold on every input and context entity; state parameters of the
            // same entity type have distinct identities.
            let action = module
                .action(record["action"]["name"].as_str().unwrap())
                .unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for p in action.params() {
                if let (Some(v), behavior_core::semantic::types::Type::Entity(e)) =
                    (cx["state"].get(p.name()), p.ty())
                {
                    assert!(
                        seen.insert((e.clone(), v["id"].to_string())),
                        "{name}: aliased {}",
                        p.name()
                    );
                }
            }
            for step in record["trace"].as_array().unwrap() {
                if step["phase"] == "constraint" {
                    assert_eq!(step["outcome"], Value::Bool(true), "{name}: {step}");
                }
            }
            assert_eq!(f["severity"], "blocking");
        }
    }
}

#[test]
fn attestations_are_deterministic() {
    for name in ["purchase", "constraint_break"] {
        let (a, _) = common::run(name);
        let (b, _) = common::run(name);
        assert_eq!(a.to_json_string(), b.to_json_string(), "{name}");
    }
}

#[test]
fn result_reflects_blocking_findings() {
    assert_eq!(common::run("purchase").0.result, "not_verified");
    assert_eq!(common::run("purchase_fixed").0.result, "verified");
}
