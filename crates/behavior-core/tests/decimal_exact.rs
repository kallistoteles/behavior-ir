#![allow(clippy::unwrap_used, clippy::expect_used)]

//! General decimals are exact (feature 004, contracts/numeric-semantics.md → Typing, Stores):
//! arithmetic yields `Exact<Decimal>` / `Exact<T>`, and a store without rescale is admitted only
//! when admission proves the value representable from types and literals alone.

mod common;

use behavior_core::{admission_report, admit, evaluate};
use serde_json::{Value, json};

fn l() -> Value {
    json!({"file": "d.py", "line": 1})
}

fn fld(f: &str) -> Value {
    json!({"op": "field", "param": "r", "field": f, "loc": l()})
}

fn lit(t: &str, v: Value) -> Value {
    json!({"op": "lit", "type": {"t": t}, "value": v, "loc": l()})
}

fn op(name: &str, a: Value, b: Value) -> Value {
    json!({"op": name, "args": [a, b], "loc": l()})
}

fn price() -> Value {
    json!({"t": "nominal", "name": "Price"})
}

/// `Row{x, y, dec, other: Decimal; i, j: Int; p, q: Price}` with `Price` an unscaled decimal
/// nominal; one derived value `d := body` (declared `ty` if given) and one action `set` with
/// `r.<target> := value` (if given).
fn module(body: Value, ty: Option<Value>, store: Option<(&str, Value)>) -> Value {
    let dec = json!({"t": "decimal"});
    let int = json!({"t": "int"});
    let fields: Vec<Value> = [
        ("x", &dec),
        ("y", &dec),
        ("dec", &dec),
        ("other", &dec),
        ("i", &int),
        ("j", &int),
        ("p", &price()),
        ("q", &price()),
    ]
    .iter()
    .map(|(n, t)| json!({"name": n, "type": t, "loc": l()}))
    .collect();
    let mut derived = json!({"name": "d", "kind": "derived", "loc": l(), "body": body,
                             "params": [{"name": "r", "type": {"t": "entity", "name": "Row"}}]});
    if let Some(t) = ty {
        derived["type"] = t;
    }
    let actions: Vec<Value> = store
        .into_iter()
        .map(|(target, value)| {
            json!({"name": "set", "loc": l(), "preconditions": [], "postconditions": [],
                   "params": [{"name": "r", "role": "state", "type": {"t": "entity", "name": "Row"}}],
                   "effects": [{"target": {"param": "r", "field": target}, "value": value, "loc": l()}]})
        })
        .collect();
    json!({
        "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [],
        "nominals": [{"name": "Price", "underlying": {"t": "decimal"},
                      "ops": ["add", "order", "ratio", "scale"], "loc": l()}],
        "entities": [{"name": "Row", "loc": l(), "fields": fields}],
        "derived": [derived],
        "actions": actions,
    })
}

fn codes(doc: &Value) -> Vec<String> {
    admission_report(&doc.to_string())
        .errors
        .iter()
        .map(|e| e.code.to_string())
        .collect()
}

fn has_type(body: Value, ty: Value) {
    let doc = module(body.clone(), Some(ty.clone()), None);
    let r = admission_report(&doc.to_string());
    assert!(r.ok, "{body} : {ty}: {:?}", r.errors);
}

fn stores(target: &str, value: Value) -> Vec<String> {
    codes(&module(lit("int", json!(0)), None, Some((target, value))))
}

#[test]
fn decimal_arithmetic_is_exact() {
    let ratio = json!({"t": "exact"});
    let exact_price = json!({"t": "exact", "name": "Price"});
    for o in ["add", "sub", "mul", "div"] {
        has_type(op(o, fld("x"), fld("y")), ratio.clone());
        has_type(op(o, fld("x"), fld("i")), ratio.clone());
    }
    has_type(op("div", fld("i"), fld("j")), ratio.clone());
    for o in ["add", "sub", "mul"] {
        has_type(op(o, fld("i"), fld("j")), json!({"t": "int"}));
    }
    has_type(op("add", fld("p"), fld("q")), exact_price.clone());
    has_type(op("sub", fld("p"), fld("q")), exact_price.clone());
    has_type(op("mul", fld("p"), fld("i")), exact_price.clone());
    has_type(op("mul", fld("p"), fld("x")), exact_price.clone());
    has_type(op("div", fld("p"), fld("q")), ratio.clone());
    has_type(
        op(
            "lt",
            op("div", fld("x"), fld("y")),
            lit("decimal", json!("0.05")),
        ),
        json!({"t": "bool"}),
    );
}

#[test]
fn stores_are_admitted_only_when_provably_representable() {
    let none: Vec<String> = vec![];
    assert_eq!(
        stores("dec", op("sub", fld("x"), fld("y"))),
        ["LOSSY_CONVERSION"]
    );
    assert_eq!(
        stores(
            "dec",
            op("div", lit("int", json!(10)), lit("int", json!(2)))
        ),
        none
    );
    assert_eq!(
        stores("dec", op("div", fld("x"), lit("int", json!(3)))),
        ["LOSSY_CONVERSION"]
    );
    assert_eq!(stores("dec", fld("i")), none);
    assert_eq!(stores("dec", fld("other")), none);
    assert_eq!(
        stores("p", op("add", fld("p"), fld("q"))),
        ["LOSSY_CONVERSION"]
    );
    assert_eq!(stores("dec", lit("decimal", json!("1.5"))), none);
    let msg = admission_report(
        &module(
            lit("int", json!(0)),
            None,
            Some(("dec", op("sub", fld("x"), fld("y")))),
        )
        .to_string(),
    )
    .errors[0]
        .message
        .clone();
    assert!(msg.contains("rescale"), "{msg}");
    // A declared derived type follows the same rule.
    assert_eq!(
        codes(&module(
            op("sub", fld("x"), fld("y")),
            Some(json!({"t": "decimal"})),
            None
        )),
        ["DECLARED_TYPE_MISMATCH"]
    );
}

#[test]
fn constant_store_evaluates_exactly() {
    let doc = module(
        lit("int", json!(0)),
        None,
        Some((
            "dec",
            op("div", lit("int", json!(10)), lit("int", json!(2))),
        )),
    );
    let m = admit(&doc.to_string()).unwrap();
    let row = json!({"id": "r1", "x": "1", "y": "2", "dec": "0", "other": "0", "i": 1, "j": 2,
                     "p": "1", "q": "2"});
    let request = json!({"action": "set", "context": {}, "data_version": "1", "input": {},
                         "state": {"r": row}});
    let rec: Value =
        serde_json::from_str(&evaluate(&m, &request.to_string()).to_json_string()).unwrap();
    assert_eq!(rec["result"], "ALLOW", "{rec}");
    assert_eq!(rec["changes"][0]["new"], "5", "{rec}");
}

#[test]
fn admission_does_not_read_entity_constraints() {
    // Bounding x and y to [0, 1] would make x - y representable, but admission proves
    // representability from types and literals only.
    let mut doc = module(
        lit("int", json!(0)),
        None,
        Some(("dec", op("sub", fld("x"), fld("y")))),
    );
    let bound = |f: &str| {
        let e = json!({"op": "field", "param": "e", "field": f, "loc": l()});
        json!({"name": format!("{f}_bounded"), "entity": "Row", "param": "e", "loc": l(),
               "body": json!({"op": "and", "args": [
                   op("ge", e.clone(), lit("decimal", json!("0"))),
                   op("le", e, lit("decimal", json!("1")))], "loc": l()})})
    };
    doc["constraints"] = json!([bound("x"), bound("y")]);
    assert_eq!(codes(&doc), ["LOSSY_CONVERSION"]);
}

fn margin(revenue: &str, cost: &str) -> Value {
    let m = admit(&common::read(
        &common::fixtures().join("wire/valid/project_margin.json"),
    ))
    .unwrap();
    let request = json!({"action": "flag_project", "context": {}, "data_version": "1", "input": {},
                         "state": {"project": {"id": "p1", "revenue": revenue, "cost": cost,
                                               "flagged": false}}});
    let rec: Value =
        serde_json::from_str(&evaluate(&m, &request.to_string()).to_json_string()).unwrap();
    let d = rec["derived"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "margin")
        .cloned();
    d.unwrap_or_else(|| panic!("no margin in {rec}"))["value"].clone()
}

#[test]
fn project_margin_is_exact() {
    assert_eq!(margin("100", "33"), "0.67");
    assert_eq!(margin("3", "2"), "1/3");
}
