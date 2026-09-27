#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US2 (feature 006): removal makes the entity absent from S' and records its last value.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admission_report, admit, evaluate, replay};
use serde_json::{Value, json};

fn accounts() -> Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/accounts.json"),
    ))
    .unwrap()
}

fn run(m: &Module, request: &Value) -> Value {
    let text = evaluate(m, &request.to_string()).to_json_string();
    let replayed = replay(m, &text);
    assert!(replayed.matches, "{:?}", replayed.diff);
    serde_json::from_str(&text).unwrap()
}

#[test]
fn close_account_removes_with_the_last_value() {
    let account = json!({"id": "a1", "owner": "c1", "balance": "0.00"});
    let rec = run(
        &accounts(),
        &json!({"action": "close_account", "data_version": "1", "input": {}, "context": {},
                "state": {"account": account}}),
    );
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    assert_eq!(
        rec["lifecycle"],
        json!([{"op": "remove", "entity": "Account", "id": "a1", "value": account}])
    );
    assert!(rec["changes"].as_array().unwrap().is_empty());
    // `not exists(account.id)` holds on S'; no fact is needed for a bound, removed entity.
    let post = rec["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["phase"] == "postcondition")
        .unwrap();
    assert_eq!(post["outcome"], true);
    assert!(rec.get("facts").is_none(), "{rec}");
}

#[test]
fn update_and_remove_of_one_parameter_is_refused_at_admission() {
    let text = common::read(&common::fixtures().join("wire/invalid/remove_and_update.json"));
    let codes: Vec<String> = admission_report(&text)
        .errors
        .iter()
        .map(|e| e.code.clone())
        .collect();
    assert_eq!(codes, ["LIFECYCLE_CONFLICT"]);
    let text = common::read(&common::fixtures().join("wire/invalid/remove_non_state.json"));
    let codes: Vec<String> = admission_report(&text)
        .errors
        .iter()
        .map(|e| e.code.clone())
        .collect();
    assert_eq!(codes, ["TYPE_MISMATCH"]);
}

#[test]
fn removed_entities_leave_the_outgoing_rules() {
    // A removed entity has no S' value: its constraints and invariants are not checked on S'.
    let rec = run(
        &accounts(),
        &json!({"action": "close_account", "data_version": "1", "input": {}, "context": {},
                "state": {"account": {"id": "a1", "owner": "c1", "balance": "0.00"}}}),
    );
    let post_rules = rec["trace"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["phase"] == "constraint_post")
        .count();
    assert_eq!(post_rules, 0, "{rec}");
}
