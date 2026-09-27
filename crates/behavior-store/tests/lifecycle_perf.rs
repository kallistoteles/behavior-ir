#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-004 (feature 006): creating and removing one entity (evaluate + commit) in a store with
//! 100,000 entities takes under 50 ms and within 2× of a store with 1,000 entities. Run with
//! `cargo test --release -p behavior-store --test lifecycle_perf -- --ignored --nocapture`.

mod common;

use std::time::{Duration, Instant};

use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use common::*;
use serde_json::json;

/// `n` entities: half customers, half accounts (each account references a customer).
fn store_of(n: usize) -> Store<InMemoryBackend> {
    let m = accounts();
    let mut seed: Vec<SeedEntity> = (0..n / 2)
        .map(|i| customer(&format!("c{i}"), "N"))
        .collect();
    seed.extend((0..n / 2).map(|i| account(&format!("a{i}"), &format!("c{i}"), "0.00")));
    seed.push(customer("hub", "H"));
    Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .unwrap()
}

/// The median time of `rounds` create-then-remove pairs.
fn lifecycle_time(s: &mut Store<InMemoryBackend>, rounds: usize) -> Duration {
    let mut times = Vec::new();
    for i in 0..rounds {
        let id = format!("new{i}");
        let start = Instant::now();
        let e = act(
            s,
            "open_account",
            &[("owner", "hub")],
            json!({"account_id": id, "initial": "0.00"}),
        );
        commit_acc(s, &e);
        let e = act(s, "close_account", &[("account", &id)], json!({}));
        commit_acc(s, &e);
        times.push(start.elapsed() / 2);
    }
    times.sort();
    times[times.len() / 2]
}

#[test]
#[ignore]
fn one_creation_or_removal_does_not_depend_on_the_store_size() {
    let mut small = store_of(1_000);
    let mut large = store_of(100_000);
    let t_small = lifecycle_time(&mut small, 21);
    let t_large = lifecycle_time(&mut large, 21);
    eprintln!("1,000 entities: {t_small:?}; 100,000 entities: {t_large:?}");
    assert!(t_large < Duration::from_millis(50), "{t_large:?}");
    assert!(
        t_large <= t_small * 2 + Duration::from_millis(1),
        "{t_small:?} vs {t_large:?}"
    );
}
