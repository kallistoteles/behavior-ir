#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The typing table of contracts/numeric-semantics.md, row by row (feature 004, US3): each
//! expression is admitted with its documented type, evaluated on example values, and replayed.
//! Values: `amount = 10.00`, `budget = 30.00` (two-decimal `Money`), `p = 4`, `q = 6` (unscaled
//! `Price`), `x = 0.5` (`Decimal`), `i = 3` (`Int`).

use behavior_core::{admission_report, admit, evaluate, replay};
use serde_json::{Value, json};

fn l() -> Value {
    json!({"file": "c.py", "line": 1})
}

fn fld(f: &str) -> Value {
    json!({"op": "field", "param": "r", "field": f, "loc": l()})
}

fn dec(v: &str) -> Value {
    json!({"op": "lit", "type": {"t": "decimal"}, "value": v, "loc": l()})
}

fn int(v: i64) -> Value {
    json!({"op": "lit", "type": {"t": "int"}, "value": v, "loc": l()})
}

fn op(name: &str, a: Value, b: Value) -> Value {
    json!({"op": name, "args": [a, b], "loc": l()})
}

fn wrap(n: &str, a: Value) -> Value {
    json!({"op": "wrap", "nominal": n, "args": [a], "loc": l()})
}

fn rescale(a: Value) -> Value {
    json!({"op": "rescale", "nominal": "Money", "rounding": "half_even", "args": [a], "loc": l()})
}

fn t(s: &str) -> Value {
    json!({"t": s})
}

fn nominal(n: &str) -> Value {
    json!({"t": "nominal", "name": n})
}

fn exact(n: &str) -> Value {
    json!({"t": "exact", "name": n})
}

fn module(derived: Vec<Value>, actions: Vec<Value>) -> Value {
    json!({
        "ir_version": "0.4", "enums": [], "constraints": [], "invariants": [],
        "nominals": [
            {"name": "Money", "underlying": t("decimal"), "ops": ["add", "order", "ratio", "scale"],
             "scale": 2, "loc": l()},
            {"name": "Price", "underlying": t("decimal"), "ops": ["add", "order", "ratio", "scale"],
             "loc": l()}],
        "entities": [{"name": "Row", "loc": l(), "fields": [
            {"name": "amount", "type": nominal("Money"), "loc": l()},
            {"name": "budget", "type": nominal("Money"), "loc": l()},
            {"name": "money", "type": nominal("Money"), "loc": l()},
            {"name": "p", "type": nominal("Price"), "loc": l()},
            {"name": "q", "type": nominal("Price"), "loc": l()},
            {"name": "x", "type": t("decimal"), "loc": l()},
            {"name": "i", "type": t("int"), "loc": l()}]}],
        "derived": derived,
        "actions": actions,
    })
}

fn row() -> Value {
    json!({"id": "r1", "amount": "10.00", "budget": "30.00", "money": "0.00", "p": "4", "q": "6",
           "x": "0.5", "i": 3})
}

fn state_param() -> Value {
    json!([{"name": "r", "role": "state", "type": {"t": "entity", "name": "Row"}}])
}

/// `(expression, documented type, exact value on the example row)`.
fn rows() -> Vec<(&'static str, Value, Value, Value)> {
    let (a, b) = (|| fld("amount"), || fld("budget"));
    let ratio = || op("div", a(), b());
    vec![
        ("I ± I", op("add", fld("i"), fld("i")), t("int"), json!(6)),
        ("I × I", op("mul", fld("i"), fld("i")), t("int"), json!(9)),
        (
            "I ÷ I",
            op("div", fld("i"), int(2)),
            t("exact"),
            json!("1.5"),
        ),
        (
            "D + I",
            op("add", fld("x"), fld("i")),
            t("exact"),
            json!("3.5"),
        ),
        (
            "D × D",
            op("mul", fld("x"), fld("x")),
            t("exact"),
            json!("0.25"),
        ),
        (
            "D ÷ I",
            op("div", fld("x"), fld("i")),
            t("exact"),
            json!("1/6"),
        ),
        ("R × N", op("mul", ratio(), int(3)), t("exact"), json!("1")),
        (
            "F ± F",
            op("add", a(), b()),
            nominal("Money"),
            json!("40.00"),
        ),
        (
            "F × I",
            op("mul", a(), fld("i")),
            nominal("Money"),
            json!("30.00"),
        ),
        (
            "I × F",
            op("mul", fld("i"), a()),
            nominal("Money"),
            json!("30.00"),
        ),
        (
            "T ± T",
            op("sub", fld("p"), fld("q")),
            exact("Price"),
            json!("-2"),
        ),
        (
            "T × I",
            op("mul", fld("p"), fld("i")),
            exact("Price"),
            json!("12"),
        ),
        (
            "amount * 0.25",
            op("mul", a(), dec("0.25")),
            exact("Money"),
            json!("2.5"),
        ),
        (
            "N × T",
            op("mul", dec("0.25"), a()),
            exact("Money"),
            json!("2.5"),
        ),
        (
            "amount / 3",
            op("div", a(), int(3)),
            exact("Money"),
            json!("10/3"),
        ),
        (
            "X ± T",
            op("add", op("mul", a(), dec("0.5")), b()),
            exact("Money"),
            json!("35"),
        ),
        (
            "X × R",
            op("mul", op("div", a(), int(3)), ratio()),
            exact("Money"),
            json!("10/9"),
        ),
        (
            "X ÷ R",
            op("div", op("div", a(), int(3)), ratio()),
            exact("Money"),
            json!("10"),
        ),
        (
            "(amount / budget) * amount",
            op("mul", ratio(), a()),
            exact("Money"),
            json!("10/3"),
        ),
        (
            "T × R",
            op("mul", a(), ratio()),
            exact("Money"),
            json!("10/3"),
        ),
        ("amount / budget", ratio(), t("exact"), json!("1/3")),
        (
            "X ÷ T",
            op("div", op("mul", a(), dec("0.5")), b()),
            t("exact"),
            json!("1/6"),
        ),
        (
            "T ÷ X",
            op("div", a(), op("mul", b(), dec("0.5"))),
            t("exact"),
            json!("2/3"),
        ),
        (
            "X ÷ X",
            op("div", op("div", a(), int(3)), op("div", b(), int(3))),
            t("exact"),
            json!("1/3"),
        ),
        (
            "amount * 1.25 <= budget",
            op("le", op("mul", a(), dec("1.25")), b()),
            t("bool"),
            json!(true),
        ),
        (
            "R <= D",
            op("le", ratio(), dec("0.25")),
            t("bool"),
            json!(false),
        ),
        (
            "D < I",
            op("lt", fld("x"), fld("i")),
            t("bool"),
            json!(true),
        ),
        ("T(R)", wrap("Money", ratio()), exact("Money"), json!("1/3")),
        (
            "rescale(amount / 3)",
            rescale(op("div", a(), int(3))),
            nominal("Money"),
            json!("3.33"),
        ),
    ]
}

#[test]
fn every_typing_row_admits_evaluates_exactly_and_replays() {
    let mut derived = Vec::new();
    let mut actions = Vec::new();
    for (k, (_, body, ty, _)) in rows().into_iter().enumerate() {
        let name = format!("d{k}");
        derived.push(
            json!({"name": name, "kind": "derived", "loc": l(), "type": ty, "body": body,
                            "params": [{"name": "r", "type": {"t": "entity", "name": "Row"}}]}),
        );
        let this = json!({"op": "derived", "name": name, "args": ["r"], "loc": l()});
        let cond = if ty == t("bool") {
            this
        } else {
            op("eq", this.clone(), this)
        };
        actions.push(
            json!({"name": format!("a{k}"), "loc": l(), "params": state_param(),
                            "preconditions": [{"expr": cond, "loc": l()}],
                            "effects": [], "postconditions": []}),
        );
    }
    let doc = module(derived, actions);
    let r = admission_report(&doc.to_string());
    let labels: Vec<_> = rows().iter().map(|r| r.0).collect();
    assert!(r.ok, "{:?}\nrows: {labels:?}", r.errors);
    let m = admit(&doc.to_string()).unwrap();
    let mut failures = Vec::new();
    for (k, (label, _, _, want)) in rows().into_iter().enumerate() {
        let request = json!({"action": format!("a{k}"), "context": {}, "data_version": "1",
                             "input": {}, "state": {"r": row()}});
        let text = evaluate(&m, &request.to_string()).to_json_string();
        let rec: Value = serde_json::from_str(&text).unwrap();
        let got = &rec["derived"][0]["value"];
        if *got != want || rec["result"] == "ERROR" {
            failures.push(format!(
                "{label}: expected {want}, got {got} ({})",
                rec["result"]
            ));
        }
        if !replay(&m, &text).matches {
            failures.push(format!("{label}: replay mismatch"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

fn store(value: Value) -> Vec<String> {
    let action = json!({"name": "set", "loc": l(), "params": state_param(),
                        "preconditions": [], "postconditions": [],
                        "effects": [{"target": {"param": "r", "field": "money"}, "value": value,
                                     "loc": l()}]});
    admission_report(&module(vec![], vec![action]).to_string())
        .errors
        .iter()
        .map(|e| e.code.to_string())
        .collect()
}

#[test]
fn stores_and_rejected_forms() {
    let a = || fld("amount");
    let none: Vec<String> = vec![];
    assert_eq!(store(op("div", a(), int(3))), ["LOSSY_CONVERSION"]);
    assert_eq!(store(rescale(op("div", a(), int(3)))), none);
    assert_eq!(store(op("mul", a(), int(2))), none);
    assert_eq!(store(op("div", a(), fld("budget"))), ["LOSSY_CONVERSION"]);
    // R ± T, T ± D, T with another nominal, unwrap of an exact value.
    assert_eq!(
        store(op("add", op("div", a(), fld("budget")), a())),
        ["TYPE_MISMATCH"]
    );
    assert_eq!(store(op("add", a(), dec("1.5"))), ["TYPE_MISMATCH"]);
    assert_eq!(store(op("add", a(), fld("p"))), ["TYPE_MISMATCH"]);
    let unwrap = json!({"op": "unwrap", "args": [op("div", a(), int(3))], "loc": l()});
    assert_eq!(store(unwrap), ["LOSSY_CONVERSION"]);
}

/// Review finding: a fixed-scale sum is a value of `Money` wherever it occurs, also inside exact
/// arithmetic or `underlying(...)`, so its range is checked in both positions (the verifier adds
/// the same obligation, `behavior-verify/tests/closure.rs`).
#[test]
fn fixed_scale_nodes_inside_exact_arithmetic_are_range_checked() {
    let sum = || op("add", fld("amount"), fld("budget"));
    let unwrap = json!({"op": "unwrap", "args": [sum()], "loc": l()});
    let bodies = [
        ("inside_exact", op("mul", sum(), dec("0.5")), exact("Money")),
        (
            "inside_underlying",
            op("mul", unwrap, dec("1.5")),
            t("exact"),
        ),
    ];
    for (label, body, ty) in bodies {
        let derived = vec![
            json!({"name": "d", "kind": "derived", "loc": l(), "type": ty,
            "body": body, "params": [{"name": "r", "type": {"t": "entity", "name": "Row"}}]}),
        ];
        let this = json!({"op": "derived", "name": "d", "args": ["r"], "loc": l()});
        let action = json!({"name": "a", "loc": l(), "params": state_param(),
            "preconditions": [{"expr": op("eq", this.clone(), this), "loc": l()}],
            "effects": [], "postconditions": []});
        let m = admit(&module(derived, vec![action]).to_string()).unwrap();
        let run = |amount: &str| {
            let mut state = row();
            state["amount"] = json!(amount);
            state["budget"] = json!(amount);
            let request = json!({"action": "a", "context": {}, "data_version": "1",
                                 "input": {}, "state": {"r": state}});
            let rec: Value =
                serde_json::from_str(&evaluate(&m, &request.to_string()).to_json_string()).unwrap();
            rec
        };
        // Each amount fits Money (26 integer digits); their sum does not.
        let big = run("60000000000000000000000000.00");
        assert_eq!(big["result"], "ERROR", "{label}: {big}");
        let msg = big["reasons"][0]["message"].as_str().unwrap();
        assert!(
            msg.contains("overflow") && msg.contains("r.amount + r.budget"),
            "{label}: {msg}"
        );
        assert_eq!(run("10.00")["result"], "ALLOW", "{label}");
    }
}
