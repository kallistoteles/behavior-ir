#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

use std::collections::BTreeMap;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{CommitBundle, EvidencePolicy, SeedEntity, StateRef};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, Evaluation, InMemoryBackend, Store};
use serde_json::{Value, json};

pub const T0: &str = "2026-09-27T12:00:00Z";

pub fn ledger() -> Module {
    behavior_core::admit(&read(&fixtures().join("wire/valid/ledger.json"))).unwrap()
}

pub fn seed(accounts: &[(&str, bool, &str)]) -> Vec<SeedEntity> {
    accounts
        .iter()
        .map(|(id, active, balance)| SeedEntity {
            entity: "Account".into(),
            value: json!({"id": id, "active": active, "balance": balance}),
        })
        .collect()
}

pub fn default_seed() -> Vec<SeedEntity> {
    seed(&[
        ("a1", true, "100.00"),
        ("a2", true, "5.00"),
        ("a3", true, "50.00"),
    ])
}

pub fn store_with<B: Backend>(backend: B, policy: EvidencePolicy) -> Store<B> {
    let m = ledger();
    Store::create(backend, &m, genesis_for(&m, policy, default_seed())).unwrap()
}

pub fn store() -> Store<InMemoryBackend> {
    store_with(InMemoryBackend::new(), EvidencePolicy::none())
}

pub fn bind(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(p, id)| (p.to_string(), id.to_string()))
        .collect()
}

pub fn transfer<B: Backend>(
    s: &Store<B>,
    from: &str,
    to: &str,
    amount: &str,
    time: &str,
) -> Evaluation {
    s.evaluate(
        &ledger(),
        "transfer",
        &bind(&[("from_", from), ("to", to)]),
        &json!({"amount": amount}),
        &json!({}),
        time,
        None,
    )
    .unwrap()
}

pub fn unary<B: Backend>(s: &Store<B>, action: &str, id: &str) -> Evaluation {
    s.evaluate(
        &ledger(),
        action,
        &bind(&[("account", id)]),
        &json!({}),
        &json!({}),
        T0,
        None,
    )
    .unwrap()
}

pub fn commit<B: Backend>(s: &mut Store<B>, b: &CommitBundle) -> behavior_store::Committed {
    let parent = b.evaluated_state.clone();
    s.commit(&ledger(), &parent, b).unwrap()
}

pub fn balance<B: Backend>(s: &Store<B>, id: &str, at: &StateRef) -> Value {
    let key = behavior_store::documents::EntityKey {
        entity: "Account".into(),
        id: id.into(),
    };
    s.load(&key, at).unwrap().value["balance"].clone()
}
