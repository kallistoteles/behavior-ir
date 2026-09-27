#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The CLI with fixed-scale decimals (feature 003) and wire IR 0.4 exact arithmetic (feature 004).

use std::path::{Path, PathBuf};
use std::process::Command;

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
    let p = std::env::temp_dir().join(format!("behavior-cli-003-{}-{name}", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn admit_eval_replay_and_verify_fixed_scale() {
    let f = fixtures();
    let wire = f.join("wire/valid/fixed_scale.json");
    let wire = wire.to_str().unwrap();
    let (code, out) = run(&["admit", wire]);
    assert_eq!(code, 0, "{out}");

    let req = |n: &str| {
        f.join(format!("requests/003/{n}.json"))
            .to_str()
            .unwrap()
            .to_string()
    };
    let (code, record) = run(&["eval", wire, &req("split3")]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&record).unwrap();
    assert_eq!(v["record_version"], "0.4");
    assert_eq!(v["changes"][0]["new"], "13.33");
    let (code, _) = run(&["replay", wire, &tmp("record.json", &record)]);
    assert_eq!(code, 0);
    let (code, _) = run(&["eval", wire, &req("off_grid_state")]);
    assert_eq!(code, 2, "INVALID_INPUT");

    let verified = f.join("verify/purchase_money2_remaining.json");
    let (code, out) = run(&["verify", verified.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn old_wire_versions_are_refused_and_exact_ratios_evaluate() {
    let f = fixtures();
    let mut old: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(f.join("wire/valid/empty.json")).unwrap())
            .unwrap();
    old["ir_version"] = serde_json::json!("0.3");
    let (code, out) = run(&["admit", &tmp("old.json", &old.to_string())]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("UNSUPPORTED_IR_VERSION"), "{out}");

    let wire = f.join("wire/valid/exact_closure.json");
    let req = f.join("requests/004/share.json");
    let (code, record) = run(&["eval", wire.to_str().unwrap(), req.to_str().unwrap()]);
    assert_eq!(code, 0, "{record}");
    let v: serde_json::Value = serde_json::from_str(&record).unwrap();
    assert_eq!(v["record_version"], "0.4");
    assert_eq!(v["derived"][0]["value"], "1/3");
}
