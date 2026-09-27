#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US1 (feature 006) in the store: creations are checked against the identity registry,
//! committed at revision 1, and insert their content into the state identity.

mod common;

use behavior_store::Backend;
use behavior_store::muhash::Accumulator;
use common::*;
use serde_json::json;

#[test]
fn a_creation_commits_version_one_and_extends_the_state() {
    let mut s = accounts_store();
    let parent = s.current().unwrap();
    let e = act(
        &s,
        "open_account",
        &[("owner", "c1")],
        json!({"account_id": "a42", "initial": "100.00"}),
    );
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let b = e.bundle.clone().unwrap();
    // `used` was answered by the backend and observed.
    assert_eq!(
        b.read_facts,
        json!({"identities": [{"entity": "Account", "id": "a42", "used": false}]})
    );
    assert_eq!(b.write_lifecycle.len(), 1);
    assert_eq!(
        b.entity_declarations.len(),
        2,
        "Customer (bound) and Account (created)"
    );
    let c = commit_acc(&mut s, &e);
    let rec = s.backend().record(1).unwrap().unwrap();
    assert_eq!(rec.created.len(), 1);
    let v = &rec.created[0];
    assert_eq!((v.revision, v.created_at), (1, 1));
    assert_eq!(
        v.value,
        json!({"id": "a42", "owner": "c1", "balance": "100.00"})
    );

    // The state identity inserts exactly the new content hash (checked against a fresh MuHash).
    let genesis = s.genesis().unwrap();
    let mut acc = Accumulator::empty();
    for seed in &genesis.seed {
        let id = seed.value["id"].as_str().unwrap();
        let sv = s.load(&key(&seed.entity, id), &parent).unwrap();
        acc.insert(&sv.content_hash).unwrap();
    }
    acc.insert(&v.content_hash).unwrap();
    assert_eq!(c.result_state.state, acc.state_id().unwrap());

    assert_eq!(
        s.load(&key("Account", "a42"), &c.result_state)
            .unwrap()
            .revision,
        1
    );
    assert_eq!(
        s.load(&key("Account", "a42"), &parent).unwrap_err().code(),
        "ENTITY_NOT_FOUND"
    );
}

#[test]
fn an_identity_used_in_the_genesis_cannot_be_created() {
    let s = accounts_store();
    let e = act(
        &s,
        "open_account",
        &[("owner", "c1")],
        json!({"account_id": "a1", "initial": "1.00"}),
    );
    assert_eq!(e.record["result"], "ENTITY_ID_ALREADY_USED");
    assert!(e.bundle.is_none());
}

#[test]
fn records_without_lifecycle_keep_the_005_shape() {
    let mut s = accounts_store();
    let e = act(
        &s,
        "deposit",
        &[("account", "a1")],
        json!({"amount": "5.00"}),
    );
    commit_acc(&mut s, &e);
    let rec = s.backend().record(1).unwrap().unwrap();
    let v = serde_json::to_value(&rec).unwrap();
    for k in ["created", "removed", "ref_changes"] {
        assert!(v.get(k).is_none(), "{k}: {v}");
    }
    let b = serde_json::to_value(&rec.bundle).unwrap();
    assert!(
        b.get("read_facts").is_none() && b.get("write_lifecycle").is_none(),
        "{b}"
    );
    assert_eq!(rec.bundle.record["record_version"], "0.4");
}

#[test]
fn the_registry_is_keyed_by_type_name_and_id() {
    let mut s = accounts_store();
    let e = act(
        &s,
        "open_account",
        &[("owner", "c1")],
        json!({"account_id": "x7", "initial": "1.00"}),
    );
    commit_acc(&mut s, &e);
    // The same id for another entity type is a different identity.
    let e = act(
        &s,
        "register_customer",
        &[],
        json!({"customer_id": "x7", "name": "New"}),
    );
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let at = s.current().unwrap().position;
    assert!(s.backend().used_at(&key("Account", "x7"), at).unwrap());
    assert!(!s.backend().used_at(&key("Customer", "x7"), at).unwrap());
}

#[test]
fn an_existence_read_is_in_the_read_facts_and_rechecked_at_commit() {
    let mut s = accounts_store();
    let e = act(&s, "check_exists", &[("note", "n1")], json!({}));
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let b = e.bundle.clone().unwrap();
    assert_eq!(
        b.read_facts,
        json!({"existence": [{"entity": "Customer", "id": "c1", "exists": true}]})
    );
    let mut forged = b.clone();
    forged.read_facts = json!({"existence": [{"entity": "Customer", "id": "c1", "exists": false}]});
    let err = s
        .commit(&accounts(), &forged.evaluated_state.clone(), &forged)
        .unwrap_err();
    assert_eq!(err.code(), "BUNDLE_INVALID", "{err}");
    assert!(
        !s.commit(&accounts(), &b.evaluated_state.clone(), &b)
            .unwrap()
            .already
    );
}
