#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Persistence documents (data-model.md): canonical hashing vectors, strict decoding, and the
//! separation of content (state identity) from history (revision, position).

mod common;

use std::collections::BTreeMap;

use behavior_store::documents::*;
use serde_json::{Value, json};

fn policy(require: Require) -> EvidencePolicy {
    EvidencePolicy {
        require,
        ..EvidencePolicy::none()
    }
}

fn genesis() -> Genesis {
    Genesis {
        format: TAG_GENESIS.into(),
        evidence_policy: EvidencePolicy::none(),
        entity_declarations: BTreeMap::from([("Account".into(), "sha256:aa".into())]),
        seed: vec![SeedEntity {
            entity: "Account".into(),
            value: json!({"id": "a1", "active": true, "balance": "100.00"}),
        }],
    }
}

fn content() -> EntityContent {
    EntityContent {
        entity: "Account".into(),
        declaration: "sha256:aa".into(),
        id: "a1".into(),
        value: json!({"id": "a1", "active": true, "balance": "100.00"}),
    }
}

fn version(revision: u64, created_at: u64) -> EntityVersion {
    EntityVersion {
        content_hash: content().hash().unwrap(),
        entity: "Account".into(),
        id: "a1".into(),
        revision,
        created_at,
        value: content().value,
    }
}

fn state_ref() -> StateRef {
    StateRef {
        state: "sha256:bb".into(),
        position: 3,
    }
}

fn bundle() -> CommitBundle {
    CommitBundle {
        format: TAG_COMMIT_BUNDLE.into(),
        evaluated_state: state_ref(),
        behavior_version: "sha256:cc".into(),
        store: "sha256:dd".into(),
        record: json!({"result": "ALLOW"}),
        transition_hash: "sha256:ee".into(),
        entity_declarations: BTreeMap::from([("Account".into(), "sha256:aa".into())]),
        read_set: vec![ReadEntry {
            entity: "Account".into(),
            id: "a1".into(),
            revision: 1,
            fields: vec!["balance".into()],
        }],
        write_set: vec![WriteEntry {
            entity: "Account".into(),
            id: "a1".into(),
            field: "balance".into(),
            old: json!("100.00"),
            new: json!("90.00"),
        }],
        read_facts: serde_json::Value::Null,
        write_lifecycle: Vec::new(),
        commit_time: "2026-09-27T12:00:00Z".into(),
        evidence: None,
    }
}

fn record() -> TransitionRecord {
    TransitionRecord {
        format: TAG_TRANSITION_RECORD.into(),
        position: 4,
        previous_record: "sha256:ff".into(),
        bundle: bundle(),
        bundle_hash: bundle().hash().unwrap(),
        evaluated_against: state_ref(),
        committed_on: state_ref(),
        result_state: StateRef {
            state: "sha256:11".into(),
            position: 4,
        },
        new_versions: vec![version(2, 4)],
        evidence_policy: EvidencePolicy::none().hash().unwrap(),
        authorization: None,
        created: Vec::new(),
        removed: Vec::new(),
        ref_changes: Vec::new(),
    }
}

#[test]
fn hashing_vectors_are_frozen() {
    let got = json!({
        "entity_content": content().hash().unwrap(),
        "entity_version": version(1, 0).hash().unwrap(),
        "evidence_policy_none": EvidencePolicy::none().hash().unwrap(),
        "evidence_policy_authorization": policy(Require::CommitAuthorization).hash().unwrap(),
        "genesis": genesis().hash().unwrap(),
        "commit_bundle": bundle().hash().unwrap(),
        "transition_record": record().hash().unwrap(),
        "data_version": data_version("sha256:dd", &state_ref()),
    });
    let path = common::fixtures().join("store/hash_vectors.json");
    if std::env::var("BLESS_STORE_VECTORS").is_ok() {
        std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&common::read(&path)).unwrap();
    assert_eq!(got, want, "store hash vectors changed");
}

#[test]
fn content_excludes_history() {
    // Two incarnations with equal content: same content hash, different version hashes.
    assert_eq!(version(1, 0).content_hash, version(7, 42).content_hash);
    assert_ne!(
        version(1, 0).hash().unwrap(),
        version(7, 42).hash().unwrap()
    );
    // The declaration and the id are part of the content.
    let mut other_decl = content();
    other_decl.declaration = "sha256:ab".into();
    assert_ne!(content().hash().unwrap(), other_decl.hash().unwrap());
    let mut other_id = content();
    other_id.id = "a2".into();
    assert_ne!(content().hash().unwrap(), other_id.hash().unwrap());
}

#[test]
fn decoding_is_strict() {
    let mut v = serde_json::to_value(bundle()).unwrap();
    v["extra"] = json!(1);
    assert!(decode::<CommitBundle>("bundle", &v).is_err());
    let bad_policy = json!({"format": TAG_EVIDENCE_POLICY, "require": "sometimes"});
    assert!(decode::<EvidencePolicy>("policy", &bad_policy).is_err());
    let ok_policy = json!({"format": TAG_EVIDENCE_POLICY, "require": "commit_authorization",
                           "trusted_execution_policies": ["sha256:12"]});
    assert!(decode::<EvidencePolicy>("policy", &ok_policy).is_ok());
    let round: CommitBundle = decode("bundle", &serde_json::to_value(bundle()).unwrap()).unwrap();
    assert_eq!(round, bundle());
}

#[test]
fn timestamps_are_rfc3339_utc() {
    assert!(valid_timestamp("2026-09-27T12:00:00Z"));
    for bad in [
        "2026-09-27 12:00:00Z",
        "2026-09-27T12:00:00+02:00",
        "yesterday",
        "",
    ] {
        assert!(!valid_timestamp(bad), "{bad}");
    }
}
