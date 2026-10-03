#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: history and replay across schema generations (FR-015–FR-018, SC-002, research
//! R10). Data replay re-checks every migration record against the schema chain and recomputes
//! the state identity; behavior replay re-runs every migration on the parent state. Tampering
//! with a migration record or a migrated value is found at its position.

mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use behavior_core::migration::{Migration, admit_migration};
use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    EntityKey, EntityVersion, EvidencePolicy, Genesis, Head, ReplayReport, SchemaRef, SeedEntity,
    StateRef, TransitionRecord,
};
use behavior_store::replay::{Migrations, replay_behavior_with, replay_data};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, Store};
use common::{bind, fixtures, read};
use proptest::prelude::*;
use serde_json::{Value, json};

const T: &str = "2026-10-02T09:00:00Z";

fn module(name: &str) -> Module {
    let p = fixtures()
        .join("migration/modules")
        .join(format!("{name}.json"));
    behavior_core::admit(&read(&p)).unwrap()
}

fn migration(name: &str, s: &Module, t: &Module) -> Migration {
    let p = fixtures()
        .join("migration/valid")
        .join(format!("{name}.json"));
    admit_migration(s, t, &read(&p)).unwrap()
}

struct Generations {
    modules: [Module; 4],
    migrations: [Migration; 3],
}

fn generations() -> Generations {
    let m = [
        module("cultures_v1"),
        module("cultures_v2"),
        module("cultures_v3"),
        module("cultures_v4"),
    ];
    let mig = [
        migration("cultures_v1_to_v2", &m[0], &m[1]),
        migration("region_required", &m[1], &m[2]),
        migration("customers_vip", &m[2], &m[3]),
    ];
    Generations {
        modules: m,
        migrations: mig,
    }
}

fn seed() -> Vec<SeedEntity> {
    let mut out = Vec::new();
    for i in 1..=3 {
        out.push(SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": format!("c{i}"), "name": "N", "email": "e"}),
        });
        out.push(SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": format!("k{i}"), "medium": "MS", "status": "ACTIVE", "ph": i,
                          "legacy_code": "L", "price": "1.00", "fee": "0.0100"}),
        });
        out.push(SeedEntity {
            entity: "Order".into(),
            value: json!({"id": format!("o{i}"), "customer": format!("c{i}"),
                          "region": "north", "qty": i}),
        });
    }
    out
}

fn run<B: Backend>(
    s: &mut Store<B>,
    m: &Module,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) {
    let e = s
        .evaluate(m, action, &bind(bindings), &input, &json!({}), T, None)
        .unwrap();
    let b = e
        .bundle
        .unwrap_or_else(|| panic!("{action} was not allowed: {}", e.record));
    s.commit(m, &b.evaluated_state.clone(), &b).unwrap();
}

/// A deterministic history of `n` transitions in four generations, with a migration between
/// each: registrations, region changes and deaths.
fn history(n: usize) -> (InMemoryBackend, Vec<u64>) {
    let g = generations();
    let mut s = Store::create(
        InMemoryBackend::new(),
        &g.modules[0],
        genesis_for(&g.modules[0], EvidencePolicy::none(), seed()),
    )
    .unwrap();
    let per = (n - 3) / 4;
    let mut migrations_at = Vec::new();
    let mut next_customer = 100;
    for (gen_index, m) in g.modules.iter().enumerate() {
        let count = if gen_index == 3 { n - 3 - 3 * per } else { per };
        for i in 0..count {
            match i % 3 {
                0 => {
                    next_customer += 1;
                    let id = format!("c{next_customer}");
                    run(
                        &mut s,
                        m,
                        "register_customer",
                        &[],
                        json!({"customer_id": id, "name": "N", "email": "e"}),
                    );
                }
                1 => {
                    let order = format!("o{}", 1 + i % 3);
                    let region = format!("r{}", i % 7);
                    run(
                        &mut s,
                        m,
                        "set_region",
                        &[("order", &order)],
                        json!({"region": region}),
                    );
                }
                _ => {
                    let culture = format!("k{}", 1 + i % 3);
                    run(&mut s, m, "kill", &[("culture", &culture)], json!({}));
                }
            }
        }
        if gen_index < 3 {
            let c = s
                .migrate(
                    &g.migrations[gen_index],
                    m,
                    &g.modules[gen_index + 1],
                    T,
                    None,
                )
                .unwrap();
            migrations_at.push(c.result_state.position);
        }
    }
    (s.into_backend(), migrations_at)
}

fn shared() -> &'static (InMemoryBackend, Vec<u64>) {
    static H: OnceLock<(InMemoryBackend, Vec<u64>)> = OnceLock::new();
    H.get_or_init(|| history(1000))
}

fn behavior_maps(g: &Generations) -> (BTreeMap<String, Module>, Migrations) {
    let modules = g
        .modules
        .iter()
        .map(|m| (m.behavior_version(), m.clone()))
        .collect();
    let migrations = g
        .migrations
        .iter()
        .zip(g.modules.windows(2))
        .map(|(m, w)| (m.hash(), (m.clone(), w[0].clone(), w[1].clone())))
        .collect();
    (modules, migrations)
}

fn ends(s: &Store<impl Backend>) -> (StateRef, StateRef) {
    (s.state_at(0).unwrap(), s.current().unwrap())
}

#[test]
fn a_history_across_three_migrations_replays_without_divergence() {
    let (backend, at) = shared();
    assert_eq!(at.len(), 3);
    let s = Store::open(backend.clone()).unwrap();
    let (from, to) = ends(&s);
    assert!(to.position >= 1000, "{}", to.position);
    let data = replay_data(&s, &from, &to);
    assert!(data.ok, "{:?}", data.divergence);
    assert_eq!(data.checked, to.position);
    let g = generations();
    let (modules, migrations) = behavior_maps(&g);
    let behavior = replay_behavior_with(&s, &modules, &migrations, &from, &to);
    assert!(behavior.ok, "{:?}", behavior.divergence);
    assert_eq!(behavior.checked, to.position);
    // The schema history names every generation.
    let history: Vec<u64> = s
        .schema_history()
        .unwrap()
        .iter()
        .map(|h| h.since)
        .collect();
    assert_eq!(history, [vec![0], at.clone()].concat());
}

#[test]
fn behavior_replay_needs_the_migration() {
    let (backend, at) = shared();
    let s = Store::open(backend.clone()).unwrap();
    let (from, to) = ends(&s);
    let g = generations();
    let (modules, _) = behavior_maps(&g);
    let r = replay_behavior_with(&s, &modules, &BTreeMap::new(), &from, &to);
    assert!(!r.ok);
    assert_eq!(r.divergence.unwrap().position, at[0]);
}

/// A read-only view of a backend whose records and versions at one position are altered.
struct Tamper<'a> {
    inner: &'a InMemoryBackend,
    record: Box<dyn Fn(TransitionRecord) -> TransitionRecord + 'a>,
    version: Box<dyn Fn(EntityVersion) -> EntityVersion + 'a>,
    at: u64,
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
        Ok(self.inner.version_at(k, p)?.map(|v| {
            if v.created_at == self.at {
                (self.version)(v)
            } else {
                v
            }
        }))
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        Ok(self.inner.version(k, r)?.map(|v| {
            if v.created_at == self.at {
                (self.version)(v)
            } else {
                v
            }
        }))
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        Ok(self
            .inner
            .record(p)?
            .map(|r| if p == self.at { (self.record)(r) } else { r }))
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

#[derive(Debug, Clone, Copy)]
enum Tampering {
    /// A migrated value differs from what the migration wrote (stored version altered).
    MigratedValue,
    /// The record names another migration (its bundle hash made consistent).
    MigrationHash,
    /// A requirement outcome is altered (its bundle hash made consistent).
    RequirementOutcome,
    /// The record names another previous schema (its bundle hash made consistent).
    PreviousSchema,
}

fn consistent(r: &mut TransitionRecord) {
    r.bundle_hash = r.migration.as_ref().unwrap().hash().unwrap();
}

fn tampered(backend: &InMemoryBackend, at: u64, kind: Tampering) -> (ReplayReport, ReplayReport) {
    let record: Box<dyn Fn(TransitionRecord) -> TransitionRecord> = match kind {
        Tampering::MigratedValue => Box::new(|r| r),
        Tampering::MigrationHash => Box::new(|mut r| {
            r.migration.as_mut().unwrap().migration_hash = format!("sha256:{}", "ab".repeat(32));
            consistent(&mut r);
            r
        }),
        Tampering::RequirementOutcome => Box::new(|mut r| {
            let m = r.migration.as_mut().unwrap();
            m.requirements
                .push(behavior_store::documents::RequirementOutcome {
                    name: "invented".into(),
                    held: true,
                });
            consistent(&mut r);
            r
        }),
        Tampering::PreviousSchema => Box::new(|mut r| {
            r.migration.as_mut().unwrap().previous_schema = Some(SchemaRef {
                hash: format!("sha256:{}", "cd".repeat(32)),
                declarations: BTreeMap::new(),
                since: 0,
                migration_record: "sha256:00".into(),
            });
            consistent(&mut r);
            r
        }),
    };
    let version: Box<dyn Fn(EntityVersion) -> EntityVersion> = match kind {
        Tampering::MigratedValue => Box::new(|mut v| {
            v.value["tampered"] = json!(true);
            v
        }),
        _ => Box::new(|v| v),
    };
    let s = Store::open(Tamper {
        inner: backend,
        record,
        version,
        at,
    })
    .unwrap();
    let (from, to) = (
        s.state_at(0).unwrap(),
        backend.head().unwrap().unwrap().state_ref,
    );
    let g = generations();
    let (modules, migrations) = behavior_maps(&g);
    (
        replay_data(&s, &from, &to),
        replay_behavior_with(&s, &modules, &migrations, &from, &to),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]
    #[test]
    fn tampering_with_a_migration_is_found_at_its_position(which in 0usize..3, kind in 0usize..4) {
        let (backend, at) = shared();
        let pos = at[which];
        let kind = [
            Tampering::MigratedValue,
            Tampering::MigrationHash,
            Tampering::RequirementOutcome,
            Tampering::PreviousSchema,
        ][kind];
        let (data, behavior) = tampered(backend, pos, kind);
        let first = [data, behavior]
            .into_iter()
            .filter_map(|r| r.divergence)
            .map(|d| d.position)
            .min();
        prop_assert_eq!(first, Some(pos), "{:?} at {}", kind, pos);
    }
}

#[test]
#[ignore]
fn ten_thousand_transitions_across_three_migrations_replay() {
    let (backend, _) = history(10_000);
    let s = Store::open(backend).unwrap();
    let (from, to) = ends(&s);
    let data = replay_data(&s, &from, &to);
    assert!(data.ok, "{:?}", data.divergence);
    let g = generations();
    let (modules, migrations) = behavior_maps(&g);
    let behavior = replay_behavior_with(&s, &modules, &migrations, &from, &to);
    assert!(behavior.ok, "{:?}", behavior.divergence);
}
