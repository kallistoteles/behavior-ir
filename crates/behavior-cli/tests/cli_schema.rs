#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: `behavior schema-hash <wire>` prints a module's SchemaHash (FR-023).

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

fn fixture(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/wire/valid")
        .join(name)
}

fn write_tmp(name: &str, v: &Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("behavior-cli-schema-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, v.to_string()).unwrap();
    p
}

#[test]
fn schema_hash_ignores_behavior_and_follows_declarations() {
    let ledger = fixture("ledger.json");
    let (code, out, err) = run(&["schema-hash", ledger.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let h = out.trim().to_string();
    let m = behavior_core::admit(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    assert_eq!(h, behavior_core::schema(&m).hash);

    let mut w: Value = serde_json::from_str(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    w["actions"] = json!([]);
    let (_, fewer_actions, _) = run(&["schema-hash", write_tmp("a.json", &w).to_str().unwrap()]);
    assert_eq!(fewer_actions.trim(), h);

    w["entities"][0]["fields"][0]["name"] = json!("enabled");
    w["derived"] = json!([]);
    w["constraints"] = json!([]);
    let (code, renamed, err) = run(&["schema-hash", write_tmp("b.json", &w).to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_ne!(renamed.trim(), h);
}

#[test]
fn schema_hash_of_an_inadmissible_module_fails() {
    let p = write_tmp("bad.json", &json!({"ir_version": "0.6"}));
    let (code, _, _) = run(&["schema-hash", p.to_str().unwrap()]);
    assert_eq!(code, 2);
}
