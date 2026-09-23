#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use behavior_core::{admit, evaluate, evaluate_intent};
use serde_json::{Value, json};

fn module() -> behavior_core::semantic::Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/invoice.json"),
    ))
    .unwrap()
}

fn host_for(intent: &str) -> String {
    let name = if intent == "valid_denied" {
        "invoice_1042_over_limit"
    } else {
        "invoice_1042"
    };
    common::read(&common::fixtures().join(format!("host/{name}.json")))
}

#[test]
fn rejected_intents_list_every_problem_and_do_not_evaluate() {
    let m = module();
    let dir = common::fixtures().join("intents");
    let mut seen = 0;
    for path in common::files(&dir, ".json") {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let expected = path.with_extension("expected.json");
        if !expected.exists() {
            continue;
        }
        seen += 1;
        let want = common::json(&expected);
        match evaluate_intent(&m, &common::read(&path), &host_for(&name)) {
            Ok(record) => panic!("{name}: expected rejection, got {}", record.result()),
            Err(rejection) => {
                let got: Value = serde_json::from_str(&rejection.to_json_string()).unwrap();
                assert_eq!(got["rejected"], true, "{name}");
                let pairs: Vec<Value> = got["errors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| json!({"code": e["code"], "path": e["path"]}))
                    .collect();
                assert_eq!(Value::Array(pairs), want["errors"], "{name}");
                for e in got["errors"].as_array().unwrap() {
                    assert!(
                        e["message"].as_str().is_some_and(|m| !m.is_empty()),
                        "{name}"
                    );
                }
            }
        }
    }
    assert!(seen >= 10, "only {seen} rejection fixtures found");
}

#[test]
fn accepted_intents_evaluate_exactly_like_direct_requests() {
    let m = module();
    for (name, result) in [("valid_allowed", "ALLOW"), ("valid_denied", "DENY")] {
        let intent_text = common::read(&common::fixtures().join(format!("intents/{name}.json")));
        let host: Value = serde_json::from_str(&host_for(name)).unwrap();
        let intent: Value = serde_json::from_str(&intent_text).unwrap();
        let record = evaluate_intent(&m, &intent_text, &host.to_string()).unwrap();
        assert_eq!(record.result(), result, "{name}");
        let direct = json!({
            "action": intent["capability"], "data_version": host["data_version"],
            "git_revision": host["git_revision"], "state": host["state"],
            "input": intent["input"], "context": host["context"],
        });
        assert_eq!(
            record.to_json_string(),
            evaluate(&m, &direct.to_string()).to_json_string(),
            "{name}"
        );
    }
}

#[test]
fn an_intent_cannot_supply_context_or_state() {
    let m = module();
    for name in ["extra_field", "state_supplied"] {
        let text = common::read(&common::fixtures().join(format!("intents/{name}.json")));
        assert!(
            evaluate_intent(&m, &text, &host_for(name)).is_err(),
            "{name}"
        );
    }
}
