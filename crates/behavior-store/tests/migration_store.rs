#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: migrating a living store (FR-011–FR-015, FR-018, research R3, R6, R7). A
//! migration is one atomic transition at the next position: every entity of a changed type gets a
//! new version under the target schema, the head names the new schema, history keeps its bytes.

mod common;

use behavior_core::migration::{Migration, admit_migration};
use behavior_core::schema;
use behavior_core::semantic::module::Module;
use behavior_store::documents::{EntityKey, EvidencePolicy, SeedEntity, StoreError};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use common::{bind, fixtures, read};
use serde_json::{Value, json};

pub const T0: &str = "2026-10-02T09:00:00Z";
pub const T1: &str = "2026-10-02T10:00:00Z";

pub fn module(name: &str) -> Module {
    let p = fixtures()
        .join("migration/modules")
        .join(format!("{name}.json"));
    behavior_core::admit(&read(&p)).unwrap()
}

pub fn migration(name: &str, source: &Module, target: &Module) -> Migration {
    let p = fixtures()
        .join("migration/valid")
        .join(format!("{name}.json"));
    admit_migration(source, target, &read(&p)).unwrap()
}

fn seed(entity: &str, value: Value) -> SeedEntity {
    SeedEntity {
        entity: entity.into(),
        value,
    }
}

/// A V1 store: customers c1–c3, cultures k1–k2, orders o1 (no region) and o2.
pub fn v1_store() -> Store<InMemoryBackend> {
    v1_store_on(InMemoryBackend::new())
}

/// [`v1_store`] over any backend.
pub fn v1_store_on<B: Backend>(backend: B) -> Store<B> {
    let v1 = module("cultures_v1");
    let seed = vec![
        seed(
            "Customer",
            json!({"id": "c1", "name": "Ada", "email": "ada@x"}),
        ),
        seed(
            "Customer",
            json!({"id": "c2", "name": "Bo", "email": "bo@x"}),
        ),
        seed(
            "Customer",
            json!({"id": "c3", "name": "Cy", "email": "cy@x"}),
        ),
        seed(
            "Culture",
            json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                   "legacy_code": "L1", "price": "1.50", "fee": "0.1235"}),
        ),
        seed(
            "Culture",
            json!({"id": "k2", "medium": "MS", "status": "ACTIVE", "ph": 4,
                   "legacy_code": "L2", "price": "2.00", "fee": "0.0001"}),
        ),
        seed(
            "Order",
            json!({"id": "o1", "customer": "c1", "region": null, "qty": 3}),
        ),
        seed(
            "Order",
            json!({"id": "o2", "customer": "c2", "region": null, "qty": 1}),
        ),
    ];
    Store::create(backend, &v1, genesis_for(&v1, EvidencePolicy::none(), seed)).unwrap()
}

pub fn run<B: Backend>(
    s: &mut Store<B>,
    m: &Module,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) -> Result<behavior_store::Committed, StoreError> {
    let e = s.evaluate(m, action, &bind(bindings), &input, &json!({}), T0, None)?;
    let b = e
        .bundle
        .unwrap_or_else(|| panic!("{action} was not allowed: {}", e.record));
    s.commit(m, &b.evaluated_state.clone(), &b)
}

/// The decision of an action that must not be allowed: its result and its first reason's code.
pub fn decide<B: Backend>(
    s: &Store<B>,
    m: &Module,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) -> (String, String) {
    let e = s
        .evaluate(m, action, &bind(bindings), &input, &json!({}), T0, None)
        .unwrap();
    assert!(e.bundle.is_none(), "{action} was allowed");
    (
        e.record["result"].as_str().unwrap_or_default().to_string(),
        e.record["reasons"][0]["code"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    )
}

/// The V1 store after three commits: o2 gets a region, k2 dies, c3 is forgotten.
pub fn v1_history() -> Store<InMemoryBackend> {
    v1_history_on(InMemoryBackend::new())
}

/// [`v1_history`] over any backend.
pub fn v1_history_on<B: Backend>(backend: B) -> Store<B> {
    let v1 = module("cultures_v1");
    let mut s = v1_store_on(backend);
    run(
        &mut s,
        &v1,
        "set_region",
        &[("order", "o2")],
        json!({"region": "north"}),
    )
    .unwrap();
    run(&mut s, &v1, "kill", &[("culture", "k2")], json!({})).unwrap();
    run(
        &mut s,
        &v1,
        "forget_customer",
        &[("customer", "c3")],
        json!({}),
    )
    .unwrap();
    s
}

fn key(entity: &str, id: &str) -> EntityKey {
    EntityKey {
        entity: entity.into(),
        id: id.into(),
    }
}

#[test]
fn a_migration_commits_as_one_transition_at_the_next_position() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    let before = s.current().unwrap();
    assert_eq!(before.position, 3);
    let c = s.migrate(&m, &v1, &v2, T1, None).unwrap();
    assert!(!c.already);
    assert_eq!(c.result_state.position, 4);
    assert_ne!(c.result_state.state, before.state);
    assert_eq!(s.current().unwrap(), c.result_state);
    // Every surviving entity of a changed type has a new version under the target schema.
    for (entity, id) in [
        ("Customer", "c1"),
        ("Customer", "c2"),
        ("Culture", "k1"),
        ("Culture", "k2"),
        ("Order", "o1"),
        ("Order", "o2"),
    ] {
        let old = s.load(&key(entity, id), &before).unwrap();
        let new = s.load(&key(entity, id), &c.result_state).unwrap();
        assert_eq!(new.revision, old.revision + 1, "{entity}#{id}");
        assert_eq!(new.created_at, 4, "{entity}#{id}");
    }
    let k2 = s.load(&key("Culture", "k2"), &c.result_state).unwrap();
    assert_eq!(k2.value["medium_type"], json!("MS"));
    assert_eq!(k2.value["status"], json!("DEAD"));
    assert!(k2.value.get("legacy_code").is_none() && k2.value.get("medium").is_none());
    // The head names the new schema, introduced by the migration record.
    let head = s.backend().head().unwrap().unwrap();
    let hs = head
        .schema
        .expect("a migrated store's head names its schema");
    assert_eq!(hs.hash, schema(&v2).hash);
    assert_eq!(hs.since, 4);
    assert_eq!(hs.migration_record, c.record_id);
    let rec = s.backend().record(4).unwrap().unwrap();
    assert_eq!(rec.kind.as_deref(), Some("migration"));
    assert!(rec.bundle.is_none());
    let mb = rec.migration.as_ref().unwrap();
    assert_eq!(mb.migration_hash, m.hash());
    assert_eq!(mb.source, schema(&v1).hash);
    assert_eq!(mb.target, schema(&v2).hash);
    assert_eq!(mb.target_declarations, schema(&v2).declarations);
    assert!(
        mb.previous_schema.is_none(),
        "the previous schema is the genesis schema"
    );
}

#[test]
fn after_the_migration_the_store_speaks_only_the_new_schema() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    let before = s.current().unwrap();
    let k1_v1 = s.load(&key("Culture", "k1"), &before).unwrap();
    let c = s.migrate(&m, &v1, &v2, T1, None).unwrap();
    // V2 behavior commits.
    run(
        &mut s,
        &v2,
        "set_region",
        &[("order", "o1")],
        json!({"region": "south"}),
    )
    .unwrap();
    // V1 behavior is refused before it runs.
    let err = run(&mut s, &v1, "kill", &[("culture", "k1")], json!({})).unwrap_err();
    assert_eq!(err.code(), "SCHEMA_MISMATCH", "{err}");
    // History is read exactly as written, under its own schema.
    let again = s.load(&key("Culture", "k1"), &before).unwrap();
    assert_eq!(again, k1_v1);
    assert_eq!(
        serde_json::to_string(&again).unwrap(),
        serde_json::to_string(&k1_v1).unwrap()
    );
    assert_eq!(s.schema_at(&before).unwrap().hash, schema(&v1).hash);
    assert_eq!(s.schema_at(&c.result_state).unwrap().hash, schema(&v2).hash);
    let history = s.schema_history().unwrap();
    let got: Vec<(u64, String)> = history.iter().map(|h| (h.since, h.hash.clone())).collect();
    assert_eq!(got, vec![(0, schema(&v1).hash), (4, schema(&v2).hash)]);
    assert_eq!(history[1].migration_record, c.record_id);
}

#[test]
fn the_identity_registry_continues_across_a_migration() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    s.migrate(&m, &v1, &v2, T1, None).unwrap();
    // c3 was forgotten under V1: its identity stays used under V2.
    let (result, _) = decide(
        &s,
        &v2,
        "register_customer",
        &[],
        json!({"customer_id": "c3", "name": "Cy", "email": "cy@x"}),
    );
    assert_eq!(result, "ENTITY_ID_ALREADY_USED");
    run(
        &mut s,
        &v2,
        "register_customer",
        &[],
        json!({"customer_id": "c4", "name": "Di", "email": "di@x"}),
    )
    .unwrap();
}

#[test]
fn renaming_a_reference_field_moves_its_incoming_edges() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    let edges = |s: &Store<InMemoryBackend>, p: u64| {
        s.backend()
            .incoming_at(&key("Customer", "c1"), p)
            .unwrap()
            .into_iter()
            .map(|e| (e.entity, e.id, e.field))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        edges(&s, 3),
        vec![("Order".into(), "o1".into(), "customer".into())]
    );
    s.migrate(&m, &v1, &v2, T1, None).unwrap();
    assert_eq!(
        edges(&s, 4),
        vec![("Order".into(), "o1".into(), "buyer".into())]
    );
    assert_eq!(
        edges(&s, 3),
        vec![("Order".into(), "o1".into(), "customer".into())]
    );
    // Forgetting a referenced customer under V2 is still refused through the moved edge.
    let (result, reason) = decide(&s, &v2, "forget_customer", &[("customer", "c1")], json!({}));
    assert_eq!(
        (result.as_str(), reason.as_str()),
        ("DENY", "DANGLING_REFERENCE")
    );
}

#[test]
fn a_retired_type_with_entities_is_refused() {
    let v1 = module("cultures_v1");
    let v2 = module("cultures_v2");
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut seed_list = vec![seed("AuditNote", json!({"id": "n1", "text": "keep"}))];
    seed_list.push(seed(
        "Customer",
        json!({"id": "c1", "name": "A", "email": "a"}),
    ));
    let mut s = Store::create(
        InMemoryBackend::new(),
        &v1,
        genesis_for(&v1, EvidencePolicy::none(), seed_list),
    )
    .unwrap();
    let before = s.current().unwrap();
    let err = s.migrate(&m, &v1, &v2, T1, None).unwrap_err();
    assert_eq!(err.code(), "RETIRED_TYPE_NOT_EMPTY", "{err}");
    assert!(err.to_string().contains("AuditNote#n1"), "{err}");
    assert_eq!(s.current().unwrap(), before);
}

#[test]
fn a_migration_whose_source_is_not_the_current_schema_is_refused() {
    let (v1, v2, v3) = (
        module("cultures_v1"),
        module("cultures_v2"),
        module("cultures_v3"),
    );
    let m = migration("region_required", &v2, &v3);
    let mut s = v1_history();
    let err = s.migrate(&m, &v2, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_SCHEMA_MISMATCH", "{err}");
    // Modules that are not the migration's source and target are refused too.
    let m12 = migration("cultures_v1_to_v2", &v1, &v2);
    let err = s.migrate(&m12, &v1, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_SCHEMA_MISMATCH", "{err}");
}

#[test]
fn a_store_without_migrations_keeps_its_bytes() {
    // Action records of a migrating store keep the action record format: `kind` and `migration`
    // are absent, `bundle` is present.
    let s = v1_history();
    let rec = s.backend().record(1).unwrap().unwrap();
    let v = serde_json::to_value(&rec).unwrap();
    assert!(
        v.get("kind").is_none() && v.get("migration").is_none(),
        "{v}"
    );
    assert!(v.get("bundle").is_some());
}

#[test]
fn the_field_index_forgets_fields_a_migration_removed() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    let wpm = json!("WPM");
    let at3 = s
        .backend()
        .keys_by_field_at("Culture", "medium", &wpm, 3)
        .unwrap();
    assert_eq!(at3, Some(vec![key("Culture", "k1")]));
    s.migrate(&m, &v1, &v2, T1, None).unwrap();
    let gone = s
        .backend()
        .keys_by_field_at("Culture", "medium", &wpm, 4)
        .unwrap();
    assert_eq!(gone, Some(vec![]));
    let renamed = s
        .backend()
        .keys_by_field_at("Culture", "medium_type", &wpm, 4)
        .unwrap();
    assert_eq!(renamed, Some(vec![key("Culture", "k1")]));
}

// --- feature 009, US2: refusals leave the store exactly as it was ------------------------------

use behavior_store::conformance::{Fault, FaultInjector};

fn snapshot<B: Backend>(s: &Store<B>) -> (String, Vec<(u64, String)>) {
    let head = serde_json::to_string(&s.backend().head().unwrap().unwrap()).unwrap();
    let history = s
        .schema_history()
        .unwrap()
        .into_iter()
        .map(|h| (h.since, h.hash))
        .collect();
    (head, history)
}

fn doc(name: &str) -> Value {
    serde_json::from_str(&read(
        &fixtures()
            .join("migration/valid")
            .join(format!("{name}.json")),
    ))
    .unwrap()
}

#[test]
fn every_refusal_leaves_position_state_head_and_schema_unchanged() {
    let (v1, v2, v3) = (
        module("cultures_v1"),
        module("cultures_v2"),
        module("cultures_v3"),
    );
    let loc = json!({"file": "t.py", "line": 1});
    let mut s = v1_history();
    // 1. A result that breaks a target constraint (ph - 10 is negative).
    let mut d = doc("cultures_v1_to_v2");
    d["transforms"][0]["fields"]["ph"] = json!({"op": "sub", "loc": loc, "args": [
        {"op": "field", "param": "old", "field": "ph", "loc": loc},
        {"op": "lit", "type": {"t": "int"}, "value": 10, "loc": loc},
    ]});
    let bad = admit_migration(&v1, &v2, &d.to_string()).unwrap();
    let before = snapshot(&s);
    let err = s.migrate(&bad, &v1, &v2, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_INVALID_RESULT", "{err}");
    assert_eq!(snapshot(&s), before);
    // 2. The schema the store is not at.
    let early = migration("region_required", &v2, &v3);
    let err = s.migrate(&early, &v2, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_SCHEMA_MISMATCH", "{err}");
    assert_eq!(snapshot(&s), before);
    // Broaden, then: 3. a requirement that does not hold yet, 4. an unproven narrowing.
    s.migrate(
        &migration("cultures_v1_to_v2", &v1, &v2),
        &v1,
        &v2,
        T1,
        None,
    )
    .unwrap();
    let before = snapshot(&s);
    let err = s.migrate(&early, &v2, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_REQUIREMENT_FAILED", "{err}");
    assert!(err.to_string().contains("1 Order entity"), "{err}");
    assert_eq!(snapshot(&s), before);
    let mut d = doc("region_required");
    d["requirements"] = json!([]);
    let unproven = admit_migration(&v2, &v3, &d.to_string()).unwrap();
    let err = s.migrate(&unproven, &v2, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_TRANSFORM_ERROR", "{err}");
    assert_eq!(snapshot(&s), before);
}

#[test]
fn a_migration_is_all_or_nothing_across_a_crash() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    // Nothing written: the store reopens under V1, unchanged.
    let mut s = v1_history_on(FaultInjector::new(InMemoryBackend::new()));
    let before = snapshot(&s);
    s.backend_mut().next = Some(Fault::BeforeWrite);
    assert_eq!(
        s.migrate(&m, &v1, &v2, T1, None).unwrap_err().code(),
        "BACKEND_ERROR"
    );
    let s = Store::open(s.into_backend()).unwrap();
    assert_eq!(snapshot(&s), before);
    // Written, acknowledgement lost: the store reopens fully migrated.
    let mut s = v1_history_on(FaultInjector::new(InMemoryBackend::new()));
    s.backend_mut().next = Some(Fault::AfterWrite);
    assert_eq!(
        s.migrate(&m, &v1, &v2, T1, None).unwrap_err().code(),
        "BACKEND_ERROR"
    );
    let s = Store::open(s.into_backend()).unwrap();
    let history: Vec<u64> = s
        .schema_history()
        .unwrap()
        .iter()
        .map(|h| h.since)
        .collect();
    assert_eq!(history, vec![0, 4]);
    let at = s.current().unwrap();
    for (entity, id) in [("Culture", "k1"), ("Culture", "k2"), ("Order", "o1")] {
        assert_eq!(s.load(&key(entity, id), &at).unwrap().created_at, 4);
    }
}

#[test]
fn an_action_evaluated_before_a_migration_conflicts_after_it() {
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let mut s = v1_history();
    let e = s
        .evaluate(
            &v1,
            "kill",
            &bind(&[("culture", "k1")]),
            &json!({}),
            &json!({}),
            T0,
            None,
        )
        .unwrap();
    let b = e.bundle.unwrap();
    s.migrate(
        &migration("cultures_v1_to_v2", &v1, &v2),
        &v1,
        &v2,
        T1,
        None,
    )
    .unwrap();
    let err = s.commit(&v1, &b.evaluated_state.clone(), &b).unwrap_err();
    assert_eq!(err.code(), "STATE_CONFLICT", "{err}");
}

#[test]
fn the_staged_path_broadens_backfills_and_then_narrows() {
    let (v1, v2, v3) = (
        module("cultures_v1"),
        module("cultures_v2"),
        module("cultures_v3"),
    );
    let mut s = v1_history();
    s.migrate(
        &migration("cultures_v1_to_v2", &v1, &v2),
        &v1,
        &v2,
        T1,
        None,
    )
    .unwrap();
    let narrow = migration("region_required", &v2, &v3);
    let err = s.migrate(&narrow, &v2, &v3, T1, None).unwrap_err();
    assert_eq!(err.code(), "MIGRATION_REQUIREMENT_FAILED");
    let StoreError::Migration(r) = &err else {
        panic!("{err}")
    };
    assert_eq!(
        (r.count, r.entities.clone()),
        (1, vec![("Order".to_string(), "o1".to_string())])
    );
    // Backfill with an ordinary action, then narrow.
    run(
        &mut s,
        &v2,
        "set_region",
        &[("order", "o1")],
        json!({"region": "south"}),
    )
    .unwrap();
    let c = s.migrate(&narrow, &v2, &v3, T1, None).unwrap();
    let o1 = s.load(&key("Order", "o1"), &c.result_state).unwrap();
    assert_eq!(o1.value["region"], json!("south"));
    let history: Vec<String> = s
        .schema_history()
        .unwrap()
        .into_iter()
        .map(|h| h.hash)
        .collect();
    assert_eq!(
        history,
        vec![schema(&v1).hash, schema(&v2).hash, schema(&v3).hash]
    );
}

#[test]
fn reference_replay_follows_the_schema_of_each_position() {
    use behavior_store::replay::{replay_index, replay_index_with};
    use std::collections::BTreeMap;
    let (v1, v2) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &v1, &v2);
    let mut s = v1_history();
    s.migrate(&m, &v1, &v2, T1, None).unwrap();
    run(
        &mut s,
        &v2,
        "set_region",
        &[("order", "o1")],
        json!({"region": "x"}),
    )
    .unwrap();
    let (from, to) = (s.state_at(0).unwrap(), s.current().unwrap());
    let modules = BTreeMap::from([
        (schema(&v1).hash, v1.clone()),
        (schema(&v2).hash, v2.clone()),
    ]);
    let r = replay_index_with(&s, &modules, &from, &to);
    assert!(r.ok, "{:?}", r.divergence);
    // One module cannot replay the other schema: that is named, not a false reference divergence.
    let single = replay_index(&s, &v2, &from, &to);
    let d = single.divergence.unwrap();
    assert_eq!((d.position, d.kind.as_str()), (0, "schema"), "{d:?}");
}
