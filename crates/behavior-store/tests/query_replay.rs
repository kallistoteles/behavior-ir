#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US4 (feature 007): histories whose decisions depend on queries replay both ways from the
//! records alone, and tampering with a recorded member, a member value or a result hash is found
//! at its position (SC-003).

mod common;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{
    EntityKey, EntityVersion, Genesis, Head, RefChange, StateRef, TransitionRecord,
};
use behavior_store::replay::{replay_behavior, replay_data};
use behavior_store::store::transition_hash;
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};
use common::*;
use proptest::prelude::*;
use serde_json::{Value, json};

/// The fast developer-loop history; the exhaustive 1,000 and 10,000-transition runs are ignored
/// (run with `--release --ignored`).
const N: u64 = 200;

static HISTORY: LazyLock<Store<InMemoryBackend>> = LazyLock::new(|| orders_history(N as usize, 7));

fn modules() -> BTreeMap<String, Module> {
    let m = orders();
    BTreeMap::from([(m.behavior_version(), m)])
}

fn money(cents: u64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// A deterministic history of `n` committed transitions over the orders module: orders placed and
/// removed, limits raised, employees hired and renumbered. Every one evaluates queries.
fn orders_history(n: usize, seed: u64) -> Store<InMemoryBackend> {
    let customers = ["c0", "c1", "c2"];
    let mut initial = vec![employee_seed("e0", "N0")];
    initial.extend(customers.iter().map(|c| customer_seed(c, "100.00")));
    let mut s = orders_store_with(InMemoryBackend::new(), initial);
    let mut rng = seed.max(1);
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut orders: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let mut limits: BTreeMap<&str, u64> = customers.iter().map(|c| (*c, 10_000)).collect();
    let mut employees: BTreeMap<String, String> = BTreeMap::from([("e0".into(), "N0".into())]);
    let mut i = 0usize;
    let mut committed = 0usize;
    while committed < n {
        i += 1;
        let c = customers[(next() % 3) as usize];
        let e = match next() % 5 {
            0 | 1 => {
                let id = format!("o{i}");
                let cents = 100 + next() % 1900;
                let total: u64 = orders
                    .values()
                    .filter(|(oc, _)| oc == c)
                    .map(|(_, a)| a)
                    .sum();
                if total + cents > limits[c] {
                    continue;
                }
                orders.insert(id.clone(), (c.to_string(), cents));
                run_orders(
                    &s,
                    "place_order",
                    &[("customer", c)],
                    json!({"order_id": id, "amount": money(cents)}),
                )
            }
            2 => {
                let Some((id, _)) = orders
                    .iter()
                    .filter(|(_, (oc, _))| oc == c)
                    .map(|(id, (_, a))| (id.clone(), *a))
                    .min_by_key(|(id, a)| (*a, id.clone()))
                else {
                    continue;
                };
                orders.remove(&id);
                run_orders(
                    &s,
                    "remove_cheapest",
                    &[("order", &id), ("customer", c)],
                    json!({}),
                )
            }
            3 => {
                let limit = limits[c] + 100 * (next() % 5);
                let max = orders.values().map(|(_, a)| *a).max().unwrap_or(0);
                if max > limits[c] {
                    continue;
                }
                limits.insert(c, limit);
                run_orders(
                    &s,
                    "raise_limit",
                    &[("customer", c)],
                    json!({"limit": money(limit)}),
                )
            }
            _ => {
                let number = format!("N{}", next() % 1000);
                if employees.values().any(|v| *v == number) {
                    continue;
                }
                if next() % 2 == 0 {
                    let emp = format!("e{i}");
                    employees.insert(emp.clone(), number.clone());
                    run_orders(
                        &s,
                        "hire",
                        &[],
                        json!({"employee_id": emp, "number": number}),
                    )
                } else {
                    let ids: Vec<String> = employees.keys().cloned().collect();
                    let emp = ids[(next() as usize) % ids.len()].clone();
                    employees.insert(emp.clone(), number.clone());
                    run_orders(
                        &s,
                        "renumber",
                        &[("employee", &emp)],
                        json!({"number": number}),
                    )
                }
            }
        };
        assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
        commit_orders(&mut s, &e);
        committed += 1;
    }
    s
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
fn a_query_history_replays_both_ways() {
    let s = &*HISTORY;
    let (from, to) = ends(s);
    let with_queries = (1..=N)
        .filter(|&p| {
            s.backend().record(p).unwrap().unwrap().bundle.record["facts"]["queries"].is_array()
        })
        .count();
    assert_eq!(with_queries, N as usize);
    let d = replay_data(s, &from, &to);
    assert!(d.ok, "{d:?}");
    assert_eq!(d.checked, N);
    let b = replay_behavior(s, &modules(), &from, &to);
    assert!(b.ok, "{b:?}");
    assert_eq!(b.checked, N);
}

#[derive(Debug, Clone, Copy)]
enum Tampering {
    /// A member dropped from a recorded query fact, every hash made consistent.
    Member,
    /// A recorded member value changed, every hash made consistent.
    FieldValue,
    /// Only the query section's result hash in the read facts.
    ResultHash,
}

fn eligible(kind: Tampering) -> Vec<u64> {
    let s = &*HISTORY;
    (2..N)
        .filter(|&p| {
            let r = s.backend().record(p).unwrap().unwrap();
            let facts = &r.bundle.record["facts"];
            match kind {
                Tampering::Member => facts["queries"].as_array().is_some_and(|qs| {
                    qs.iter()
                        .any(|q| !q["members"].as_array().unwrap().is_empty())
                }),
                Tampering::FieldValue => facts["fields"].is_array(),
                Tampering::ResultHash => r.bundle.read_facts["queries"].is_array(),
            }
        })
        .collect()
}

fn consistent(r: &mut TransitionRecord) {
    r.bundle.transition_hash = transition_hash(&r.bundle.record).unwrap();
    let hashes: Vec<Value> = r.bundle.read_facts["queries"]
        .as_array()
        .map(|qs| qs.iter().map(|q| q["result_hash"].clone()).collect())
        .unwrap_or_default();
    r.bundle.read_facts = r.bundle.record["facts"].clone();
    if let Some(qs) = r.bundle.read_facts["queries"].as_array_mut() {
        for (q, h) in qs.iter_mut().zip(hashes) {
            q["result_hash"] = h;
        }
    }
    r.bundle_hash = r.bundle.hash().unwrap();
}

fn tamper(kind: Tampering, rec: &TransitionRecord) -> TransitionRecord {
    let mut r = rec.clone();
    match kind {
        Tampering::Member => {
            let qs = r.bundle.record["facts"]["queries"].as_array_mut().unwrap();
            let q = qs
                .iter_mut()
                .find(|q| !q["members"].as_array().unwrap().is_empty())
                .unwrap();
            q["members"].as_array_mut().unwrap().pop();
            consistent(&mut r);
        }
        Tampering::FieldValue => {
            let f = &mut r.bundle.record["facts"]["fields"][0]["value"];
            *f = if f.is_string() && f != "0.01" {
                json!("0.01")
            } else {
                json!("N-tampered")
            };
            consistent(&mut r);
        }
        Tampering::ResultHash => {
            r.bundle.read_facts["queries"][0]["result_hash"] = json!("sha256:00");
            r.bundle_hash = r.bundle.hash().unwrap();
        }
    }
    r
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn query_tampering_is_found_at_its_position(k in 0usize..3, pick in any::<prop::sample::Index>()) {
        let kind = [Tampering::Member, Tampering::FieldValue, Tampering::ResultHash][k];
        let positions = eligible(kind);
        prop_assume!(!positions.is_empty());
        let p = positions[pick.index(positions.len())];
        let base = &*HISTORY;
        let original = base.backend().record(p).unwrap().unwrap();
        let altered = tamper(kind, &original);
        let t = Tamper { inner: base.backend(), alter: Box::new(move |q| (q == p).then(|| altered.clone())) };
        let s = Store::open(t).unwrap();
        let from = base.state_at(p - 1).unwrap();
        let to = base.state_at(p).unwrap();
        let report = replay_behavior(&s, &modules(), &from, &to);
        let d = report.divergence.clone().unwrap_or_else(|| panic!("{kind:?} at {p} not found"));
        prop_assert_eq!(d.position, p, "{:?}: {:?}", kind, d);
        prop_assert_eq!(d.kind.as_str(), "decision", "{:?}: {:?}", kind, d);
    }
}

#[test]
#[ignore = "1,000 transitions (SC-003 as planned, T019); run with --release --ignored"]
fn a_thousand_transition_query_history_replays() {
    let s = orders_history(1_000, 7);
    let (from, to) = ends(&s);
    assert_eq!(replay_data(&s, &from, &to).checked, 1_000);
    assert!(replay_data(&s, &from, &to).ok);
    assert!(replay_behavior(&s, &modules(), &from, &to).ok);
}

#[test]
#[ignore = "10,000 transitions; run with --release --ignored"]
fn a_long_query_history_replays() {
    let s = orders_history(10_000, 11);
    let (from, to) = ends(&s);
    assert!(replay_data(&s, &from, &to).ok);
    assert!(replay_behavior(&s, &modules(), &from, &to).ok);
}
