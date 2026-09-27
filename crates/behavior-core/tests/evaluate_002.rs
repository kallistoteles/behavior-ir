#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Runtime changes of feature 002: entity constraints by role, the binding check, and change
//! entries that address state cells.

mod common;

use behavior_core::{admit, evaluate};
use serde_json::Value;

#[test]
fn records_meet_002_expectations() {
    let module = admit(&common::read(
        &common::fixtures().join("wire/valid/constraints.json"),
    ))
    .unwrap();
    let exp = common::json(&common::fixtures().join("requests/002/expectations.json"));
    let mut failures = Vec::new();
    for (name, e) in exp.as_object().unwrap() {
        let request = common::read(&common::fixtures().join(format!("requests/002/{name}.json")));
        let record: Value =
            serde_json::from_str(&evaluate(&module, &request).to_json_string()).unwrap();
        let mut check = |what: &str, ok: bool, got: String| {
            if !ok {
                failures.push(format!("{name}: {what}: {got}"));
            }
        };
        check(
            "record_version",
            record["record_version"] == "0.4",
            record["record_version"].to_string(),
        );
        check(
            "result",
            record["result"] == e["result"],
            record["result"].to_string(),
        );
        let trace = record["trace"].as_array().unwrap();
        let phases: Vec<&Value> = trace.iter().map(|s| &s["phase"]).collect();
        check(
            "phases",
            phases == e["phases"].as_array().unwrap().iter().collect::<Vec<_>>(),
            format!("{phases:?}"),
        );
        let outcomes: Vec<&Value> = trace.iter().map(|s| &s["outcome"]).collect();
        check(
            "outcomes",
            outcomes == e["outcomes"].as_array().unwrap().iter().collect::<Vec<_>>(),
            format!("{outcomes:?}"),
        );
        if let Some(roles) = e.get("roles") {
            let got: Vec<&Value> = trace.iter().map(|s| &s["role"]).collect();
            check(
                "roles",
                got == roles.as_array().unwrap().iter().collect::<Vec<_>>(),
                format!("{got:?}"),
            );
        }
        let codes: Vec<&Value> = record["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["code"])
            .collect();
        check(
            "reasons",
            codes == e["reasons"].as_array().unwrap().iter().collect::<Vec<_>>(),
            format!("{codes:?}"),
        );
        check(
            "changes",
            record["changes"] == e["changes"],
            record["changes"].to_string(),
        );
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn alias_reason_names_both_parameters() {
    let module = admit(&common::read(
        &common::fixtures().join("wire/valid/constraints.json"),
    ))
    .unwrap();
    let request = common::read(&common::fixtures().join("requests/002/alias.json"));
    let record: Value =
        serde_json::from_str(&evaluate(&module, &request).to_json_string()).unwrap();
    let message = record["reasons"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("from_") && message.contains("to"),
        "{message}"
    );
}
