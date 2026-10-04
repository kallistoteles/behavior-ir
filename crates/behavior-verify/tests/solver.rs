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

// --- feature 008: the solver prerequisite ---------------------------------------------------

#[test]
fn a_missing_solver_names_the_prerequisite() {
    let err = Z3Process::new("/nonexistent/z3".into(), std::time::Duration::from_secs(1))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with(
            "verification needs the Z3 SMT solver (supported: 4.16.0); install z3 on PATH or set BEHAVIOR_Z3"
        ),
        "{err}"
    );
    assert!(err.contains("/nonexistent/z3"), "the cause is kept: {err}");
}

#[test]
fn another_solver_version_is_reported() {
    assert_eq!(behavior_verify::solver::SUPPORTED_Z3, "4.16.0");
    assert_eq!(z3().version_mismatch(), None);
    let dir = std::env::temp_dir().join(format!("behavior-fake-z3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("z3");
    std::fs::write(&fake, "#!/bin/sh\necho 'Z3 version 4.15.0 - 64 bit'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A process another test thread forks while the script is being written inherits the write
    // handle until it execs, and running the script meanwhile fails with ETXTBSY ("Text file
    // busy"). That is a property of writing and running a file in one multi-threaded process,
    // not of the solver check, so the start is retried briefly.
    let mut tries = 0;
    let old = loop {
        match Z3Process::new(fake.clone(), std::time::Duration::from_secs(1)) {
            Err(e) if e.to_string().contains("Text file busy") && tries < 100 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            r => break r.unwrap(),
        }
    };
    let notice = old.version_mismatch().unwrap();
    assert!(
        notice.contains("4.15.0") && notice.contains("4.16.0"),
        "{notice}"
    );
}
