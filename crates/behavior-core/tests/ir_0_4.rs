#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Wire IR 0.4 (feature 004): only 0.4 documents are admitted; the version step changes the
//! envelope, not the semantic identity of unchanged modules.

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::{Value, json};

/// Modules without decimal arithmetic: their meaning does not change in feature 004.
const UNCHANGED: [&str; 8] = [
    "wire/valid/empty.json",
    "verify/dead_and_vacuous.json",
    "verify/overflow.json",
    "verify/purchase_money2.json",
    "verify/purchase_money2_fixed.json",
    "verify/purchase_money2_remaining.json",
    "verify/invoice_money2.json",
    "verify/rescale_modes.json",
];

#[test]
fn old_versions_are_rejected_with_a_migration_message() {
    let mut doc = common::json(&common::fixtures().join("wire/valid/empty.json"));
    for v in ["0.1", "0.2", "0.3"] {
        doc["ir_version"] = json!(v);
        let r = admission_report(&doc.to_string());
        assert!(!r.ok, "{v}");
        let e = &r.errors[0];
        assert_eq!(e.code.to_string(), "UNSUPPORTED_IR_VERSION", "{v}");
        assert!(
            e.message.contains("0.4") && e.message.contains("scale"),
            "{}",
            e.message
        );
    }
}

#[test]
fn unchanged_modules_keep_their_semantic_identity() {
    let frozen = common::json(&common::fixtures().join("frozen_versions.json"));
    for f in UNCHANGED {
        let text = common::read(&common::fixtures().join(f));
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["ir_version"], "0.4", "{f}");
        let r = admission_report(&text);
        assert_eq!(r.behavior_version.as_deref(), frozen[f].as_str(), "{f}");
    }
}

#[test]
fn serialization_always_writes_0_4() {
    for f in [
        "wire/valid/empty.json",
        "wire/valid/project_margin.json",
        "wire/valid/fixed_scale.json",
    ] {
        let m = admit(&common::read(&common::fixtures().join(f))).unwrap();
        let v: Value = serde_json::from_str(&to_wire_json(&m)).unwrap();
        assert_eq!(v["ir_version"], "0.4", "{f}");
        assert!(v["constraints"].is_array(), "{f}");
    }
}

#[test]
fn exact_type_forms_decode() {
    // `{"t": "exact"}` and `exact` of an unscaled nominal are well-formed 0.4 types; whether a
    // body has that type is a typing question, never a decode or version error.
    let l = json!({"file": "t.py", "line": 1});
    for ty in [
        json!({"t": "exact"}),
        json!({"t": "exact", "name": "Plain"}),
    ] {
        let doc = json!({
            "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [], "actions": [],
            "nominals": [{"name": "Plain", "underlying": {"t": "decimal"}, "ops": ["add"], "loc": l}],
            "entities": [{"name": "E", "loc": l, "fields": [{"name": "p", "type": {"t": "nominal", "name": "Plain"}, "loc": l}]}],
            "derived": [{"name": "d", "kind": "derived", "loc": l, "type": ty,
                         "params": [{"name": "x", "type": {"t": "entity", "name": "E"}}],
                         "body": {"op": "field", "param": "x", "field": "p", "loc": l}}],
        });
        let r = admission_report(&doc.to_string());
        let codes: Vec<String> = r.errors.iter().map(|e| e.code.to_string()).collect();
        assert!(
            codes.iter().all(|c| c != "DECODE_ERROR"
                && c != "UNSUPPORTED_IR_VERSION"
                && c != "EXACT_NOT_FIXED_SCALE"),
            "{ty}: {codes:?}"
        );
    }
}
