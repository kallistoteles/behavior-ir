#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The CLI with relational queries (feature 007): wire IR 0.6 modules, requests with universe,
//! query and field facts, replay of records with query facts, and verification over sets.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_behavior"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
    )
}

fn tmp(name: &str, text: &str) -> String {
    let p = std::env::temp_dir().join(format!("behavior-cli-007-{}-{name}", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

fn req(n: &str) -> String {
    fixtures()
        .join(format!("requests/007/{n}.json"))
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn eval_and_replay_with_query_facts() {
    let wire = fixtures().join("wire/valid/orders.json");
    let wire = wire.to_str().unwrap();
    let (code, out) = run(&["admit", wire]);
    assert_eq!(code, 0, "{out}");

    let (code, record) = run(&["eval", wire, &req("close_customer_no_open")]);
    assert_eq!(code, 0, "{record}");
    let v: Value = serde_json::from_str(&record).unwrap();
    assert_eq!(v["record_version"], "0.6");
    assert_eq!(v["facts"]["queries"][0]["members"], serde_json::json!([]));
    let (code, out) = run(&["replay", wire, &tmp("record.json", &record)]);
    assert_eq!(code, 0, "{out}");

    // The observed facts alone reproduce the decision.
    let mut request: Value =
        serde_json::from_str(&std::fs::read_to_string(req("close_customer_open")).unwrap())
            .unwrap();
    let (code, open) = run(&["eval", wire, &req("close_customer_open")]);
    assert_eq!(code, 1, "a denial: {open}");
    let open: Value = serde_json::from_str(&open).unwrap();
    request["facts"] = open["facts"].clone();
    let (code, again) = run(&["eval", wire, &tmp("observed.json", &request.to_string())]);
    assert_eq!(code, 1);
    assert_eq!(serde_json::from_str::<Value>(&again).unwrap(), open);

    let (code, _) = run(&["eval", wire, &req("close_customer_unknown")]);
    assert_eq!(code, 3, "UNKNOWN_FACT is an evaluation error");
    let (code, _) = run(&["eval", wire, &req("universe_inconsistent")]);
    assert_eq!(code, 2, "INCONSISTENT_FACTS is invalid input");
}

#[test]
fn a_query_in_a_0_5_module_is_refused() {
    let wire = fixtures().join("wire/invalid/query_in_0_5.json");
    let (code, out) = run(&["admit", wire.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(out.contains("UNSUPPORTED_IR_VERSION"), "{out}");
}

#[test]
fn verify_reports_proven_counterexample_and_inconclusive_over_sets() {
    let wire = fixtures().join("wire/valid/orders.json");
    let (_, out) = run(&["verify", wire.to_str().unwrap()]);
    let v: Value = serde_json::from_str(&out).unwrap();
    let outcome = |action: &str, kind: &str| {
        v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["action"]["name"] == action && c["kind"] == kind)
            .map(|c| c["outcome"].as_str().unwrap().to_string())
            .unwrap()
    };
    assert_eq!(outcome("add_counted_order", "postcondition"), "proven");
    assert_eq!(outcome("hire", "preservation"), "proven");
    assert_eq!(outcome("hire_unchecked", "preservation"), "counterexample");
    assert_eq!(outcome("raise_limit", "postcondition"), "inconclusive");
}
