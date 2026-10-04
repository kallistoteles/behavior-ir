#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Store reads (feature 010, US1/US2): a read evaluates against one exact store state and never
//! writes; its record names that state.

mod common;

use std::collections::BTreeMap;

use behavior_core::read::{ReadExecution, ReadSource};
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EvidencePolicy, SeedEntity, data_version};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use serde_json::{Value, json};

use common::{T0, bind};

pub fn lab() -> Module {
    behavior_core::admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap()
}

fn seed() -> Vec<SeedEntity> {
    let e = |entity: &str, value: Value| SeedEntity {
        entity: entity.into(),
        value,
    };
    vec![
        e(
            "Culture",
            json!({"id": "c1", "name": "Basil", "stage": "GROWTH", "active": true,
                            "measurements": 2, "ph_total": 14}),
        ),
        e(
            "Culture",
            json!({"id": "c2", "name": "Mint", "stage": null, "active": false,
                            "measurements": 0, "ph_total": 0}),
        ),
        e(
            "Customer",
            json!({"id": "k1", "name": "Ada", "credit_limit": 100}),
        ),
        e(
            "Order",
            json!({"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}),
        ),
    ]
}

pub fn lab_store() -> Store<InMemoryBackend> {
    let m = lab();
    Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed()),
    )
    .unwrap()
}

/// Evaluates and commits one action of the lab module.
pub fn act(s: &mut Store<InMemoryBackend>, action: &str, b: &[(&str, &str)], input: Value) {
    let m = lab();
    let e = s
        .evaluate(&m, action, &bind(b), &input, &json!({}), T0, None)
        .unwrap();
    let bundle = e
        .bundle
        .unwrap_or_else(|| panic!("{action} was not allowed: {}", e.record));
    let parent = bundle.evaluated_state.clone();
    s.commit(&m, &parent, &bundle).unwrap();
}

fn read(s: &Store<InMemoryBackend>, name: &str, b: &[(&str, &str)], input: Value) -> ReadExecution {
    s.read(
        &lab(),
        &ReadSource::Declared(name.into()),
        &bind(b),
        &input,
        &json!({}),
        None,
    )
    .unwrap()
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

/// A read needs only a shared reference to the store: it cannot write.
fn count_through_shared<B: Backend>(s: &Store<B>, m: &Module) -> Value {
    s.read(
        m,
        &ReadSource::Declared("active_count".into()),
        &BTreeMap::new(),
        &json!({}),
        &json!({}),
        None,
    )
    .unwrap()
    .record
    .as_json()["value"]
        .clone()
}

#[test]
fn a_read_at_the_head_returns_the_value_and_names_the_state() {
    let mut s = lab_store();
    act(
        &mut s,
        "start_culture",
        &[],
        json!({"culture_id": "c3", "name": "Thyme"}),
    );
    let x = read(&s, "active_count", &[], json!({}));
    let r = x.record.as_json();
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], 2);
    let head = s.current().unwrap();
    assert_eq!(
        r["data_version"],
        data_version(&s.store_id().unwrap(), &head)
    );
    assert_eq!(count_through_shared(&s, &lab()), 2);

    let x = read(&s, "open_total", &[("customer", "k1")], json!({}));
    assert_eq!(x.record.as_json()["value"], 30);
    assert_eq!(x.record.as_json()["state"]["customer"]["name"], "Ada");
}

#[test]
fn reads_never_change_the_store() {
    let mut s = lab_store();
    act(&mut s, "measure", &[("culture", "c1")], json!({"ph": 7}));
    act(
        &mut s,
        "place_order",
        &[("customer", "k1")],
        json!({"order_id": "o2", "amount": 5}),
    );
    let before = snapshot(&s);
    for i in 0..100 {
        match i % 4 {
            0 => read(&s, "active_count", &[], json!({})),
            1 => read(&s, "open_total", &[("customer", "k1")], json!({})),
            2 => read(&s, "smallest_order", &[], json!({})),
            _ => read(&s, "average_ph", &[], json!({})),
        };
    }
    assert_eq!(snapshot(&s), before);
}

#[test]
fn an_unknown_binding_is_an_invalid_binding_record() {
    let s = lab_store();
    let x = read(&s, "open_total", &[("customer", "nobody")], json!({}));
    let r = x.record.as_json();
    assert_eq!(r["result"], "INVALID_BINDING", "{r}");
    assert_eq!(r["derived"], json!([]));
    assert!(r.get("value").is_none());
    let existence = r["facts"]["existence"].as_array().unwrap();
    assert_eq!(
        existence,
        &[json!({"entity": "Customer", "id": "nobody", "exists": false})],
        "{r}"
    );
    assert_eq!(x.response.result(), "INVALID_BINDING");
}

#[test]
fn a_binding_must_name_a_state_parameter() {
    let s = lab_store();
    let e = s.read(
        &lab(),
        &ReadSource::Declared("active_count".into()),
        &bind(&[("culture", "c1")]),
        &json!({}),
        &json!({}),
        None,
    );
    assert!(e.is_err(), "{e:?}");
}

// --- projections (US2) ---------------------------------------------------------------------------

#[test]
fn a_store_projection_equals_the_plain_projection_of_the_same_state() {
    let mut s = lab_store();
    act(
        &mut s,
        "start_culture",
        &[],
        json!({"culture_id": "c3", "name": "Thyme"}),
    );
    act(&mut s, "measure", &[("culture", "c3")], json!({"ph": 6}));
    let x = read(&s, "active_cultures", &[], json!({}));
    let r = x.record.as_json();
    assert_eq!(r["result"], "VALUE", "{r}");
    // The same state, supplied as universes.
    let at = s.current().unwrap();
    let mut members = Vec::new();
    for id in ["c1", "c2", "c3"] {
        let key = behavior_store::documents::EntityKey {
            entity: "Culture".into(),
            id: id.into(),
        };
        members.push(s.load(&key, &at).unwrap().value);
    }
    let request = json!({"data_version": r["data_version"], "state": {}, "input": {},
                         "context": {}, "facts": {"universe": [
                             {"entity": "Culture", "members": members}]}});
    let plain = behavior_core::read::evaluate_read(
        &lab(),
        &ReadSource::Declared("active_cultures".into()),
        &request.to_string(),
    );
    assert_eq!(plain.record.as_json()["value"], r["value"]);
    assert_eq!(
        r["value"].as_array().unwrap().len(),
        2,
        "c1 and c3 are active: {r}"
    );
}

#[test]
fn an_entity_projection_of_an_unknown_identity_is_an_invalid_binding() {
    let s = lab_store();
    let x = read(&s, "order_view", &[("order", "o9")], json!({}));
    assert_eq!(x.record.as_json()["result"], "INVALID_BINDING");
    let x = read(&s, "order_view", &[("order", "o1")], json!({}));
    assert_eq!(
        x.record.as_json()["value"],
        json!({"id": "o1", "status": "OPEN", "amount": 30})
    );
}

/// A backend that counts its `version_at` calls (research R8: a projection fetches each member
/// once, however many items it projects).
struct Counting {
    inner: InMemoryBackend,
    version_at: std::cell::Cell<usize>,
}

impl Backend for Counting {
    fn genesis(
        &self,
    ) -> Result<Option<behavior_store::documents::Genesis>, behavior_store::BackendError> {
        self.inner.genesis()
    }
    fn head(
        &self,
    ) -> Result<Option<behavior_store::documents::Head>, behavior_store::BackendError> {
        self.inner.head()
    }
    fn create(
        &mut self,
        genesis: &behavior_store::documents::Genesis,
        head: &behavior_store::documents::Head,
        seed: &[behavior_store::documents::EntityVersion],
        seed_refs: &[behavior_store::documents::RefChange],
    ) -> Result<(), behavior_store::BackendError> {
        self.inner.create(genesis, head, seed, seed_refs)
    }
    fn version_at(
        &self,
        key: &behavior_store::documents::EntityKey,
        position: u64,
    ) -> Result<Option<behavior_store::documents::EntityVersion>, behavior_store::BackendError>
    {
        self.version_at.set(self.version_at.get() + 1);
        self.inner.version_at(key, position)
    }
    fn version(
        &self,
        key: &behavior_store::documents::EntityKey,
        revision: u64,
    ) -> Result<Option<behavior_store::documents::EntityVersion>, behavior_store::BackendError>
    {
        self.inner.version(key, revision)
    }
    fn record(
        &self,
        position: u64,
    ) -> Result<Option<behavior_store::documents::TransitionRecord>, behavior_store::BackendError>
    {
        self.inner.record(position)
    }
    fn removed_at(
        &self,
        key: &behavior_store::documents::EntityKey,
    ) -> Result<Option<u64>, behavior_store::BackendError> {
        self.inner.removed_at(key)
    }
    fn incoming_at(
        &self,
        target: &behavior_store::documents::EntityKey,
        position: u64,
    ) -> Result<Vec<behavior_store::RefEdge>, behavior_store::BackendError> {
        self.inner.incoming_at(target, position)
    }
    fn keys_at(
        &self,
        entity_type: &str,
        position: u64,
    ) -> Result<Vec<behavior_store::documents::EntityKey>, behavior_store::BackendError> {
        self.inner.keys_at(entity_type, position)
    }
    fn commit(
        &mut self,
        expected_last_record: &str,
        versions: &[behavior_store::documents::EntityVersion],
        removals: &[behavior_store::documents::EntityKey],
        ref_changes: &[behavior_store::documents::RefChange],
        record: &behavior_store::documents::TransitionRecord,
        new_head: &behavior_store::documents::Head,
    ) -> Result<behavior_store::CasOutcome, behavior_store::BackendError> {
        self.inner.commit(
            expected_last_record,
            versions,
            removals,
            ref_changes,
            record,
            new_head,
        )
    }
}

#[test]
fn a_projection_fetches_each_member_once() {
    let m = lab();
    let mut seed = Vec::new();
    for i in 0..20 {
        seed.push(SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": format!("c{i:02}"), "name": format!("culture {i}"),
                          "stage": null, "active": true, "measurements": 1, "ph_total": 7}),
        });
    }
    let backend = Counting {
        inner: InMemoryBackend::new(),
        version_at: std::cell::Cell::new(0),
    };
    let s = Store::create(backend, &m, genesis_for(&m, EvidencePolicy::none(), seed)).unwrap();
    s.backend().version_at.set(0);
    let x = s
        .read(
            &m,
            &ReadSource::Declared("active_cultures".into()),
            &BTreeMap::new(),
            &json!({}),
            &json!({}),
            None,
        )
        .unwrap();
    assert_eq!(x.record.as_json()["value"].as_array().unwrap().len(), 20);
    // The query checks each candidate once; the projection fetches each member once, for its two
    // stored fields and the two fields its derived item reads.
    let calls = s.backend().version_at.get();
    assert!(calls <= 40, "{calls} version_at calls for 20 members");
}
