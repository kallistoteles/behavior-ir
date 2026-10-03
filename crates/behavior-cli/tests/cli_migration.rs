#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: `behavior migration admit|apply` (FR-023, contracts/migration-api.md). There is no
//! persistent store on the command line: `apply` runs in plain mode over a supplied universe.

use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_behavior"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn fixtures() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
}

fn arg(p: PathBuf) -> String {
    p.to_str().unwrap().to_string()
}

fn module(name: &str) -> String {
    arg(fixtures().join("modules").join(format!("{name}.json")))
}

fn write_tmp(name: &str, v: &Value) -> String {
    let dir = std::env::temp_dir().join(format!("behavior-cli-migration-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, v.to_string()).unwrap();
    arg(p)
}

#[test]
fn admit_prints_the_resolved_migration_and_its_summary() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = arg(fixtures().join("valid/cultures_v1_to_v2.json"));
    let (code, out, err) = run(&["migration", "admit", &v1, &v2, &m]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], json!(true));
    assert!(v["hash"].as_str().unwrap().starts_with("sha256:"));
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("valid/cultures_v1_to_v2.expected.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(v["summary"], expected["summary"]);
    assert_eq!(v["resolved"]["retire"], json!(["AuditNote"]));
}

#[test]
fn admit_refuses_an_invalid_migration_with_exit_code_2() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = arg(fixtures().join("invalid/missing_field.json"));
    let (code, out, _) = run(&["migration", "admit", &v1, &v2, &m]);
    assert_eq!(code, 2);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["errors"][0]["code"], json!("MISSING_MIGRATION_FIELD"));
}

#[test]
fn apply_migrates_a_supplied_universe() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = arg(fixtures().join("valid/cultures_v1_to_v2.json"));
    let facts = write_tmp(
        "universe.json",
        &json!([
            {"entity": "Customer", "value": {"id": "c1", "name": "Ada", "email": "a@x"}},
            {"entity": "Order", "value": {"id": "o1", "customer": "c1", "region": null, "qty": 2}},
        ]),
    );
    let (code, out, err) = run(&["migration", "apply", &v1, &v2, &m, &facts]);
    assert_eq!(code, 0, "{err}{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["result"], json!("MIGRATED"));
    let order = v["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == "Order")
        .unwrap();
    assert_eq!(order["value"]["buyer"], json!("c1"));
}

#[test]
fn apply_reports_a_refusal_with_exit_code_1() {
    let (v2, v3) = (module("cultures_v2"), module("cultures_v3"));
    let m = arg(fixtures().join("valid/region_required.json"));
    let facts = write_tmp(
        "early.json",
        &json!([
            {"entity": "Customer", "value": {"id": "c1", "name": "Ada", "email": "a@x"}},
            {"entity": "Order", "value": {"id": "o1", "buyer": "c1", "region": null, "qty": 2}},
        ]),
    );
    let (code, out, _) = run(&["migration", "apply", &v2, &v3, &m, &facts]);
    assert_eq!(code, 1, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["result"], json!("MIGRATION_REQUIREMENT_FAILED"));
    assert_eq!(v["rule"], json!("every_order_has_region"));
    assert_eq!(v["count"], json!(1));
}

fn run_env(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_behavior"));
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn verify_prints_the_attestation_with_its_exit_code() {
    let (v2, v3) = (module("cultures_v2"), module("cultures_v3"));
    let m = arg(fixtures().join("valid/region_required.json"));
    let (code, out, err) = run(&["migration", "verify", &v2, &v3, &m]);
    assert_eq!(code, 0, "{err}{out}");
    let a: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(a["subject"], json!("migration"));
    assert_eq!(a["result"], json!("verified"));
    // V1 → V2 widens Money to four decimals, which narrows its range: a real counterexample.
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = arg(fixtures().join("valid/cultures_v1_to_v2.json"));
    let (code, out, _) = run(&["migration", "verify", &v1, &v2, &m]);
    assert_eq!(code, 1, "{out}");
    let a: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(a["result"], json!("not_verified"));
}

#[test]
fn verify_without_the_solver_exits_3() {
    let (v2, v3) = (module("cultures_v2"), module("cultures_v3"));
    let m = arg(fixtures().join("valid/region_required.json"));
    let (code, _, err) = run_env(
        &["migration", "verify", &v2, &v3, &m],
        &[("BEHAVIOR_Z3", "/nonexistent/z3")],
    );
    assert_eq!(code, 3, "{err}");
}
