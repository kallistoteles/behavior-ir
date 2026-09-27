#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The CLI accepts wire IR 0.3 (feature 003, FR-016).

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
    assert_eq!(v["record_version"], "0.3");
    assert_eq!(v["changes"][0]["new"], "13.33");
    let (code, _) = run(&["replay", wire, &tmp("record.json", &record)]);
    assert_eq!(code, 0);
    let (code, _) = run(&["eval", wire, &req("off_grid_state")]);
    assert_eq!(code, 2, "INVALID_INPUT");

    let verified = f.join("verify/purchase_money2_remaining.json");
    let (code, out) = run(&["verify", verified.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
}
