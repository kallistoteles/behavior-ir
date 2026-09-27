#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US4 (feature 006): mixed lifecycle histories replay both ways, the index never diverges, and
//! tampering with creations, removals, reference changes, facts or the registry is found at its
//! position (SC-002).

mod common;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    EntityKey, EntityVersion, Genesis, Head, RefChange, StateRef, TransitionRecord,
};
use behavior_store::replay::{replay_behavior, replay_data, replay_index};
use behavior_store::store::transition_hash;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};
use common::*;
use proptest::prelude::*;

static HISTORY: LazyLock<Store<InMemoryBackend>> = LazyLock::new(|| lifecycle_history(1000, 7));

fn modules() -> BTreeMap<String, Module> {
    let m = accounts();
    BTreeMap::from([(m.behavior_version(), m)])
}

struct Tamper<'a> {
    inner: &'a InMemoryBackend,
    alter: Box<dyn Fn(u64) -> Option<TransitionRecord> + 'a>,
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
        _: &[RefChange],
    ) -> Result<(), BackendError> {
        Err(BackendError("read-only".into()))
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        match (self.alter)(p) {
            Some(r) => Ok(Some(r)),
            None => self.inner.record(p),
        }
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(k)
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(t, p)
    }
    fn commit(
        &mut self,
        _: &str,
        _: &[EntityVersion],
        _: &[EntityKey],
        _: &[RefChange],
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
fn a_mixed_history_replays_three_ways() {
    let s = &*HISTORY;
    let (from, to) = ends(s);
    let recs: Vec<TransitionRecord> = (1..=1000)
        .map(|p| s.backend().record(p).unwrap().unwrap())
        .collect();
    assert!(recs.iter().any(|r| !r.created.is_empty()));
    assert!(recs.iter().any(|r| !r.removed.is_empty()));
    assert!(
        recs.iter()
            .any(|r| r.ref_changes.iter().any(|c| c.op == "drop") && r.new_versions.len() == 1)
    );
    let d = replay_data(s, &from, &to);
    assert!(d.ok, "{d:?}");
    assert_eq!(d.checked, 1000);
    let b = replay_behavior(s, &modules(), &from, &to);
    assert!(b.ok, "{b:?}");
    let i = replay_index(s, &accounts(), &from, &to);
    assert!(i.ok, "{i:?}");
}

#[derive(Debug, Clone, Copy)]
enum Tampering {
    CreatedValue,
    Removal,
    RefChange,
    Fact,
    Registry,
    ResultState,
    Chain,
}

/// Positions of the history where a tampering applies.
fn eligible(kind: Tampering) -> Vec<u64> {
    let s = &*HISTORY;
    (2..1000)
        .filter(|&p| {
            let r = s.backend().record(p).unwrap().unwrap();
            match kind {
                Tampering::CreatedValue | Tampering::Registry => !r.created.is_empty(),
                Tampering::Removal => !r.removed.is_empty(),
                Tampering::RefChange => !r.ref_changes.is_empty(),
                Tampering::Fact => !r.bundle.read_facts.is_null(),
                Tampering::ResultState | Tampering::Chain => true,
            }
        })
        .collect()
}

fn tamper(kind: Tampering, rec: &TransitionRecord) -> TransitionRecord {
    let mut r = rec.clone();
    match kind {
        Tampering::CreatedValue => r.created[0].value["balance"] = serde_json::json!("7.77"),
        Tampering::Removal => r.removed[0].last_revision += 1,
        Tampering::RefChange => {
            r.ref_changes.pop();
        }
        Tampering::Fact => {
            let facts = &mut r.bundle.record["facts"];
            if let Some(refs) = facts["references"].as_array_mut() {
                refs[0]["incoming"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"entity": "Account", "id": "zz", "field": "owner"}));
            } else {
                let text = facts.to_string();
                let text = if text.contains("false") {
                    text.replacen("false", "true", 1)
                } else {
                    text.replacen("true", "false", 1)
                };
                *facts = serde_json::from_str(&text).unwrap();
            }
            // Consistent hashes: only the decision itself is wrong.
            r.bundle.transition_hash = transition_hash(&r.bundle.record).unwrap();
            r.bundle.read_facts = r.bundle.record["facts"].clone();
            r.bundle_hash = r.bundle.hash().unwrap();
        }
        Tampering::Registry => {
            // Re-create a seed identity, with every hash made consistent.
            let l = &mut r.bundle.record["lifecycle"][0];
            let entity = l["entity"].as_str().unwrap().to_string();
            let id = if entity == "Customer" { "c1" } else { "a1" };
            l["id"] = serde_json::json!(id);
            l["value"]["id"] = serde_json::json!(id);
            r.bundle.transition_hash = transition_hash(&r.bundle.record).unwrap();
            r.bundle_hash = r.bundle.hash().unwrap();
        }
        Tampering::ResultState => r.result_state.state = "sha256:00".into(),
        Tampering::Chain => r.previous_record = "sha256:00".into(),
    }
    r
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn tampering_is_found_at_its_position(k in 0usize..7, pick in any::<prop::sample::Index>()) {
        let kind = [
            Tampering::CreatedValue, Tampering::Removal, Tampering::RefChange, Tampering::Fact,
            Tampering::Registry, Tampering::ResultState, Tampering::Chain,
        ][k];
        let positions = eligible(kind);
        prop_assume!(!positions.is_empty());
        let p = positions[pick.index(positions.len())];
        let base = &*HISTORY;
        let original = base.backend().record(p).unwrap().unwrap();
        let altered = tamper(kind, &original);
        let t = Tamper { inner: base.backend(), alter: Box::new(move |q| (q == p).then(|| altered.clone())) };
        let s = Store::open(t).unwrap();
        // Replay the window around the tampered position.
        let from = base.state_at(p - 1).unwrap();
        let to = base.state_at(p).unwrap();
        let (report, want) = match kind {
            Tampering::CreatedValue | Tampering::Removal | Tampering::Registry => (replay_data(&s, &from, &to), "lifecycle"),
            Tampering::RefChange => (replay_index(&s, &accounts(), &from, &to), "references"),
            Tampering::Fact => (replay_behavior(&s, &modules(), &from, &to), "decision"),
            Tampering::ResultState => (replay_data(&s, &from, &to), "state"),
            Tampering::Chain => (replay_data(&s, &from, &to), "chain"),
        };
        let d = report.divergence.clone().unwrap_or_else(|| panic!("{kind:?} at {p} not found"));
        prop_assert_eq!(d.position, p, "{:?}: {:?}", kind, d);
        prop_assert_eq!(d.kind.as_str(), want, "{:?}: {:?}", kind, d);
    }
}

#[test]
#[ignore = "10,000 transitions; run with --release --ignored"]
fn a_long_mixed_history_replays() {
    let s = lifecycle_history(10_000, 11);
    let (from, to) = ends(&s);
    assert!(replay_data(&s, &from, &to).ok);
    assert!(replay_behavior(&s, &modules(), &from, &to).ok);
    assert!(replay_index(&s, &accounts(), &from, &to).ok);
}
