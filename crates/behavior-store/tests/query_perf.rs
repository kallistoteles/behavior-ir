#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-007 (feature 007): a selective query (≤ 10 matches) over 100,000 entities, with a field
//! index for its predicate field, evaluates in under 50 ms and within 2× of a store with 1,000
//! entities; without the index the result is the same. The index is only a performance
//! precondition, never semantic. Run with
//! `cargo test --release -p behavior-store --test query_perf -- --ignored --nocapture`.

mod common;

use std::time::{Duration, Instant};

use behavior_store::documents::{
    EntityKey, EntityVersion, EvidencePolicy, Genesis, Head, RefChange, SeedEntity,
    TransitionRecord,
};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};
use common::*;
use serde_json::json;

/// `n` entities: customers `c0…`, and orders spread over them so that customer `target` has
/// exactly 10.
fn seed_of(n: usize) -> Vec<SeedEntity> {
    let customers = n / 20;
    let mut seed: Vec<SeedEntity> = (0..customers)
        .map(|i| customer_seed(&format!("c{i}"), "1000000.00"))
        .collect();
    seed.push(customer_seed("target", "1000.00"));
    for i in 0..(n - customers - 11) {
        seed.push(order_seed(
            &format!("o{i}"),
            &format!("c{}", i % customers),
            "1.00",
            "open",
        ));
    }
    for i in 0..10 {
        seed.push(order_seed(&format!("t{i}"), "target", "2.00", "open"));
    }
    seed
}

fn store_of<B: Backend>(backend: B, n: usize) -> Store<B> {
    let m = orders();
    Store::create(
        backend,
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed_of(n)),
    )
    .unwrap()
}

/// The median time of evaluating `check_orders` for the target customer.
fn query_time<B: Backend>(s: &Store<B>, rounds: usize) -> Duration {
    let mut times: Vec<Duration> = (0..rounds)
        .map(|_| {
            let start = Instant::now();
            let e = run_orders(s, "check_orders", &[("customer", "target")], json!({}));
            let t = start.elapsed();
            assert_eq!(e.record["result"], "ALLOW");
            t
        })
        .collect();
    times.sort();
    times[times.len() / 2]
}

/// The reference backend without its field index.
struct Unindexed(InMemoryBackend);

impl Backend for Unindexed {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.0.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.0.head()
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        seed: &[EntityVersion],
        refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.0.create(g, h, seed, refs)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        self.0.keys_at(t, p)
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.0.removed_at(k)
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.0.incoming_at(t, p)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.0.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.0.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.0.record(p)
    }
    fn commit(
        &mut self,
        x: &str,
        v: &[EntityVersion],
        rm: &[EntityKey],
        refs: &[RefChange],
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.0.commit(x, v, rm, refs, r, h)
    }
}

#[test]
#[ignore]
fn a_selective_query_does_not_depend_on_the_store_size() {
    let small = store_of(InMemoryBackend::new(), 1_000);
    let large = store_of(InMemoryBackend::new(), 100_000);
    let t_small = query_time(&small, 21);
    let t_large = query_time(&large, 21);
    println!("selective query: 1,000 entities {t_small:?}, 100,000 entities {t_large:?}");
    assert!(t_large < Duration::from_millis(50), "{t_large:?}");
    assert!(
        t_large <= t_small * 2 + Duration::from_millis(1),
        "{t_large:?} vs {t_small:?}"
    );
}

#[test]
#[ignore]
fn the_index_never_changes_a_result() {
    let indexed = store_of(InMemoryBackend::new(), 20_000);
    let scanned = store_of(Unindexed(InMemoryBackend::new()), 20_000);
    let a = run_orders(
        &indexed,
        "check_orders",
        &[("customer", "target")],
        json!({}),
    );
    let b = run_orders(
        &scanned,
        "check_orders",
        &[("customer", "target")],
        json!({}),
    );
    assert_eq!(a.record, b.record);
    assert_eq!(
        a.record["facts"]["queries"][0]["members"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
}
