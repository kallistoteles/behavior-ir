#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US4: actions that can never run and conditions that never matter are warnings.

mod common;

use serde_json::Value;

#[test]
fn warnings_match_expectations_and_do_not_block() {
    let (a, expected) = common::run("dead_and_vacuous");
    assert_eq!(
        common::outcomes(&a),
        common::expected_outcomes(&expected),
        "{}",
        a.to_json_string()
    );
    assert_eq!(a.result, expected["result"].as_str().unwrap());
    let findings = a.findings();
    let kinds: Vec<&str> = findings
        .iter()
        .map(|f| f["kind"].as_str().unwrap())
        .collect();
    for k in ["dead_action", "redundant_precondition", "always_true"] {
        assert_eq!(
            kinds.iter().filter(|x| **x == k).count(),
            1,
            "{k}: {kinds:?}"
        );
    }
    for f in &findings {
        assert_eq!(f["severity"], "warning");
        assert!(f.get("counterexample").is_none());
        assert!(!f["explanation"].as_str().unwrap().is_empty());
    }
    let rule_checks: Vec<&Value> = a.value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["action"].is_null())
        .collect();
    assert_eq!(rule_checks.len(), 4);
    // Checks without an action sort first.
    assert!(a.value["checks"][0]["action"].is_null());
}
