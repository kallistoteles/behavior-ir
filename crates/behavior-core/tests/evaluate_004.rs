#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Exact closure at runtime (feature 004): exact ratios, exact general decimals, replay.
//! `requests/004/expectations.json` uses the subset format of `common::subset_problems`.

mod common;

use behavior_core::{admit, evaluate, replay};
use serde_json::Value;

#[test]
fn records_meet_004_expectations_and_replay() {
    let m = admit(&common::read(
        &common::fixtures().join("wire/valid/exact_closure.json"),
    ))
    .unwrap();
    let exp = common::json(&common::fixtures().join("requests/004/expectations.json"));
    let mut failures = Vec::new();
    for (name, e) in exp.as_object().unwrap() {
        let request = common::read(&common::fixtures().join(format!("requests/004/{name}.json")));
        let text = evaluate(&m, &request).to_json_string();
        let rec: Value = serde_json::from_str(&text).unwrap();
        let mut out = common::subset_problems(e, &rec);
        if rec["record_version"] != "0.4" {
            out.push(format!("record_version {}", rec["record_version"]));
        }
        if !replay(&m, &text).matches {
            out.push("replay mismatch".into());
        }
        if !out.is_empty() {
            failures.push(format!(
                "{name}:\n    {}\n    record: {rec}",
                out.join("\n    ")
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
