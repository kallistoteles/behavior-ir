#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use behavior_core::admission_report;
use proptest::prelude::*;
use serde_json::{Value, json};

fn load(name: &str) -> Value {
    common::json(&common::fixtures().join("wire/valid").join(name))
}

fn version(doc: &Value) -> String {
    let r = admission_report(&doc.to_string());
    assert!(r.ok, "{:?}", r.errors);
    r.behavior_version.unwrap()
}

fn items(doc: &Value) -> std::collections::BTreeMap<String, String> {
    admission_report(&doc.to_string()).items
}

/// Rewrites every `loc` in the document with `f(file, line)`.
fn map_locs(v: &mut Value, f: &dyn Fn(&str, u64) -> (String, u64)) {
    match v {
        Value::Object(map) => {
            if let Some(Value::Object(l)) = map.get_mut("loc") {
                let file = l["file"].as_str().unwrap().to_string();
                let line = l["line"].as_u64().unwrap();
                let (nf, nl) = f(&file, line);
                l.insert("file".into(), json!(nf));
                l.insert("line".into(), json!(nl));
            }
            for (k, child) in map.iter_mut() {
                if k != "loc" {
                    map_locs(child, f);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|c| map_locs(c, f)),
        _ => {}
    }
}

proptest! {
    #[test]
    fn source_locations_do_not_change_identity(shift in 1u64..10_000, prefix in "[a-z]{1,8}") {
        for name in ["invoice.json", "project_margin.json"] {
            let doc = load(name);
            let mut moved = doc.clone();
            map_locs(&mut moved, &|file, line| (format!("{prefix}/{file}"), line + shift));
            prop_assert_eq!(version(&doc), version(&moved));
        }
    }

    #[test]
    fn declaration_order_does_not_change_identity(seed in any::<u64>()) {
        let doc = load("invoice.json");
        let mut shuffled = doc.clone();
        for key in ["entities", "actions"] {
            let arr = shuffled[key].as_array_mut().unwrap();
            if seed % 2 == 1 { arr.reverse(); }
            if seed % 3 == 1 { arr.rotate_left(1); }
        }
        prop_assert_eq!(version(&doc), version(&shuffled));
    }
}

#[test]
fn reordering_every_declaration_list_keeps_identity() {
    let doc = load("project_margin.json");
    let mut rev = doc.clone();
    for key in [
        "enums",
        "nominals",
        "entities",
        "derived",
        "invariants",
        "actions",
    ] {
        rev[key].as_array_mut().unwrap().reverse();
    }
    assert_eq!(version(&doc), version(&rev));
}

#[test]
fn renaming_a_derived_value_keeps_its_item_hash() {
    let doc = load("project_margin.json");
    let mut renamed = doc.clone();
    renamed["derived"][0]["name"] = json!("gross_margin");
    renamed["derived"][1]["body"]["args"][0]["name"] = json!("gross_margin");
    let before = items(&doc);
    let after = items(&renamed);
    assert_eq!(before["derived:margin"], after["derived:gross_margin"]);
    assert_eq!(before["derived:high_risk"], after["derived:high_risk"]);
    assert_ne!(version(&doc), version(&renamed));
}

#[test]
fn renaming_an_action_or_invariant_keeps_item_hash() {
    let doc = load("invoice.json");
    let mut renamed = doc.clone();
    renamed["actions"][0]["name"] = json!("approve");
    renamed["invariants"][0]["name"] = json!("amount_not_negative");
    let before = items(&doc);
    let after = items(&renamed);
    assert_eq!(before["action:approve_invoice"], after["action:approve"]);
    assert_eq!(
        before["invariant:non_negative_amount"],
        after["invariant:amount_not_negative"]
    );
    assert_ne!(version(&doc), version(&renamed));
}

#[test]
fn implicit_and_explicit_conversions_hash_the_same() {
    let doc = load("invoice.json");
    let mut explicit = doc.clone();
    // approved_by := actor.id  (implicit some)  vs  approved_by := some(actor.id)
    let value = explicit["actions"][0]["effects"][1]["value"].clone();
    let at = value["loc"].clone();
    explicit["actions"][0]["effects"][1]["value"] =
        json!({"op": "some", "args": [value], "loc": at});
    assert_eq!(version(&doc), version(&explicit));

    let margin = load("project_margin.json");
    let mut explicit = margin.clone();
    // 0.05 as a decimal literal vs to_decimal applied to... keep decimal; check int promotion:
    let at = explicit["derived"][1]["body"]["loc"].clone();
    explicit["derived"][1]["body"]["args"][1] =
        json!({"op": "lit", "type": {"t": "decimal"}, "value": "0.050", "loc": at});
    assert_eq!(version(&margin), version(&explicit));
}

#[test]
fn int_literal_promoted_to_decimal_equals_decimal_literal() {
    let mut a = load("project_margin.json");
    let mut b = a.clone();
    let at = a["derived"][1]["body"]["loc"].clone();
    a["derived"][1]["body"]["args"][1] =
        json!({"op": "lit", "type": {"t": "int"}, "value": 1, "loc": at});
    let at = b["derived"][1]["body"]["loc"].clone();
    b["derived"][1]["body"]["args"][1] = json!({"op": "to_decimal", "args": [{"op": "lit", "type": {"t": "int"}, "value": 1, "loc": at}], "loc": at});
    let c = {
        let mut c = b.clone();
        c["derived"][1]["body"]["args"][1] =
            json!({"op": "lit", "type": {"t": "decimal"}, "value": "1", "loc": at});
        c
    };
    assert_eq!(version(&a), version(&b));
    assert_eq!(version(&a), version(&c));
}

#[test]
fn identical_expressions_share_hashes() {
    // The guard `project.revenue` read appears in margin and flag_project: compare via
    // two actions with identical bodies under different names.
    let doc = load("project_margin.json");
    let mut twin = doc.clone();
    let mut copy = twin["actions"][0].clone();
    copy["name"] = json!("flag_project_again");
    twin["actions"].as_array_mut().unwrap().push(copy);
    let it = items(&twin);
    assert_eq!(it["action:flag_project"], it["action:flag_project_again"]);
}

#[test]
fn semantic_changes_change_identity() {
    let doc = load("invoice.json");
    let base = version(&doc);

    let mut swapped = doc.clone();
    swapped["actions"][0]["preconditions"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert_ne!(base, version(&swapped));

    let mut literal = doc.clone();
    literal["actions"][0]["preconditions"][1]["expr"]["args"][1]["value"] = json!("controller");
    assert_ne!(base, version(&literal));

    let mut field = doc.clone();
    field["entities"][0]["fields"][0]["name"] = json!("title");
    field["actions"][0]["preconditions"][1]["expr"]["args"][0]["field"] = json!("title");
    assert_ne!(base, version(&field));

    let mut ops = doc.clone();
    ops["nominals"][0]["ops"] = json!(["add", "order", "ratio"]);
    assert_ne!(base, version(&ops));
}

#[test]
fn cycles_get_no_hash() {
    let wire = common::read(&common::fixtures().join("wire/invalid/cycle_a_b_c.json"));
    let r = admission_report(&wire);
    assert!(!r.ok);
    assert!(r.behavior_version.is_none());
    assert!(r.items.is_empty());
}
