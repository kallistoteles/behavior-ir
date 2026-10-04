#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-004 (feature 010): a query projection of five items (three stored fields and two derived
//! values) over 10,000 entities completes in under 2 s, and records every projected stored field
//! as observed (30,000 field facts). Run with
//! `cargo test --release -p behavior-store --test read_perf -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use behavior_core::read::ReadSource;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::store::genesis_for;
use behavior_store::{InMemoryBackend, Store};
use serde_json::{Value, json};

fn module() -> behavior_core::semantic::module::Module {
    let l = json!({"file": "perf.py", "line": 1});
    let f = |name: &str| json!({"op": "field", "param": "s", "field": name, "loc": l});
    let int = json!({"t": "int"});
    let wire = json!({
        "ir_version": "0.7", "enums": [], "nominals": [], "invariants": [], "constraints": [],
        "actions": [],
        "entities": [{"name": "Sample", "loc": l, "fields": [
            {"name": "label", "type": {"t": "string"}, "loc": l},
            {"name": "weight", "type": int, "loc": l},
            {"name": "count", "type": int, "loc": l},
            {"name": "total", "type": int, "loc": l}]}],
        "derived": [
            {"name": "mean", "kind": "derived", "loc": l,
             "params": [{"name": "s", "type": {"t": "entity", "name": "Sample"}}],
             "body": {"op": "div", "args": [f("total"), f("count")], "loc": l}},
            {"name": "heavy", "kind": "derived", "loc": l,
             "params": [{"name": "s", "type": {"t": "entity", "name": "Sample"}}],
             "body": {"op": "gt", "args": [f("weight"), {"op": "lit", "type": int,
                                                         "value": 500, "loc": l}], "loc": l}}],
        "reads": [{"name": "samples", "params": [], "loc": l, "body": {"project": {
            "over": {"op": "select", "entity": "Sample", "loc": l}, "param": "s",
            "items": [{"field": "label"}, {"field": "weight"}, {"field": "count"},
                      {"derived": "mean"}, {"derived": "heavy"}]}}}],
    });
    behavior_core::admit(&wire.to_string()).unwrap()
}

#[test]
#[ignore = "release-mode performance test (SC-004)"]
fn a_five_item_projection_of_ten_thousand_entities_takes_under_two_seconds() {
    let m = module();
    let seed: Vec<SeedEntity> = (0..10_000)
        .map(|i| SeedEntity {
            entity: "Sample".into(),
            value: json!({"id": format!("s{i:05}"), "label": format!("sample {i}"),
                          "weight": i % 1000, "count": 1 + i % 7, "total": i}),
        })
        .collect();
    let s = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )
    .unwrap();
    let started = Instant::now();
    let x = s
        .read(
            &m,
            &ReadSource::Declared("samples".into()),
            &BTreeMap::new(),
            &json!({}),
            &json!({}),
            None,
        )
        .unwrap();
    let elapsed = started.elapsed();
    let r = x.record.as_json();
    assert_eq!(r["result"], "VALUE");
    assert_eq!(r["value"].as_array().unwrap().len(), 10_000);
    let observed: std::collections::BTreeSet<(String, String)> = r["facts"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f: &Value| {
            (
                f["id"].as_str().unwrap().to_string(),
                f["field"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let projected = observed
        .iter()
        .filter(|(_, f)| ["label", "weight", "count"].contains(&f.as_str()))
        .count();
    assert_eq!(
        projected, 30_000,
        "every projected stored field is observed"
    );
    eprintln!("projection of 5 items over 10,000 entities: {elapsed:?}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}
