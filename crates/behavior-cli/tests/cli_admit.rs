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

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let n = p.to_string_lossy();
            n.ends_with(".json") && !n.ends_with(".expected.json")
        })
        .collect();
    v.sort();
    v
}

#[test]
fn admit_valid_exits_zero_with_version() {
    let f = fixtures().join("wire/valid/invoice.json");
    let (code, out) = run(&["admit", f.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true);
    let (code, version) = run(&["version", f.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(version.trim(), v["behavior_version"].as_str().unwrap());
    let (code, hashes) = run(&["hashes", f.to_str().unwrap()]);
    assert_eq!(code, 0);
    let h: serde_json::Value = serde_json::from_str(&hashes).unwrap();
    assert_eq!(h, v["items"]);
}

#[test]
fn admit_invalid_exits_two_with_errors() {
    for f in files(&fixtures().join("wire/invalid")) {
        let (code, out) = run(&["admit", f.to_str().unwrap()]);
        assert_eq!(code, 2, "{}", f.display());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert!(v.get("behavior_version").is_none());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(f.with_extension("expected.json")).unwrap(),
        )
        .unwrap();
        let codes: Vec<&serde_json::Value> = v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| &e["code"])
            .collect();
        let want: Vec<&serde_json::Value> = expected["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| &e["code"])
            .collect();
        assert_eq!(codes, want, "{}", f.display());
        let (code, _) = run(&["version", f.to_str().unwrap()]);
        assert_eq!(code, 2);
    }
}

#[test]
fn usage_errors_exit_64() {
    let (code, _) = run(&["admit", "/nonexistent/file.json"]);
    assert_eq!(code, 64);
    let (code, _) = run(&["bogus"]);
    assert_eq!(code, 64);
}
