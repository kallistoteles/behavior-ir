#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3 (feature 004): exact ratios inside one rescale are verified exactly, and every
//! counterexample reproduces on the runtime (SC-005).

mod common;

use behavior_core::{admit, evaluate};
use serde_json::Value;

#[test]
fn share_bound_is_proven_only_with_the_budget_check() {
    let (a, expected) = common::run("share_bound");
    assert_eq!(
        Value::Array(common::outcomes(&a)),
        Value::Array(common::expected_outcomes(&expected))
    );
    let inconclusive = a
        .findings()
        .into_iter()
        .filter(|f| f["kind"] == "inconclusive");
    assert_eq!(inconclusive.count(), 0);
}

#[test]
fn share_bound_counterexamples_are_confirmed_by_the_runtime() {
    let (a, _) = common::run("share_bound");
    let m = admit(&common::read(
        &common::fixtures().join("verify/share_bound.json"),
    ))
    .unwrap();
    let mut seen = 0;
    for f in a.findings() {
        let Some(cx) = f.get("counterexample") else {
            continue;
        };
        seen += 1;
        let record = &cx["record"];
        assert_eq!(record["record_version"], "0.4", "{f}");
        assert!(
            ["DENY", "ERROR"].contains(&record["result"].as_str().unwrap()),
            "{f}"
        );
        // Re-evaluating the counterexample's request gives the same decision.
        let request = serde_json::json!({
            "action": record["action"]["name"], "data_version": record["data_version"],
            "state": cx["state"], "input": cx["input"], "context": cx["context"],
        });
        let again: Value =
            serde_json::from_str(&evaluate(&m, &request.to_string()).to_json_string()).unwrap();
        assert_eq!(again["result"], record["result"], "{f}");
    }
    assert!(
        seen >= 2,
        "expected the postcondition and overflow counterexamples"
    );
}

/// Review finding: a fixed-scale sum inside exact arithmetic or `underlying(...)` is range-checked
/// by the runtime, so the verifier must find the overflow too (it used to prove the
/// `underlying` form safe) — and the counterexample must reproduce.
#[test]
fn fixed_scale_sums_inside_exact_arithmetic_have_range_obligations() {
    use behavior_verify::solver::Z3Process;
    use behavior_verify::{CheckKind, Profile, verify};
    use serde_json::json;
    let l = || json!({"file": "o.py", "line": 1});
    let f = |n: &str| json!({"op": "field", "param": "r", "field": n, "loc": l()});
    let sum = || json!({"op": "add", "args": [f("x"), f("y")], "loc": l()});
    let half = json!({"op": "lit", "type": {"t": "decimal"}, "value": "0.5", "loc": l()});
    let forms = [
        json!({"op": "mul", "args": [sum(), half.clone()], "loc": l()}),
        json!({"op": "mul", "args": [{"op": "unwrap", "args": [sum()], "loc": l()}, half], "loc": l()}),
    ];
    for body in forms {
        let money = json!({"t": "nominal", "name": "Money"});
        let doc = json!({
            "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [], "derived": [],
            "nominals": [{"name": "Money", "underlying": {"t": "decimal"},
                          "ops": ["add", "order", "ratio", "scale"], "scale": 2, "loc": l()}],
            "entities": [{"name": "Row", "loc": l(), "fields": [
                {"name": "x", "type": money, "loc": l()}, {"name": "y", "type": money, "loc": l()}]}],
            "actions": [{"name": "check", "loc": l(), "effects": [], "postconditions": [],
                "params": [{"name": "r", "role": "state", "type": {"t": "entity", "name": "Row"}}],
                "preconditions": [{"expr": {"op": "eq", "args": [body.clone(), body], "loc": l()}, "loc": l()}]}],
        });
        let m = behavior_core::admit(&doc.to_string()).unwrap();
        let profile = Profile {
            checks: vec![CheckKind::EvaluationError],
            ..Profile::default()
        };
        let a = verify(&m, &profile, None, &Z3Process::from_env().unwrap());
        let overflow = a
            .findings()
            .into_iter()
            .find(|f| {
                f["kind"] == "evaluation_error"
                    && f["explanation"].as_str().unwrap().contains("r.x + r.y")
            })
            .unwrap_or_else(|| panic!("no range finding: {}", a.to_json_string()));
        let record = &overflow["counterexample"]["record"];
        assert_eq!(record["result"], "ERROR", "{overflow}");
    }
}
