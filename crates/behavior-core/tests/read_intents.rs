#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Read intents (feature 010, US5): untrusted callers call declared reads by name through the
//! validated capability boundary, get only the declared result and the record identity, and every
//! malformed call is rejected with all its problems before evaluation (FR-016, FR-016a, SC-006).

mod common;

use behavior_core::read::evaluate_read_intent;
use behavior_core::{admit, evaluate_intent};
use serde_json::{Value, json};

fn lab() -> behavior_core::semantic::Module {
    admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap()
}

fn host() -> String {
    common::read(&common::fixtures().join("read_intents/host.json"))
}

#[test]
fn malformed_read_intents_are_rejected_with_every_problem() {
    let m = lab();
    let dir = common::fixtures().join("read_intents");
    let mut seen = 0;
    for path in common::files(&dir, ".json") {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let expected = path.with_extension("expected.json");
        if name == "host" || !expected.exists() {
            continue;
        }
        seen += 1;
        let want = common::json(&expected);
        match evaluate_read_intent(&m, &common::read(&path), &host()) {
            Ok(x) => panic!(
                "{name}: expected rejection, got {}",
                x.record.to_json_string()
            ),
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
            }
        }
    }
    assert!(seen >= 11, "only {seen} rejection fixtures found");
}

#[test]
fn a_response_holds_the_declared_result_and_nothing_else() {
    let m = lab();
    let intent = common::read(&common::fixtures().join("read_intents/valid_value.json"));
    let x = evaluate_read_intent(&m, &intent, &host()).unwrap();
    let response: Value = serde_json::from_str(&x.response.to_json_string()).unwrap();
    let keys: Vec<&str> = response
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["record_id", "result", "value"]);
    assert_eq!(response["value"], 1);
    assert_eq!(response["record_id"], x.record.record_id());
}

#[test]
fn internal_observations_stay_in_the_record() {
    let m = lab();
    let intent = common::read(&common::fixtures().join("read_intents/valid_projection.json"));
    let x = evaluate_read_intent(&m, &intent, &host()).unwrap();
    // `standing` reads the customer's internal credit limit: evidence for the host only.
    let record = x.record.as_json();
    assert!(
        record["observed"]
            .as_array()
            .unwrap()
            .contains(&json!(["customer", "credit_limit"])),
        "{record}"
    );
    let response = x.response.to_json_string();
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["value"],
        json!({"id": "k1", "name": "Ada", "standing": true})
    );
    assert!(!response.contains("credit_limit"), "{response}");
    assert!(!response.contains("observed"), "{response}");
    assert!(!response.contains("facts"), "{response}");
}

#[test]
fn an_action_intent_cannot_name_a_read() {
    let m = lab();
    let host = json!({"data_version": "1", "state": {}, "context": {}}).to_string();
    let rejection = evaluate_intent(
        &m,
        &json!({"capability": "active_count"}).to_string(),
        &host,
    )
    .unwrap_err();
    assert_eq!(rejection.errors[0].code, "UNKNOWN_CAPABILITY");
}

#[test]
fn host_entities_named_like_inputs_stay_out_of_the_state() {
    // The host holds an entity keyed `threshold` for some other capability; `big_orders` has an
    // input of that name. Only the read's `state` parameters take host entities.
    let m = lab();
    let mut host: Value = serde_json::from_str(&host()).unwrap();
    host["state"]["threshold"] = json!({"id": "k9", "name": "Other", "credit_limit": 1});
    let intent = json!({"capability": "big_orders", "input": {"threshold": 10}});
    let x = evaluate_read_intent(&m, &intent.to_string(), &host.to_string()).unwrap();
    assert_eq!(
        x.record.as_json()["result"],
        "VALUE",
        "{}",
        x.record.to_json_string()
    );
}
