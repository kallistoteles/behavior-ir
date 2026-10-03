#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: migration admission (FR-006–FR-010, research R4, R5). A migration is admitted
//! against its source and target modules; automatic copies are resolved into explicit `copy`
//! entries, removed fields need an explicit drop, and the resolved form is what is hashed.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use behavior_core::migration::admit_migration;
use behavior_core::semantic::module::Module;
use serde_json::{Value, json};

fn dir() -> std::path::PathBuf {
    common::fixtures().join("migration")
}

fn module(name: &str) -> Module {
    let p = dir().join("modules").join(format!("{name}.json"));
    behavior_core::admit(&common::read(&p)).unwrap()
}

fn case(kind: &str, name: &str) -> (String, Value) {
    let p = dir().join(kind);
    (
        common::read(&p.join(format!("{name}.json"))),
        common::json(&p.join(format!("{name}.expected.json"))),
    )
}

fn names(kind: &str) -> Vec<String> {
    common::files(&dir().join(kind), ".json")
        .iter()
        .map(|p| p.file_stem().unwrap().to_string_lossy().to_string())
        .collect()
}

#[test]
fn the_fixture_modules_name_the_schemas_the_migrations_relate() {
    let mut by_hash = BTreeMap::new();
    for m in ["cultures_v1", "cultures_v2", "cultures_v3", "cultures_v4"] {
        by_hash.insert(behavior_core::schema(&module(m)).hash, m.to_string());
    }
    for name in names("valid") {
        let (text, expected) = case("valid", &name);
        let doc: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(by_hash[doc["source"].as_str().unwrap()], expected["source"]);
        assert_eq!(by_hash[doc["target"].as_str().unwrap()], expected["target"]);
    }
}

#[test]
fn valid_migrations_are_admitted_with_their_summary() {
    let all = names("valid");
    assert!(all.len() >= 2, "{all:?}");
    for name in all {
        let (text, expected) = case("valid", &name);
        let source = module(expected["source"].as_str().unwrap());
        let target = module(expected["target"].as_str().unwrap());
        let m = admit_migration(&source, &target, &text)
            .unwrap_or_else(|e| panic!("{name}: {}", e.to_json_string()));
        assert_eq!(m.summary(), expected["summary"], "{name}");
        assert_eq!(m.source_schema(), behavior_core::schema(&source).hash);
        assert_eq!(m.target_schema(), behavior_core::schema(&target).hash);
        assert!(m.hash().starts_with("sha256:"));
    }
}

#[test]
fn automatic_copies_are_explicit_in_the_resolved_document() {
    let (text, _) = case("valid", "cultures_v1_to_v2");
    let m = admit_migration(&module("cultures_v1"), &module("cultures_v2"), &text).unwrap();
    let resolved = m.resolved();
    let transforms: BTreeMap<String, Value> = resolved["transforms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["entity"].as_str().unwrap().to_string(), t.clone()))
        .collect();
    // Every changed type has a resolved transform, even one the author did not mention.
    assert_eq!(
        transforms.keys().cloned().collect::<Vec<_>>(),
        vec!["Culture", "Customer", "Order"]
    );
    let culture = &transforms["Culture"]["fields"];
    assert_eq!(culture["status"], json!({"copy": "status"}));
    assert_eq!(culture["id"], json!({"copy": "id"}));
    // A field whose type changed is never copied automatically.
    assert!(culture["medium_type"].get("copy").is_none());
    assert!(culture["ph"].get("copy").is_none());
    assert_eq!(transforms["Culture"]["drops"], json!(["legacy_code"]));
    // Only the field order of Customer changed: all copies.
    for f in ["id", "name", "email"] {
        assert_eq!(transforms["Customer"]["fields"][f], json!({"copy": f}));
    }
    assert_eq!(resolved["retire"], json!(["AuditNote"]));
    // Unchanged types (none here besides the retired one) have no transform.
    let (text, _) = case("valid", "region_required");
    let m = admit_migration(&module("cultures_v2"), &module("cultures_v3"), &text).unwrap();
    let resolved = m.resolved();
    let entities: Vec<&str> = resolved["transforms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["entity"].as_str().unwrap())
        .collect();
    assert_eq!(
        entities,
        vec!["Order"],
        "Culture and Customer are unchanged"
    );
}

#[test]
fn the_resolved_document_is_admitted_to_the_same_migration() {
    let (text, _) = case("valid", "cultures_v1_to_v2");
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let m = admit_migration(&s, &t, &text).unwrap();
    let again = admit_migration(&s, &t, &m.resolved().to_string())
        .unwrap_or_else(|e| panic!("{}\n{}", e.to_json_string(), m.resolved()));
    assert_eq!(again.hash(), m.hash());
    assert_eq!(again.resolved(), m.resolved());
}

fn strip_locs(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if o.contains_key("loc") {
                o.insert("loc".into(), json!({"file": "elsewhere.py", "line": 99}));
            }
            for x in o.values_mut() {
                strip_locs(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_locs),
        _ => {}
    }
}

#[test]
fn the_identity_ignores_name_and_locations_but_not_content() {
    let (text, _) = case("valid", "cultures_v1_to_v2");
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    let h = admit_migration(&s, &t, &text).unwrap().hash();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    doc["name"] = json!("another_name");
    strip_locs(&mut doc);
    assert_eq!(admit_migration(&s, &t, &doc.to_string()).unwrap().hash(), h);
    // Writing an automatic copy explicitly is the same migration.
    doc["transforms"][0]["fields"]["status"] = json!({"copy": "status"});
    assert_eq!(admit_migration(&s, &t, &doc.to_string()).unwrap().hash(), h);
    // A different mapping is a different migration.
    doc["transforms"][0]["fields"]["notes"] = json!({
        "op": "lit", "type": {"t": "option", "of": {"t": "string"}}, "value": "n/a",
        "loc": {"file": "m.py", "line": 1},
    });
    assert_ne!(admit_migration(&s, &t, &doc.to_string()).unwrap().hash(), h);
}

#[test]
fn invalid_migrations_are_refused_with_their_codes() {
    let all = names("invalid");
    assert!(all.len() >= 8, "{all:?}");
    for name in all {
        let (text, expected) = case("invalid", &name);
        let source = module(expected["source"].as_str().unwrap());
        let target = module(expected["target"].as_str().unwrap());
        let r = match admit_migration(&source, &target, &text) {
            Ok(_) => panic!("{name}: admitted"),
            Err(r) => r,
        };
        let got: Vec<&str> = r.errors.iter().map(|e| e.code.as_str()).collect();
        let want: Vec<&str> = expected["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_str().unwrap())
            .collect();
        assert_eq!(got, want, "{name}: {}", r.to_json_string());
    }
}

#[test]
fn errors_name_what_is_wrong() {
    let (text, _) = case("invalid", "missing_field");
    let r = admit_migration(&module("cultures_v1"), &module("cultures_v2"), &text).unwrap_err();
    assert!(
        r.errors[0].message.contains("notes"),
        "{}",
        r.errors[0].message
    );
    let (text, _) = case("invalid", "partial_enum_map");
    let r = admit_migration(&module("cultures_v1"), &module("cultures_v2"), &text).unwrap_err();
    assert!(
        r.errors[0].message.contains("WPM"),
        "{}",
        r.errors[0].message
    );
    let (text, _) = case("invalid", "unacknowledged_drop");
    let r = admit_migration(&module("cultures_v1"), &module("cultures_v2"), &text).unwrap_err();
    assert!(
        r.errors[0].message.contains("legacy_code"),
        "{}",
        r.errors[0].message
    );
}

#[test]
fn malformed_documents_are_decode_errors() {
    let (s, t) = (module("cultures_v1"), module("cultures_v2"));
    for bad in [
        json!({"migration_ir": "0.1"}),
        json!({"migration_ir": "9.9", "name": "x", "source": "sha256:00", "target": "sha256:01", "transforms": []}),
    ] {
        let r = admit_migration(&s, &t, &bad.to_string()).unwrap_err();
        assert_eq!(r.errors[0].code, "DECODE_ERROR", "{bad}");
    }
    let (text, _) = case("valid", "cultures_v1_to_v2");
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    doc["transforms"][0]["fields"]["ph"]["op"] = json!("frobnicate");
    let r = admit_migration(&s, &t, &doc.to_string()).unwrap_err();
    assert_eq!(r.errors[0].code, "DECODE_ERROR");
}

#[test]
fn migration_operators_are_not_part_of_modules() {
    let p: &Path = &dir().join("modules/cultures_v2.json");
    let mut w = common::json(p);
    w["constraints"][0]["body"] = json!({
        "op": "ge", "loc": {"file": "x.py", "line": 1}, "args": [
            {"op": "strict_unwrap", "loc": {"file": "x.py", "line": 1},
             "args": [{"op": "field", "param": "c", "field": "notes", "loc": {"file": "x.py", "line": 1}}]},
            {"op": "lit", "type": {"t": "string"}, "value": "", "loc": {"file": "x.py", "line": 1}},
        ],
    });
    let r = behavior_core::admit(&w.to_string()).unwrap_err();
    assert_eq!(r.errors[0].code, "DECODE_ERROR", "{}", r.to_json_string());
}
