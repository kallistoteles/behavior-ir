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
fn intent_exit_codes() {
    let wire = fixtures().join("wire/valid/invoice.json");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(fixtures().join("intents"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| !p.to_string_lossy().ends_with(".expected.json"))
        .collect();
    entries.sort();
    for intent in entries {
        let name = intent.file_stem().unwrap().to_string_lossy().to_string();
        let host = if name == "valid_denied" {
            "invoice_1042_over_limit"
        } else {
            "invoice_1042"
        };
        let host = fixtures().join(format!("host/{host}.json"));
        let (code, out) = run(&[
            "intent",
            wire.to_str().unwrap(),
            intent.to_str().unwrap(),
            host.to_str().unwrap(),
        ]);
        let want = match name.as_str() {
            "valid_allowed" => 0,
            "valid_denied" => 1,
            _ => 2,
        };
        assert_eq!(code, want, "{name}: {out}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        if want == 2 {
            assert_eq!(v["rejected"], true, "{name}");
        } else {
            assert!(v.get("result").is_some(), "{name}");
        }
    }
}
