#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use behavior_core::testing::typing_case;

#[test]
fn engine_agrees_with_shared_typing_table() {
    let doc = common::json(&common::fixtures().join("typing_cases.json"));
    let env = &doc["env"];
    let mut failures = Vec::new();
    for case in doc["cases"].as_array().unwrap() {
        let got = match typing_case(env, case) {
            Ok(ty) => ty,
            Err(code) => serde_json::json!({ "error": code }),
        };
        if got != case["expect"] {
            failures.push(format!("{case}\n    got: {got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "typing mismatches:\n{}",
        failures.join("\n")
    );
}
