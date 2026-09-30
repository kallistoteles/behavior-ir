#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-002 and SC-008 (feature 007): the backend's iteration order and its indexes never leak into
//! behavior. 1,000 permutations of the order in which `keys_at` and `keys_by_field_at` list keys,
//! with and without the field index (a rebuilt or missing index), give identical results, traces
//! and records, and behavior replay agrees. Run with
//! `cargo test --release -p behavior-store --test query_order_props -- --ignored`.

mod common;

use std::collections::BTreeMap;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    EntityKey, EntityVersion, Genesis, Head, RefChange, TransitionRecord,
};
use behavior_store::replay::replay_behavior;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};
use common::*;
use serde_json::{Value, json};

/// A read-only view of a backend that permutes key listings with `seed` and optionally hides
/// the field index.
struct Permuted<'a> {
    inner: &'a InMemoryBackend,
    seed: u64,
    indexed: bool,
}

fn permute(mut keys: Vec<EntityKey>, seed: u64) -> Vec<EntityKey> {
    let mut rng = seed.max(1);
    for i in (1..keys.len()).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        keys.swap(i, (rng as usize) % (i + 1));
    }
    keys
}

impl Backend for Permuted<'_> {
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
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        Ok(permute(self.inner.keys_at(t, p)?, self.seed))
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        if !self.indexed {
            return Ok(None);
        }
        Ok(self
            .inner
            .keys_by_field_at(t, f, v, p)?
            .map(|k| permute(k, self.seed.wrapping_mul(31))))
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(k)
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(t, p)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(k, p)
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
        _: &[RefChange],
        _: &TransitionRecord,
        _: &Head,
    ) -> Result<CasOutcome, BackendError> {
        Err(BackendError("read-only".into()))
    }
}

fn modules() -> BTreeMap<String, Module> {
    let m = orders();
    BTreeMap::from([(m.behavior_version(), m)])
}

/// A store whose sets have many members and ties (equal amounts, equal keys to look up).
fn history() -> Store<InMemoryBackend> {
    let mut seed = orders_seed();
    for i in 0..12 {
        seed.push(order_seed(
            &format!("p{i:02}"),
            "c1",
            "5.00",
            if i % 3 == 0 { "closed" } else { "open" },
        ));
    }
    for i in 0..6 {
        seed.push(employee_seed(
            &format!("e{}", i + 2),
            &format!("N{}", i + 2),
        ));
    }
    let mut s = orders_store_with(InMemoryBackend::new(), seed);
    for (action, bindings, input) in [
        (
            "remove_cheapest",
            vec![("order", "p00"), ("customer", "c1")],
            json!({}),
        ),
        (
            "raise_limit",
            vec![("customer", "c2")],
            json!({"limit": "150.00"}),
        ),
        ("hire", vec![], json!({"employee_id": "e9", "number": "N9"})),
        (
            "renumber",
            vec![("employee", "e2")],
            json!({"number": "N20"}),
        ),
    ] {
        let e = run_orders(&s, action, &bindings, input);
        commit_orders(&mut s, &e);
    }
    s
}

type Probe = (&'static str, Vec<(&'static str, &'static str)>, Value);

/// Query-heavy evaluations at the head, as records.
fn decisions<B: Backend>(s: &Store<B>) -> Vec<Value> {
    let probes: [Probe; 7] = [
        ("check_orders", vec![("customer", "c1")], json!({})),
        ("check_orders", vec![("customer", "c2")], json!({})),
        ("close_customer", vec![("customer", "c1")], json!({})),
        (
            "remove_cheapest",
            vec![("order", "p01"), ("customer", "c1")],
            json!({}),
        ),
        (
            "place_order",
            vec![("customer", "c1")],
            json!({"order_id": "q1", "amount": "1.00"}),
        ),
        (
            "hire_unchecked",
            vec![],
            json!({"employee_id": "e10", "number": "N3"}),
        ),
        (
            "renumber",
            vec![("employee", "e3")],
            json!({"number": "N4"}),
        ),
    ];
    probes
        .into_iter()
        .map(|(a, b, i)| run_orders(s, a, &b, i).record)
        .collect()
}

#[test]
#[ignore = "1,000 permutations; run with --release --ignored"]
fn backend_order_and_indexes_never_change_a_decision() {
    let base = history();
    let reference = decisions(&base);
    let (from, to) = (base.state_at(0).unwrap(), base.current().unwrap());
    for seed in 1..=1000u64 {
        let view = Store::open(Permuted {
            inner: base.backend(),
            seed,
            indexed: seed % 2 == 0,
        })
        .unwrap();
        assert_eq!(decisions(&view), reference, "seed {seed}");
        if seed % 100 == 0 {
            let r = replay_behavior(&view, &modules(), &from, &to);
            assert!(r.ok, "seed {seed}: {r:?}");
        }
    }
}
