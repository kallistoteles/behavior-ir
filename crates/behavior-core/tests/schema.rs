#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: the store schema and its SchemaHash (FR-001, research R1). The schema is the set of
//! entity declarations; behavior-only definitions never change it, every declaration change does.
//! Run with `BLESS_SCHEMA_HASHES=1` to fill in missing snapshot entries.

mod common;

use std::collections::BTreeMap;

use behavior_core::semantic::module::{Kind, Module};
use behavior_core::semantic::types::hash_display;
use behavior_core::{StoreSchema, schema};
use serde_json::{Value, json};

fn loc() -> Value {
    json!({"file": "m.py", "line": 1})
}

fn field(name: &str, ty: Value) -> Value {
    json!({"name": name, "type": ty, "loc": loc()})
}

/// A small module: an enum, two entities (one referencing the other), a derived value, a rule,
/// an invariant and an action.
fn base() -> Value {
    json!({
        "ir_version": "0.6",
        "enums": [{"name": "Status", "values": ["OPEN", "DONE"], "loc": loc()}],
        "nominals": [],
        "entities": [
            {"name": "Customer", "loc": loc(), "fields": [field("name", json!({"t": "string"}))]},
            {"name": "Order", "loc": loc(), "fields": [
                field("status", json!({"t": "enum", "name": "Status"})),
                field("qty", json!({"t": "int"})),
                field("customer", json!({"t": "ref", "entity": "Customer"})),
            ]},
        ],
        "derived": [{
            "name": "is_open", "kind": "derived", "loc": loc(),
            "params": [{"name": "o", "type": {"t": "entity", "name": "Order"}}],
            "body": {"op": "eq", "loc": loc(), "args": [
                {"op": "field", "param": "o", "field": "status", "loc": loc()},
                {"op": "lit", "type": {"t": "enum", "name": "Status"}, "value": "OPEN", "loc": loc()},
            ]},
        }],
        "invariants": [],
        "constraints": [],
        "actions": [{
            "name": "finish", "loc": loc(),
            "params": [{"name": "o", "role": "state", "type": {"t": "entity", "name": "Order"}}],
            "preconditions": [{"loc": loc(), "expr": {"op": "derived", "name": "is_open", "args": ["o"], "loc": loc()}}],
            "effects": [{"target": {"param": "o", "field": "status"}, "loc": loc(),
                         "value": {"op": "lit", "type": {"t": "enum", "name": "Status"}, "value": "DONE", "loc": loc()}}],
            "postconditions": [],
        }],
    })
}

fn admit(w: &Value) -> Module {
    behavior_core::admit(&w.to_string()).unwrap_or_else(|e| panic!("{}", e.to_json_string()))
}

fn order_fields(w: &mut Value) -> &mut Vec<Value> {
    w["entities"][1]["fields"].as_array_mut().unwrap()
}

#[test]
fn behavior_only_changes_keep_the_schema_hash() {
    let a = admit(&base());
    // No action at all.
    let mut b = base();
    b["actions"] = json!([]);
    // A different effect value and an extra precondition.
    let mut c = base();
    c["actions"][0]["effects"][0]["value"]["value"] = json!("OPEN");
    // A constraint (rules are behavior, not schema).
    let mut d = base();
    d["constraints"] = json!([{
        "name": "positive_qty", "entity": "Order", "param": "o", "loc": loc(),
        "body": {"op": "gt", "loc": loc(), "args": [
            {"op": "field", "param": "o", "field": "qty", "loc": loc()},
            {"op": "lit", "type": {"t": "int"}, "value": 0, "loc": loc()},
        ]},
    }]);
    let h = schema(&a).hash;
    for (what, w) in [("no actions", b), ("other effect", c), ("a constraint", d)] {
        let m = admit(&w);
        assert_ne!(m.behavior_version(), a.behavior_version(), "{what}");
        assert_eq!(schema(&m).hash, h, "{what} must not change the schema");
    }
}

#[test]
fn every_declaration_change_changes_the_schema_hash() {
    let h = schema(&admit(&base())).hash;
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut w = base();
    w["enums"][0]["values"] = json!(["OPEN", "DONE", "HELD"]);
    cases.push(("an enum gains a value", w));
    let mut w = base();
    order_fields(&mut w).push(field("note", json!({"t": "option", "of": {"t": "string"}})));
    cases.push(("a field is added", w));
    let mut w = base();
    order_fields(&mut w)[1]["name"] = json!("quantity");
    cases.push(("a field is renamed", w));
    let mut w = base();
    order_fields(&mut w)[1]["type"] = json!({"t": "decimal"});
    cases.push(("a field type changes", w));
    let mut w = base();
    order_fields(&mut w)[2]["type"] = json!({"t": "id", "entity": "Customer"});
    cases.push(("a reference becomes a plain id", w));
    let mut w = base();
    order_fields(&mut w).swap(0, 1);
    cases.push(("the field order changes", w));
    let mut w = base();
    w["entities"].as_array_mut().unwrap().push(json!(
        {"name": "Note", "loc": loc(), "fields": [field("text", json!({"t": "string"}))]}
    ));
    cases.push(("an entity type is added", w));
    for (what, w) in cases {
        assert_ne!(schema(&admit(&w)).hash, h, "{what} must change the schema");
    }
}

#[test]
fn the_schema_lists_every_entity_declaration() {
    let m = admit(&base());
    let expected: BTreeMap<String, String> = m
        .name_table()
        .iter()
        .filter(|((k, _), _)| *k == Kind::Entity)
        .map(|((_, n), h)| (n.clone(), hash_display(h)))
        .collect();
    let s = schema(&m);
    assert_eq!(s.declarations, expected);
    assert_eq!(s.declarations.len(), 2);
    // The schema of a stored declaration map is the same schema.
    assert_eq!(StoreSchema::of(expected), s);
    assert!(s.hash.starts_with("sha256:"));
}

#[test]
fn differing_names_the_entity_types_whose_declarations_differ() {
    let a = schema(&admit(&base()));
    let mut w = base();
    w["enums"][0]["values"] = json!(["OPEN", "DONE", "HELD"]);
    w["entities"].as_array_mut().unwrap().push(json!(
        {"name": "Note", "loc": loc(), "fields": [field("text", json!({"t": "string"}))]}
    ));
    let b = schema(&admit(&w));
    assert_eq!(
        a.differing(&b),
        vec!["Note".to_string(), "Order".to_string()]
    );
    assert!(a.differing(&a).is_empty());
}

/// A snapshot of the SchemaHash of every valid wire fixture: the hash is part of every store's
/// identity from 0.9 on and must never change by accident.
#[test]
fn schema_hashes_of_the_wire_fixtures_are_frozen() {
    let path = common::fixtures().join("schema_hashes.json");
    let bless = std::env::var("BLESS_SCHEMA_HASHES").is_ok();
    let mut doc: serde_json::Map<String, Value> = if path.exists() {
        serde_json::from_str(&common::read(&path)).unwrap()
    } else {
        serde_json::Map::new()
    };
    let mut missing = Vec::new();
    let mut changed = false;
    for p in common::files(&common::fixtures().join("wire/valid"), ".json") {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let got = json!(schema(&behavior_core::admit(&common::read(&p)).unwrap()).hash);
        match doc.get(&name) {
            Some(want) => assert_eq!(&got, want, "{name}: the SchemaHash changed"),
            None if bless => {
                doc.insert(name, got);
                changed = true;
            }
            None => missing.push(name),
        }
    }
    if changed {
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
    assert!(
        missing.is_empty(),
        "no snapshot for {missing:?} (run with BLESS_SCHEMA_HASHES=1)"
    );
}
