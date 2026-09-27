#![allow(clippy::unwrap_used, clippy::expect_used)]

use behavior_verify::solver::{Query, Solver, SolverAnswer, UnknownReason, Z3Process};

const BUDGET: &str = "(declare-const budget Real) (declare-const spent Real) (declare-const amount Real)
(declare-const limit Real)
(assert (<= spent budget)) (assert (<= amount limit)) (assert (>= amount 0.0)) (assert (>= spent 0.0))
(assert (not (<= (+ spent amount) budget)))";

fn z3() -> Z3Process {
    Z3Process::from_env().unwrap()
}

#[test]
fn reports_its_version() {
    assert_eq!(z3().version(), "z3 4.16.0");
}

#[test]
fn sat_models_are_deterministic() {
    let q = Query {
        script: BUDGET.into(),
        get: vec!["budget".into(), "spent".into(), "amount".into()],
        rlimit: 10_000_000,
    };
    let a = z3().check(&q);
    let b = z3().check(&q);
    assert!(matches!(a, SolverAnswer::Sat(_)), "{a:?}");
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[test]
fn unsat_is_reported() {
    let q = Query {
        script: "(declare-const x Int) (assert (> x 1)) (assert (< x 0))".into(),
        get: vec![],
        rlimit: 1_000_000,
    };
    assert!(matches!(z3().check(&q), SolverAnswer::Unsat));
}

#[test]
fn tiny_resource_limit_is_unknown() {
    let q = Query {
        script: "(declare-const x Real) (declare-const y Real) (declare-const z Real)
                 (assert (= (+ (* x x x) (* y y y)) (* z z z))) (assert (> x 1.5)) (assert (> y 2.5)) (assert (> z 0.0))"
            .into(),
        get: vec![],
        rlimit: 1,
    };
    match z3().check(&q) {
        SolverAnswer::Unknown(UnknownReason::ResourceLimit) => {}
        other => panic!("{other:?}"),
    }
}
