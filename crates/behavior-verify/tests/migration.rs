#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: migration verification (FR-020, FR-021, research R9). Local proofs per migrated
//! type under the named source requirements, counterexamples confirmed by running the transform,
//! and honest inconclusive results for whole-state properties outside the model.

mod common;

use behavior_core::migration::{Migration, admit_migration};
use behavior_core::semantic::module::Module;
use behavior_verify::solver::Z3Process;
use behavior_verify::{Profile, verify_migration};
use serde_json::{Value, json};

fn wire(path: &str) -> Value {
    common::json(&common::fixtures().join("migration").join(path))
}

fn admit(w: &Value) -> Module {
    behavior_core::admit(&w.to_string()).unwrap_or_else(|e| panic!("{}", e.to_json_string()))
}

fn loc() -> Value {
    json!({"file": "t.py", "line": 1})
}

fn unique_qty() -> Value {
    json!([{
        "name": "quantities_unique", "loc": loc(),
        "body": {"op": "unique", "param": "o", "loc": loc(),
                 "args": [{"op": "select", "entity": "Order", "loc": loc()}],
                 "body": {"op": "field", "param": "o", "field": "qty", "loc": loc()}},
    }])
}

/// The case's modules and migration, with its variant applied.
fn build(case: &Value) -> (Module, Module, Migration) {
    let mut s = wire(&format!(
        "modules/{}.json",
        case["source"].as_str().unwrap()
    ));
    let mut t = wire(&format!(
        "modules/{}.json",
        case["target"].as_str().unwrap()
    ));
    let mut d = wire(&format!(
        "valid/{}.json",
        case["migration"].as_str().unwrap()
    ));
    match case["variant"].as_str().unwrap() {
        "as_is" => {}
        "no_requirements" => d["requirements"] = json!([]),
        "ph_minus_ten" => {
            d["transforms"][0]["fields"]["ph"] = json!({"op": "sub", "loc": loc(), "args": [
                {"op": "field", "param": "old", "field": "ph", "loc": loc()},
                {"op": "lit", "type": {"t": "int"}, "value": 10, "loc": loc()},
            ]});
        }
        "target_unique_order_qty" => t["invariants"] = unique_qty(),
        "both_unique_order_qty" => {
            s["invariants"] = unique_qty();
            t["invariants"] = unique_qty();
        }
        "target_new_culture_rules" => {
            let rule = |name: &str, bound: &str| {
                json!({"name": name, "entity": "Culture", "param": "c", "loc": loc(),
                "body": {"op": "ge", "loc": loc(), "args": [
                    {"op": "field", "param": "c", "field": "ph", "loc": loc()},
                    {"op": "lit", "type": {"t": "decimal"}, "value": bound, "loc": loc()},
                ]}})
            };
            let cs = t["constraints"].as_array_mut().unwrap();
            cs.push(rule("ph_positive", "1"));
            cs.push(rule("ph_above_minus_one", "-1"));
        }
        other => panic!("unknown variant {other}"),
    }
    let (s, t) = (admit(&s), admit(&t));
    d["source"] = json!(behavior_core::schema(&s).hash);
    d["target"] = json!(behavior_core::schema(&t).hash);
    let m = admit_migration(&s, &t, &d.to_string()).unwrap();
    (s, t, m)
}

#[test]
fn migration_verification_matches_the_fixture() {
    let expected = common::json(&common::fixtures().join("verify/migration.expected.json"));
    let z3 = Z3Process::from_env().unwrap();
    for case in expected["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let (s, t, m) = build(case);
        let a = verify_migration(&m, &s, &t, &Profile::default(), None, &z3);
        let doc = &a.value;
        assert_eq!(doc["subject"], json!("migration"), "{name}");
        assert_eq!(doc["migration_hash"], json!(m.hash()), "{name}");
        let checks = doc["checks"].as_array().unwrap();
        for want in case["checks"].as_array().unwrap() {
            let found = checks
                .iter()
                .find(|c| c["kind"] == want["check"] && c["subject"]["name"] == want["subject"]);
            let Some(c) = found else {
                panic!(
                    "{name}: no {} check of {}: {}",
                    want["check"],
                    want["subject"],
                    serde_json::to_string_pretty(checks).unwrap()
                );
            };
            assert_eq!(c["outcome"], want["outcome"], "{name}: {c}");
            if let Some(under) = want.get("under") {
                let got = c.get("under").cloned().unwrap_or(json!([]));
                assert_eq!(&got, under, "{name}: {c}");
            }
        }
        assert_eq!(
            doc["result"],
            case["result"],
            "{name}: {}",
            a.to_json_string()
        );
    }
}

#[test]
fn counterexamples_are_confirmed_by_running_the_transform() {
    let expected = common::json(&common::fixtures().join("verify/migration.expected.json"));
    let z3 = Z3Process::from_env().unwrap();
    let case = expected["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "narrow_without_requirement")
        .unwrap();
    let (s, t, m) = build(case);
    let a = verify_migration(&m, &s, &t, &Profile::default(), None, &z3);
    let finding = a
        .findings()
        .into_iter()
        .find(|f| f["kind"] == "migration_narrowing")
        .unwrap();
    let cx = &finding["counterexample"];
    assert_eq!(cx["entity"], json!("Order"));
    assert_eq!(cx["value"]["region"], json!(null));
    assert_eq!(cx["refusal"]["code"], json!("MIGRATION_TRANSFORM_ERROR"));
}

#[test]
fn nothing_outside_the_model_is_reported_as_proven() {
    // A module invariant over a type the migration changes is never assumed.
    let expected = common::json(&common::fixtures().join("verify/migration.expected.json"));
    let z3 = Z3Process::from_env().unwrap();
    let case = expected["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "module_invariant_over_a_changed_type")
        .unwrap();
    let (s, t, m) = build(case);
    let a = verify_migration(&m, &s, &t, &Profile::default(), None, &z3);
    assert_eq!(a.result, "not_verified");
    let blocking: Vec<Value> = a
        .findings()
        .into_iter()
        .filter(|f| f["severity"] == "blocking")
        .collect();
    assert!(
        blocking.iter().any(|f| f["kind"] == "inconclusive"),
        "{blocking:?}"
    );
}

#[test]
fn a_migration_attestation_is_content_addressed() {
    let expected = common::json(&common::fixtures().join("verify/migration.expected.json"));
    let z3 = Z3Process::from_env().unwrap();
    let case = &expected["cases"][1];
    let (s, t, m) = build(case);
    let a = verify_migration(&m, &s, &t, &Profile::default(), None, &z3);
    let b = verify_migration(&m, &s, &t, &Profile::default(), None, &z3);
    assert_eq!(a.hash, b.hash);
    assert_eq!(
        a.value["verifier_version"],
        json!(behavior_verify::VERIFIER_VERSION)
    );
    assert_eq!(behavior_verify::VERIFIER_VERSION, "0.5.0");
}
