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

// --- feature 006: the accounts lifecycle module ------------------------------------------------

pub fn accounts() -> Module {
    behavior_core::admit(&read(&fixtures().join("wire/valid/accounts.json"))).unwrap()
}

pub fn customer(id: &str, name: &str) -> SeedEntity {
    SeedEntity {
        entity: "Customer".into(),
        value: json!({"id": id, "name": name}),
    }
}

pub fn account(id: &str, owner: &str, balance: &str) -> SeedEntity {
    SeedEntity {
        entity: "Account".into(),
        value: json!({"id": id, "owner": owner, "balance": balance}),
    }
}

pub fn note(id: &str, about: &str) -> SeedEntity {
    SeedEntity {
        entity: "AuditNote".into(),
        value: json!({"id": id, "about": about, "text": "hi"}),
    }
}

/// Customers c1, c2, c3; account a1 owned by c1; note n1 about c1.
pub fn accounts_seed() -> Vec<SeedEntity> {
    vec![
        customer("c1", "Ada"),
        customer("c2", "Bo"),
        customer("c3", "Cy"),
        account("a1", "c1", "0.00"),
        note("n1", "c1"),
    ]
}

pub fn accounts_store() -> Store<InMemoryBackend> {
    let m = accounts();
    Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), accounts_seed()),
    )
    .unwrap()
}

pub fn act<B: Backend>(
    s: &Store<B>,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) -> Evaluation {
    s.evaluate(
        &accounts(),
        action,
        &bind(bindings),
        &input,
        &json!({}),
        T0,
        None,
    )
    .unwrap()
}

pub fn commit_acc<B: Backend>(s: &mut Store<B>, e: &Evaluation) -> behavior_store::Committed {
    let b = e.bundle.as_ref().unwrap_or_else(|| panic!("{}", e.record));
    s.commit(&accounts(), &b.evaluated_state.clone(), b)
        .unwrap()
}

pub fn key(entity: &str, id: &str) -> behavior_store::documents::EntityKey {
    behavior_store::documents::EntityKey {
        entity: entity.into(),
        id: id.into(),
    }
}

pub fn edge(entity: &str, id: &str, field: &str) -> behavior_store::RefEdge {
    behavior_store::RefEdge {
        entity: entity.into(),
        id: id.into(),
        field: field.into(),
    }
}

/// A deterministic mixed lifecycle history of `n` committed transitions: registrations, account
/// openings, deposits, closings, retarget-and-remove, and customer removals.
pub fn lifecycle_history(n: usize, seed: u64) -> Store<InMemoryBackend> {
    let mut s = accounts_store();
    let mut rng = seed.max(1);
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    // The model: customers, and accounts with (owner, balance in cents).
    let mut customers: Vec<String> = vec!["c1".into(), "c2".into(), "c3".into()];
    let mut accounts: BTreeMap<String, (String, u64)> =
        BTreeMap::from([("a1".into(), ("c1".into(), 0))]);
    let mut i = 0usize;
    let mut committed = 0usize;
    while committed < n {
        i += 1;
        let pick = |v: &[String], r: u64| v[(r as usize) % v.len()].clone();
        let referenced = |accounts: &BTreeMap<String, (String, u64)>, c: &str| {
            accounts.values().filter(|(o, _)| o == c).count()
        };
        let e = match next() % 6 {
            0 => {
                let c = format!("c{}", 100 + i);
                customers.push(c.clone());
                act(
                    &s,
                    "register_customer",
                    &[],
                    json!({"customer_id": c, "name": "N"}),
                )
            }
            1 if !customers.is_empty() => {
                let owner = pick(&customers, next());
                let a = format!("a{}", 100 + i);
                let cents = if next() % 2 == 0 { 0 } else { 500 };
                accounts.insert(a.clone(), (owner.clone(), cents));
                act(
                    &s,
                    "open_account",
                    &[("owner", &owner)],
                    json!({"account_id": a, "initial": format!("{}.{:02}", cents / 100, cents % 100)}),
                )
            }
            2 if !accounts.is_empty() => {
                let ids: Vec<String> = accounts.keys().cloned().collect();
                let a = pick(&ids, next());
                if let Some(x) = accounts.get_mut(&a) {
                    x.1 += 100;
                }
                act(&s, "deposit", &[("account", &a)], json!({"amount": "1.00"}))
            }
            3 => {
                let zero: Vec<String> = accounts
                    .iter()
                    .filter(|(_, (_, b))| *b == 0)
                    .map(|(k, _)| k.clone())
                    .collect();
                if zero.is_empty() {
                    continue;
                }
                let a = pick(&zero, next());
                accounts.remove(&a);
                act(&s, "close_account", &[("account", &a)], json!({}))
            }
            4 => {
                // Retarget an account away from an owner it alone references, then remove that owner.
                let cands: Vec<String> = accounts
                    .iter()
                    .filter(|(_, (o, _))| referenced(&accounts, o) == 1)
                    .map(|(k, _)| k.clone())
                    .collect();
                if cands.is_empty() || customers.len() < 2 {
                    continue;
                }
                let a = pick(&cands, next());
                let old = accounts[&a].0.clone();
                let others: Vec<String> =
                    customers.iter().filter(|c| **c != old).cloned().collect();
                let new = pick(&others, next());
                if let Some(x) = accounts.get_mut(&a) {
                    x.0 = new.clone();
                }
                customers.retain(|c| *c != old);
                act(
                    &s,
                    "switch_and_remove",
                    &[("account", &a), ("old", &old), ("new", &new)],
                    json!({}),
                )
            }
            _ => {
                let free: Vec<String> = customers
                    .iter()
                    .filter(|c| referenced(&accounts, c) == 0 && *c != "c1")
                    .cloned()
                    .collect();
                if free.is_empty() {
                    continue;
                }
                let c = pick(&free, next());
                customers.retain(|x| *x != c);
                act(&s, "remove_customer", &[("customer", &c)], json!({}))
            }
        };
        commit_acc(&mut s, &e);
        committed += 1;
    }
    s
}

// --- feature 007: the orders query module ------------------------------------------------------

pub fn orders() -> Module {
    behavior_core::admit(&read(&fixtures().join("wire/valid/orders.json"))).unwrap()
}

pub fn order_seed(id: &str, customer: &str, amount: &str, status: &str) -> SeedEntity {
    SeedEntity {
        entity: "Order".into(),
        value: json!({"id": id, "customer": customer, "amount": amount, "status": status,
                      "region": "north"}),
    }
}

pub fn customer_seed(id: &str, limit: &str) -> SeedEntity {
    SeedEntity {
        entity: "Customer".into(),
        value: json!({"id": id, "name": id, "credit_limit": limit, "region": "north"}),
    }
}

pub fn employee_seed(id: &str, number: &str) -> SeedEntity {
    SeedEntity {
        entity: "Employee".into(),
        value: json!({"id": id, "personnel_number": number}),
    }
}

/// Customers c1 (limit 100.00) and c2; orders o1 (c1, closed) and o2 (c2, open); employee e1.
pub fn orders_seed() -> Vec<SeedEntity> {
    vec![
        customer_seed("c1", "100.00"),
        customer_seed("c2", "100.00"),
        order_seed("o1", "c1", "10.00", "closed"),
        order_seed("o2", "c2", "5.00", "open"),
        employee_seed("e1", "N1"),
    ]
}

pub fn orders_store_with<B: Backend>(backend: B, seed: Vec<SeedEntity>) -> Store<B> {
    let m = orders();
    Store::create(backend, &m, genesis_for(&m, EvidencePolicy::none(), seed)).unwrap()
}

pub fn orders_store() -> Store<InMemoryBackend> {
    orders_store_with(InMemoryBackend::new(), orders_seed())
}

pub fn run_orders<B: Backend>(
    s: &Store<B>,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) -> Evaluation {
    s.evaluate(
        &orders(),
        action,
        &bind(bindings),
        &input,
        &json!({}),
        T0,
        None,
    )
    .unwrap()
}

pub fn commit_orders<B: Backend>(s: &mut Store<B>, e: &Evaluation) -> behavior_store::Committed {
    let b = e.bundle.as_ref().unwrap_or_else(|| panic!("{}", e.record));
    s.commit(&orders(), &b.evaluated_state.clone(), b).unwrap()
}
