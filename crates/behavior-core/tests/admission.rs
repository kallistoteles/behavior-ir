#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use behavior_core::admission_report;
use serde_json::Value;

#[test]
fn valid_fixtures_are_admitted() {
    for path in common::files(&common::fixtures().join("wire/valid"), ".json") {
        let result = admission_report(&common::read(&path));
        assert!(
            result.ok,
            "{} not admitted: {:?}",
            path.display(),
            result.errors
        );
        let version = result.behavior_version.clone().unwrap();
        assert!(
            version.starts_with("sha256:") && version.len() == 71,
            "{version}"
        );
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "project_margin.json" {
            assert_eq!(result.evaluation_order, vec!["margin", "high_risk"]);
        }
        if name == "empty.json" {
            assert!(result.items.is_empty());
            assert!(result.evaluation_order.is_empty());
        }
    }
}

#[test]
fn invalid_fixtures_report_exact_errors() {
    let dir = common::fixtures().join("wire/invalid");
    let mut failures = Vec::new();
    for path in common::files(&dir, ".json") {
        let expected_path = path.with_extension("expected.json");
        let expected = common::json(&expected_path);
        let result = admission_report(&common::read(&path));
        let json: Value = serde_json::from_str(&result.to_json_string()).unwrap();
        assert_eq!(json["ok"], Value::Bool(false), "{}", path.display());
        assert!(json.get("behavior_version").is_none(), "{}", path.display());
        let got: Vec<Value> = json["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let mut o = serde_json::Map::new();
                o.insert("code".into(), e["code"].clone());
                if let Some(l) = e.get("loc") {
                    o.insert("loc".into(), l.clone());
                }
                Value::Object(o)
            })
            .collect();
        if Value::Array(got.clone()) != expected["errors"] {
            failures.push(format!(
                "{}\n  expected: {}\n  got:      {}\n  full: {}",
                path.display(),
                expected["errors"],
                Value::Array(got),
                json["errors"]
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn cycle_error_names_the_full_cycle() {
    let path = common::fixtures().join("wire/invalid/cycle_a_b_c.json");
    let result = admission_report(&common::read(&path));
    assert_eq!(result.errors.len(), 1);
    let e = &result.errors[0];
    assert_eq!(e.message, "derived values form a cycle: a → b → c → a");
    assert_eq!(e.related_locs.len(), 3);
}

#[test]
fn admission_output_is_byte_identical_across_runs() {
    let dir = common::fixtures().join("wire");
    for sub in ["valid", "invalid"] {
        for path in common::files(&dir.join(sub), ".json") {
            let wire = common::read(&path);
            assert_eq!(
                admission_report(&wire).to_json_string(),
                admission_report(&wire).to_json_string(),
                "{}",
                path.display()
            );
        }
    }
}
