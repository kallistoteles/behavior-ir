#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Plain-mode reads (feature 010, US1/US2): a read observes one supplied state and changes
//! nothing; its record is canonical, content-addressed and self-contained.

mod common;

use behavior_core::admit;
use behavior_core::read::{ReadExecution, ReadSource, admit_read, evaluate_read};
use behavior_core::semantic::module::Module;
use serde_json::{Value, json};

fn lab() -> Module {
    admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap()
}

fn culture(id: &str, active: bool, measurements: i64, ph_total: i64) -> Value {
    json!({"id": id, "name": format!("culture {id}"), "stage": null, "active": active,
           "measurements": measurements, "ph_total": ph_total})
}

fn universe(entity: &str, members: Vec<Value>) -> Value {
    json!({"entity": entity, "members": members})
}

fn request(state: Value, input: Value, facts: Option<Value>) -> String {
    let mut r = json!({"data_version": "test:1", "state": state, "input": input, "context": {}});
    if let Some(f) = facts {
        r["facts"] = f;
    }
    r.to_string()
}

fn declared(m: &Module, name: &str, req: &str) -> ReadExecution {
    evaluate_read(m, &ReadSource::Declared(name.into()), req)
}

fn record(x: &ReadExecution) -> Value {
    x.record.as_json().clone()
}

fn codes(rec: &Value) -> Vec<String> {
    rec["reasons"]
        .as_array()
        .map(|rs| {
            rs.iter()
                .map(|r| r["code"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn cultures() -> Value {
    json!({"universe": [universe("Culture", vec![
        culture("c1", true, 2, 14), culture("c2", false, 0, 0), culture("c3", true, 1, 6)])]})
}

#[test]
fn a_parameterless_read_answers_from_the_supplied_state() {
    let m = lab();
    let x = declared(
        &m,
        "active_count",
        &request(json!({}), json!({}), Some(cultures())),
    );
    let r = record(&x);
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], 2);
    assert_eq!(r["format"], "behavior.read_record.v1");
    assert_eq!(r["behavior_version"], m.behavior_version());
    assert_eq!(r["data_version"], "test:1");
    assert_eq!(r["read"]["name"], "active_count");
    assert_eq!(r["read"]["declared"], true);
    assert!(r["read"].get("definition").is_none());
    assert!(
        !r["facts"]["queries"].as_array().unwrap().is_empty(),
        "the query instance is an observed fact: {r}"
    );
    // A read is not a decision: no action, no changes.
    for key in ["action", "changes", "lifecycle"] {
        assert!(r.get(key).is_none(), "{key} in {r}");
    }
    assert_eq!(x.response.value(), Some(&json!(2)));
    assert_eq!(x.response.record_id(), x.record.record_id());
}

#[test]
fn bound_entities_and_inputs_are_parameters() {
    let m = lab();
    let orders = json!({"universe": [universe("Order", vec![
        json!({"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}),
        json!({"id": "o2", "customer": "k1", "amount": 12, "status": "CLOSED"}),
        json!({"id": "o3", "customer": "k2", "amount": 7, "status": "OPEN"})])]});
    let customer = json!({"customer": {"id": "k1", "name": "Ada", "credit_limit": 100}});
    let r = record(&declared(
        &m,
        "open_total",
        &request(customer, json!({}), Some(orders.clone())),
    ));
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], 30);
    let observed = r["observed"].as_array().unwrap();
    assert!(
        observed.contains(&json!(["customer", "id"])),
        "the bound entity's read field is observed: {r}"
    );
    assert_eq!(r["state"]["customer"]["name"], "Ada");

    // An ad-hoc read with an input parameter.
    let doc = json!({"ir_version": "0.7", "read": {
        "name": "orders_above", "loc": {"file": "q.py", "line": 1},
        "params": [{"name": "min", "role": "input", "type": {"t": "int"}}],
        "body": {"value": {"op": "count", "loc": {"file": "q.py", "line": 1}, "args": [
            {"op": "where", "param": "o", "loc": {"file": "q.py", "line": 1},
             "args": [{"op": "select", "entity": "Order", "loc": {"file": "q.py", "line": 1}}],
             "body": {"op": "gt", "loc": {"file": "q.py", "line": 1}, "args": [
                {"op": "field", "param": "o", "field": "amount", "loc": {"file": "q.py", "line": 1}},
                {"op": "param", "param": "min", "loc": {"file": "q.py", "line": 1}}]}}]}}}});
    let item = admit_read(&m, &doc.to_string()).unwrap();
    let x = evaluate_read(
        &m,
        &ReadSource::AdHoc(Box::new(item)),
        &request(json!({}), json!({"min": 10}), Some(orders)),
    );
    let r = record(&x);
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], 2);
    assert_eq!(r["input"], json!({"min": 10}));
    assert_eq!(r["read"]["declared"], false);
    assert_eq!(
        r["read"]["definition"]["read"]["name"], "orders_above",
        "an ad-hoc record carries its definition: {r}"
    );
}

#[test]
fn invalid_parameters_are_refused_before_evaluation() {
    let m = lab();
    // Missing bound entity, an extra input, and an extra section key: every problem is listed.
    let r = record(&declared(
        &m,
        "open_total",
        &request(json!({}), json!({"x": 1}), None),
    ));
    assert_eq!(r["result"], "INVALID_INPUT", "{r}");
    let c = codes(&r);
    assert!(c.contains(&"MISSING_ARGUMENT".to_string()), "{r}");
    assert!(c.contains(&"EXTRA_ARGUMENT".to_string()), "{r}");
    assert_eq!(r["derived"], json!([]));
    assert!(r.get("value").is_none());
    assert!(r.get("facts").is_none(), "nothing was evaluated: {r}");

    // A wrong input type.
    let doc = common::read(&common::fixtures().join("reads/valid/orders_over.json"));
    let item = admit_read(&m, &doc).unwrap();
    let r = record(&evaluate_read(
        &m,
        &ReadSource::AdHoc(Box::new(item)),
        &request(json!({}), json!({"min": "many"}), None),
    ));
    assert_eq!(r["result"], "INVALID_INPUT", "{r}");
    assert_eq!(codes(&r), ["WRONG_TYPE"]);

    // An unknown declared read.
    let r = record(&declared(&m, "nope", &request(json!({}), json!({}), None)));
    assert_eq!(r["result"], "INVALID_INPUT", "{r}");
    assert_eq!(codes(&r), ["UNKNOWN_READ"]);
}

#[test]
fn an_evaluation_failure_is_a_result() {
    let m = lab();
    // No cultures: the average divides by a count of zero.
    let facts = json!({"universe": [universe("Culture", vec![])]});
    let r = record(&declared(
        &m,
        "average_ph",
        &request(json!({}), json!({}), Some(facts)),
    ));
    assert_eq!(r["result"], "EVALUATION_ERROR", "{r}");
    assert_eq!(codes(&r), ["EVALUATION_ERROR"]);
    assert!(r["reasons"][0]["loc"].is_object(), "{r}");
    assert!(r.get("value").is_none());
}

#[test]
fn missing_and_inconsistent_facts() {
    let m = lab();
    let r = record(&declared(
        &m,
        "active_count",
        &request(json!({}), json!({}), None),
    ));
    assert_eq!(r["result"], "EVALUATION_ERROR", "{r}");
    assert_eq!(codes(&r), ["UNKNOWN_FACT"]);

    let bad = json!({"universe": [universe("Culture", vec![culture("c1", true, -1, 0)])]});
    let r = record(&declared(
        &m,
        "active_count",
        &request(json!({}), json!({}), Some(bad)),
    ));
    assert_eq!(r["result"], "INVALID_INPUT", "{r}");
    assert_eq!(codes(&r), ["INCONSISTENT_FACTS"]);
}

#[test]
fn reads_are_deterministic() {
    let m = lab();
    let req = request(json!({}), json!({}), Some(cultures()));
    let a = declared(&m, "active_count", &req);
    let b = declared(&m, "active_count", &req);
    assert_eq!(a.record.to_json_string(), b.record.to_json_string());
    assert_eq!(a.record.record_id(), b.record.record_id());
    // The record parses back with its identity intact.
    let again = behavior_core::read::ReadRecord::from_json(&a.record.to_json_string()).unwrap();
    assert_eq!(again.record_id(), a.record.record_id());
}

#[test]
fn aggregates_over_nothing_have_their_defined_values() {
    let m = lab();
    let none = json!({"universe": [universe("Culture", vec![]), universe("Order", vec![])]});
    let r = record(&declared(
        &m,
        "active_count",
        &request(json!({}), json!({}), Some(none.clone())),
    ));
    assert_eq!(r["value"], 0, "{r}");
    let r = record(&declared(
        &m,
        "smallest_order",
        &request(json!({}), json!({}), Some(none)),
    ));
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], Value::Null, "an absent minimum: {r}");
}

// --- projections (US2) ---------------------------------------------------------------------------

fn field_facts(rec: &Value) -> Vec<(String, String, String)> {
    rec["facts"]["fields"]
        .as_array()
        .map(|fs| {
            fs.iter()
                .map(|f| {
                    (
                        f["entity"].as_str().unwrap().to_string(),
                        f["id"].as_str().unwrap().to_string(),
                        f["field"].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has(facts: &[(String, String, String)], e: &str, id: &str, f: &str) -> bool {
    facts.contains(&(e.to_string(), id.to_string(), f.to_string()))
}

#[test]
fn a_query_projection_returns_exactly_the_projected_items_in_identity_order() {
    let m = lab();
    let mut c3 = culture("c3", true, 2, 12);
    c3["stage"] = json!("GROWTH");
    let facts = json!({"universe": [universe("Culture", vec![
        c3, culture("c2", false, 0, 0), culture("c1", true, 2, 14)])]});
    let r = record(&declared(
        &m,
        "active_cultures",
        &request(json!({}), json!({}), Some(facts)),
    ));
    assert_eq!(r["result"], "VALUE", "{r}");
    let rows = r["value"].as_array().unwrap();
    let ids: Vec<&str> = rows.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["c1", "c3"], "identity order: {r}");
    for row in rows {
        let keys: Vec<&str> = row
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["id", "name", "ph_avg", "stage"], "{row}");
    }
    // An absent optional value is present, as absent.
    assert_eq!(rows[0]["stage"], Value::Null);
    assert_eq!(rows[1]["stage"], "GROWTH");
    assert_eq!(rows[0]["ph_avg"], "7");
    // Every projected stored field of every member is observed, and so is what the derived
    // item read.
    let f = field_facts(&r);
    for id in ["c1", "c3"] {
        for field in ["name", "stage", "ph_total", "measurements"] {
            assert!(has(&f, "Culture", id, field), "{id}.{field}: {r}");
        }
    }
    assert!(
        !has(&f, "Culture", "c2", "name"),
        "a non-member is not projected: {r}"
    );
    assert_eq!(r["derived"].as_array().unwrap().len(), 2, "{r}");
}

#[test]
fn every_projected_field_is_observed_even_when_nothing_else_reads_it() {
    let m = lab();
    let doc = common::read(&common::fixtures().join("reads/valid/orders_over.json"));
    let item = admit_read(&m, &doc).unwrap();
    let orders = json!({"universe": [universe("Order", vec![
        json!({"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}),
        json!({"id": "o2", "customer": "k1", "amount": 3, "status": "OPEN"})])]});
    let r = record(&evaluate_read(
        &m,
        &ReadSource::AdHoc(Box::new(item)),
        &request(json!({}), json!({"min": 10}), Some(orders)),
    ));
    assert_eq!(r["value"], json!([{"id": "o1", "amount": 30}]), "{r}");
    assert!(has(&field_facts(&r), "Order", "o1", "amount"), "{r}");
}

#[test]
fn an_entity_projection_returns_one_record() {
    let m = lab();
    let order = json!({"order": {"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}});
    let r = record(&declared(
        &m,
        "order_view",
        &request(order, json!({}), None),
    ));
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(
        r["value"],
        json!({"id": "o1", "status": "OPEN", "amount": 30})
    );
    let observed = r["observed"].as_array().unwrap();
    assert!(observed.contains(&json!(["order", "status"])), "{r}");
    assert!(observed.contains(&json!(["order", "amount"])), "{r}");

    // A derived item over the bound entity, which itself queries other entities.
    let orders = json!({"universe": [universe("Order", vec![
        json!({"id": "o1", "customer": "k1", "amount": 130, "status": "OPEN"})])]});
    let customer = json!({"customer": {"id": "k1", "name": "Ada", "credit_limit": 100}});
    let r = record(&declared(
        &m,
        "customer_summary",
        &request(customer, json!({}), Some(orders)),
    ));
    assert_eq!(
        r["value"],
        json!({"id": "k1", "name": "Ada", "standing": false}),
        "{r}"
    );
    // What the derived value read internally is evidence in the record.
    assert!(
        r["observed"]
            .as_array()
            .unwrap()
            .contains(&json!(["customer", "credit_limit"])),
        "{r}"
    );
}

#[test]
fn one_failing_member_fails_the_whole_read() {
    let m = lab();
    let facts = json!({"universe": [universe("Culture", vec![
        culture("c1", true, 2, 14), culture("c2", true, 0, 0)])]});
    let r = record(&declared(
        &m,
        "active_cultures",
        &request(json!({}), json!({}), Some(facts)),
    ));
    assert_eq!(r["result"], "EVALUATION_ERROR", "{r}");
    assert!(r.get("value").is_none(), "no partial result: {r}");
    let message = r["reasons"][0]["message"].as_str().unwrap();
    assert!(message.contains("Culture#c2"), "{message}");
    assert!(message.contains("ph_avg"), "{message}");
}

#[test]
fn an_empty_query_projects_to_an_empty_list() {
    let m = lab();
    let facts = json!({"universe": [universe("Culture", vec![culture("c2", false, 0, 0)])]});
    let r = record(&declared(
        &m,
        "active_cultures",
        &request(json!({}), json!({}), Some(facts)),
    ));
    assert_eq!(r["result"], "VALUE", "{r}");
    assert_eq!(r["value"], json!([]));
}
