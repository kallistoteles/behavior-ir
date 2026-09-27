#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Admission-time representation analysis (feature 004, contracts/numeric-semantics.md → Bound
//! analysis). Facts per expression: numerator bits `nb`, denominator bits `db`, decimal scale,
//! coefficient digits `cd`. Leaves: fixed-scale (scale s: nb 94, db bits(10^s), cd 28), general
//! decimal (scale 28, cd 56, nb 94, db 94), integer (scale 0, cd 19, nb 63, db 1).

mod common;

use behavior_core::admission_report;
use behavior_core::admit::bounds::{Facts, derived_facts};
use serde_json::{Value, json};

fn l() -> Value {
    json!({"file": "b.py", "line": 1})
}

fn fld(f: &str) -> Value {
    json!({"op": "field", "param": "x", "field": f, "loc": l()})
}

fn lit(t: &str, v: Value) -> Value {
    json!({"op": "lit", "type": {"t": t}, "value": v, "loc": l()})
}

fn op(name: &str, a: Value, b: Value) -> Value {
    json!({"op": name, "args": [a, b], "loc": l()})
}

fn der(name: &str) -> Value {
    json!({"op": "derived", "name": name, "args": ["x"], "loc": l()})
}

/// A module with `Money` (scale 2) and `Row{a, b: Money; d, e: Decimal; i: Int}`, one derived
/// value per `(name, body)`.
fn module(bodies: &[(&str, Value)]) -> Value {
    let money = json!({"t": "nominal", "name": "Money"});
    let derived: Vec<Value> = bodies
        .iter()
        .map(|(n, body)| {
            json!({"name": n, "kind": "derived", "loc": l(), "body": body,
                   "params": [{"name": "x", "type": {"t": "entity", "name": "Row"}}]})
        })
        .collect();
    json!({
        "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [], "actions": [],
        "nominals": [{"name": "Money", "underlying": {"t": "decimal"},
                      "ops": ["add", "order", "ratio", "scale"], "scale": 2, "loc": l()}],
        "entities": [{"name": "Row", "loc": l(), "fields": [
            {"name": "a", "type": money, "loc": l()}, {"name": "b", "type": money, "loc": l()},
            {"name": "d", "type": {"t": "decimal"}, "loc": l()},
            {"name": "e", "type": {"t": "decimal"}, "loc": l()},
            {"name": "i", "type": {"t": "int"}, "loc": l()}]}],
        "derived": derived,
    })
}

fn facts(bodies: &[(&str, Value)]) -> std::collections::BTreeMap<String, Facts> {
    let doc = module(bodies);
    let m = behavior_core::admit(&doc.to_string()).unwrap_or_else(|r| panic!("{:?}", r.errors));
    derived_facts(&m)
}

fn f(nb: u32, db: u32, scale: Option<u32>, cd: Option<u32>) -> Facts {
    Facts { nb, db, scale, cd }
}

#[test]
fn leaves() {
    let got = facts(&[("money", fld("a")), ("dec", fld("d")), ("int", fld("i"))]);
    assert_eq!(got["money"], f(94, 7, Some(2), Some(28)));
    assert_eq!(got["dec"], f(94, 94, Some(28), Some(56)));
    assert_eq!(got["int"], f(63, 1, Some(0), Some(19)));
}

#[test]
fn literals_are_exact() {
    let got = facts(&[
        ("half", lit("decimal", json!("0.5"))),
        ("big", lit("decimal", json!("1250.25"))),
    ]);
    assert_eq!(got["half"], f(1, 2, Some(1), Some(1)));
    // 1250.25 = 100020/80 = 5001/4: numerator 13 bits, denominator 3 bits; scale 2, 6 digits.
    assert_eq!(got["big"], f(13, 3, Some(2), Some(6)));
}

#[test]
fn operations() {
    let got = facts(&[
        ("sum", op("add", fld("a"), fld("b"))),
        ("product", op("mul", fld("a"), fld("d"))),
        ("by_three", op("div", fld("a"), lit("int", json!(3)))),
        ("by_four", op("div", fld("a"), lit("int", json!(4)))),
        ("by_half", op("div", fld("a"), lit("decimal", json!("0.5")))),
        ("general", op("div", fld("a"), fld("d"))),
    ]);
    // a ± b: nb = max(94+7, 94+7)+1, db = 7+7, scale 2, cd = max(28, 28)+1.
    assert_eq!(got["sum"], f(102, 14, Some(2), Some(29)));
    // a · d: nb 94+94, db 7+94, scale 2+28, cd 28+56.
    assert_eq!(got["product"], f(188, 101, Some(30), Some(84)));
    // ÷ 3: nb 94+1, db 7+2; 3 is not 2^i·5^j, so no finite scale.
    assert_eq!(got["by_three"], f(95, 9, None, None));
    // ÷ 4 = 2^2: scale 2+2, cd 28+2+0.
    assert_eq!(got["by_four"], f(95, 10, Some(4), Some(30)));
    // ÷ 0.5 = 5·10^-1: scale 2+1, cd 28+1+1.
    assert_eq!(got["by_half"], f(96, 8, Some(3), Some(30)));
    // General division: nb 94+94, db 7+94, no scale.
    assert_eq!(got["general"], f(188, 101, None, None));
}

#[test]
fn derived_references_reuse_the_body_bound() {
    let got = facts(&[
        ("quarter", op("div", fld("a"), lit("int", json!(4)))),
        ("twice", op("mul", der("quarter"), lit("int", json!(2)))),
    ]);
    // quarter (95, 10, 4, 30) · 2 (nb 2, db 1).
    assert_eq!(got["twice"], f(97, 11, Some(4), Some(31)));
}

fn chain(n: usize) -> Value {
    (0..n).fold(fld("a"), |acc, _| op("div", acc, fld("d")))
}

#[test]
fn division_chains_cross_the_bound_at_the_computed_step() {
    // a ÷ d ÷ d ÷ …: nb grows by 94 per division from 94: 4 divisions → 470 bits, 5 → 564.
    let got = facts(&[("four", chain(4))]);
    assert_eq!(got["four"].nb, 470);
    let r = admission_report(&module(&[("five", chain(5))]).to_string());
    assert!(!r.ok);
    let e = r
        .errors
        .iter()
        .find(|e| e.code == "EXACT_BOUND_EXCEEDED")
        .expect("bound error");
    assert!(
        e.message.contains("564") && e.message.contains("511"),
        "{}",
        e.message
    );
}

#[test]
fn integer_nominal_arithmetic_is_not_bounded_as_exact() {
    // Integer arithmetic never enters the exact domain (the runtime's 64-bit check bounds it),
    // also for an integer nominal: `n · i · i · …` (12 factors, `Count × Int` stays `Count`) is
    // admitted; as exact values the bits would add up to 756 > 511.
    let count = json!({"t": "nominal", "name": "Count"});
    let body = (0..11).fold(fld("n"), |acc, _| op("mul", acc, fld("i")));
    let doc = json!({
        "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [], "actions": [],
        "nominals": [{"name": "Count", "underlying": {"t": "int"}, "ops": ["add", "order", "scale"],
                      "loc": l()}],
        "entities": [{"name": "Row", "loc": l(), "fields": [
            {"name": "n", "type": count, "loc": l()}, {"name": "i", "type": {"t": "int"}, "loc": l()}]}],
        "derived": [{"name": "big", "kind": "derived", "loc": l(), "body": body,
                     "params": [{"name": "x", "type": {"t": "entity", "name": "Row"}}]}],
    });
    let r = admission_report(&doc.to_string());
    assert!(r.ok, "{:?}", r.errors);
}
