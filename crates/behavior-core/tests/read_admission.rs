#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Read admission (feature 010): declared reads in a module and ad-hoc read documents are
//! admitted with the derived-value expression context; declared reads are behavior items (part of
//! the behavior version, never of the schema). Invalid fixtures report their codes.

mod common;

use behavior_core::read::admit_read;
use behavior_core::semantic::module::Kind;
use behavior_core::serialize::to_wire_json;
use behavior_core::{admission_report, admit, schema};
use serde_json::{Value, json};

fn text(path: &str) -> String {
    common::read(&common::fixtures().join(path))
}

fn lab() -> behavior_core::semantic::module::Module {
    admit(&text("reads/modules/lab.json")).unwrap()
}

fn codes(r: &behavior_core::AdmissionResult) -> Vec<String> {
    r.errors.iter().map(|e| e.code.clone()).collect()
}

#[test]
fn declared_reads_are_admitted_in_name_order() {
    let m = lab();
    let names: Vec<&str> = m.reads().keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "active_count",
            "active_cultures",
            "average_ph",
            "big_orders",
            "customer_summary",
            "open_total",
            "order_view",
            "smallest_order",
        ]
    );
    let open_total = m.read("open_total").unwrap();
    assert!(open_total.declared());
    assert_eq!(open_total.params().len(), 1);
}

#[test]
fn declared_reads_are_name_table_items() {
    let m = lab();
    let reads: Vec<&str> = m
        .name_table()
        .keys()
        .filter(|(k, _)| *k == Kind::Read)
        .map(|(_, n)| n.as_str())
        .collect();
    assert_eq!(reads.len(), 8);
    let r = admission_report(&text("reads/modules/lab.json"));
    assert!(r.ok, "{:?}", r.errors);
    assert!(
        r.items.contains_key("read:open_total"),
        "{:?}",
        r.items.keys()
    );
}

#[test]
fn reads_change_the_behavior_version_but_not_the_schema() {
    let with = lab();
    let without = admit(&text("reads/modules/lab_base.json")).unwrap();
    assert_ne!(with.behavior_version(), without.behavior_version());
    assert_eq!(schema(&with).hash, schema(&without).hash);
    // An empty `reads` section is no read: the module keeps the identity it has without one.
    let empty = admit(&text("reads/modules/lab_empty_reads.json")).unwrap();
    assert_eq!(empty.behavior_version(), without.behavior_version());
    assert_eq!(empty.name_table(), without.name_table());
}

#[test]
fn a_module_with_reads_round_trips_at_0_7() {
    let m = lab();
    let out: Value = serde_json::from_str(&to_wire_json(&m)).unwrap();
    assert_eq!(out["ir_version"], "0.7");
    assert_eq!(out["reads"].as_array().unwrap().len(), 8);
    let again = admit(&out.to_string()).unwrap();
    assert_eq!(again.behavior_version(), m.behavior_version());
    // A module without reads serializes without the section, at its old version.
    let base = admit(&text("reads/modules/lab_base.json")).unwrap();
    let out: Value = serde_json::from_str(&to_wire_json(&base)).unwrap();
    assert_eq!(out["ir_version"], "0.6");
    assert!(out.get("reads").is_none());
}

#[test]
fn ad_hoc_read_documents_are_admitted_against_the_module() {
    let m = lab();
    let base = admit(&text("reads/modules/lab_base.json")).unwrap();
    for case in ["customer_count", "culture_ph", "orders_over"] {
        let doc = text(&format!("reads/valid/{case}.json"));
        let r = admit_read(&m, &doc).unwrap_or_else(|e| panic!("{case}: {:?}", e.errors));
        assert!(!r.declared(), "{case}");
        assert_eq!(r.name(), case);
        // The same document has the same hash against any module that declares what it uses.
        let r2 = admit_read(&base, &doc).unwrap();
        assert_eq!(r.hash(), r2.hash(), "{case}");
    }
    // Admitting an ad-hoc read never changes the module.
    assert_eq!(m.reads().len(), 8);
}

#[test]
fn a_read_hash_does_not_depend_on_its_name_or_location() {
    let m = lab();
    let mut doc = common::json(&common::fixtures().join("reads/valid/customer_count.json"));
    let h = *admit_read(&m, &doc.to_string()).unwrap().hash();
    doc["read"]["name"] = json!("how_many_customers");
    doc["read"]["loc"]["line"] = json!(999);
    assert_eq!(*admit_read(&m, &doc.to_string()).unwrap().hash(), h);
    doc["read"]["body"]["value"]["args"][0]["entity"] = json!("Order");
    assert_ne!(*admit_read(&m, &doc.to_string()).unwrap().hash(), h);
}

#[test]
fn read_parameters_need_a_role_and_state_parameters_are_entities() {
    let m = lab();
    let mut no_role = common::json(&common::fixtures().join("reads/valid/orders_over.json"));
    no_role["read"]["params"][0]
        .as_object_mut()
        .unwrap()
        .remove("role");
    let e = admit_read(&m, &no_role.to_string()).unwrap_err();
    assert_eq!(codes(&e), ["DECODE_ERROR"], "{:?}", e.errors);
    let mut state_int = common::json(&common::fixtures().join("reads/valid/orders_over.json"));
    state_int["read"]["params"][0]["role"] = json!("state");
    let e = admit_read(&m, &state_int.to_string()).unwrap_err();
    assert_eq!(codes(&e), ["TYPE_MISMATCH"], "{:?}", e.errors);
}

/// Every `reads/invalid/<case>.json` is refused with exactly the codes of its `.expected.json`.
/// A read document is admitted against `lab`; a module document is admitted on its own.
#[test]
fn invalid_read_fixtures_report_their_codes() {
    let m = lab();
    let dir = common::fixtures().join("reads/invalid");
    let cases = common::files(&dir, ".json");
    assert!(!cases.is_empty());
    for path in cases {
        let doc: Value = common::json(&path);
        let expected = common::json(&path.with_extension("expected.json"));
        let want: Vec<String> = expected["codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().to_string())
            .collect();
        let got = if doc.get("read").is_some() {
            match admit_read(&m, &doc.to_string()) {
                Ok(_) => Vec::new(),
                Err(e) => codes(&e),
            }
        } else {
            codes(&admission_report(&doc.to_string()))
        };
        assert_eq!(got, want, "{}", path.display());
        if let Some(fragment) = expected.get("message").and_then(Value::as_str) {
            let r = if doc.get("read").is_some() {
                admit_read(&m, &doc.to_string()).unwrap_err()
            } else {
                admission_report(&doc.to_string())
            };
            assert!(
                r.errors.iter().any(|e| e.message.contains(fragment)),
                "{}: {:?}",
                path.display(),
                r.errors
            );
        }
    }
}

/// Code review (feature 010): a projection's record keys are its item names, so two projections
/// of derived values with identical bodies but different names are different reads.
#[test]
fn a_projection_hash_binds_its_item_names() {
    let mut w = common::json(&common::fixtures().join("reads/modules/lab.json"));
    let mut twin = w["derived"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "ph_avg")
        .unwrap()
        .clone();
    twin["name"] = json!("ph_avg2");
    w["derived"].as_array_mut().unwrap().push(twin);
    let m = admit(&w.to_string()).unwrap();
    let l = json!({"file": "q.py", "line": 1});
    let doc = |item: &str| {
        json!({"ir_version": "0.7", "read": {"name": "averages", "params": [], "loc": l,
            "body": {"project": {"over": {"op": "select", "entity": "Culture", "loc": l},
                                 "param": "c", "items": [{"derived": item}]}}}})
        .to_string()
    };
    let a = admit_read(&m, &doc("ph_avg")).unwrap();
    let b = admit_read(&m, &doc("ph_avg2")).unwrap();
    assert_ne!(a.hash(), b.hash());
}
