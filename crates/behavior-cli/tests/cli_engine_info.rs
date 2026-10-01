#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 008: `behavior engine-info` reports every version of the release (research R2).

use std::process::Command;

use serde_json::Value;

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
fn engine_info_reports_every_version() {
    let (code, out) = run(&["engine-info"]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        out.trim_end(),
        behavior_core::canonical::to_canonical_string(&v).unwrap()
    );
    assert_eq!(v["engine"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["wire_ir"], behavior_core::format_versions()["wire_ir"]);
    assert_eq!(v["records"], behavior_core::format_versions()["records"]);
    assert_eq!(v["verifier"], behavior_verify::VERIFIER_VERSION);
    let docs = include_str!("../../behavior-store/src/documents.rs");
    let tags: Vec<&str> = docs
        .lines()
        .filter(|l| l.starts_with("pub const TAG_"))
        .filter_map(|l| l.split('"').nth(1))
        .collect();
    assert!(!tags.is_empty());
    let reported: Vec<&str> = v["store_documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    for t in &tags {
        assert!(reported.contains(t), "{t} missing from {reported:?}");
    }
    assert_eq!(reported.len(), tags.len());
}

#[test]
fn version_of_a_module_is_unchanged() {
    let wire = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/wire/valid/orders.json");
    let (code, out) = run(&["version", wire.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.trim().starts_with("sha256:"), "{out}");
}
