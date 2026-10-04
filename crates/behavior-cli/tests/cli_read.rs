#![allow(clippy::unwrap_used, clippy::expect_used)]

//! `behavior read` and `behavior read-replay` (feature 010, FR-019): plain reads print their
//! record with the CLI's exit-code convention; read records replay.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/reads")
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
fn read_prints_the_golden_record_with_its_exit_code() {
    let lab = fixtures().join("modules/lab.json");
    let mut n = 0;
    for entry in std::fs::read_dir(fixtures().join("requests")).unwrap() {
        let req = entry.unwrap().path();
        let name = req.file_stem().unwrap().to_string_lossy().to_string();
        let (code, out) = run(&["read", lab.to_str().unwrap(), req.to_str().unwrap()]);
        let golden =
            std::fs::read_to_string(fixtures().join(format!("records/{name}.expected.json")))
                .unwrap();
        assert_eq!(out, golden, "{name}");
        let record: serde_json::Value = serde_json::from_str(&out).unwrap();
        let want = match record["result"].as_str().unwrap() {
            "VALUE" => 0,
            "INVALID_INPUT" | "INVALID_BINDING" => 2,
            "EVALUATION_ERROR" => 3,
            other => panic!("{other}"),
        };
        assert_eq!(code, want, "{name}");

        let rec = std::env::temp_dir().join(format!("behavior-cli-read-{name}.json"));
        std::fs::write(&rec, &out).unwrap();
        let (code, out) = run(&["read-replay", lab.to_str().unwrap(), rec.to_str().unwrap()]);
        assert_eq!((code, out.as_str()), (0, r#"{"matches":true}"#), "{name}");
        n += 1;
    }
    assert!(n >= 8);
}

#[test]
fn a_tampered_read_record_does_not_replay() {
    let lab = fixtures().join("modules/lab.json");
    let golden = std::fs::read_to_string(fixtures().join("records/value.expected.json")).unwrap();
    let rec = std::env::temp_dir().join("behavior-cli-read-tampered.json");
    std::fs::write(&rec, golden.replace("\"value\":2", "\"value\":3")).unwrap();
    let (code, out) = run(&["read-replay", lab.to_str().unwrap(), rec.to_str().unwrap()]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("\"matches\":false"), "{out}");
}

#[test]
fn a_malformed_read_request_is_refused() {
    let lab = fixtures().join("modules/lab.json");
    let req = std::env::temp_dir().join("behavior-cli-read-bad.json");
    std::fs::write(&req, r#"{"data_version": "x"}"#).unwrap();
    let (code, out) = run(&["read", lab.to_str().unwrap(), req.to_str().unwrap()]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("DECODE_ERROR"), "{out}");
}

#[test]
fn read_intent_prints_only_the_response_and_writes_the_record() {
    let lab = fixtures().join("modules/lab.json");
    let intents = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/read_intents");
    let host = intents.join("host.json");
    let record = std::env::temp_dir().join("behavior-cli-read-intent-record.json");
    let _ = std::fs::remove_file(&record);
    let (code, out) = run(&[
        "read-intent",
        lab.to_str().unwrap(),
        intents.join("valid_projection.json").to_str().unwrap(),
        host.to_str().unwrap(),
        "--record",
        record.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}");
    let response: serde_json::Value = serde_json::from_str(&out).unwrap();
    let keys: Vec<&str> = response
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["record_id", "result", "value"]);
    assert!(!out.contains("credit_limit"), "{out}");
    let rec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(rec["record_id"], response["record_id"]);
    assert!(
        rec["observed"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(["customer", "credit_limit"]))
    );

    // A rejected intent prints its problems and exits 2.
    let (code, out) = run(&[
        "read-intent",
        lab.to_str().unwrap(),
        intents.join("multiple_problems.json").to_str().unwrap(),
        host.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("\"rejected\":true"), "{out}");
}
