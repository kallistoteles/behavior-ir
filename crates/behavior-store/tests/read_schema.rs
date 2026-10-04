#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Reads across a migration (feature 010, US3; feature 009): a read is bound to the schema of the
//! position it reads, with no implicit conversion (FR-013).

mod common;

use behavior_core::migration::admit_migration;
use behavior_core::read::{ReadSource, admit_read};
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity, StateRef, StoreError};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use serde_json::json;

use common::{T0, bind, fixtures, read};

fn module(name: &str) -> Module {
    behavior_core::admit(&read(
        &fixtures().join(format!("migration/modules/{name}.json")),
    ))
    .unwrap()
}

/// A read that both schema generations can express: how many customers exist.
fn customer_count(m: &Module) -> ReadSource {
    let doc = json!({"ir_version": "0.7", "read": {
        "name": "customer_count", "params": [], "loc": {"file": "r.py", "line": 1},
        "body": {"value": {"op": "count", "loc": {"file": "r.py", "line": 1},
                           "args": [{"op": "select", "entity": "Customer",
                                     "loc": {"file": "r.py", "line": 1}}]}}}});
    ReadSource::AdHoc(Box::new(admit_read(m, &doc.to_string()).unwrap()))
}

/// A V1 store with two committed transitions, migrated to V2 at position 3.
fn migrated() -> Store<InMemoryBackend> {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let seed = |entity: &str, value: serde_json::Value| SeedEntity {
        entity: entity.into(),
        value,
    };
    let mut s = Store::create(
        InMemoryBackend::new(),
        &v1,
        genesis_for(
            &v1,
            EvidencePolicy::none(),
            vec![
                seed(
                    "Customer",
                    json!({"id": "c1", "name": "Ada", "email": "ada@x"}),
                ),
                seed(
                    "Customer",
                    json!({"id": "c2", "name": "Bo", "email": "bo@x"}),
                ),
                seed(
                    "Order",
                    json!({"id": "o1", "customer": "c1", "region": "north", "qty": 3}),
                ),
            ],
        ),
    )
    .unwrap();
    for (action, b, input) in [
        (
            "set_region",
            vec![("order", "o1")],
            json!({"region": "south"}),
        ),
        ("forget_customer", vec![("customer", "c2")], json!({})),
    ] {
        let e = s
            .evaluate(&v1, action, &bind(&b), &input, &json!({}), T0, None)
            .unwrap();
        let bundle = e.bundle.unwrap();
        s.commit(&v1, &bundle.evaluated_state.clone(), &bundle)
            .unwrap();
    }
    let p = fixtures().join("migration/valid/cultures_v1_to_v2.json");
    let m = admit_migration(&v1, &v2, &read(&p)).unwrap();
    s.migrate(&m, &v1, &v2, T0, None).unwrap();
    assert_eq!(s.current().unwrap().position, 3);
    s
}

fn count_at(
    s: &Store<InMemoryBackend>,
    m: &Module,
    at: &StateRef,
) -> Result<serde_json::Value, StoreError> {
    let x = s.read(
        m,
        &customer_count(m),
        &bind(&[]),
        &json!({}),
        &json!({}),
        Some(at),
    )?;
    Ok(x.record.as_json()["value"].clone())
}

#[test]
fn a_read_uses_the_schema_of_its_position() {
    let s = migrated();
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let before = s.state_at(0).unwrap();
    let head = s.current().unwrap();
    assert_eq!(count_at(&s, &v1, &before).unwrap(), 2);
    assert_eq!(count_at(&s, &v2, &head).unwrap(), 1);
    for (m, at) in [(&v2, &before), (&v1, &head)] {
        match count_at(&s, m, at) {
            Err(StoreError::SchemaMismatch { .. }) => {}
            other => panic!("expected SCHEMA_MISMATCH, got {other:?}"),
        }
    }
}
