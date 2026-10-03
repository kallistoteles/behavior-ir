#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009, SC-005: migrating a store with 100,000 entities of a changed type completes as
//! one transition in under 60 seconds (release build, reference machine), and a failure at the
//! write leaves the store exactly as before. Ignored by default: run with
//! `cargo test --release -- --ignored`.

mod common;

use std::time::{Duration, Instant};

use behavior_core::migration::admit_migration;
use behavior_store::conformance::{Fault, FaultInjector};
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use common::{fixtures, read};
use serde_json::json;

const N: usize = 100_000;

#[test]
#[ignore]
fn a_hundred_thousand_entities_migrate_in_one_transition_within_a_minute() {
    let module = |n: &str| {
        behavior_core::admit(&read(
            &fixtures()
                .join("migration/modules")
                .join(format!("{n}.json")),
        ))
        .unwrap()
    };
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = admit_migration(
        &v1,
        &v2,
        &read(&fixtures().join("migration/valid/cultures_v1_to_v2.json")),
    )
    .unwrap();
    let seed: Vec<SeedEntity> = (0..N)
        .map(|i| SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": format!("k{i:06}"), "medium": if i % 2 == 0 { "MS" } else { "WPM" },
                          "status": "ACTIVE", "ph": i % 14, "legacy_code": "L",
                          "price": "12.50", "fee": "0.1235"}),
        })
        .collect();
    let mut s = Store::create(
        FaultInjector::new(InMemoryBackend::new()),
        &v1,
        genesis_for(&v1, EvidencePolicy::none(), seed),
    )
    .unwrap();
    // A failure at the write leaves the store as it was.
    let before = s.backend().head().unwrap();
    s.backend_mut().next = Some(Fault::BeforeWrite);
    assert!(
        s.migrate(&m, &v1, &v2, "2026-10-02T10:00:00Z", None)
            .is_err()
    );
    assert_eq!(s.backend().head().unwrap(), before);
    assert_eq!(s.schema_history().unwrap().len(), 1);
    // The migration itself.
    let started = Instant::now();
    let c = s
        .migrate(&m, &v1, &v2, "2026-10-02T10:00:00Z", None)
        .unwrap();
    let took = started.elapsed();
    eprintln!("migrated {N} entities in {took:?}");
    assert_eq!(c.result_state.position, 1);
    let rec = s.backend().record(1).unwrap().unwrap();
    assert_eq!(rec.new_versions.len(), N);
    assert!(took < Duration::from_secs(60), "{took:?}");
}
