#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-005: admit 200 derived values/rules and 50 actions in < 2 s; evaluate one action in
//! < 10 ms. Run with `cargo test --release -p behavior-core -- --ignored perf`.

use std::time::{Duration, Instant};

use behavior_core::{admission_report, admit, evaluate};
use serde_json::{Value, json};

fn loc(line: u64) -> Value {
    json!({"file": "perf.py", "line": line})
}

fn field(f: &str) -> Value {
    json!({"op": "field", "param": "e", "field": f, "loc": loc(1)})
}

/// A module with a chain of 200 derived values (each using the previous) and 50 actions.
fn large_module() -> String {
    let e = json!({"t": "entity", "name": "E"});
    let mut derived = Vec::new();
    for i in 0..200 {
        let prev = if i == 0 {
            field("x")
        } else {
            json!({"op": "derived", "name": format!("d{}", i - 1), "args": ["e"], "loc": loc(2)})
        };
        let body = json!({"op": "add", "args": [prev, field("y")], "loc": loc(3)});
        derived.push(
            json!({"name": format!("d{i}"), "kind": "derived", "loc": loc(4),
                            "params": [{"name": "e", "type": e}], "body": body}),
        );
    }
    let mut actions = Vec::new();
    for i in 0..50 {
        let top = json!({"op": "derived", "name": "d199", "args": ["e"], "loc": loc(5)});
        let cond = json!({"op": "ge", "args": [top, json!({"op": "lit", "type": {"t": "int"}, "value": i, "loc": loc(5)})], "loc": loc(5)});
        actions.push(json!({"name": format!("a{i}"), "loc": loc(6),
            "params": [{"name": "e", "role": "state", "type": e}],
            "preconditions": [{"expr": cond, "loc": loc(7)}],
            "effects": [{"target": {"param": "e", "field": "x"},
                         "value": {"op": "add", "args": [field("x"), json!({"op": "lit", "type": {"t": "int"}, "value": 1, "loc": loc(8)})], "loc": loc(8)},
                         "loc": loc(8)}],
            "postconditions": []}));
    }
    json!({"ir_version": "0.4", "constraints": [], "enums": [], "nominals": [], "invariants": [],
           "entities": [{"name": "E", "loc": loc(9), "fields": [
               {"name": "x", "type": {"t": "int"}, "loc": loc(9)},
               {"name": "y", "type": {"t": "int"}, "loc": loc(9)}]}],
           "derived": derived, "actions": actions})
    .to_string()
}

#[test]
#[ignore]
fn perf_admission_and_evaluation() {
    let wire = large_module();
    let start = Instant::now();
    let report = admission_report(&wire);
    let admit_time = start.elapsed();
    assert!(report.ok, "{:?}", report.errors);
    assert!(
        admit_time < Duration::from_secs(2),
        "admission took {admit_time:?}"
    );

    let module = admit(&wire).unwrap();
    let request = json!({"action": "a7", "data_version": "1",
        "state": {"e": {"id": "e1", "x": 1, "y": 2}}, "input": {}, "context": {}})
    .to_string();
    let _ = evaluate(&module, &request); // warm up
    let start = Instant::now();
    let record = evaluate(&module, &request);
    let eval_time = start.elapsed();
    assert_eq!(record.result(), "ALLOW");
    assert!(
        eval_time < Duration::from_millis(10),
        "evaluation took {eval_time:?}"
    );
    eprintln!("admission: {admit_time:?}, evaluation: {eval_time:?}");
}

/// SC-006 (feature 004): the bound analysis adds at most 10% or 50 ms to admission. Measured as
/// the cost of re-running the analysis over the admitted example modules (the analysis runs once
/// per admission) against the whole admission time.
#[test]
#[ignore]
fn perf_bound_analysis_overhead() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut wires: Vec<String> = [
        "wire/valid/invoice.json",
        "wire/valid/project_margin.json",
        "wire/valid/constraints.json",
        "wire/valid/fixed_scale.json",
        "wire/valid/exact_closure.json",
        "wire/valid/exact_bound_ok.json",
    ]
    .iter()
    .map(|f| std::fs::read_to_string(root.join(f)).unwrap())
    .collect();
    wires.push(large_module());
    const N: u32 = 50;
    for wire in &wires {
        let start = Instant::now();
        for _ in 0..N {
            admit(wire).unwrap();
        }
        let admission = start.elapsed() / N;
        let m = admit(wire).unwrap();
        let start = Instant::now();
        for _ in 0..N {
            std::hint::black_box(behavior_core::admit::bounds::derived_facts(&m));
        }
        let analysis = start.elapsed() / N;
        eprintln!("admission {admission:?}, bound analysis {analysis:?}");
        assert!(
            analysis <= admission / 10 || analysis <= Duration::from_millis(50),
            "bound analysis {analysis:?} vs admission {admission:?}"
        );
    }
}
