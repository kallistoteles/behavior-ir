#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Every capability a binding needs (feature 011, FR-006b), reached through `behavior_engine`
//! alone: build and admit modules, evaluate and read, use stores, migrate, verify, authorize,
//! and report versions. The conformance fixtures are the core's own (`../tests/fixtures`).

use std::path::PathBuf;

use behavior_engine::read::ReadSource;
use behavior_engine::store::InMemoryBackend;
use behavior_engine::{admission_result, admit, evaluate, evaluate_intent, replay};
use serde_json::{Value, json};

fn fixture(path: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn builds_and_admits_a_module() {
    let _builder = behavior_engine::builder::Builder::new();
    let m = admit(&fixture("wire/valid/invoice.json")).unwrap();
    let report = admission_result(&m);
    assert!(report.ok, "{:?}", report.errors);
    let again = admit(&behavior_engine::serialize::to_wire_json(&m)).unwrap();
    assert_eq!(admission_result(&again).behavior_version, report.behavior_version);
}

#[test]
fn evaluates_and_replays_a_decision() {
    let m = admit(&fixture("wire/valid/invoice.json")).unwrap();
    let record = evaluate(&m, &fixture("requests/discount_breaks_invariant.json"));
    let text = record.to_json_string();
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["result"], "DENY");
    assert!(replay(&m, &text).matches);
}

#[test]
fn runs_a_declared_read() {
    let m = admit(&fixture("reads/modules/lab.json")).unwrap();
    let x = behavior_engine::read::evaluate_read(
        &m,
        &ReadSource::Declared("active_count".into()),
        &json!({"data_version": "consumer:1", "state": {}, "input": {}, "context": {},
                "facts": {"universe": [{"entity": "Culture", "members": []}]}})
        .to_string(),
    );
    assert_eq!(x.record.as_json()["result"], "VALUE");
}

#[test]
fn a_store_passes_the_persistence_conformance_suite() {
    let report = behavior_engine::store::conformance::run(InMemoryBackend::new);
    let failed: Vec<_> = report.cases.iter().filter(|c| !c.ok).collect();
    assert!(!report.cases.is_empty());
    assert!(failed.is_empty(), "{:?}", failed.iter().map(|c| c.name).collect::<Vec<_>>());
}

#[test]
fn admits_a_migration() {
    let src = admit(&fixture("migration/modules/cultures_v1.json")).unwrap();
    let dst = admit(&fixture("migration/modules/cultures_v2.json")).unwrap();
    let migration = behavior_engine::migration::admit_migration(
        &src,
        &dst,
        &fixture("migration/valid/cultures_v1_to_v2.json"),
    );
    assert!(migration.is_ok());
}

#[test]
fn verifies_when_a_solver_is_present() {
    use behavior_engine::verify::{Profile, solver::Z3Process, verify};
    let m = admit(&fixture("wire/valid/invoice.json")).unwrap();
    match Z3Process::from_env() {
        Ok(z3) => {
            let att = verify(&m, &Profile::default(), None, &z3);
            assert!(att.value["behavior_version"].is_string(), "{}", att.value);
        }
        // Without the solver verification is unavailable, and nothing else is affected.
        Err(_) => assert!(admit(&fixture("wire/valid/invoice.json")).is_ok()),
    }
}

#[test]
fn authorizes_an_intent() {
    let m = admit(&fixture("wire/valid/invoice.json")).unwrap();
    let record = evaluate_intent(
        &m,
        &fixture("intents/valid_allowed.json"),
        &fixture("host/invoice_1042.json"),
    )
    .unwrap();
    let v: Value = serde_json::from_str(&record.to_json_string()).unwrap();
    assert_eq!(v["result"], "ALLOW");
}

#[test]
fn reports_its_versions() {
    let info = behavior_engine::engine_info();
    for key in ["engine", "wire_ir", "records", "store_documents", "verifier"] {
        assert!(info.get(key).is_some(), "{key} missing from {info}");
    }
    assert_eq!(info["verifier"], behavior_engine::verify::VERIFIER_VERSION);
}
