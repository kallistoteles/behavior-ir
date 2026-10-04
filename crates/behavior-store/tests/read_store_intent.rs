#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Read intents against a store (feature 010, US5): targets are bound at the read's position, a
//! missing one is listed with every other problem; declared reads change the behavior version,
//! never the schema, so a store keeps working without a migration.

mod common;

use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use serde_json::{Value, json};

use common::{T0, bind};

fn module(name: &str) -> Module {
    behavior_core::admit(&common::read(
        &common::fixtures().join(format!("reads/modules/{name}.json")),
    ))
    .unwrap()
}

/// A store created by the module without reads.
fn store() -> Store<InMemoryBackend> {
    let base = module("lab_base");
    let seed = vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "k1", "name": "Ada", "credit_limit": 100}),
        },
        SeedEntity {
            entity: "Order".into(),
            value: json!({"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}),
        },
    ];
    Store::create(
        InMemoryBackend::new(),
        &base,
        genesis_for(&base, EvidencePolicy::none(), seed),
    )
    .unwrap()
}

fn intent(
    s: &Store<InMemoryBackend>,
    i: Value,
    at: Option<&behavior_store::documents::StateRef>,
) -> Result<behavior_core::read::ReadExecution, Value> {
    s.read_intent(&module("lab"), &i.to_string(), &json!({}), at)
        .unwrap()
        .map_err(|r| serde_json::from_str(&r.to_json_string()).unwrap())
}

#[test]
fn a_declared_read_needs_no_migration() {
    let (base, lab) = (module("lab_base"), module("lab"));
    assert_ne!(base.behavior_version(), lab.behavior_version());
    assert_eq!(
        behavior_core::schema(&base).hash,
        behavior_core::schema(&lab).hash
    );
    let mut s = store();
    // The module with reads evaluates and commits against the store the base module created.
    let e = s
        .evaluate(
            &lab,
            "close_order",
            &bind(&[("order", "o1")]),
            &json!({}),
            &json!({}),
            T0,
            None,
        )
        .unwrap();
    let bundle = e.bundle.unwrap();
    s.commit(&lab, &bundle.evaluated_state.clone(), &bundle)
        .unwrap();
    let x = intent(
        &s,
        json!({"capability": "open_total", "targets": {"customer": "k1"}}),
        None,
    )
    .unwrap();
    assert_eq!(x.response.value(), Some(&json!(0)));
}

#[test]
fn read_intents_at_the_head_and_in_the_past() {
    let mut s = store();
    let lab = module("lab");
    let before = s.current().unwrap();
    let e = s
        .evaluate(
            &lab,
            "close_order",
            &bind(&[("order", "o1")]),
            &json!({}),
            &json!({}),
            T0,
            None,
        )
        .unwrap();
    let bundle = e.bundle.unwrap();
    s.commit(&lab, &bundle.evaluated_state.clone(), &bundle)
        .unwrap();
    let i = json!({"capability": "open_total", "targets": {"customer": "k1"}});
    let now = intent(&s, i.clone(), None).unwrap();
    let then = intent(&s, i, Some(&before)).unwrap();
    assert_eq!(now.response.value(), Some(&json!(0)));
    assert_eq!(then.response.value(), Some(&json!(30)));
    assert_eq!(then.response.record_id(), then.record.record_id());
}

#[test]
fn a_missing_target_is_listed_with_every_other_problem() {
    let s = store();
    let rejection = intent(
        &s,
        json!({"capability": "big_orders", "input": {"threshold": "many"}}),
        None,
    )
    .unwrap_err();
    assert_eq!(
        rejection["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["code"].as_str().unwrap(), e["path"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [("WRONG_TYPE", "input.threshold")]
    );
    let rejection = intent(
        &s,
        json!({"capability": "customer_summary", "targets": {"customer": "nobody"},
               "input": {"x": 1}}),
        None,
    )
    .unwrap_err();
    assert_eq!(
        rejection["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["code"].as_str().unwrap(), e["path"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [
            ("EXTRA_ARGUMENT", "input.x"),
            ("UNKNOWN_TARGET", "targets.customer")
        ]
    );
}
