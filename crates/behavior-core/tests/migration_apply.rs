#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: applying a migration to a supplied source universe (FR-007, FR-008, FR-011,
//! research R6). Transforms are entity-local: the result does not depend on the order in which
//! entities are supplied, identities are preserved, and unchanged types are carried over as they
//! are.

mod common;

use behavior_core::migration::{Migration, SourceEntity, admit_migration, apply_migration};
use behavior_core::semantic::module::Module;
use serde_json::{Value, json};

fn module(name: &str) -> Module {
    let p = common::fixtures()
        .join("migration/modules")
        .join(format!("{name}.json"));
    behavior_core::admit(&common::read(&p)).unwrap()
}

fn migration(name: &str, source: &Module, target: &Module) -> Migration {
    let p = common::fixtures()
        .join("migration/valid")
        .join(format!("{name}.json"));
    admit_migration(source, target, &common::read(&p)).unwrap()
}

fn e(entity: &str, value: Value) -> SourceEntity {
    SourceEntity {
        entity: entity.into(),
        value,
    }
}

/// A V1 universe: two customers, two cultures, two orders (one without a region).
pub fn v1_universe() -> Vec<SourceEntity> {
    vec![
        e(
            "Customer",
            json!({"id": "c1", "name": "Ada", "email": "ada@x"}),
        ),
        e(
            "Customer",
            json!({"id": "c2", "name": "Bo", "email": "bo@x"}),
        ),
        e(
            "Culture",
            json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                   "legacy_code": "L1", "price": "1.50", "fee": "0.1235"}),
        ),
        e(
            "Culture",
            json!({"id": "k2", "medium": "MS", "status": "DEAD", "ph": 0,
                   "legacy_code": "L2", "price": "2.00", "fee": "0.0001"}),
        ),
        e(
            "Order",
            json!({"id": "o1", "customer": "c1", "region": null, "qty": 3}),
        ),
        e(
            "Order",
            json!({"id": "o2", "customer": "c2", "region": "north", "qty": 1}),
        ),
    ]
}

fn canonical(target: &Module, entity: &str, v: Value) -> Value {
    behavior_core::canonical_entity(target, entity, &v).unwrap()
}

fn value_of<'a>(out: &'a behavior_core::migration::Migrated, entity: &str, id: &str) -> &'a Value {
    &out.entities
        .iter()
        .find(|m| m.entity == entity && m.id == id)
        .unwrap_or_else(|| panic!("{entity}#{id} missing"))
        .value
}

#[test]
fn a_migration_produces_the_target_values() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let out = apply_migration(&m, &s, &t, &v1_universe()).unwrap();
    assert_eq!(
        value_of(&out, "Culture", "k1"),
        &canonical(
            &t,
            "Culture",
            json!({"id": "k1", "medium_type": "WPM", "status": "ACTIVE", "ph": "7",
                   "notes": null, "price": "1.50", "fee": "0.12"})
        )
    );
    assert_eq!(
        value_of(&out, "Culture", "k2"),
        &canonical(
            &t,
            "Culture",
            json!({"id": "k2", "medium_type": "MS", "status": "DEAD", "ph": "0",
                   "notes": null, "price": "2", "fee": "0.00"})
        )
    );
    assert_eq!(
        value_of(&out, "Order", "o1"),
        &canonical(
            &t,
            "Order",
            json!({"id": "o1", "buyer": "c1", "region": null, "qty": 3})
        )
    );
    assert_eq!(
        value_of(&out, "Customer", "c2"),
        &canonical(
            &t,
            "Customer",
            json!({"id": "c2", "name": "Bo", "email": "bo@x"})
        )
    );
}

#[test]
fn identities_are_preserved_one_to_one() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let input = v1_universe();
    let out = apply_migration(&m, &s, &t, &input).unwrap();
    let mut got: Vec<(String, String)> = out
        .entities
        .iter()
        .map(|x| (x.entity.clone(), x.id.clone()))
        .collect();
    let mut want: Vec<(String, String)> = input
        .iter()
        .map(|x| {
            (
                x.entity.clone(),
                x.value["id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    got.sort();
    want.sort();
    assert_eq!(got, want);
    for x in &out.entities {
        assert_eq!(x.value["id"], json!(x.id));
    }
}

#[test]
fn the_result_does_not_depend_on_the_order_of_the_universe() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let forward = apply_migration(&m, &s, &t, &v1_universe()).unwrap();
    let mut reversed_input = v1_universe();
    reversed_input.reverse();
    let reversed = apply_migration(&m, &s, &t, &reversed_input).unwrap();
    assert_eq!(forward, reversed);
}

#[test]
fn unchanged_types_are_carried_over_as_they_are() {
    let (s2, s3) = (module("cultures_v2"), module("cultures_v3"));
    let m = migration("region_required", &s2, &s3);
    let universe = vec![
        e(
            "Customer",
            json!({"id": "c1", "name": "Ada", "email": "ada@x"}),
        ),
        e(
            "Culture",
            json!({"id": "k1", "medium_type": "B5", "status": "ACTIVE", "ph": "7.5",
                   "notes": null, "price": "1.0000", "fee": "0.10"}),
        ),
        e(
            "Order",
            json!({"id": "o1", "buyer": "c1", "region": "north", "qty": 3}),
        ),
    ];
    let out = apply_migration(&m, &s2, &s3, &universe).unwrap();
    for x in &out.entities {
        assert_eq!(x.migrated, x.entity == "Order", "{}", x.entity);
    }
    assert_eq!(
        value_of(&out, "Culture", "k1"),
        &canonical(&s2, "Culture", universe[1].value.clone())
    );
    assert_eq!(value_of(&out, "Order", "o1")["region"], json!("north"));
}

#[test]
fn modules_that_are_not_the_migrations_schemas_are_refused() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let r = apply_migration(&m, &t, &t, &[]).unwrap_err();
    assert_eq!(r.code, "MIGRATION_SCHEMA_MISMATCH");
}

#[test]
fn a_retired_type_must_be_empty() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let mut universe = v1_universe();
    universe.push(e("AuditNote", json!({"id": "n1", "text": "hi"})));
    let r = apply_migration(&m, &s, &t, &universe).unwrap_err();
    assert_eq!(r.code, "RETIRED_TYPE_NOT_EMPTY");
    assert!(r.message.contains("AuditNote#n1"), "{}", r.message);
}

// --- feature 009, US2: every refusal names its rule and the entities ---------------------------

fn loc() -> Value {
    json!({"file": "t.py", "line": 1})
}

fn doc(name: &str) -> Value {
    common::json(
        &common::fixtures()
            .join("migration/valid")
            .join(format!("{name}.json")),
    )
}

fn module_wire(name: &str) -> Value {
    common::json(
        &common::fixtures()
            .join("migration/modules")
            .join(format!("{name}.json")),
    )
}

fn admit_module(w: &Value) -> Module {
    behavior_core::admit(&w.to_string()).unwrap_or_else(|e| panic!("{}", e.to_json_string()))
}

/// A V2 universe with `n` orders without a region and two with one.
fn v2_orders(missing: usize) -> Vec<SourceEntity> {
    let mut u = vec![e(
        "Customer",
        json!({"id": "c1", "name": "Ada", "email": "a@x"}),
    )];
    for i in 0..missing {
        u.push(e(
            "Order",
            json!({"id": format!("o{i:02}"), "buyer": "c1", "region": null, "qty": 1}),
        ));
    }
    for id in ["x1", "x2"] {
        u.push(e(
            "Order",
            json!({"id": id, "buyer": "c1", "region": "north", "qty": 1}),
        ));
    }
    u
}

#[test]
fn a_failed_requirement_names_itself_and_counts_the_violators() {
    let (s, t) = (module("cultures_v2"), module("cultures_v3"));
    let m = migration("region_required", &s, &t);
    let r = apply_migration(&m, &s, &t, &v2_orders(17)).unwrap_err();
    assert_eq!(r.code, "MIGRATION_REQUIREMENT_FAILED");
    assert_eq!(r.rule.as_deref(), Some("every_order_has_region"));
    assert_eq!(r.count, 17);
    assert_eq!(r.entities.len(), 10, "the first ten are named");
    assert_eq!(r.entities[0], ("Order".to_string(), "o00".to_string()));
    assert!(r.message.contains("17 Order entities"), "{}", r.message);
    assert!(r.message.contains("Order#o00"), "{}", r.message);
    // With the data fixed, the same migration applies.
    assert!(apply_migration(&m, &s, &t, &v2_orders(0)).is_ok());
}

#[test]
fn an_unproven_strict_unwrap_of_an_absent_value_is_a_transform_error() {
    let (s, t) = (module("cultures_v2"), module("cultures_v3"));
    let mut d = doc("region_required");
    d["requirements"] = json!([]);
    let m = admit_migration(&s, &t, &d.to_string()).unwrap();
    let r = apply_migration(&m, &s, &t, &v2_orders(1)).unwrap_err();
    assert_eq!(r.code, "MIGRATION_TRANSFORM_ERROR");
    assert_eq!(r.entities, vec![("Order".to_string(), "o00".to_string())]);
    assert!(r.message.contains("Order#o00.region"), "{}", r.message);
}

#[test]
fn a_violated_target_constraint_refuses_the_whole_migration() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let mut d = doc("cultures_v1_to_v2");
    // ph - 10: k1 (7) becomes -3, which breaks the target constraint `ph_not_negative`.
    d["transforms"][0]["fields"]["ph"] = json!({"op": "sub", "loc": loc(), "args": [
        {"op": "field", "param": "old", "field": "ph", "loc": loc()},
        {"op": "lit", "type": {"t": "int"}, "value": 10, "loc": loc()},
    ]});
    let m = admit_migration(&s, &t, &d.to_string()).unwrap();
    let r = apply_migration(&m, &s, &t, &v1_universe()).unwrap_err();
    assert_eq!(r.code, "MIGRATION_INVALID_RESULT");
    assert_eq!(r.rule.as_deref(), Some("ph_not_negative"));
    assert_eq!(
        r.entities,
        vec![
            ("Culture".to_string(), "k1".to_string()),
            ("Culture".to_string(), "k2".to_string())
        ]
    );
}

#[test]
fn a_violated_target_entity_invariant_refuses_the_migration() {
    let s = module("cultures_v1");
    let mut tw = module_wire("cultures_v2");
    tw["invariants"] = json!([{
        "name": "notes_stay_empty", "entity": "Culture", "param": "c", "loc": loc(),
        "body": {"op": "is_none", "loc": loc(),
                 "args": [{"op": "field", "param": "c", "field": "notes", "loc": loc()}]},
    }]);
    let t = admit_module(&tw);
    let mut d = doc("cultures_v1_to_v2");
    d["transforms"][0]["fields"]["notes"] = json!({
        "op": "lit", "type": {"t": "option", "of": {"t": "string"}}, "value": "x", "loc": loc(),
    });
    let m = admit_migration(&s, &t, &d.to_string()).unwrap();
    let r = apply_migration(&m, &s, &t, &v1_universe()).unwrap_err();
    assert_eq!(r.code, "MIGRATION_INVALID_RESULT");
    assert_eq!(r.rule.as_deref(), Some("notes_stay_empty"));
    assert_eq!(r.count, 2);
}

#[test]
fn a_violated_target_module_invariant_refuses_the_migration() {
    let s = module("cultures_v1");
    let mut tw = module_wire("cultures_v2");
    tw["invariants"] = json!([{
        "name": "quantities_unique", "loc": loc(),
        "body": {"op": "unique", "param": "o", "loc": loc(),
                 "args": [{"op": "select", "entity": "Order", "loc": loc()}],
                 "body": {"op": "field", "param": "o", "field": "qty", "loc": loc()}},
    }]);
    let t = admit_module(&tw);
    let m = admit_migration(&s, &t, &doc("cultures_v1_to_v2").to_string()).unwrap();
    let mut universe = v1_universe();
    universe.push(e(
        "Order",
        json!({"id": "o3", "customer": "c1", "region": null, "qty": 3}),
    ));
    let r = apply_migration(&m, &s, &t, &universe).unwrap_err();
    assert_eq!(r.code, "MIGRATION_INVALID_RESULT");
    assert_eq!(r.rule.as_deref(), Some("quantities_unique"));
}

#[test]
fn a_dangling_reference_refuses_the_migration() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let mut d = doc("cultures_v1_to_v2");
    d["transforms"][1]["fields"]["buyer"] = json!({
        "op": "lit", "type": {"t": "id", "entity": "Customer"}, "value": "ghost", "loc": loc(),
    });
    d["transforms"][1]["drops"] = json!(["customer"]);
    let m = admit_migration(&s, &t, &d.to_string()).unwrap();
    let r = apply_migration(&m, &s, &t, &v1_universe()).unwrap_err();
    assert_eq!(r.code, "MIGRATION_INVALID_RESULT");
    assert_eq!(r.count, 2, "{}", r.message);
    assert!(
        r.entities.iter().all(|(e, _)| e == "Order"),
        "{:?}",
        r.entities
    );
}

#[test]
fn a_source_state_that_breaks_a_source_rule_is_refused_before_anything_is_transformed() {
    // The source rules are behavior, not schema: data written under earlier behavior of the same
    // schema may break them, and verification assumes them.
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = migration("cultures_v1_to_v2", &s, &t);
    let mut universe = v1_universe();
    universe[2].value["ph"] = json!(-1);
    let r = apply_migration(&m, &s, &t, &universe).unwrap_err();
    assert_eq!(r.code, "MIGRATION_SOURCE_INVALID");
    assert_eq!(r.rule.as_deref(), Some("ph_not_negative"));
    assert_eq!(r.entities, vec![("Culture".to_string(), "k1".to_string())]);
}
