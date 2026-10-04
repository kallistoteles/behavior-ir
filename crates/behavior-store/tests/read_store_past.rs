#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Reading the past (feature 010, US3): a read of a past position gives what that state
//! determines, independent of anything committed later (FR-002, FR-014, SC-005).

mod common;

use std::collections::BTreeMap;

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
                          "measurements": 0, "ph_total": 0}),
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

fn read_at(
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
fn a_past_read_is_independent_of_later_transitions() {
    let mut s = store();
    for i in 0..4 {
        act(
            &mut s,
            "place_order",
            &[("customer", "k1")],
            json!({"order_id": format!("o{i}"), "amount": 10 + i}),
        );
    }
    let at4 = s.current().unwrap();
    assert_eq!(at4.position, 4);
    let total_then = read_at(&s, "open_total", &[("customer", "k1")], None);
    let summary_then = read_at(&s, "customer_summary", &[("customer", "k1")], None);
    let measure_then = read_at(&s, "active_cultures", &[], None);
    // Six more transitions change every value those reads depend on.
    act(&mut s, "close_order", &[("order", "o0")], json!({}));
    act(
        &mut s,
        "place_order",
        &[("customer", "k1")],
        json!({"order_id": "o9", "amount": 500}),
    );
    act(&mut s, "measure", &[("culture", "c1")], json!({"ph": 7}));
    act(&mut s, "retire", &[("culture", "c1")], json!({}));
    act(
        &mut s,
        "start_culture",
        &[],
        json!({"culture_id": "c2", "name": "Mint"}),
    );
    act(&mut s, "measure", &[("culture", "c2")], json!({"ph": 6}));
    assert_eq!(s.current().unwrap().position, 10);
    assert_ne!(
        read_at(&s, "open_total", &[("customer", "k1")], None),
        total_then
    );
    // At position 4, every record is byte-identical to the one taken then.
    let at = s.state_at(4).unwrap();
    assert_eq!(at, at4);
    assert_eq!(
        read_at(&s, "open_total", &[("customer", "k1")], Some(&at)),
        total_then
    );
    assert_eq!(
        read_at(&s, "customer_summary", &[("customer", "k1")], Some(&at)),
        summary_then
    );
    assert_eq!(read_at(&s, "active_cultures", &[], Some(&at)), measure_then);
}

#[test]
fn a_position_must_be_a_state_of_this_store() {
    let mut s = store();
    act(&mut s, "measure", &[("culture", "c1")], json!({"ph": 7}));
    let read = |at: &StateRef| {
        s.read(
            &lab(),
            &ReadSource::Declared("active_count".into()),
            &BTreeMap::new(),
            &json!({}),
            &json!({}),
            Some(at),
        )
    };
    // A state of this store, named at the wrong position.
    let mut wrong = s.state_at(0).unwrap();
    wrong.state = s.state_at(1).unwrap().state;
    assert!(
        read(&wrong).is_err(),
        "a state at the wrong position is refused"
    );
    // A position beyond the head.
    let mut beyond = s.current().unwrap();
    beyond.position += 1;
    assert!(read(&beyond).is_err(), "a future position is refused");
}
