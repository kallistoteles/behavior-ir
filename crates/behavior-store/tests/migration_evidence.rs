#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: evidence for migrations (FR-019a–c, research R8). The store's evidence policy
//! decides which evidence a migration needs, per transition kind; runtime validation is never
//! optional; the record states what was established.

mod common;

use behavior_core::migration::{Migration, admit_migration};
use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    Evidence, EvidencePolicy, MigrationEvidence, Require, SeedEntity, data_version,
};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use behavior_verify::governance::authorize_migration;
use behavior_verify::hashing::{TAG_VERIFICATION, document_hash};
use common::{bind, fixtures, read};
use serde_json::{Value, json};

const T1: &str = "2026-10-02T10:00:00Z";

fn module(name: &str) -> Module {
    let p = fixtures()
        .join("migration/modules")
        .join(format!("{name}.json"));
    behavior_core::admit(&read(&p)).unwrap()
}

fn migration(v1: &Module, v2: &Module) -> Migration {
    let p = fixtures().join("migration/valid/cultures_v1_to_v2.json");
    admit_migration(v1, v2, &read(&p)).unwrap()
}

fn store(policy: EvidencePolicy) -> Store<InMemoryBackend> {
    let v1 = module("cultures_v1");
    let seed = vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "c1", "name": "Ada", "email": "a@x"}),
        },
        SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                          "legacy_code": "L1", "price": "1.50", "fee": "0.1235"}),
        },
    ];
    Store::create(InMemoryBackend::new(), &v1, genesis_for(&v1, policy, seed)).unwrap()
}

fn policy(require: Require, migration: Option<Require>) -> EvidencePolicy {
    EvidencePolicy {
        require,
        migration: migration.map(|require| MigrationEvidence {
            require,
            trusted_execution_policies: None,
        }),
        ..EvidencePolicy::none()
    }
}

fn execution_policy() -> String {
    read(&fixtures().join("governance/require_verified.json"))
}

/// A verified attestation of `m` (the shape `verify_migration` produces), hashed consistently.
fn attestation(m: &Migration) -> Value {
    let mut a = json!({
        "attestation_version": "1",
        "subject": "migration",
        "migration_hash": m.hash(),
        "source": m.source_schema(),
        "target": m.target_schema(),
        "profile": {"hash": "sha256:00"},
        "verifier_version": behavior_verify::VERIFIER_VERSION,
        "result": "verified",
        "checks": [],
        "findings": [],
    });
    let h = document_hash(TAG_VERIFICATION, &a).unwrap();
    a["hash"] = json!(h);
    a
}

fn authorized<B: Backend>(s: &Store<B>, m: &Migration) -> Evidence {
    let at = s.current().unwrap();
    let dv = data_version(&s.store_id().unwrap(), &at);
    let a = attestation(m);
    let auth = authorize_migration(
        &execution_policy(),
        m,
        &dv,
        Some(&a.to_string()),
        &[],
        &[],
        T1,
    )
    .unwrap();
    assert_eq!(auth.decision, "allow", "{}", auth.to_json_string());
    Evidence {
        authorization: auth.value,
        execution_policy: None,
        attestation: Some(a),
        waivers: Vec::new(),
    }
}

#[test]
fn without_a_migration_section_migrations_need_what_actions_need() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration(&v1, &v2);
    let mut open = store(EvidencePolicy::none());
    open.migrate(&m, &v1, &v2, T1, None).unwrap();
    let mut strict = store(policy(Require::CommitAuthorization, None));
    let err = strict.migrate(&m, &v1, &v2, T1, None).unwrap_err();
    assert_eq!(err.code(), "EVIDENCE_REQUIRED", "{err}");
    let e = authorized(&strict, &m);
    let c = strict.migrate(&m, &v1, &v2, T1, Some(e)).unwrap();
    assert_eq!(c.evidence_trust, Some("structural"));
}

#[test]
fn a_policy_can_demand_more_for_migrations_than_for_actions() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration(&v1, &v2);
    let mut s = store(policy(Require::None, Some(Require::CommitAuthorization)));
    // Actions commit without evidence …
    let ev = s
        .evaluate(
            &v1,
            "kill",
            &bind(&[("culture", "k1")]),
            &json!({}),
            &json!({}),
            T1,
            None,
        )
        .unwrap();
    let b = ev.bundle.unwrap();
    s.commit(&v1, &b.evaluated_state.clone(), &b).unwrap();
    // … migrations do not.
    let err = s.migrate(&m, &v1, &v2, T1, None).unwrap_err();
    assert_eq!(err.code(), "EVIDENCE_REQUIRED", "{err}");
    let e = authorized(&s, &m);
    let auth_hash = e.authorization["hash"].as_str().unwrap().to_string();
    let c = s.migrate(&m, &v1, &v2, T1, Some(e)).unwrap();
    // The record states the policy, the authorization, the cited verification and that runtime
    // validation passed (FR-019c).
    let rec = s
        .backend()
        .record(c.result_state.position)
        .unwrap()
        .unwrap();
    assert_eq!(rec.authorization.as_deref(), Some(auth_hash.as_str()));
    assert_eq!(
        rec.evidence_policy,
        s.genesis().unwrap().evidence_policy.hash().unwrap()
    );
    let mb = rec.migration.unwrap();
    assert!(mb.source_validated && mb.target_validated);
    assert_eq!(mb.verification.unwrap()["result"], json!("verified"));
}

#[test]
fn an_authorization_binds_one_migration_on_one_state() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration(&v1, &v2);
    let mut s = store(policy(Require::None, Some(Require::CommitAuthorization)));
    let stale = authorized(&s, &m);
    let ev = s
        .evaluate(
            &v1,
            "kill",
            &bind(&[("culture", "k1")]),
            &json!({}),
            &json!({}),
            T1,
            None,
        )
        .unwrap();
    let b = ev.bundle.unwrap();
    s.commit(&v1, &b.evaluated_state.clone(), &b).unwrap();
    let err = s.migrate(&m, &v1, &v2, T1, Some(stale)).unwrap_err();
    assert_eq!(err.code(), "EVIDENCE_MISMATCH", "{err}");
    // An authorization of another migration is refused too.
    let mut other_doc: Value = serde_json::from_str(&read(
        &fixtures().join("migration/valid/cultures_v1_to_v2.json"),
    ))
    .unwrap();
    other_doc["transforms"][0]["fields"]["notes"] = json!({
        "op": "lit", "type": {"t": "option", "of": {"t": "string"}}, "value": "n/a",
        "loc": {"file": "m.py", "line": 1},
    });
    let other = admit_migration(&v1, &v2, &other_doc.to_string()).unwrap();
    let wrong = authorized(&s, &other);
    let err = s.migrate(&m, &v1, &v2, T1, Some(wrong)).unwrap_err();
    assert_eq!(err.code(), "EVIDENCE_MISMATCH", "{err}");
}

#[test]
fn authorize_migration_refuses_without_a_verified_attestation_of_that_migration() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration(&v1, &v2);
    let s = store(EvidencePolicy::none());
    let dv = data_version(&s.store_id().unwrap(), &s.current().unwrap());
    let none = authorize_migration(&execution_policy(), &m, &dv, None, &[], &[], T1).unwrap();
    assert_eq!(none.decision, "refuse");
    let mut other = attestation(&m);
    other["migration_hash"] = json!("sha256:ff");
    let h = document_hash(TAG_VERIFICATION, &other).unwrap();
    other["hash"] = json!(h);
    let wrong = authorize_migration(
        &execution_policy(),
        &m,
        &dv,
        Some(&other.to_string()),
        &[],
        &[],
        T1,
    )
    .unwrap();
    assert_eq!(wrong.decision, "refuse", "{}", wrong.to_json_string());
}

#[test]
fn existing_evidence_policies_keep_their_bytes_and_hashes() {
    let v = serde_json::to_value(EvidencePolicy::none()).unwrap();
    assert!(v.get("migration").is_none(), "{v}");
    let p = policy(Require::CommitAuthorization, None);
    let strict_hash = p.hash().unwrap();
    let with_section = policy(
        Require::CommitAuthorization,
        Some(Require::CommitAuthorization),
    );
    assert_ne!(with_section.hash().unwrap(), strict_hash);
    assert!(
        serde_json::to_value(&with_section)
            .unwrap()
            .get("migration")
            .is_some()
    );
}
