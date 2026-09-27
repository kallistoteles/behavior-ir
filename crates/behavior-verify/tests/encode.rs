#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Encoding checks: each query is satisfiable or unsatisfiable exactly as the runtime semantics
//! require.

use behavior_core::admit;
use behavior_verify::encode::{ActionEncoding, StepKind, and_all};
use behavior_verify::solver::{Query, Solver, SolverAnswer, Z3Process};
use serde_json::{Value, json};

fn l() -> Value {
    json!({"file": "t.py", "line": 1})
}
fn f(p: &str, x: &str) -> Value {
    json!({"op": "field", "param": p, "field": x, "loc": l()})
}
fn lit(t: Value, v: Value) -> Value {
    json!({"op": "lit", "type": t, "value": v, "loc": l()})
}
fn op(name: &str, args: Vec<Value>) -> Value {
    json!({"op": name, "args": args, "loc": l()})
}
fn cond(e: Value) -> Value {
    json!({"expr": e, "loc": l()})
}
fn module(fields: Value, enums: Value, invariants: Value, actions: Value) -> String {
    json!({"ir_version": "0.1", "enums": enums, "nominals": [], "derived": [],
           "entities": [{"name": "C", "loc": l(), "fields": fields}],
           "invariants": invariants, "actions": actions})
    .to_string()
}
fn action(
    name: &str,
    params: Value,
    pre: Vec<Value>,
    effects: Vec<Value>,
    post: Vec<Value>,
) -> Value {
    json!({"name": name, "loc": l(), "params": params, "preconditions": pre, "effects": effects, "postconditions": post})
}
fn state(name: &str) -> Value {
    json!({"name": name, "role": "state", "type": {"t": "entity", "name": "C"}})
}
fn eff(p: &str, x: &str, v: Value) -> Value {
    json!({"target": {"param": p, "field": x}, "value": v, "loc": l()})
}

fn sat(enc: &ActionEncoding<'_>, assertions: &[String]) -> bool {
    let q = Query {
        script: enc
            .enc
            .script(assertions, behavior_verify::encode::Nice::Raw),
        get: vec![],
        rlimit: 50_000_000,
    };
    match Z3Process::from_env().unwrap().check(&q) {
        SolverAnswer::Sat(_) => true,
        SolverAnswer::Unsat => false,
        other => panic!("{other:?}"),
    }
}

#[test]
fn int_overflow_is_reachable() {
    let wire = module(
        json!([{"name": "n", "type": {"t": "int"}, "loc": l()}]),
        json!([]),
        json!([]),
        json!([action(
            "inc",
            json!([state("c")]),
            vec![],
            vec![eff(
                "c",
                "n",
                op("add", vec![f("c", "n"), lit(json!({"t": "int"}), json!(1))])
            )],
            vec![]
        )]),
    );
    let m = admit(&wire).unwrap();
    let enc = ActionEncoding::build(&m, "inc").unwrap();
    let k = enc
        .steps
        .iter()
        .position(|s| s.kind == StepKind::Effect)
        .unwrap();
    let o = &enc.steps[k].obligations[0];
    assert!(sat(
        &enc,
        &[enc.path(k), and_all(&[o.guard.clone(), o.cond.clone()])]
    ));
}

#[test]
fn enum_values_are_limited_to_the_declaration() {
    let wire = module(
        json!([{"name": "st", "type": {"t": "enum", "name": "S"}, "loc": l()}]),
        json!([{"name": "S", "values": ["a", "b"], "loc": l()}]),
        json!([]),
        json!([action("noop", json!([state("c")]), vec![], vec![], vec![])]),
    );
    let m = admit(&wire).unwrap();
    let enc = ActionEncoding::build(&m, "noop").unwrap();
    assert!(!sat(&enc, &["(= state.c.st 2)".into()]));
    assert!(sat(&enc, &["(= state.c.st 1)".into()]));
}

#[test]
fn options_have_presence_and_value() {
    let wire = module(
        json!([{"name": "o", "type": {"t": "option", "of": {"t": "int"}}, "loc": l()}]),
        json!([]),
        json!([]),
        json!([action(
            "check",
            json!([state("c")]),
            vec![cond(op("is_none", vec![f("c", "o")]))],
            vec![],
            vec![]
        )]),
    );
    let m = admit(&wire).unwrap();
    let enc = ActionEncoding::build(&m, "check").unwrap();
    let pre = enc.steps[0].cond.clone().unwrap();
    assert!(sat(&enc, std::slice::from_ref(&pre)));
    assert!(!sat(&enc, &[pre, "state.c.o!some".into()]));
}

#[test]
fn state_identities_are_distinct() {
    let wire = module(
        json!([{"name": "n", "type": {"t": "int"}, "loc": l()}]),
        json!([]),
        json!([]),
        json!([action(
            "pair",
            json!([state("a"), state("b")]),
            vec![],
            vec![],
            vec![]
        )]),
    );
    let m = admit(&wire).unwrap();
    let enc = ActionEncoding::build(&m, "pair").unwrap();
    assert!(!sat(&enc, &["(= state.a.id state.b.id)".into()]));
}

fn decrement(with_guard: bool) -> String {
    let int = json!({"t": "int"});
    let pre = if with_guard {
        vec![cond(op(
            "gt",
            vec![f("c", "n"), lit(int.clone(), json!(0))],
        ))]
    } else {
        vec![]
    };
    module(
        json!([{"name": "n", "type": int, "loc": l()}]),
        json!([]),
        json!([{"name": "non_negative", "entity": "C", "param": "c", "loc": l(),
                "body": op("ge", vec![f("c", "n"), lit(int.clone(), json!(0))])}]),
        json!([action(
            "dec",
            json!([state("c")]),
            pre,
            vec![eff(
                "c",
                "n",
                op("sub", vec![f("c", "n"), lit(int, json!(1))])
            )],
            vec![]
        )]),
    )
}

#[test]
fn post_state_uses_the_effects() {
    for (guarded, breakable) in [(true, false), (false, true)] {
        let m = admit(&decrement(guarded)).unwrap();
        let enc = ActionEncoding::build(&m, "dec").unwrap();
        let k = enc
            .steps
            .iter()
            .position(|s| s.kind == StepKind::InvariantPost)
            .unwrap();
        let broken = format!("(not {})", enc.steps[k].cond.clone().unwrap());
        assert_eq!(
            sat(&enc, &[enc.path(k), enc.noerr(k), broken]),
            breakable,
            "guarded={guarded}"
        );
    }
}

fn rounding(bound: &str) -> String {
    let dec = json!({"t": "decimal"});
    module(
        json!([{"name": "a", "type": dec, "loc": l()}]),
        json!([]),
        json!([]),
        json!([action(
            "third",
            json!([state("c")]),
            vec![cond(op(
                "eq",
                vec![f("c", "a"), lit(dec.clone(), json!("1"))]
            ))],
            vec![],
            vec![cond(op(
                "le",
                vec![
                    op("div", vec![f("c", "a"), lit(dec.clone(), json!("3"))]),
                    lit(dec, json!(bound))
                ]
            ))]
        )]),
    )
}

#[test]
fn rounding_bounds_decide_only_with_margin() {
    // 1/3 exceeds 0.333…3 (28 digits) by less than the rounding bound: neither side is proven.
    let m = admit(&rounding("0.3333333333333333333333333333")).unwrap();
    let enc = ActionEncoding::build(&m, "third").unwrap();
    let k = enc
        .steps
        .iter()
        .position(|s| s.kind == StepKind::Postcondition)
        .unwrap();
    let post = enc.steps[k].cond.clone().unwrap();
    assert!(sat(&enc, &[enc.path(k), post.clone()]));
    assert!(sat(&enc, &[enc.path(k), format!("(not {post})")]));
    // With a clear margin the postcondition is proven.
    let m = admit(&rounding("0.34")).unwrap();
    let enc = ActionEncoding::build(&m, "third").unwrap();
    let k = enc
        .steps
        .iter()
        .position(|s| s.kind == StepKind::Postcondition)
        .unwrap();
    let post = enc.steps[k].cond.clone().unwrap();
    assert!(!sat(&enc, &[enc.path(k), format!("(not {post})")]));
}
