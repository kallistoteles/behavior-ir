#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Replaying read records against the store (feature 010, US4): the facts are derived again at
//! the recorded position and the record is reproduced byte for byte (FR-011).

mod common;

use behavior_core::read::ReadSource;
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity, StateRef};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use serde_json::{Value, json};

use common::{T0, bind};

fn lab() -> Module {
    behavior_core::admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap()
}

fn store() -> Store<InMemoryBackend> {
    let m = lab();
    let seed = vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "k1", "name": "Ada", "credit_limit": 100}),
        },
        SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": "c1", "name": "Basil", "stage": null, "active": true,
                          "measurements": 1, "ph_total": 7}),
        },
    ];
    Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .unwrap()
}

fn act(s: &mut Store<InMemoryBackend>, action: &str, b: &[(&str, &str)], input: Value) {
    let m = lab();
    let e = s
        .evaluate(&m, action, &bind(b), &input, &json!({}), T0, None)
        .unwrap();
    let bundle = e.bundle.unwrap();
    s.commit(&m, &bundle.evaluated_state.clone(), &bundle)
        .unwrap();
}

fn read(
    s: &Store<InMemoryBackend>,
    name: &str,
    b: &[(&str, &str)],
    at: Option<&StateRef>,
) -> String {
    s.read(
        &lab(),
        &ReadSource::Declared(name.into()),
        &bind(b),
        &json!({}),
        &json!({}),
        at,
    )
    .unwrap()
    .record
    .to_json_string()
}

#[test]
fn records_replay_against_the_store_at_their_position() {
    let mut s = store();
    act(
        &mut s,
        "place_order",
        &[("customer", "k1")],
        json!({"order_id": "o1", "amount": 30}),
    );
    let early = [
        read(&s, "open_total", &[("customer", "k1")], None),
        read(&s, "active_cultures", &[], None),
        read(&s, "customer_summary", &[("customer", "k1")], None),
        read(&s, "order_view", &[("order", "o9")], None),
    ];
    // Later commits do not affect a record of an earlier position.
    act(&mut s, "close_order", &[("order", "o1")], json!({}));
    act(&mut s, "measure", &[("culture", "c1")], json!({"ph": 6}));
    let past = s.state_at(0).unwrap();
    let late = [
        read(&s, "active_count", &[], None),
        s.read(
            &lab(),
            &ReadSource::Declared("big_orders".into()),
            &bind(&[]),
            &json!({"threshold": 10}),
            &json!({}),
            None,
        )
        .unwrap()
        .record
        .to_json_string(),
        read(&s, "open_total", &[("customer", "k1")], Some(&past)),
    ];
    for record in early.iter().chain(&late) {
        let r = s.replay_read(&lab(), record).unwrap();
        assert!(r.matches, "{:?}\n{record}", r.diff);
    }
}

#[test]
fn a_record_of_another_store_or_state_is_reported() {
    let mut s = store();
    act(&mut s, "measure", &[("culture", "c1")], json!({"ph": 6}));
    let record: Value = serde_json::from_str(&read(&s, "active_count", &[], None)).unwrap();
    // A store with another genesis (the same genesis would be the same store).
    let m = lab();
    let other = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(
            &m,
            EvidencePolicy::none(),
            vec![SeedEntity {
                entity: "Customer".into(),
                value: json!({"id": "k7", "name": "Bo", "credit_limit": 1}),
            }],
        ),
    )
    .unwrap();
    assert_ne!(other.store_id().unwrap(), s.store_id().unwrap());
    for forged in [
        record["data_version"]
            .as_str()
            .unwrap()
            .replace(&s.store_id().unwrap(), &other.store_id().unwrap()),
        record["data_version"]
            .as_str()
            .unwrap()
            .replace(";position:1", ";position:0"),
    ] {
        let mut r = record.clone();
        r["data_version"] = json!(forged);
        let result = s.replay_read(&lab(), &r.to_string()).unwrap();
        assert!(!result.matches, "{forged}");
        assert!(result.diff.unwrap().contains("data_version"));
    }
    // An altered value is found by evaluating again at the position.
    let mut r = record.clone();
    r["value"] = json!(9);
    assert!(!s.replay_read(&lab(), &r.to_string()).unwrap().matches);
}
