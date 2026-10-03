#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US1: guarded commit against one consistent snapshot (FR-001–FR-010, FR-018–FR-022).

mod common;

use behavior_store::documents::{EntityKey, EvidencePolicy, StoreError, data_version};
use behavior_store::muhash::Accumulator;
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use common::*;
use serde_json::json;

#[test]
fn genesis_is_validated_and_gives_s0() {
    let s = store();
    let s0 = s.current().unwrap();
    assert_eq!(s0.position, 0);
    let m = ledger();
    for (bad, why) in [
        (seed(&[("a1", true, "1.005")]), "off-grid money"),
        (seed(&[("a1", true, "-1.00")]), "constraint violation"),
        (
            seed(&[("a1", true, "1.00"), ("a1", true, "2.00")]),
            "duplicate key",
        ),
    ] {
        let g = genesis_for(&m, EvidencePolicy::none(), bad);
        assert!(
            Store::create(InMemoryBackend::new(), &m, g).is_err(),
            "{why}"
        );
    }
    let mut extra = default_seed();
    extra[0].value["color"] = json!("red");
    let g = genesis_for(&m, EvidencePolicy::none(), extra);
    assert!(
        Store::create(InMemoryBackend::new(), &m, g).is_err(),
        "unknown field"
    );
}

#[test]
fn a_transfer_commits_a_new_state() {
    let mut s = store();
    let s0 = s.current().unwrap();
    let ev = transfer(&s, "a1", "a2", "20.00", T0);
    assert_eq!(
        ev.record["data_version"],
        data_version(&s.store_id().unwrap(), &s0).as_str()
    );
    let b = ev.bundle.unwrap();
    let c = commit(&mut s, &b);
    assert!(!c.already);
    assert_eq!(c.result_state.position, 1);
    assert_eq!(s.current().unwrap(), c.result_state);
    assert_eq!(balance(&s, "a1", &c.result_state), json!("80.00"));
    assert_eq!(balance(&s, "a2", &c.result_state), json!("25.00"));
    assert_eq!(
        balance(&s, "a1", &s0),
        json!("100.00"),
        "the past state is unchanged"
    );
    let key = |id: &str| EntityKey {
        entity: "Account".into(),
        id: id.into(),
    };
    assert_eq!(s.load(&key("a1"), &c.result_state).unwrap().revision, 2);
    assert_eq!(s.load(&key("a3"), &c.result_state).unwrap().revision, 1);
    // The identity is a fresh multiset hash of the new content.
    let mut acc = Accumulator::empty();
    for id in ["a1", "a2", "a3"] {
        acc.insert(&s.load(&key(id), &c.result_state).unwrap().content_hash)
            .unwrap();
    }
    assert_eq!(acc.state_id().unwrap(), c.result_state.state);
    let rec = s.backend().record(1).unwrap_or_default().unwrap();
    use behavior_store::Backend as _;
    assert_eq!(rec.previous_record, s.store_id().unwrap());
    assert_eq!(rec.evaluated_against, rec.committed_on);
    assert_eq!(rec.committed_on, s0);
    // The full context snapshot is in the record (FR-022).
    assert_eq!(rec.bundle.as_ref().unwrap().record["context"], json!({}));
}

#[test]
fn outdated_parents_conflict_and_change_nothing() {
    let mut s = store();
    let first = transfer(&s, "a1", "a2", "10.00", T0).bundle.unwrap();
    let disjoint = unary(&s, "freeze", "a3").bundle.unwrap();
    commit(&mut s, &first);
    let head = s.current().unwrap();
    let err = s
        .commit(&ledger(), &disjoint.evaluated_state.clone(), &disjoint)
        .unwrap_err();
    match err {
        StoreError::StateConflict { current, changed } => {
            assert_eq!(current, head);
            let ids: Vec<_> = changed.iter().map(|k| k.id.as_str()).collect();
            assert_eq!(ids, ["a1", "a2"]);
        }
        e => panic!("{e}"),
    }
    assert_eq!(s.current().unwrap(), head);
    use behavior_store::Backend as _;
    assert!(s.backend().record(2).unwrap().is_none());
}

#[test]
fn denied_decisions_have_no_bundle() {
    let mut s = store();
    let ev = transfer(&s, "a2", "a1", "500.00", T0);
    assert_eq!(ev.record["result"], "DENY");
    assert!(ev.bundle.is_none());
    let mut b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    b.record["result"] = json!("DENY");
    let e = s
        .commit(&ledger(), &b.evaluated_state.clone(), &b)
        .unwrap_err();
    assert_eq!(e.code(), "NOTHING_TO_COMMIT");
}

#[test]
fn a_no_op_keeps_the_identity_and_returning_content_repeats_it() {
    let mut s = store();
    let s0 = s.current().unwrap();
    let c = {
        let b = unary(&s, "touch", "a1").bundle.unwrap();
        commit(&mut s, &b)
    };
    assert_eq!(c.result_state.state, s0.state);
    assert_eq!(c.result_state.position, 1);
    // Out and back: the same content at a later position has the same identity.
    let there = {
        let b = transfer(&s, "a1", "a2", "10.00", T0).bundle.unwrap();
        commit(&mut s, &b)
    };
    let back = {
        let b = transfer(&s, "a2", "a1", "10.00", T0).bundle.unwrap();
        commit(&mut s, &b)
    };
    assert_ne!(there.result_state.state, s0.state);
    assert_eq!(back.result_state.state, s0.state);
    assert_eq!(back.result_state.position, 3);
    let key = EntityKey {
        entity: "Account".into(),
        id: "a1".into(),
    };
    assert_eq!(s.load(&key, &back.result_state).unwrap().revision, 3);
}

#[test]
fn resubmission_and_late_retries_are_idempotent() {
    let mut s = store();
    let t1 = transfer(&s, "a1", "a2", "10.00", T0).bundle.unwrap();
    let c1 = commit(&mut s, &t1);
    // Same transition, new commit time: already committed.
    let mut again = t1.clone();
    again.commit_time = "2026-09-27T12:05:00Z".into();
    let r = commit(&mut s, &again);
    assert!(r.already);
    assert_eq!(r.record_id, c1.record_id);
    // Another host commits; the late retry of T1 is still recognized, not a conflict.
    {
        let b = transfer(&s, "a3", "a2", "1.00", T0).bundle.unwrap();
        commit(&mut s, &b)
    };
    let late = commit(&mut s, &t1);
    assert!(late.already);
    assert_eq!(late.result_state, c1.result_state);
    assert_eq!(s.current().unwrap().position, 2, "nothing applied twice");
}

#[test]
fn invalid_bundles_are_refused() {
    let mut s = store();
    let good = transfer(&s, "a1", "a2", "10.00", T0).bundle.unwrap();
    let m = ledger();
    let code = |s: &mut Store<InMemoryBackend>, b: &behavior_store::documents::CommitBundle| {
        s.commit(&m, &b.evaluated_state.clone(), b)
            .unwrap_err()
            .code()
    };
    let mut b = good.clone();
    b.read_set[0].fields.push("active".into());
    b.read_set[0].fields.dedup();
    b.read_set.retain(|r| r.id != "a2");
    assert_eq!(code(&mut s, &b), "BUNDLE_INVALID", "altered read set");
    let mut b = good.clone();
    b.write_set[0].new = json!("0.00");
    assert_eq!(code(&mut s, &b), "BUNDLE_INVALID", "altered write set");
    let mut b = good.clone();
    b.transition_hash = "sha256:00".into();
    assert_eq!(
        code(&mut s, &b),
        "BUNDLE_INVALID",
        "altered transition hash"
    );
    let mut b = good.clone();
    b.record["input"]["amount"] = json!("99.00");
    b.transition_hash = behavior_store::store::transition_hash(&b.record).unwrap();
    assert_eq!(
        code(&mut s, &b),
        "BUNDLE_INVALID",
        "record does not reproduce"
    );
    let mut b = good.clone();
    b.store = "sha256:other".into();
    assert_eq!(code(&mut s, &b), "BUNDLE_INVALID", "another store");
    // Parent mismatch.
    let other = s.current().unwrap();
    let mut wrong = other.clone();
    wrong.position = 7;
    assert_eq!(
        s.commit(&m, &wrong, &good).unwrap_err().code(),
        "BUNDLE_INVALID"
    );
    // The untouched good bundle still commits.
    assert!(!s.commit(&m, &other, &good).unwrap().already);
}

#[test]
fn another_declaration_is_refused() {
    let s = store();
    let mut doc: serde_json::Value =
        serde_json::from_str(&read(&fixtures().join("wire/valid/ledger.json"))).unwrap();
    doc["entities"][0]["fields"][0]["name"] = json!("enabled");
    let text = doc.to_string().replace("\"active\"", "\"enabled\"");
    let m2 = behavior_core::admit(&text).unwrap();
    let e = s
        .evaluate(
            &m2,
            "transfer",
            &bind(&[("from_", "a1"), ("to", "a2")]),
            &json!({"amount": "1.00"}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    assert_eq!(e.code(), "SCHEMA_MISMATCH");
}

#[test]
fn unknown_entities_are_not_found() {
    let s = store();
    let e = s
        .evaluate(
            &ledger(),
            "transfer",
            &bind(&[("from_", "a1"), ("to", "zz")]),
            &json!({"amount": "1.00"}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    assert_eq!(e.code(), "ENTITY_NOT_FOUND");
}

#[test]
fn crashes_show_all_or_nothing_and_retries_apply_once() {
    use behavior_store::conformance::{Fault, FaultInjector};
    use behavior_store::documents::EvidencePolicy;
    let mut s = store_with(
        FaultInjector::new(InMemoryBackend::new()),
        EvidencePolicy::none(),
    );
    let m = ledger();
    // Crash before the write: nothing is visible; the retry applies once.
    let b = transfer(&s, "a1", "a2", "10.00", T0).bundle.unwrap();
    s.backend_mut().next = Some(Fault::BeforeWrite);
    assert_eq!(
        s.commit(&m, &b.evaluated_state.clone(), &b)
            .unwrap_err()
            .code(),
        "BACKEND_ERROR"
    );
    assert_eq!(s.current().unwrap().position, 0);
    assert!(
        !s.commit(&m, &b.evaluated_state.clone(), &b)
            .unwrap()
            .already
    );
    // Crash after the durable write: the retry (new commit time, after another host's commit)
    // is recognized as already committed.
    let b2 = transfer(&s, "a3", "a2", "1.00", T0).bundle.unwrap();
    s.backend_mut().next = Some(Fault::AfterWrite);
    assert!(s.commit(&m, &b2.evaluated_state.clone(), &b2).is_err());
    assert_eq!(s.current().unwrap().position, 2);
    let b3 = unary(&s, "touch", "a1").bundle.unwrap();
    commit(&mut s, &b3);
    let mut retry = b2.clone();
    retry.commit_time = "2026-09-27T12:30:00Z".into();
    let r = s
        .commit(&m, &retry.evaluated_state.clone(), &retry)
        .unwrap();
    assert!(r.already);
    assert_eq!(s.current().unwrap().position, 3, "no duplicate record");
}

#[test]
fn evaluation_reads_one_consistent_snapshot() {
    use behavior_store::Backend as _;
    use behavior_store::conformance::Interleave;
    // Prepare a commit on a copy of the store, to land between the head read and the entity reads.
    let base = store();
    let mut other = Store::open(base.backend().clone()).unwrap();
    let b = transfer(&other, "a1", "a2", "20.00", T0).bundle.unwrap();
    commit(&mut other, &b);
    let rec = other.backend().record(1).unwrap().unwrap();
    let head = other.backend().head().unwrap().unwrap();
    let landing = behavior_store::conformance::Landing::of(base.store_id().unwrap(), rec, head);
    let s = Store::open(Interleave::new(base.into_backend(), landing)).unwrap();
    let s0 = s.state_at(0).unwrap_or_else(|_| s.current().unwrap());
    let ev = transfer(&s, "a1", "a3", "5.00", T0);
    // The commit landed during the reads, yet every read is from S0.
    assert_eq!(s.current().unwrap().position, 1);
    let bundle = ev.bundle.unwrap();
    assert_eq!(bundle.evaluated_state.position, 0);
    assert_eq!(bundle.record["state"]["from_"]["balance"], json!("100.00"));
    assert_eq!(
        bundle
            .read_set
            .iter()
            .find(|r| r.id == "a1")
            .unwrap()
            .revision,
        1
    );
    let _ = s0;
}

#[test]
fn the_module_must_be_the_behavior_that_made_the_bundle() {
    // hash(module) == bundle.behavior_version == record.behavior_version, before any use of the
    // module (including idempotency and re-evaluation).
    let mut s = store();
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_str(&read(&fixtures().join("wire/valid/ledger.json"))).unwrap();
    doc["actions"][2]["name"] = json!("lock");
    let other = behavior_core::admit(&doc.to_string()).unwrap();
    assert_ne!(other.behavior_version(), b.behavior_version);
    let e = s
        .commit(&other, &b.evaluated_state.clone(), &b)
        .unwrap_err();
    assert_eq!(e.code(), "BUNDLE_INVALID");
    assert!(e.to_string().contains(&other.behavior_version()), "{e}");
    // A bundle whose declared version disagrees with its record is refused too.
    let mut forged = b.clone();
    forged.behavior_version = other.behavior_version();
    assert_eq!(
        s.commit(&other, &forged.evaluated_state.clone(), &forged)
            .unwrap_err()
            .code(),
        "BUNDLE_INVALID"
    );
    assert!(!commit(&mut s, &b).already, "the right module commits");
}

#[test]
fn declarations_are_derived_from_the_action_not_the_bundle() {
    // A module that declares `Account` differently (Money without `ratio`) while encoding values
    // the same; a hand-made bundle claims no declarations at all.
    let mut s = store();
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_str(&read(&fixtures().join("wire/valid/ledger.json"))).unwrap();
    doc["nominals"][0]["ops"] = json!(["add", "order", "scale"]);
    let other = behavior_core::admit(&doc.to_string()).unwrap();
    let mut forged = b.clone();
    forged.entity_declarations.clear();
    forged.behavior_version = other.behavior_version();
    forged.record["behavior_version"] = json!(other.behavior_version());
    forged.transition_hash = behavior_store::store::transition_hash(&forged.record).unwrap();
    let e = s
        .commit(&other, &forged.evaluated_state.clone(), &forged)
        .unwrap_err();
    assert_eq!(e.code(), "SCHEMA_MISMATCH", "{e}");
    // With the right module, a bundle whose declaration list is not the action's is refused.
    let mut partial = b.clone();
    partial.entity_declarations.clear();
    assert_eq!(
        s.commit(&ledger(), &partial.evaluated_state.clone(), &partial)
            .unwrap_err()
            .code(),
        "BUNDLE_INVALID"
    );
    assert!(!commit(&mut s, &b).already);
}
