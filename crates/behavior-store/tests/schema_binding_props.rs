#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009, US5: only real schema changes need migrations (FR-004, FR-005, SC-006). Random
//! behavior-only edits always evaluate against the store; every declaration edit is refused with
//! `SCHEMA_MISMATCH` for every action, whether the action touches the edited type or not. The same
//! holds after a migration, relative to the new schema.

mod common;

use std::sync::OnceLock;

use behavior_core::migration::admit_migration;
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use common::{bind, fixtures, read};
use proptest::prelude::*;
use serde_json::{Value, json};

const T: &str = "2026-10-02T09:00:00Z";

fn wire(name: &str) -> Value {
    serde_json::from_str(&read(
        &fixtures()
            .join("migration/modules")
            .join(format!("{name}.json")),
    ))
    .unwrap()
}

fn seed() -> Vec<SeedEntity> {
    vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "c1", "name": "N", "email": "e"}),
        },
        SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": "k1", "medium": "MS", "status": "ACTIVE", "ph": 1,
                          "legacy_code": "L", "price": "1.00", "fee": "0.0100"}),
        },
        SeedEntity {
            entity: "Order".into(),
            value: json!({"id": "o1", "customer": "c1", "region": "north", "qty": 1}),
        },
    ]
}

/// The store under V1, and the same store migrated to V2.
fn stores() -> &'static [(Store<InMemoryBackend>, Value); 2] {
    static S: OnceLock<[(Store<InMemoryBackend>, Value); 2]> = OnceLock::new();
    S.get_or_init(|| {
        let (w1, w2) = (wire("cultures_v1"), wire("cultures_v2"));
        let (v1, v2) = (admit(&w1).unwrap(), admit(&w2).unwrap());
        let make = || {
            Store::create(
                InMemoryBackend::new(),
                &v1,
                genesis_for(&v1, EvidencePolicy::none(), seed()),
            )
            .unwrap()
        };
        let mut migrated = make();
        let doc = read(&fixtures().join("migration/valid/cultures_v1_to_v2.json"));
        let m = admit_migration(&v1, &v2, &doc).unwrap();
        migrated.migrate(&m, &v1, &v2, T, None).unwrap();
        [(make(), w1), (migrated, w2)]
    })
}

fn admit(w: &Value) -> Option<Module> {
    behavior_core::admit(&w.to_string()).ok()
}

/// Every action of `m` the store can evaluate, with bindings and input.
/// An action call: name, state bindings and input.
type Call = (&'static str, Vec<(&'static str, &'static str)>, Value);

fn calls(m: &Module) -> Vec<Call> {
    let mut out = Vec::new();
    for (name, binding, input) in [
        ("kill", vec![("culture", "k1")], json!({})),
        (
            "set_region",
            vec![("order", "o1")],
            json!({"region": "south"}),
        ),
        (
            "register_customer",
            vec![],
            json!({"customer_id": "c9", "name": "N", "email": "e"}),
        ),
    ] {
        if m.action(name).is_some() {
            out.push((name, binding, input));
        }
    }
    out
}

fn loc() -> Value {
    json!({"file": "p.py", "line": 1})
}

/// A behavior-only edit: the declarations stay identical.
fn behavior_edit(mut w: Value, kind: usize, i: usize) -> Value {
    let actions = w["actions"].as_array_mut().unwrap();
    match kind % 3 {
        0 => {
            // Remove one action (never the last one).
            if actions.len() > 1 {
                actions.remove(i % actions.len());
            }
        }
        1 => {
            // An extra precondition on one action.
            let n = i % actions.len();
            actions[n]["preconditions"].as_array_mut().unwrap().push(
                json!({"loc": loc(), "expr": {"op": "lit", "type": {"t": "bool"},
                                                    "value": true, "loc": loc()}}),
            );
        }
        _ => {
            // A new derived value.
            w["derived"].as_array_mut().unwrap().push(json!({
                "name": format!("extra_{i}"), "kind": "derived", "loc": loc(), "params": [],
                "body": {"op": "lit", "type": {"t": "int"}, "value": i, "loc": loc()},
            }));
        }
    }
    w
}

/// A declaration edit: some entity declaration changes.
fn declaration_edit(mut w: Value, kind: usize, i: usize) -> Value {
    let entities = w["entities"].as_array_mut().unwrap();
    let n = i % entities.len();
    match kind % 5 {
        0 => {
            let enums = w["enums"].as_array_mut().unwrap();
            let e = i % enums.len();
            enums[e]["values"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!("NEW{i}")));
        }
        1 => entities[n]["fields"].as_array_mut().unwrap().push(json!({
            "name": format!("extra_{i}"), "type": {"t": "option", "of": {"t": "string"}},
            "loc": loc(),
        })),
        2 => {
            let fields = entities[n]["fields"].as_array_mut().unwrap();
            let f = i % fields.len();
            let name = fields[f]["name"].as_str().unwrap().to_string();
            fields[f]["name"] = json!(format!("{name}_renamed"));
        }
        3 => {
            let fields = entities[n]["fields"].as_array_mut().unwrap();
            let f = i % fields.len();
            fields[f]["type"] = json!({"t": "option", "of": fields[f]["type"].clone()});
        }
        _ => {
            let fields = entities[n]["fields"].as_array_mut().unwrap();
            fields.reverse();
            if fields.len() < 2 {
                fields.push(json!({"name": "pad", "type": {"t": "bool"}, "loc": loc()}));
            }
        }
    }
    w
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn behavior_only_edits_always_evaluate(generation in 0usize..2, kind in 0usize..3, i in 0usize..8) {
        let (store, w) = &stores()[generation];
        let edited = behavior_edit(w.clone(), kind, i);
        let m = admit(&edited);
        prop_assume!(m.is_some());
        let m = m.unwrap();
        for (action, binding, input) in calls(&m) {
            let r = store.evaluate(&m, action, &bind(&binding), &input, &json!({}), T, None);
            prop_assert!(r.is_ok(), "{action}: {:?}", r.err());
        }
    }

    #[test]
    fn every_declaration_edit_is_refused_for_every_action(
        generation in 0usize..2, kind in 0usize..5, i in 0usize..8,
    ) {
        let (store, w) = &stores()[generation];
        let edited = declaration_edit(w.clone(), kind, i);
        // An edit the behavior no longer type-checks against is not a module at all.
        let m = admit(&edited);
        prop_assume!(m.is_some());
        let m = m.unwrap();
        prop_assume!(behavior_core::schema(&m) != behavior_core::schema(&admit(w).unwrap()));
        for (action, binding, input) in calls(&m) {
            let r = store.evaluate(&m, action, &bind(&binding), &input, &json!({}), T, None);
            let code = r.as_ref().err().map(|e| e.code());
            prop_assert_eq!(code, Some("SCHEMA_MISMATCH"), "{} after edit {}", action, kind);
        }
    }
}
