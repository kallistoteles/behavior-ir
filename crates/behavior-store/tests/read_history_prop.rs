#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Reads over generated histories (feature 010, SC-002, SC-003, SC-005): interleaving reads with
//! transitions and a migration leaves the store exactly as the same history without reads; every
//! read record replays from its facts and against the store; any altered record is detected; reads
//! of past positions repeat byte for byte.

mod common;

use behavior_core::migration::{Migration, admit_migration};
use behavior_core::read::{ReadSource, admit_read, replay_read};
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity, StateRef};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use common::{bind, fixtures, read};
use proptest::prelude::*;
use serde_json::{Value, json};

const T: &str = "2026-10-03T09:00:00Z";

fn module(name: &str) -> Module {
    behavior_core::admit(&read(
        &fixtures().join(format!("migration/modules/{name}.json")),
    ))
    .unwrap()
}

fn migration(v1: &Module, v2: &Module) -> Migration {
    let p = fixtures().join("migration/valid/cultures_v1_to_v2.json");
    admit_migration(v1, v2, &read(&p)).unwrap()
}

#[derive(Debug, Clone)]
enum Op {
    Register(u8),
    SetRegion(u8, u8),
    Kill(u8),
    Forget(u8),
    Read(u8),
    ReadPast(u8),
    Migrate,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        1 => (0u8..6).prop_map(Op::Register),
        1 => (0u8..2, 0u8..3).prop_map(|(o, r)| Op::SetRegion(o, r)),
        1 => (0u8..2).prop_map(Op::Kill),
        1 => (0u8..6).prop_map(Op::Forget),
        4 => (0u8..4).prop_map(Op::Read),
        1 => (0u8..4).prop_map(Op::ReadPast),
        1 => Just(Op::Migrate),
    ]
}

/// Reads both schema generations can express, as ad-hoc read documents.
fn read_doc(kind: u8) -> Value {
    let l = json!({"file": "r.py", "line": 1});
    let select = |e: &str| json!({"op": "select", "entity": e, "loc": l});
    let body = match kind % 4 {
        0 => json!({"value": {"op": "count", "args": [select("Customer")], "loc": l}}),
        1 => json!({"project": {"over": select("Order"), "param": "o",
                                "items": [{"field": "qty"}, {"field": "region"}]}}),
        2 => json!({"project": {"over": select("Culture"), "param": "c",
                                "items": [{"field": "status"}, {"field": "ph"}]}}),
        _ => json!({"value": {"op": "count", "loc": l, "args": [{
            "op": "where", "param": "c", "loc": l, "args": [select("Culture")],
            "body": {"op": "eq", "loc": l, "args": [
                {"op": "field", "param": "c", "field": "status", "loc": l},
                {"op": "lit", "type": {"t": "enum", "name": "Status"}, "value": "ACTIVE",
                 "loc": l}]}}]}}),
    };
    json!({"ir_version": "0.7", "read": {"name": format!("r{}", kind % 4), "params": [],
                                         "body": body, "loc": l}})
}

fn store() -> Store<InMemoryBackend> {
    let v1 = module("cultures_v1");
    let seed = |entity: &str, value: Value| SeedEntity {
        entity: entity.into(),
        value,
    };
    Store::create(
        InMemoryBackend::new(),
        &v1,
        genesis_for(
            &v1,
            EvidencePolicy::none(),
            vec![
                seed(
                    "Customer",
                    json!({"id": "c0", "name": "Ada", "email": "a@x"}),
                ),
                seed(
                    "Culture",
                    json!({"id": "k0", "medium": "MS", "status": "ACTIVE", "ph": 7,
                           "legacy_code": "L", "price": "1.00", "fee": "0.0100"}),
                ),
                seed(
                    "Culture",
                    json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 5,
                           "legacy_code": "M", "price": "2.00", "fee": "0.0200"}),
                ),
                seed(
                    "Order",
                    json!({"id": "o0", "customer": "c0", "region": null, "qty": 1}),
                ),
                seed(
                    "Order",
                    json!({"id": "o1", "customer": "c0", "region": "east", "qty": 2}),
                ),
            ],
        ),
    )
    .unwrap()
}

/// One history: transitions and the migration, and, when `reads` is set, the reads with their
/// records (`(module index, record, position read)`).
struct Run {
    store: Store<InMemoryBackend>,
    records: Vec<(usize, String)>,
    past: Vec<(usize, StateRef, u8, String)>,
}

fn run(ops: &[Op], reads: bool) -> Run {
    let modules = [module("cultures_v1"), module("cultures_v2")];
    let mig = migration(&modules[0], &modules[1]);
    let mut s = store();
    let mut generation = 0;
    let mut records = Vec::new();
    let mut past = Vec::new();
    for o in ops {
        let m = &modules[generation];
        let transition =
            |s: &mut Store<InMemoryBackend>, action: &str, b: Vec<(&str, String)>, input: Value| {
                let pairs: Vec<(&str, &str)> = b.iter().map(|(k, v)| (*k, v.as_str())).collect();
                // An unknown binding or a denied decision is no transition, in both runs.
                if let Ok(e) = s.evaluate(m, action, &bind(&pairs), &input, &json!({}), T, None)
                    && let Some(bundle) = e.bundle
                {
                    s.commit(m, &bundle.evaluated_state.clone(), &bundle)
                        .unwrap();
                }
            };
        match o {
            Op::Register(i) => transition(
                &mut s,
                "register_customer",
                vec![],
                json!({"customer_id": format!("c{i}"), "name": "N", "email": "e"}),
            ),
            Op::SetRegion(o, r) => {
                let region = ["north", "south", "west"][usize::from(*r) % 3];
                transition(
                    &mut s,
                    "set_region",
                    vec![("order", format!("o{o}"))],
                    json!({ "region": region }),
                )
            }
            Op::Kill(k) => transition(
                &mut s,
                "kill",
                vec![("culture", format!("k{k}"))],
                json!({}),
            ),
            Op::Forget(i) => transition(
                &mut s,
                "forget_customer",
                vec![("customer", format!("c{i}"))],
                json!({}),
            ),
            Op::Migrate if generation == 0 => {
                // A refused migration (its requirements fail) is refused in both runs.
                if s.migrate(&mig, &modules[0], &modules[1], T, None).is_ok() {
                    generation = 1;
                }
            }
            Op::Migrate => {}
            Op::Read(k) if reads => {
                let item = admit_read(m, &read_doc(*k).to_string()).unwrap();
                let x = s
                    .read(
                        m,
                        &ReadSource::AdHoc(Box::new(item)),
                        &bind(&[]),
                        &json!({}),
                        &json!({}),
                        None,
                    )
                    .unwrap();
                records.push((generation, x.record.to_json_string()));
            }
            Op::ReadPast(k) if reads => {
                // A read of the current position, kept to be repeated at the end.
                let at = s.current().unwrap();
                let item = admit_read(m, &read_doc(*k).to_string()).unwrap();
                let x = s
                    .read(
                        m,
                        &ReadSource::AdHoc(Box::new(item)),
                        &bind(&[]),
                        &json!({}),
                        &json!({}),
                        Some(&at),
                    )
                    .unwrap();
                past.push((generation, at, *k, x.record.to_json_string()));
            }
            Op::Read(_) | Op::ReadPast(_) => {}
        }
    }
    Run {
        store: s,
        records,
        past,
    }
}

/// Everything a store is: its head, every record and every state reference.
fn snapshot(s: &Store<InMemoryBackend>) -> String {
    let head = s.backend().head().unwrap();
    let position = head.as_ref().unwrap().state_ref.position;
    let mut out = vec![serde_json::to_string(&head).unwrap()];
    for p in 1..=position {
        out.push(serde_json::to_string(&s.backend().record(p).unwrap()).unwrap());
    }
    for p in 0..=position {
        out.push(serde_json::to_string(&s.state_at(p).unwrap()).unwrap());
    }
    out.join("\n")
}

fn check(ops: &[Op]) -> Result<usize, TestCaseError> {
    let modules = [module("cultures_v1"), module("cultures_v2")];
    let with = run(ops, true);
    let without = run(ops, false);
    // SC-002: reads leave no trace in the store.
    prop_assert_eq!(snapshot(&with.store), snapshot(&without.store));
    // SC-003: every record replays from its facts and against the store.
    for (generation, record) in &with.records {
        let plain = replay_read(&modules[*generation], record);
        prop_assert!(plain.matches, "plain: {:?}\n{}", plain.diff, record);
        let stored = with
            .store
            .replay_read(&modules[*generation], record)
            .unwrap();
        prop_assert!(stored.matches, "store: {:?}\n{}", stored.diff, record);
    }
    // Any altered record is detected.
    if let Some((generation, record)) = with.records.first() {
        let mut v: Value = serde_json::from_str(record).unwrap();
        v["data_version"] = json!(format!("{}x", v["data_version"].as_str().unwrap()));
        prop_assert!(!replay_read(&modules[*generation], &v.to_string()).matches);
        let mut v: Value = serde_json::from_str(record).unwrap();
        v["result"] = json!("EVALUATION_ERROR");
        prop_assert!(
            !with
                .store
                .replay_read(&modules[*generation], &v.to_string())
                .unwrap()
                .matches
        );
    }
    // SC-005: a read of a past position repeats byte for byte after later transitions.
    for (generation, at, k, record) in &with.past {
        let item = admit_read(&modules[*generation], &read_doc(*k).to_string()).unwrap();
        let again = with
            .store
            .read(
                &modules[*generation],
                &ReadSource::AdHoc(Box::new(item)),
                &bind(&[]),
                &json!({}),
                &json!({}),
                Some(at),
            )
            .unwrap();
        prop_assert_eq!(&again.record.to_json_string(), record);
    }
    Ok(with.records.len() + with.past.len())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]
    #[test]
    fn reads_leave_no_trace_and_replay(ops in prop::collection::vec(op(), 1..80)) {
        check(&ops)?;
    }
}

/// One long history with more than 1,000 reads (SC-002, SC-003).
#[test]
fn a_thousand_reads_leave_no_trace_and_all_replay() {
    let mut ops = Vec::new();
    let mut x: u32 = 7;
    for i in 0..1500u32 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ops.push(match (i, (x >> 16) % 8) {
            (750, _) => Op::Migrate,
            (_, 0) => Op::Register((i % 6) as u8),
            (_, 1) => Op::SetRegion((i % 2) as u8, (i % 3) as u8),
            _ => Op::Read((i % 4) as u8),
        });
    }
    ops.push(Op::ReadPast(0));
    let n = check(&ops).unwrap();
    assert!(n >= 1000, "{n} reads");
}
