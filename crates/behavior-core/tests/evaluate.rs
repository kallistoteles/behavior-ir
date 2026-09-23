#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Evaluation against hand-written expectations and reviewed golden records.
//! Golden records are created once with `BLESS_RECORDS=1` and reviewed by hand.

mod common;

use behavior_core::{admit, evaluate};
use proptest::prelude::*;
use serde_json::{Value, json};

fn module(name: &str) -> behavior_core::semantic::Module {
    let wire = common::read(&common::fixtures().join(format!("wire/valid/{name}.json")));
    admit(&wire).unwrap()
}

fn expectations() -> Value {
    common::json(&common::fixtures().join("requests/expectations.json"))
}

#[test]
fn records_meet_hand_written_expectations() {
    let mut failures = Vec::new();
    for (name, exp) in expectations().as_object().unwrap() {
        let m = module(exp["wire"].as_str().unwrap());
        let request = common::read(&common::fixtures().join(format!("requests/{name}.json")));
        let record: Value = serde_json::from_str(&evaluate(&m, &request).to_json_string()).unwrap();
        let mut check = |what: &str, ok: bool, detail: String| {
            if !ok {
                failures.push(format!("{name}: {what}: {detail}"));
            }
        };
        check(
            "result",
            record["result"] == exp["result"],
            format!("{}", record["result"]),
        );
        let trace = record["trace"].as_array().unwrap();
        let phases: Vec<&Value> = trace.iter().map(|s| &s["phase"]).collect();
        let want: Vec<&Value> = exp["phases"].as_array().unwrap().iter().collect();
        check("phases", phases == want, format!("{phases:?}"));
        for (i, (step, outcome)) in trace
            .iter()
            .zip(exp["outcomes"].as_array().unwrap())
            .enumerate()
        {
            let ok = match outcome.get("error_prefix") {
                Some(p) => step["outcome"]["error"]
                    .as_str()
                    .is_some_and(|e| e.starts_with(p.as_str().unwrap())),
                None => step["outcome"] == *outcome,
            };
            check(&format!("outcome[{i}]"), ok, format!("{}", step["outcome"]));
        }
        for (i, reads) in exp["reads"].as_object().unwrap() {
            let i: usize = i.parse().unwrap();
            check(
                &format!("reads[{i}]"),
                trace[i]["reads"] == *reads,
                format!("{}", trace[i]["reads"]),
            );
        }
        check(
            "changes",
            record["changes"] == exp["changes"],
            format!("{}", record["changes"]),
        );
        let codes: Vec<&Value> = record["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["code"])
            .collect();
        let want: Vec<&Value> = exp["reasons"].as_array().unwrap().iter().collect();
        check("reasons", codes == want, format!("{}", record["reasons"]));
        if let Some(derived) = exp.get("derived") {
            let got: Vec<Value> = record["derived"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| json!([d["name"], d["phase_state"], d["value"]]))
                .collect();
            check(
                "derived",
                Value::Array(got.clone()) == *derived,
                format!("{got:?}"),
            );
        }
        for step in trace {
            check(
                "step hash",
                step["hash"]
                    .as_str()
                    .is_some_and(|h| h.starts_with("sha256:")),
                step.to_string(),
            );
        }
        check(
            "behavior_version",
            record["behavior_version"] == json!(m.behavior_version()),
            format!("{}", record["behavior_version"]),
        );
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn records_match_reviewed_goldens_and_are_deterministic() {
    let bless = std::env::var("BLESS_RECORDS").is_ok();
    let mut failures = Vec::new();
    for (name, exp) in expectations().as_object().unwrap() {
        let m = module(exp["wire"].as_str().unwrap());
        let request = common::read(&common::fixtures().join(format!("requests/{name}.json")));
        let first = evaluate(&m, &request).to_json_string();
        let second = evaluate(&m, &request).to_json_string();
        assert_eq!(first, second, "{name} is not deterministic");
        let golden = common::fixtures().join(format!("records/{name}.json"));
        match std::fs::read_to_string(&golden) {
            Ok(g) if g == first => {}
            Ok(_) => failures.push(format!("{name}: differs from golden")),
            Err(_) if bless => std::fs::write(&golden, &first).unwrap(),
            Err(_) => failures.push(format!("{name}: no golden (BLESS_RECORDS=1)")),
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

proptest! {
    #[test]
    fn allow_exactly_when_amount_within_limit(amount in 0u64..200_000, limit in 0u64..200_000, cents in 0u8..100) {
        let m = module("invoice");
        let amount = format!("{amount}.{cents:02}");
        let request = json!({
            "action": "approve_invoice", "data_version": "1",
            "state": {"invoice": {"id": "1", "amount": amount, "status": "pending", "approved_by": null}},
            "input": {},
            "context": {"actor": {"id": "a", "role": "manager", "approval_limit": limit.to_string()}}
        });
        let record: Value = serde_json::from_str(&evaluate(&m, &request.to_string()).to_json_string()).unwrap();
        let within = amount.parse::<f64>().unwrap() <= limit as f64 + 1e-9
            && behavior_core::decimal::Dec::parse_str(&amount).unwrap()
                <= behavior_core::decimal::Dec::from_i64(limit as i64);
        prop_assert_eq!(record["result"] == "ALLOW", within);
        prop_assert_eq!(record["changes"].as_array().unwrap().is_empty(), !within);
    }
}
