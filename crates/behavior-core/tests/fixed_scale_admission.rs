#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Fixed-scale nominals, exact quantities, and rescale: admission, typing, hashing
//! (specs/003-fixed-scale-decimals, contracts/numeric-semantics.md → Typing).

mod common;

use behavior_core::admission_report;
use serde_json::{Value, json};

fn module() -> Value {
    common::json(&common::fixtures().join("wire/valid/fixed_scale.json"))
}

fn version(doc: &Value) -> String {
    let r = admission_report(&doc.to_string());
    assert!(r.ok, "{:?}", r.errors);
    r.behavior_version.unwrap()
}

fn codes(doc: &Value) -> Vec<String> {
    admission_report(&doc.to_string())
        .errors
        .iter()
        .map(|e| e.code.to_string())
        .collect()
}

fn l() -> Value {
    json!({"file": "t.py", "line": 1})
}

fn fld(f: &str) -> Value {
    json!({"op": "field", "param": "x", "field": f, "loc": l()})
}

fn lit(t: Value, v: Value) -> Value {
    json!({"op": "lit", "type": t, "value": v, "loc": l()})
}

fn op(name: &str, args: Vec<Value>) -> Value {
    json!({"op": name, "args": args, "loc": l()})
}

fn money() -> Value {
    json!({"t": "nominal", "name": "Money"})
}

fn exact() -> Value {
    json!({"t": "exact", "name": "Money"})
}

/// A module with `Money` (scale 2) and one derived value over `x: Row{a: Money, b: Money}`
/// declared with `ty`.
fn typed(body: Value, ty: Value) -> Value {
    json!({
        "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [], "actions": [],
        "nominals": [{"name": "Money", "underlying": {"t": "decimal"},
                      "ops": ["add", "order", "ratio", "scale"], "scale": 2, "loc": l()}],
        "entities": [{"name": "Row", "loc": l(), "fields": [
            {"name": "a", "type": money(), "loc": l()}, {"name": "b", "type": money(), "loc": l()}]}],
        "derived": [{"name": "d", "kind": "derived", "loc": l(), "type": ty,
                     "params": [{"name": "x", "type": {"t": "entity", "name": "Row"}}], "body": body}],
    })
}

fn has_type(body: Value, ty: Value) {
    let doc = typed(body.clone(), ty.clone());
    let r = admission_report(&doc.to_string());
    assert!(r.ok, "{body} : {ty}: {:?}", r.errors);
}

#[test]
fn fixed_scale_module_is_admitted() {
    let r = admission_report(&module().to_string());
    assert!(r.ok, "{:?}", r.errors);
    assert!(r.items.contains_key("nominal:Money"));
}

#[test]
fn typing_table() {
    let int = |i: i64| lit(json!({"t": "int"}), json!(i));
    let dec = |s: &str| lit(json!({"t": "decimal"}), json!(s));
    let rescale = |e: Value| json!({"op": "rescale", "nominal": "Money", "rounding": "half_even", "args": [e], "loc": l()});
    has_type(op("add", vec![fld("a"), fld("b")]), money());
    has_type(op("sub", vec![fld("a"), fld("b")]), money());
    has_type(op("mul", vec![fld("a"), int(3)]), money());
    has_type(op("mul", vec![int(3), fld("a")]), money());
    has_type(op("mul", vec![fld("a"), dec("0.25")]), exact());
    has_type(op("mul", vec![fld("a"), dec("2.0")]), exact());
    has_type(op("div", vec![fld("a"), int(3)]), exact());
    has_type(op("div", vec![fld("a"), dec("1.5")]), exact());
    has_type(
        op("add", vec![op("mul", vec![fld("a"), dec("0.5")]), fld("b")]),
        exact(),
    );
    has_type(
        op("add", vec![fld("b"), op("mul", vec![fld("a"), dec("0.5")])]),
        exact(),
    );
    has_type(
        op("mul", vec![op("div", vec![fld("a"), int(3)]), int(2)]),
        exact(),
    );
    has_type(
        op("le", vec![op("mul", vec![fld("a"), dec("1.25")]), fld("b")]),
        json!({"t": "bool"}),
    );
    // Feature 004: same-type division is an exact dimensionless ratio (was a rounded Decimal).
    let ratio = json!({"t": "exact"});
    let div = |a: Value, b: Value| op("div", vec![a, b]);
    has_type(div(fld("a"), fld("b")), ratio.clone());
    has_type(
        div(op("mul", vec![fld("a"), dec("0.5")]), fld("b")),
        ratio.clone(),
    );
    has_type(
        div(fld("a"), op("mul", vec![fld("b"), dec("0.5")])),
        ratio.clone(),
    );
    has_type(
        div(
            op("mul", vec![fld("a"), dec("0.5")]),
            op("mul", vec![fld("b"), dec("2")]),
        ),
        ratio.clone(),
    );
    has_type(op("mul", vec![div(fld("a"), fld("b")), fld("a")]), exact());
    has_type(op("mul", vec![fld("a"), div(fld("a"), fld("b"))]), exact());
    has_type(
        op("add", vec![div(fld("a"), fld("b")), dec("1")]),
        ratio.clone(),
    );
    has_type(
        op(
            "mul",
            vec![div(fld("a"), fld("b")), div(fld("b"), fld("a"))],
        ),
        ratio.clone(),
    );
    has_type(
        op("le", vec![div(fld("a"), fld("b")), dec("0.25")]),
        json!({"t": "bool"}),
    );
    has_type(
        json!({"op": "wrap", "nominal": "Money", "args": [div(fld("a"), fld("b"))], "loc": l()}),
        exact(),
    );
    has_type(rescale(op("mul", vec![fld("a"), dec("0.25")])), money());
    has_type(rescale(dec("1.005")), money());
    has_type(rescale(int(7)), money());
    has_type(rescale(fld("a")), money());
    has_type(
        json!({"op": "wrap", "nominal": "Money", "args": [int(3)], "loc": l()}),
        money(),
    );
    has_type(op("unwrap", vec![fld("a")]), json!({"t": "decimal"}));
    has_type(lit(money(), json!("12.30")), money());
}

#[test]
fn lossy_and_invalid_forms_are_rejected() {
    let dec = |s: &str| lit(json!({"t": "decimal"}), json!(s));
    let wrap_dec = json!({"op": "wrap", "nominal": "Money", "args": [dec("1.5")], "loc": l()});
    assert_eq!(codes(&typed(wrap_dec, money())), ["LOSSY_CONVERSION"]);
    // A non-numeric value is not "lossy", it is the wrong type (no rescale hint).
    let wrap_bool = json!({"op": "wrap", "nominal": "Money",
                           "args": [lit(json!({"t": "bool"}), json!(true))], "loc": l()});
    assert_eq!(codes(&typed(wrap_bool, money())), ["TYPE_MISMATCH"]);
    let unwrap_exact = op("unwrap", vec![op("mul", vec![fld("a"), dec("0.5")])]);
    assert_eq!(
        codes(&typed(unwrap_exact.clone(), json!({"t": "exact"}))),
        ["LOSSY_CONVERSION"]
    );
    let msg = admission_report(&typed(unwrap_exact, json!({"t": "exact"})).to_string()).errors[0]
        .message
        .clone();
    assert!(
        msg.contains("nominal"),
        "unwrap of an exact value erases its unit: {msg}"
    );
    let ratio_plus_amount = op("add", vec![op("div", vec![fld("a"), fld("b")]), fld("a")]);
    assert_eq!(codes(&typed(ratio_plus_amount, exact())), ["TYPE_MISMATCH"]);
    assert_eq!(
        codes(&typed(op("mul", vec![fld("a"), dec("0.5")]), money())),
        ["DECLARED_TYPE_MISMATCH"]
    );
    assert_eq!(
        codes(&typed(lit(money(), json!("0.005")), money())),
        ["OFF_GRID_LITERAL"]
    );
    assert_eq!(
        codes(&typed(op("add", vec![fld("a"), dec("1.5")]), money())),
        ["TYPE_MISMATCH"],
        "fixed-scale plus general decimal"
    );
    let msg = admission_report(&typed(op("mul", vec![fld("a"), dec("0.5")]), money()).to_string())
        .errors[0]
        .message
        .clone();
    assert!(msg.contains("rescale"), "{msg}");
}

#[test]
fn hashing_of_scale_rounding_and_declarations() {
    let base = module();
    // Renaming the nominal keeps item hashes of things that do not depend on its name.
    let mut scale3 = base.clone();
    scale3["nominals"][0]["scale"] = json!(3);
    assert_ne!(version(&base), version(&scale3));
    let mut floor = base.clone();
    floor["actions"][0]["effects"][0]["value"]["rounding"] = json!("floor");
    assert_ne!(version(&base), version(&floor));
    // A declared derived type equal to the inferred one is not hashed.
    let mut undeclared = base.clone();
    for d in undeclared["derived"].as_array_mut().unwrap() {
        d.as_object_mut().unwrap().remove("type");
    }
    assert_eq!(version(&base), version(&undeclared));
    // Names are not hashed: renaming an action keeps its item hash.
    let mut renamed = base.clone();
    renamed["actions"][0]["name"] = json!("charge_fee");
    let a = admission_report(&base.to_string());
    let b = admission_report(&renamed.to_string());
    assert_eq!(a.items["action:charge"], b.items["action:charge_fee"]);
}

/// `fee := <value>` in the `charge` action of the fixed-scale fixture.
fn store(value: Value) -> Value {
    let mut m = module();
    m["actions"][0]["effects"][0]["value"] = value;
    m
}

#[test]
fn exact_stores_are_admitted_only_when_provably_on_the_grid() {
    let amount = || json!({"op": "field", "param": "invoice", "field": "amount", "loc": l()});
    let dec = |s: &str| lit(json!({"t": "decimal"}), json!(s));
    let int = |i: i64| lit(json!({"t": "int"}), json!(i));
    // 2.0 normalizes to 2: scale 2 ≤ 2.
    assert_eq!(
        codes(&store(op("mul", vec![amount(), dec("2.0")]))),
        Vec::<String>::new()
    );
    // Scale 3 or 4, or a non-terminating quotient: not provably on the two-decimal grid.
    assert_eq!(
        codes(&store(op("mul", vec![amount(), dec("0.5")]))),
        ["LOSSY_CONVERSION"]
    );
    assert_eq!(
        codes(&store(op("div", vec![amount(), int(4)]))),
        ["LOSSY_CONVERSION"]
    );
    assert_eq!(
        codes(&store(op("div", vec![amount(), int(3)]))),
        ["LOSSY_CONVERSION"]
    );
    let msg = admission_report(&store(op("div", vec![amount(), int(3)])).to_string()).errors[0]
        .message
        .clone();
    assert!(msg.contains("rescale"), "{msg}");
}
