#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US2 (feature 006) in the store: removal takes the entity out of the state, keeps its history,
//! and never frees its identity.

mod common;

use behavior_store::Backend;
use common::*;
use serde_json::json;

#[test]
fn a_removal_keeps_history_and_frees_nothing() {
    let mut s = accounts_store();
    let before = s.current().unwrap();
    let old = s.load(&key("Account", "a1"), &before).unwrap();
    let e = act(&s, "close_account", &[("account", "a1")], json!({}));
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let c = commit_acc(&mut s, &e);
    let rec = s.backend().record(1).unwrap().unwrap();
    assert_eq!(rec.removed.len(), 1);
    assert_eq!(rec.removed[0].last_revision, 1);
    assert_eq!(rec.removed[0].last_content_hash, old.content_hash);
    assert!(rec.new_versions.is_empty() && rec.created.is_empty());
    assert_eq!(
        s.backend().removed_at(&key("Account", "a1")).unwrap(),
        Some(1)
    );
    // No placeholder version is written.
    assert!(
        s.backend()
            .version(&key("Account", "a1"), 2)
            .unwrap()
            .is_none()
    );

    assert_eq!(
        s.load(&key("Account", "a1"), &c.result_state)
            .unwrap_err()
            .code(),
        "ENTITY_NOT_FOUND"
    );
    assert_eq!(s.load(&key("Account", "a1"), &before).unwrap(), old);

    // The state identity no longer contains the account: the same as a genesis without it.
    let m = accounts();
    let without = behavior_store::Store::create(
        behavior_store::InMemoryBackend::new(),
        &m,
        behavior_store::store::genesis_for(
            &m,
            behavior_store::documents::EvidencePolicy::none(),
            accounts_seed()
                .into_iter()
                .filter(|x| x.value["id"] != "a1")
                .collect(),
        ),
    )
    .unwrap();
    assert_eq!(c.result_state.state, without.current().unwrap().state);

    // An identity names one lifetime (SC-007).
    let e = act(
        &s,
        "open_account",
        &[("owner", "c1")],
        json!({"account_id": "a1", "initial": "0.00"}),
    );
    assert_eq!(e.record["result"], "ENTITY_ID_ALREADY_USED", "{}", e.record);
    // A removed entity cannot be bound any more.
    let err = s
        .evaluate(
            &accounts(),
            "deposit",
            &bind(&[("account", "a1")]),
            &json!({"amount": "1.00"}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    assert_eq!(err.code(), "ENTITY_NOT_FOUND");
}
