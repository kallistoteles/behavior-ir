#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Wire IR 0.6 (feature 007): relational forms need 0.6, earlier documents without them stay
//! valid, and every module without a 007 form keeps its identity and item hashes (SC-006).

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::{Value, json};

fn orders() -> Value {
    common::json(&common::fixtures().join("wire/valid/orders.json"))
}

#[test]
fn modules_without_007_forms_keep_identity_and_version() {
    let mut failures = Vec::new();
    for snapshot in ["frozen_versions_006.json", "frozen_versions_007.json"] {
        let frozen = common::json(&common::fixtures().join(snapshot));
        for (file, expected) in frozen["modules"].as_object().unwrap() {
            let text = common::read(&common::fixtures().join(file));
            let r = admission_report(&text);
            if r.behavior_version.as_deref() != expected["behavior_version"].as_str() {
                failures.push(format!("{snapshot} {file}: {:?}", r.behavior_version));
            }
            if serde_json::to_value(&r.items).unwrap() != expected["items"] {
                failures.push(format!("{snapshot} {file}: item hashes changed"));
            }
            let before: Value = serde_json::from_str(&text).unwrap();
            let after: Value = serde_json::from_str(&to_wire_json(&admit(&text).unwrap())).unwrap();
            let want = if before["ir_version"] == "0.5" {
                "0.5"
            } else {
                "0.4"
            };
            if after["ir_version"] != want {
                failures.push(format!("{file}: serialized as {}", after["ir_version"]));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn orders_round_trips_as_0_6() {
    let text = common::read(&common::fixtures().join("wire/valid/orders.json"));
    let r = admission_report(&text);
    assert!(r.ok, "{:?}", r.errors);
    let out = to_wire_json(&admit(&text).unwrap());
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ir_version"], "0.6");
    assert_eq!(admission_report(&out).behavior_version, r.behavior_version);
    let module_invariant = v["invariants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "personnel_numbers_unique")
        .unwrap();
    assert!(module_invariant.get("entity").is_none());
}

#[test]
fn each_007_form_needs_0_6() {
    let l = json!({"file": "t.py", "line": 1});
    let q = json!({"op": "select", "entity": "E", "loc": l});
    let body = json!({"op": "field", "param": "x", "field": "n", "loc": l});
    let lam = |op: &str| json!({"op": op, "args": [q.clone()], "param": "x", "body": body.clone(), "loc": l});
    let boolean = |e: Value| json!({"op": "ge", "args": [e, {"op": "lit", "type": {"t": "int"}, "value": 0, "loc": l}], "loc": l});
    let forms: Vec<(&str, Value)> = vec![
        (
            "select",
            boolean(json!({"op": "count", "args": [q.clone()], "loc": l})),
        ),
        (
            "where",
            boolean(
                json!({"op": "count", "loc": l, "args": [{"op": "where", "args": [q.clone()], "param": "x",
            "body": boolean(body.clone()), "loc": l}]}),
            ),
        ),
        (
            "union",
            boolean(
                json!({"op": "count", "loc": l, "args": [{"op": "union", "args": [q.clone(), q.clone()], "loc": l}]}),
            ),
        ),
        ("sum", boolean(lam("sum"))),
        (
            "unique",
            json!({"op": "unique", "args": [q.clone()], "param": "x", "body": body.clone(), "loc": l}),
        ),
    ];
    for (name, expr) in forms {
        let doc = |v: &str| {
            json!({"ir_version": v, "enums": [], "nominals": [], "derived": [], "constraints": [], "actions": [],
                   "entities": [{"name": "E", "loc": l, "fields": [{"name": "n", "type": {"t": "int"}, "loc": l}]}],
                   "invariants": [{"name": "inv", "loc": l, "body": expr.clone()}]})
        };
        for old in ["0.4", "0.5"] {
            let r = admission_report(&doc(old).to_string());
            let codes: Vec<&str> = r.errors.iter().map(|e| e.code.as_str()).collect();
            assert_eq!(codes, ["UNSUPPORTED_IR_VERSION"], "{name} in {old}");
        }
        let r = admission_report(&doc("0.6").to_string());
        assert!(r.ok, "{name}: {:?}", r.errors);
        let out: Value =
            serde_json::from_str(&to_wire_json(&admit(&doc("0.6").to_string()).unwrap())).unwrap();
        assert_eq!(out["ir_version"], "0.6", "{name}");
    }
}

#[test]
fn invalid_query_fixtures_report_their_codes() {
    for (case, code) in [
        ("query_in_constraint", "QUERY_NOT_ALLOWED"),
        ("query_in_entity_invariant", "QUERY_NOT_ALLOWED"),
        ("nested_query", "QUERY_NOT_ALLOWED"),
        ("exists_in_filter", "NON_LOCAL_PREDICATE"),
        ("mixed_set_types", "TYPE_MISMATCH"),
        ("sum_of_string", "TYPE_MISMATCH"),
        ("min_of_unordered", "TYPE_MISMATCH"),
        ("module_invariant_reads_input", "UNKNOWN_PARAM"),
        ("query_as_effect", "DECODE_ERROR"),
        ("query_in_0_5", "UNSUPPORTED_IR_VERSION"),
    ] {
        let text = common::read(&common::fixtures().join(format!("wire/invalid/{case}.json")));
        let r = admission_report(&text);
        let codes: Vec<&str> = r.errors.iter().map(|e| e.code.as_str()).collect();
        assert_eq!(codes, [code], "{case}: {:?}", r.errors);
    }
}

#[test]
fn definition_identity_ignores_lambda_names_and_follows_predicates() {
    let base = orders();
    let items = |d: &Value| admission_report(&d.to_string()).items;
    let rename = |v: &mut Value, from: &str, to: &str| {
        fn walk(v: &mut Value, from: &str, to: &str) {
            match v {
                Value::Object(m) => {
                    if m.get("param").and_then(Value::as_str) == Some(from)
                        && (m.contains_key("body") || m.get("op") == Some(&json!("field")))
                    {
                        m.insert("param".into(), json!(to));
                    }
                    m.values_mut().for_each(|x| walk(x, from, to));
                }
                Value::Array(xs) => xs.iter_mut().for_each(|x| walk(x, from, to)),
                _ => {}
            }
        }
        walk(v, from, to);
    };
    // Renaming the lambda parameter `o` everywhere keeps every item hash.
    let mut renamed = base.clone();
    rename(&mut renamed["derived"], "o", "order");
    assert_eq!(
        items(&renamed)["derived:open_order_count"],
        items(&base)["derived:open_order_count"]
    );
    // Changing the predicate changes it.
    let mut changed = base.clone();
    changed["derived"][0]["body"]["args"][0]["body"]["args"][1]["value"] = json!("blocked");
    assert_ne!(
        items(&changed)["derived:open_order_count"],
        items(&base)["derived:open_order_count"]
    );
}
