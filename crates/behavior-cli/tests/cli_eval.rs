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
fn eval_exit_codes_and_output_match_goldens() {
    let exp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("requests/expectations.json")).unwrap(),
    )
    .unwrap();
    for (name, e) in exp.as_object().unwrap() {
        let wire = fixtures().join(format!("wire/valid/{}.json", e["wire"].as_str().unwrap()));
        let req = fixtures().join(format!("requests/{name}.json"));
        let (code, out) = run(&["eval", wire.to_str().unwrap(), req.to_str().unwrap()]);
        let want = match e["result"].as_str().unwrap() {
            "ALLOW" => 0,
            "DENY" => 1,
            "INVALID_INPUT" | "INVALID_STATE" => 2,
            "ERROR" => 3,
            other => panic!("{other}"),
        };
        assert_eq!(code, want, "{name}");
        let golden =
            std::fs::read_to_string(fixtures().join(format!("records/{name}.json"))).unwrap();
        assert_eq!(out, golden, "{name}");

        let rec = std::env::temp_dir().join(format!("behavior-cli-{name}.json"));
        std::fs::write(&rec, &out).unwrap();
        let (code, out) = run(&["replay", wire.to_str().unwrap(), rec.to_str().unwrap()]);
        assert_eq!((code, out.as_str()), (0, r#"{"matches":true}"#), "{name}");
    }
}

#[test]
fn replay_mismatch_exits_two() {
    let wire = fixtures().join("wire/valid/invoice.json");
    let rec = std::env::temp_dir().join("behavior-cli-tampered.json");
    let golden = std::fs::read_to_string(fixtures().join("records/invoice_allowed.json")).unwrap();
    // Tamper with the recorded outcome only; the stored request stays intact.
    std::fs::write(
        &rec,
        golden.replace("\"result\":\"ALLOW\"", "\"result\":\"DENY\""),
    )
    .unwrap();
    let (code, _) = run(&["replay", wire.to_str().unwrap(), rec.to_str().unwrap()]);
    assert_eq!(code, 2);
}
