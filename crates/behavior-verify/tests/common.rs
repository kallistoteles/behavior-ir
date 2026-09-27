//! Helpers shared by verifier tests (included with `mod common;`).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use behavior_verify::solver::Z3Process;
use behavior_verify::{Attestation, CheckKind, Profile, verify};
use serde_json::Value;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

pub fn json(p: &Path) -> Value {
    serde_json::from_str(&read(p)).unwrap()
}

pub fn kinds(names: &Value) -> Vec<CheckKind> {
    names
        .as_array()
        .unwrap()
        .iter()
        .map(|n| CheckKind::parse(n.as_str().unwrap()).unwrap())
        .collect()
}

/// Verifies the fixture `name` (or the wire file its expectations point to).
pub fn run(name: &str) -> (Attestation, Value) {
    let (a, e, _) = run_with_module(name);
    (a, e)
}

pub fn run_with_module(
    name: &str,
) -> (Attestation, Value, behavior_core::semantic::module::Module) {
    let dir = fixtures().join("verify");
    let expected = json(&dir.join(format!("{name}.expected.json")));
    let wire = match expected.get("wire") {
        Some(w) => read(&dir.join(w.as_str().unwrap())),
        None => read(&dir.join(format!("{name}.json"))),
    };
    let module = behavior_core::admit(&wire).unwrap();
    let profile = Profile {
        checks: kinds(&expected["profile"]),
        ..Profile::default()
    };
    let solver = Z3Process::from_env().unwrap();
    (verify(&module, &profile, None, &solver), expected, module)
}

/// `(kind, action, subject name, param, outcome)` of every check in the attestation.
fn sorted(mut v: Vec<Value>) -> Vec<Value> {
    v.sort_by_key(|c| c.to_string());
    v
}

/// Order-insensitive: check order depends on content hashes.
pub fn outcomes(a: &Attestation) -> Vec<Value> {
    sorted(outcomes_in_order(a))
}

fn outcomes_in_order(a: &Attestation) -> Vec<Value> {
    a.value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            serde_json::json!({
                "kind": c["kind"],
                "action": c["action"]["name"],
                "subject": c["subject"]["name"],
                "param": c["subject"].get("param").cloned().unwrap_or(Value::Null),
                "outcome": c["outcome"],
            })
        })
        .collect()
}

pub fn expected_outcomes(expected: &Value) -> Vec<Value> {
    sorted(expected_in_order(expected))
}

fn expected_in_order(expected: &Value) -> Vec<Value> {
    expected["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            serde_json::json!({
                "kind": c["kind"], "action": c.get("action").cloned().unwrap_or(Value::Null),
                "subject": c["subject"], "param": c.get("param").cloned().unwrap_or(Value::Null),
                "outcome": c["outcome"],
            })
        })
        .collect()
}
