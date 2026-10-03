#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US2: history queries, data and behavior replay, snapshot verification, and tamper detection at
//! the first affected position (FR-011, FR-012, SC-002).

mod common;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    EntityKey, EntityVersion, Genesis, Head, StateRef, TransitionRecord,
};
use behavior_store::replay::{replay_behavior, replay_data, verify_snapshot};
use behavior_store::store::transition_hash;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, Store};
use common::*;
use proptest::prelude::*;

/// A deterministic history of `n` committed ledger transitions.
fn history(n: u64) -> Store<InMemoryBackend> {
    let mut s = store();
    let ids = ["a1", "a2", "a3"];
    let mut x: u64 = 7;
    while s.current().unwrap().position < n {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let (f, t) = (ids[(x >> 33) as usize % 3], ids[(x >> 40) as usize % 3]);
        let ev = if f == t {
            unary(&s, "touch", f)
        } else {
            transfer(
                &s,
                f,
                t,
                &format!("{}.{:02}", (x >> 20) % 7, (x >> 10) % 100),
                T0,
            )
        };
        if let Some(b) = ev.bundle {
            commit(&mut s, &b);
        }
    }
    s
}

static HISTORY: LazyLock<Store<InMemoryBackend>> = LazyLock::new(|| history(1000));

fn modules() -> BTreeMap<String, Module> {
    let m = ledger();
    BTreeMap::from([(m.behavior_version(), m)])
}

/// A backend that alters what it returns for records.
struct Tamper<'a> {
    inner: &'a InMemoryBackend,
    alter: Box<dyn Fn(u64) -> Option<Option<TransitionRecord>> + 'a>,
}

impl Backend for Tamper<'_> {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.inner.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.inner.head()
    }
    fn create(
        &mut self,
        _: &Genesis,
        _: &Head,
        _: &[EntityVersion],
        _: &[behavior_store::documents::RefChange],
    ) -> Result<(), BackendError> {
        Err(BackendError("read-only".into()))
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        self.inner.keys_at(t, p)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        self.inner.keys_by_field_at(t, f, v, p)
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(k)
    }
    fn incoming_at(
        &self,
        t: &EntityKey,
        p: u64,
    ) -> Result<Vec<behavior_store::RefEdge>, BackendError> {
        self.inner.incoming_at(t, p)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        match (self.alter)(p) {
            Some(r) => Ok(r),
            None => self.inner.record(p),
        }
    }
    fn commit(
        &mut self,
        _: &str,
        _: &[EntityVersion],
        _: &[EntityKey],
        _: &[behavior_store::documents::RefChange],
        _: &TransitionRecord,
        _: &Head,
    ) -> Result<CasOutcome, BackendError> {
        Err(BackendError("read-only".into()))
    }
}

fn ends(s: &Store<impl Backend>) -> (StateRef, StateRef) {
    (s.state_at(0).unwrap(), s.current().unwrap())
}

#[test]
fn a_clean_history_replays_both_ways() {
    let s = &*HISTORY;
    let (from, to) = ends(s);
    let d = replay_data(s, &from, &to);
    assert!(d.ok, "{d:?}");
    assert_eq!(d.checked, 1000);
    let b = replay_behavior(s, &modules(), &from, &to);
    assert!(b.ok, "{b:?}");
    assert_eq!(b.checked, 1000);
    // History queries return exactly the records, in order.
    let mid = s.state_at(400).unwrap();
    let recs = s.transitions(&mid, &s.state_at(410).unwrap()).unwrap();
    assert_eq!(
        recs.iter().map(|r| r.position).collect::<Vec<_>>(),
        (401..=410).collect::<Vec<_>>()
    );
    // A missing module is reported, not skipped.
    let none = replay_behavior(s, &BTreeMap::new(), &from, &to);
    assert_eq!(none.divergence.unwrap().kind, "record");
}

#[test]
fn snapshots_are_verified_not_trusted() {
    let s = &*HISTORY;
    let at = s.state_at(500).unwrap();
    let keys = ["a1", "a2", "a3"].map(|id| EntityKey {
        entity: "Account".into(),
        id: id.into(),
    });
    let snap: Vec<EntityVersion> = keys.iter().map(|k| s.load(k, &at).unwrap()).collect();
    assert!(verify_snapshot(s, &at, &snap).ok);
    let mut wrong = snap.clone();
    wrong[0] = s.load(&keys[0], &s.state_at(499).unwrap()).unwrap();
    if wrong[0] != snap[0] {
        assert!(!verify_snapshot(s, &at, &wrong).ok);
    }
    let mut forged = snap.clone();
    forged[1].value["balance"] = serde_json::json!("999.99");
    assert!(!verify_snapshot(s, &at, &forged).ok);
    assert_eq!(s.current().unwrap().position, 1000, "unchanged");
}

#[derive(Debug, Clone, Copy)]
enum Tampering {
    WriteValue,
    Input,
    InputRehashed,
    Context,
    ResultState,
    BehaviorVersion,
    Swap,
    Drop,
    Invariant,
}

fn tampered(kind: Tampering, k: u64) -> impl Fn(u64) -> Option<Option<TransitionRecord>> {
    let base = &*HISTORY;
    move |p| {
        let rec = |q| base.backend().record(q).unwrap();
        match kind {
            Tampering::Swap if p == k => Some(rec(k + 1)),
            Tampering::Swap if p == k + 1 => Some(rec(k)),
            Tampering::Drop if p >= k => Some(rec(p + 1)),
            _ if p != k => None,
            _ => {
                let mut r = rec(k).unwrap();
                match kind {
                    Tampering::WriteValue => {
                        if let Some(w) = r.bundle.as_mut().unwrap().write_set.first_mut() {
                            w.new = serde_json::json!("0.00");
                        } else {
                            r.bundle.as_mut().unwrap().commit_time = "2000-01-01T00:00:00Z".into();
                        }
                    }
                    Tampering::Input => {
                        r.bundle.as_mut().unwrap().record["input"] =
                            serde_json::json!({"amount": "0.01"})
                    }
                    Tampering::InputRehashed => {
                        r.bundle.as_mut().unwrap().record["input"] =
                            serde_json::json!({"amount": "0.02"});
                        r.bundle.as_mut().unwrap().transition_hash =
                            transition_hash(&r.bundle.as_ref().unwrap().record).unwrap();
                        r.bundle_hash = r.bundle.as_ref().unwrap().hash().unwrap();
                    }
                    Tampering::Context => {
                        r.bundle.as_mut().unwrap().record["context"] = serde_json::json!({"x": 1})
                    }
                    Tampering::ResultState => r.result_state.state = "sha256:00".into(),
                    Tampering::BehaviorVersion => {
                        r.bundle.as_mut().unwrap().behavior_version = "sha256:00".into()
                    }
                    Tampering::Invariant => r.committed_on.state = "sha256:11".into(),
                    Tampering::Swap | Tampering::Drop => unreachable!(),
                }
                Some(Some(r))
            }
        }
    }
}

fn check(kind: Tampering, k: u64, n: u64) -> Result<(), TestCaseError> {
    let base = &*HISTORY;
    let backend = Tamper {
        inner: base.backend(),
        alter: Box::new(tampered(kind, k)),
    };
    let s = Store::open(backend).unwrap();
    // Replay the window around the tamper (from just before it): replays of a clean prefix are
    // covered by `a_clean_history_replays_both_ways`.
    let from = base.state_at(k - 1).unwrap();
    let to = base.state_at(n).unwrap();
    let d = replay_data(&s, &from, &to);
    let b = replay_behavior(&s, &modules(), &from, &to);
    let first = |r: &behavior_store::documents::ReplayReport| {
        r.divergence.as_ref().map(|d| (d.position, d.kind.clone()))
    };
    match kind {
        // Rehashed input: the data still apply; behavior replay finds the forged decision at k,
        // and the chain breaks at k + 1 (or at the head).
        Tampering::InputRehashed => {
            prop_assert_eq!(first(&b).map(|x| x.0), Some(k), "{:?}", b);
            prop_assert!(!d.ok);
        }
        Tampering::BehaviorVersion => {
            prop_assert_eq!(first(&d).map(|x| x.0), Some(k));
            prop_assert_eq!(first(&b), Some((k, "record".to_string())));
        }
        _ => {
            let fd = first(&d);
            prop_assert!(
                fd.is_some(),
                "{:?} at {} undetected by data replay",
                kind,
                k
            );
            prop_assert_eq!(fd.clone().unwrap().0, k, "{:?}: {:?}", kind, d);
            let expected_kind = match kind {
                Tampering::ResultState => "state",
                Tampering::Swap | Tampering::Drop => "chain",
                Tampering::Invariant => "invariant",
                _ => "record",
            };
            prop_assert_eq!(fd.unwrap().1, expected_kind, "{:?}", d);
        }
    }
    Ok(())
}

fn tampering() -> impl Strategy<Value = Tampering> {
    prop::sample::select(vec![
        Tampering::WriteValue,
        Tampering::Input,
        Tampering::InputRehashed,
        Tampering::Context,
        Tampering::ResultState,
        Tampering::BehaviorVersion,
        Tampering::Swap,
        Tampering::Drop,
        Tampering::Invariant,
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn every_tamper_is_found_at_its_position(kind in tampering(), k in 1u64..998) {
        check(kind, k, 999)?;
    }
}

/// SC-002 at 10,000 transitions (release: `cargo test --release -p behavior-store --test replay -- --ignored`).
#[test]
#[ignore]
fn ten_thousand_transitions_replay_and_tampers_are_found() {
    let s = history(10_000);
    let (from, to) = ends(&s);
    assert!(replay_data(&s, &from, &to).ok);
    assert!(replay_behavior(&s, &modules(), &from, &to).ok);
    for (kind, k) in [(Tampering::ResultState, 9_000u64), (Tampering::Swap, 5_000)] {
        let tampered = |p: u64| -> Option<Option<TransitionRecord>> {
            let rec = |q| s.backend().record(q).unwrap();
            match kind {
                Tampering::Swap if p == k => Some(rec(k + 1)),
                Tampering::Swap if p == k + 1 => Some(rec(k)),
                Tampering::ResultState if p == k => {
                    let mut r = rec(k).unwrap();
                    r.result_state.state = "sha256:00".into();
                    Some(Some(r))
                }
                _ => None,
            }
        };
        let st = Store::open(Tamper {
            inner: s.backend(),
            alter: Box::new(tampered),
        })
        .unwrap();
        let d = replay_data(&st, &from, &to);
        assert_eq!(d.divergence.unwrap().position, k);
    }
}

/// A backend that alters the stored value of one entity but keeps its content hash.
struct AlteredValue<'a> {
    inner: &'a InMemoryBackend,
    id: &'static str,
}

impl Backend for AlteredValue<'_> {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.inner.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.inner.head()
    }
    fn create(
        &mut self,
        _: &Genesis,
        _: &Head,
        _: &[EntityVersion],
        _: &[behavior_store::documents::RefChange],
    ) -> Result<(), BackendError> {
        Err(BackendError("read-only".into()))
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        self.inner.keys_at(t, p)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        self.inner.keys_by_field_at(t, f, v, p)
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(k)
    }
    fn incoming_at(
        &self,
        t: &EntityKey,
        p: u64,
    ) -> Result<Vec<behavior_store::RefEdge>, BackendError> {
        self.inner.incoming_at(t, p)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        let mut v = self.inner.version_at(k, p)?;
        if k.id == self.id
            && let Some(v) = v.as_mut()
        {
            v.value["balance"] = serde_json::json!("999999.00");
        }
        Ok(v)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.inner.record(p)
    }
    fn commit(
        &mut self,
        _: &str,
        _: &[EntityVersion],
        _: &[EntityKey],
        _: &[behavior_store::documents::RefChange],
        _: &TransitionRecord,
        _: &Head,
    ) -> Result<CasOutcome, BackendError> {
        Err(BackendError("read-only".into()))
    }
}

#[test]
fn stored_values_are_checked_against_their_content_hashes() {
    // a3 is never written after the genesis; altering its stored value (not its hash) is found.
    let mut s = store();
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    commit(&mut s, &b);
    let (from, to) = ends(&s);
    assert!(replay_data(&s, &from, &to).ok);
    let t = Store::open(AlteredValue {
        inner: s.backend(),
        id: "a3",
    })
    .unwrap();
    let d = replay_data(&t, &from, &to);
    let div = d.divergence.expect("an altered stored value must be found");
    assert_eq!((div.position, div.kind.as_str()), (0, "state"));
}
