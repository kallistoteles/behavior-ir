#![allow(clippy::unwrap_used, clippy::expect_used)]

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

#[test]
fn verify_exit_codes_and_out_file() {
    let fixed = fixtures().join("verify/purchase_fixed.json");
    let broken = fixtures().join("verify/purchase.json");
    let invalid = fixtures().join("wire/invalid/cycle_a_b_c.json");
    let profile = fixtures().join("governance/preservation_profile.json");
    let (code, _) = run(&[
        "verify",
        fixed.to_str().unwrap(),
        "--profile",
        profile.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let out = std::env::temp_dir().join("behavior-cli-verify.json");
    let (code, stdout) = run(&[
        "verify",
        broken.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["result"], "not_verified");
    let (code, _) = run(&["verify", invalid.to_str().unwrap()]);
    assert_eq!(code, 2);
}
