#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3 (feature 007): queries on the resulting state are derived,
//! `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS` (FR-013, FR-013a).

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admit, evaluate, replay};
use serde_json::{Value, json};

fn orders() -> Module {
    admit(&common::read(
        &common::fixtures().join("wire/valid/orders.json"),
    ))
    .unwrap()
}

fn eval(m: &Module, request: &Value) -> Value {
    let text = evaluate(m, &request.to_string()).to_json_string();
    let r = replay(m, &text);
    assert!(r.matches, "{:?}", r.diff);
    serde_json::from_str(&text).unwrap()
}

fn order(id: &str, customer: &str, amount: &str) -> Value {
    json!({"id": id, "customer": customer, "amount": amount, "status": "open", "region": "north"})
}

fn customer(limit: &str) -> Value {
    json!({"id": "c1", "name": "Ada", "credit_limit": limit, "region": "north"})
}

fn queries(rec: &Value) -> Vec<Value> {
    rec["facts"]["queries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn field_ids(rec: &Value) -> Vec<String> {
    rec["facts"]["fields"]
        .as_array()
        .map(|fs| {
            fs.iter()
                .map(|f| f["id"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn place(action: &str, amount: &str) -> Value {
    json!({"action": action, "data_version": "1", "context": {},
        "state": {"customer": customer("100.00")},
        "input": {"order_id": "o9", "amount": amount},
        "facts": {"identities": [{"entity": "Order", "id": "o9", "used": false}],
                  "universe": [{"entity": "Order", "members": [
                      order("o1", "c1", "60.00"), order("o2", "c2", "500.00")]}]}})
}

#[test]
fn a_created_member_joins_the_resulting_set() {
    let m = orders();
    let ok = eval(&m, &place("place_order_unchecked", "40.00"));
    assert_eq!(ok["result"], "ALLOW", "{ok}");
    let over = eval(&m, &place("place_order_unchecked", "40.01"));
    assert_eq!(over["result"], "DENY", "{over}");
    assert_eq!(over["reasons"][0]["code"], "POSTCONDITION_FAILED");
    // One fixed instance (the capture `customer.id` is unchanged) observed once on S; the created
    // order is never a fact, and the non-matching order's amount is never read.
    let qs = queries(&over);
    assert_eq!(qs.len(), 1, "{qs:?}");
    assert_eq!(qs[0]["members"], json!([{"id": "o1"}]));
    assert_eq!(field_ids(&over), ["o1"]);
}

#[test]
fn a_removed_member_leaves_the_resulting_set() {
    let m = orders();
    let request = json!({"action": "remove_cheapest", "data_version": "1", "context": {}, "input": {},
        "state": {"order": order("o1", "c1", "5.00"), "customer": customer("100.00")},
        "facts": {"universe": [{"entity": "Order", "members": [
            order("o1", "c1", "5.00"), order("o2", "c1", "10.00")]}]}});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    let post = rec["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["phase"] == "postcondition")
        .unwrap();
    assert_eq!(post["outcome"], true, "{post}");
    let reads = post["reads"].as_object().unwrap();
    assert!(reads.values().any(|v| v == "10.00"), "{post}");
    assert_eq!(queries(&rec).len(), 1);
}

#[test]
fn a_changed_capture_is_a_new_instance_observed_against_s() {
    let m = orders();
    let request = json!({"action": "raise_limit", "data_version": "1", "context": {},
        "state": {"customer": customer("160.00")}, "input": {"limit": "200.00"},
        "facts": {"universe": [{"entity": "Order", "members": [
            order("o1", "c1", "50.00"), order("o2", "c1", "150.00")]}]}});
    let rec = eval(&m, &request);
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    let qs = queries(&rec);
    assert_eq!(qs.len(), 2, "{qs:?}");
    assert_eq!(qs[0]["definition"], qs[1]["definition"]);
    let limits: Vec<&Value> = qs.iter().map(|q| &q["captures"][0]["value"]).collect();
    assert!(
        limits.contains(&&json!("160.00")) && limits.contains(&&json!("200.00")),
        "{qs:?}"
    );
    assert!(qs.iter().all(|q| q["members"] == json!([])), "{qs:?}");
}

#[test]
fn a_bound_member_is_reclassified_on_its_resulting_value() {
    let m = orders();
    let request = |number: &str| {
        json!({"action": "renumber", "data_version": "1", "context": {},
            "state": {"employee": {"id": "e1", "personnel_number": "N1"}}, "input": {"number": number},
            "facts": {"universe": [{"entity": "Employee", "members": [
                {"id": "e1", "personnel_number": "N1"}, {"id": "e2", "personnel_number": "N2"}]}]}})
    };
    assert_eq!(eval(&m, &request("N2"))["result"], "DENY");
    let ok = eval(&m, &request("N1b"));
    assert_eq!(ok["result"], "ALLOW", "{ok}");
    // Only e1's new key is looked up: e2 is never read.
    assert!(!field_ids(&ok).contains(&"e2".to_string()), "{ok}");
}
