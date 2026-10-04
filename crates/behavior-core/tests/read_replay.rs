#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Replaying read records (feature 010, US4): a record replays from its own facts, byte for byte;
//! any altered field is reported at its first differing path (FR-011).

mod common;

use behavior_core::admit;
use behavior_core::read::{ReadSource, admit_read, evaluate_read, replay_read};
use behavior_core::semantic::module::Module;
use serde_json::{Value, json};

fn lab() -> Module {
    admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap()
}

fn universe(entity: &str, members: Vec<Value>) -> Value {
    json!({"entity": entity, "members": members})
}

fn culture(id: &str, active: bool, measurements: i64, ph_total: i64) -> Value {
    json!({"id": id, "name": format!("culture {id}"), "stage": null, "active": active,
           "measurements": measurements, "ph_total": ph_total})
}

fn request(state: Value, input: Value, facts: Value) -> String {
    json!({"data_version": "test:1", "state": state, "input": input, "context": {},
           "facts": facts})
    .to_string()
}

/// Records of every kind: declared and ad-hoc, value and projection, error and refusal.
fn records(m: &Module) -> Vec<(&'static str, String)> {
    let cultures = json!({"universe": [universe("Culture", vec![
        culture("c1", true, 2, 14), culture("c2", false, 0, 0), culture("c3", true, 1, 6)])]});
    let orders = json!({"universe": [universe("Order", vec![
        json!({"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}),
        json!({"id": "o2", "customer": "k1", "amount": 3, "status": "CLOSED"})])]});
    let customer = json!({"customer": {"id": "k1", "name": "Ada", "credit_limit": 100}});
    let declared = |name: &str, req: String| {
        evaluate_read(m, &ReadSource::Declared(name.into()), &req)
            .record
            .to_json_string()
    };
    let adhoc = |case: &str, req: String| {
        let doc = common::read(&common::fixtures().join(format!("reads/valid/{case}.json")));
        let item = admit_read(m, &doc).unwrap();
        evaluate_read(m, &ReadSource::AdHoc(Box::new(item)), &req)
            .record
            .to_json_string()
    };
    vec![
        (
            "value",
            declared(
                "active_count",
                request(json!({}), json!({}), cultures.clone()),
            ),
        ),
        (
            "bound value",
            declared(
                "open_total",
                request(customer.clone(), json!({}), orders.clone()),
            ),
        ),
        (
            "query projection",
            declared(
                "active_cultures",
                request(json!({}), json!({}), cultures.clone()),
            ),
        ),
        (
            "entity projection",
            declared(
                "customer_summary",
                request(customer.clone(), json!({}), orders.clone()),
            ),
        ),
        (
            "evaluation error",
            declared(
                "average_ph",
                request(
                    json!({}),
                    json!({}),
                    json!({"universe": [universe("Culture", vec![])]}),
                ),
            ),
        ),
        (
            "invalid input",
            declared("open_total", request(json!({}), json!({"x": 1}), json!({}))),
        ),
        (
            "invalid binding",
            declared(
                "open_total",
                request(
                    json!({"customer": {"id": "nobody"}}),
                    json!({}),
                    json!({"existence": [{"entity": "Customer", "id": "nobody", "exists": false}]}),
                ),
            ),
        ),
        (
            "ad-hoc projection",
            adhoc(
                "orders_over",
                request(json!({}), json!({"min": 10}), orders),
            ),
        ),
    ]
}

#[test]
fn every_kind_of_record_replays_from_its_own_facts() {
    let m = lab();
    let mut results = std::collections::BTreeSet::new();
    for (case, record) in records(&m) {
        let r = replay_read(&m, &record);
        assert!(r.matches, "{case}: {:?}\n{record}", r.diff);
        let v: Value = serde_json::from_str(&record).unwrap();
        results.insert(v["result"].as_str().unwrap().to_string());
    }
    assert_eq!(
        results.into_iter().collect::<Vec<_>>(),
        [
            "EVALUATION_ERROR",
            "INVALID_BINDING",
            "INVALID_INPUT",
            "VALUE"
        ]
    );
}

#[test]
fn an_ad_hoc_record_replays_from_its_definition_alone() {
    let m = lab();
    let (_, record) = records(&m)
        .into_iter()
        .find(|(c, _)| *c == "ad-hoc projection")
        .unwrap();
    let v: Value = serde_json::from_str(&record).unwrap();
    assert_eq!(v["read"]["declared"], false);
    // The module has no read of that name: the definition in the record is what replays.
    assert!(m.read("orders_over").is_none());
    assert!(replay_read(&m, &record).matches);
}

/// Replaces the value at a dotted path (array indices as numbers) in a JSON value.
fn set(v: &mut Value, path: &[&str], to: Value) {
    let mut cur = v;
    for p in path {
        cur = match p.parse::<usize>() {
            Ok(i) => &mut cur[i],
            Err(_) => &mut cur[*p],
        };
    }
    *cur = to;
}

#[test]
fn every_altered_field_is_reported() {
    let m = lab();
    let all = records(&m);
    let get = |case: &str| -> Value {
        serde_json::from_str(&all.iter().find(|(c, _)| *c == case).unwrap().1).unwrap()
    };
    let cases: Vec<(&str, &[&str], Value)> = vec![
        ("value", &["value"], json!(3)),
        (
            "value",
            &["facts", "queries", "0", "members", "0", "id"],
            json!("c9"),
        ),
        (
            "query projection",
            &["facts", "fields", "0", "value"],
            json!("changed"),
        ),
        ("bound value", &["input"], json!({"x": 1})),
        ("bound value", &["read", "hash"], json!("sha256:00")),
        ("bound value", &["behavior_version"], json!("sha256:00")),
        ("value", &["record_id"], json!("read:sha256:00")),
        (
            "entity projection",
            &["state", "customer", "credit_limit"],
            json!(1000),
        ),
    ];
    for (case, path, to) in cases {
        let mut rec = get(case);
        set(&mut rec, path, to);
        let r = replay_read(&m, &rec.to_string());
        assert!(!r.matches, "{case} {path:?} was not detected");
        assert!(r.diff.is_some(), "{case} {path:?}");
    }
    // A record that is not a read record at all.
    assert!(!replay_read(&m, "{\"format\": \"other\"}").matches);
    assert!(!replay_read(&m, "not json").matches);
}
