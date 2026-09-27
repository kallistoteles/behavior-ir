#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Performance targets (SC-003, SC-004). Ignored by default; run with
//! `cargo test --release -p behavior-verify --test perf -- --ignored --nocapture`.

mod common;

use std::time::{Duration, Instant};

use behavior_verify::solver::Z3Process;
use behavior_verify::{Profile, verify};
use serde_json::{Value, json};

fn wire(path: &str) -> String {
    common::read(&common::fixtures().join(path))
}

#[test]
#[ignore]
fn example_modules_verify_within_ten_seconds() {
    let z3 = Z3Process::from_env().unwrap();
    let start = Instant::now();
    for f in [
        "wire/valid/invoice.json",
        "wire/valid/project_margin.json",
        "verify/purchase.json",
    ] {
        let m = behavior_core::admit(&wire(f)).unwrap();
        let a = verify(&m, &Profile::default(), None, &z3);
        eprintln!("{f}: {} checks, {}", a.checks.len(), a.result);
    }
    let total = start.elapsed();
    eprintln!("total: {total:?}");
    assert!(total < Duration::from_secs(10), "{total:?}");
}

/// `purchase_fixed` with `n` distinct copies of `approve`, each with its own limit; `bump`
/// changes the limit of the first copy.
fn many_actions(n: usize, bump: bool) -> String {
    let mut m: Value = serde_json::from_str(&wire("verify/purchase_fixed.json")).unwrap();
    let approve = m["actions"][0].clone();
    let mut actions = Vec::new();
    for i in 0..n {
        let mut a = approve.clone();
        a["name"] = json!(format!("approve_{i:02}"));
        let limit = if bump && i == 0 { 1_000_000 } else { 1000 + i };
        let loc = a["loc"].clone();
        a["preconditions"].as_array_mut().unwrap().push(json!({
            "expr": {"op": "le", "loc": loc, "args": [
                {"op": "field", "param": "purchase", "field": "amount", "loc": loc},
                {"op": "lit", "type": {"t": "nominal", "name": "Money"}, "value": limit.to_string(), "loc": loc},
            ]},
            "loc": loc,
        }));
        actions.push(a);
    }
    m["actions"] = Value::Array(actions);
    m.to_string()
}

#[test]
#[ignore]
fn warm_reverification_after_one_change_is_cheap() {
    let z3 = Z3Process::from_env().unwrap();
    let dir = std::env::temp_dir().join(format!("behavior-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let base = behavior_core::admit(&many_actions(50, false)).unwrap();
    let changed = behavior_core::admit(&many_actions(50, true)).unwrap();

    let start = Instant::now();
    let cold = verify(&base, &Profile::default(), Some(&dir), &z3);
    let cold_t = start.elapsed();
    let start = Instant::now();
    let warm = verify(&changed, &Profile::default(), Some(&dir), &z3);
    let warm_t = start.elapsed();
    let _ = std::fs::remove_dir_all(&dir);

    let recomputed = warm.checks.iter().filter(|c| !c.cached).count();
    eprintln!(
        "cold {cold_t:?} ({} checks), warm {warm_t:?} ({recomputed} recomputed)",
        cold.checks.len()
    );
    assert!(recomputed > 0 && recomputed * 10 < warm.checks.len());
    assert!(warm_t * 5 < cold_t, "warm {warm_t:?} vs cold {cold_t:?}");
}
