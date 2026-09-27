#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-004: one transfer (evaluate + commit) on a store with 100,000 accounts takes under 50 ms and
//! does not grow with the store size. Run with
//! `cargo test --release -p behavior-store --test perf -- --ignored --nocapture`.

mod common;

use std::time::{Duration, Instant};

use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use common::*;
use serde_json::json;

fn store_of(n: usize) -> Store<InMemoryBackend> {
    let m = ledger();
    let seed: Vec<SeedEntity> = (0..n)
        .map(|i| SeedEntity {
            entity: "Account".into(),
            value: json!({"id": format!("a{i}"), "active": true, "balance": "1000.00"}),
        })
        .collect();
    Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .unwrap()
}

/// The median time of `rounds` transfers (evaluate + commit).
fn transfer_time(s: &mut Store<InMemoryBackend>, rounds: usize) -> Duration {
    let mut times = Vec::new();
    for i in 0..rounds {
        let (f, t) = (format!("a{}", 2 * i), format!("a{}", 2 * i + 1));
        let start = Instant::now();
        let b = transfer(s, &f, &t, "1.00", T0).bundle.unwrap();
        commit(s, &b);
        times.push(start.elapsed());
    }
    times.sort();
    times[times.len() / 2]
}

#[test]
#[ignore]
fn one_commit_does_not_depend_on_the_store_size() {
    let mut small = store_of(1_000);
    let mut large = store_of(100_000);
    let t_small = transfer_time(&mut small, 21);
    let t_large = transfer_time(&mut large, 21);
    eprintln!("1,000 accounts: {t_small:?}; 100,000 accounts: {t_large:?}");
    assert!(t_large < Duration::from_millis(50), "{t_large:?}");
    assert!(
        t_large <= t_small * 2 + Duration::from_millis(1),
        "{t_small:?} vs {t_large:?}"
    );
}
