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

fn tmp(name: &str, text: &str) -> String {
    let p = std::env::temp_dir().join(format!("behavior-cli-auth-{}-{name}", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn authorize_exit_codes_and_waiver_hash() {
    let f = fixtures();
    let wire = f.join("verify/purchase_fixed.json");
    let wire = wire.to_str().unwrap();
    let gov = |n: &str| f.join("governance").join(n).to_str().unwrap().to_string();
    let (code, record) = run(&["eval", wire, &gov("approve_request.json")]);
    assert_eq!(code, 0);
    let record = tmp("record.json", &record);

    // Verified: allowed.
    let (code, attestation) = run(&[
        "verify",
        wire,
        "--profile",
        &gov("preservation_profile.json"),
    ]);
    assert_eq!(code, 0);
    let verified = tmp("verified.json", &attestation);
    let now = ["--now", "2026-09-25T12:00:00Z"];
    let (code, out) = run(&[
        &[
            "authorize",
            wire,
            &record,
            "--policy",
            &gov("require_verified.json"),
            "--attestation",
            &verified,
        ][..],
        &now,
    ]
    .concat());
    assert_eq!(code, 0, "{out}");
    let auth: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(auth["decision"], "allow");

    // No attestation: refused.
    let (code, _) = run(&[
        &[
            "authorize",
            wire,
            &record,
            "--policy",
            &gov("require_verified.json"),
        ][..],
        &now,
    ]
    .concat());
    assert_eq!(code, 1);

    // Invalid time or policy: invalid input.
    let (code, _) = run(&[
        "authorize",
        wire,
        &record,
        "--policy",
        &gov("require_verified.json"),
        "--now",
        "yesterday",
    ]);
    assert_eq!(code, 2);
    let (code, _) = run(&[
        &["authorize", wire, &record, "--policy", &gov("keys.json")][..],
        &now,
    ]
    .concat());
    assert_eq!(code, 2);

    // Waiver hash and signature agree.
    let a: serde_json::Value = serde_json::from_str(&attestation).unwrap();
    let waiver = serde_json::json!({
        "behavior_version": a["behavior_version"], "finding_hash": a["behavior_version"],
        "profile_hash": a["profile"]["hash"], "verifier_version": a["verifier_version"],
        "rationale": "test",
    });
    let waiver = tmp("waiver.json", &waiver.to_string());
    let (code, hash) = run(&["waiver-hash", &waiver]);
    assert_eq!(code, 0);
    assert!(hash.trim().starts_with("sha256:"));
    let keys: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(gov("keys.json")).unwrap()).unwrap();
    let seed = tmp("seed", keys["A"]["seed"].as_str().unwrap());
    let (code, signed) = run(&["sign-waiver", &waiver, "--seed", &seed]);
    assert_eq!(code, 0);
    let signed: serde_json::Value = serde_json::from_str(&signed).unwrap();
    assert_eq!(signed["waiver_hash"], hash.trim());
    assert_eq!(signed["key_id"], keys["A"]["key_id"]);
}
