#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3 (feature 006) in the store: seed references, the derived reverse-reference index as of a
//! position, and referential integrity on S'.

mod common;

use behavior_store::documents::EvidencePolicy;
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use common::*;
use serde_json::json;

#[test]
fn a_seed_reference_must_point_at_a_seed_entity() {
    let m = accounts();
    let mut seed = accounts_seed();
    seed.push(account("a9", "c9", "0.00"));
    let err = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .err()
    .unwrap();
    assert_eq!(err.code(), "GENESIS_INVALID", "{err}");
    // A plain identity may name anything.
    let mut seed = accounts_seed();
    seed.push(note("n9", "c9"));
    assert!(
        Store::create(
            InMemoryBackend::new(),
            &m,
            genesis_for(&m, EvidencePolicy::none(), seed)
        )
        .is_ok()
    );
}

#[test]
fn the_index_follows_creations_retargets_and_removals() {
    let mut s = accounts_store();
    let b = s.backend();
    assert_eq!(
        b.incoming_at(&key("Customer", "c1"), 0).unwrap(),
        [edge("Account", "a1", "owner")]
    );
    assert!(b.incoming_at(&key("Customer", "c2"), 0).unwrap().is_empty());

    // Creation adds an edge.
    let e = act(
        &s,
        "open_account",
        &[("owner", "c2")],
        json!({"account_id": "a2", "initial": "1.00"}),
    );
    commit_acc(&mut s, &e);
    assert_eq!(
        s.backend().incoming_at(&key("Customer", "c2"), 1).unwrap(),
        [edge("Account", "a2", "owner")]
    );

    // Retarget and remove in one transition: valid on S'.
    let e = act(
        &s,
        "switch_and_remove",
        &[("account", "a1"), ("old", "c1"), ("new", "c2")],
        json!({}),
    );
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    assert_eq!(
        e.bundle.as_ref().unwrap().read_facts["references"][0]["incoming"],
        json!([{"entity": "Account", "id": "a1", "field": "owner"}])
    );
    commit_acc(&mut s, &e);
    let rec = s.backend().record(2).unwrap().unwrap();
    let ops: Vec<(String, String)> = rec
        .ref_changes
        .iter()
        .map(|c| (c.target.id.clone(), c.op.clone()))
        .collect();
    assert_eq!(
        ops,
        [
            ("c1".to_string(), "drop".to_string()),
            ("c2".to_string(), "add".to_string())
        ]
    );
    let b = s.backend();
    assert!(b.incoming_at(&key("Customer", "c1"), 2).unwrap().is_empty());
    assert_eq!(
        b.incoming_at(&key("Customer", "c1"), 1).unwrap(),
        [edge("Account", "a1", "owner")]
    );
    assert_eq!(
        b.incoming_at(&key("Customer", "c2"), 2).unwrap(),
        [
            edge("Account", "a1", "owner"),
            edge("Account", "a2", "owner")
        ]
    );

    // Removing a referenced customer is refused by the decision itself.
    let e = act(
        &s,
        "remove_customer_unchecked",
        &[("customer", "c2")],
        json!({}),
    );
    assert_eq!(e.record["result"], "DENY");
    assert_eq!(e.record["reasons"][0]["code"], "DANGLING_REFERENCE");

    // Closing a1 drops its edge; a2 still references c2.
    let e = act(&s, "close_account", &[("account", "a1")], json!({}));
    commit_acc(&mut s, &e);
    let at = s.current().unwrap().position;
    assert_eq!(
        s.backend().incoming_at(&key("Customer", "c2"), at).unwrap(),
        [edge("Account", "a2", "owner")]
    );
}

#[test]
fn a_reference_read_is_rechecked_at_commit() {
    let mut s = accounts_store();
    let e = act(&s, "remove_customer", &[("customer", "c3")], json!({}));
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let b = e.bundle.clone().unwrap();
    let mut forged = b.clone();
    forged.read_facts = json!({"references": [{"entity": "Customer", "id": "c3", "incoming": [
        {"entity": "Account", "id": "a1", "field": "owner"}]}]});
    assert_eq!(
        s.commit(&accounts(), &forged.evaluated_state.clone(), &forged)
            .unwrap_err()
            .code(),
        "BUNDLE_INVALID"
    );
    commit_acc(&mut s, &e);
    assert_eq!(
        s.backend().removed_at(&key("Customer", "c3")).unwrap(),
        Some(1)
    );
}

#[test]
fn seed_constraints_may_use_existence_and_references() {
    // A user-written constraint over a plain optional identity, answered from the seed itself.
    let l = json!({"file": "t.py", "line": 1});
    let parent = json!({"op": "field", "param": "n", "field": "parent", "loc": l});
    let own_id = json!({"op": "field", "param": "n", "field": "id", "loc": l});
    let wire = json!({
        "ir_version": "0.5", "enums": [], "nominals": [], "derived": [], "invariants": [],
        "actions": [],
        "entities": [{"name": "Node", "loc": l, "fields": [
            {"name": "parent", "type": {"t": "option", "of": {"t": "id", "entity": "Node"}}, "loc": l}]}],
        "constraints": [
            {"name": "parent_alive", "entity": "Node", "param": "n", "loc": l,
             "body": {"op": "or", "loc": l, "args": [
                 {"op": "is_none", "args": [parent.clone()], "loc": l},
                 {"op": "exists", "args": [parent], "loc": l}]}},
            {"name": "self_exists", "entity": "Node", "param": "n", "loc": l,
             "body": {"op": "exists", "args": [own_id], "loc": l}},
        ],
    });
    let m = behavior_core::admit(&wire.to_string()).unwrap();
    let node = |id: &str, parent: Option<&str>| behavior_store::documents::SeedEntity {
        entity: "Node".into(),
        value: json!({"id": id, "parent": parent}),
    };
    let ok = vec![node("n1", None), node("n2", Some("n1"))];
    assert!(
        Store::create(
            InMemoryBackend::new(),
            &m,
            genesis_for(&m, EvidencePolicy::none(), ok)
        )
        .is_ok()
    );
    let orphan = vec![node("n1", None), node("n3", Some("n9"))];
    let err = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), orphan),
    )
    .err()
    .unwrap();
    assert_eq!(err.code(), "GENESIS_INVALID");
    assert!(err.to_string().contains("parent_alive"), "{err}");
}
