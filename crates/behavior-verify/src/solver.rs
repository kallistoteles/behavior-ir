//! The solver behind a trait, and the pinned Z3 binary as a subprocess (research R1, R2).
//!
//! Determinism: fixed seeds, one thread, and a resource budget (`rlimit`) rather than a
//! wall-clock timeout. The wall-clock guard only stops runaway processes; results it produces
//! are reported as `WallClockGuard` and are never cached.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::smt::{Sexp, SmtValue, parse, value};

/// One satisfiability question: declarations and assertions, the symbols whose values are
/// wanted if satisfiable, and the resource budget.
#[derive(Debug, Clone)]
pub struct Query {
    pub script: String,
    pub get: Vec<String>,
    pub rlimit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    SolverUnknown,
    ResourceLimit,
    WallClockGuard,
    Error(String),
}

impl UnknownReason {
    pub fn as_str(&self) -> &str {
        match self {
            UnknownReason::SolverUnknown => "solver_unknown",
            UnknownReason::ResourceLimit => "resource_limit",
            UnknownReason::WallClockGuard => "wall_clock_guard",
            UnknownReason::Error(_) => "solver_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolverAnswer {
    Sat(BTreeMap<String, SmtValue>),
    Unsat,
    Unknown(UnknownReason),
}

pub trait Solver {
    fn check(&self, query: &Query) -> SolverAnswer;
    fn version(&self) -> String;
}

#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    #[error("cannot run the solver at {0}: {1}")]
    Spawn(String, String),
}

/// The Z3 binary, driven over stdin/stdout with SMT-LIB 2.
#[derive(Debug, Clone)]
pub struct Z3Process {
    path: PathBuf,
    version: String,
    guard: Duration,
}

impl Z3Process {
    /// Uses `BEHAVIOR_Z3` if set, otherwise `z3` on `PATH`.
    pub fn from_env() -> Result<Self, SolverError> {
        let path = std::env::var_os("BEHAVIOR_Z3")
            .map(PathBuf::from)
            .unwrap_or_else(|| "z3".into());
        Self::new(path, Duration::from_secs(60))
    }

    pub fn new(path: PathBuf, guard: Duration) -> Result<Self, SolverError> {
        let out = Command::new(&path)
            .arg("--version")
            .output()
            .map_err(|e| SolverError::Spawn(path.display().to_string(), e.to_string()))?;
        let text = String::from_utf8_lossy(&out.stdout);
        // "Z3 version 4.16.0 - 64 bit"
        let number = text.split_whitespace().nth(2).unwrap_or("unknown");
        Ok(Z3Process {
            path,
            version: format!("z3 {number}"),
            guard,
        })
    }

    pub fn with_guard(mut self, guard: Duration) -> Self {
        self.guard = guard;
        self
    }

    fn run(&self, script: &str) -> Result<String, UnknownReason> {
        let mut child = Command::new(&self.path)
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| UnknownReason::Error(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(script.as_bytes())
                .map_err(|e| UnknownReason::Error(e.to_string()))?;
        }
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| UnknownReason::Error("no stdout".into()))?;
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stdout.read_to_string(&mut s);
            s
        });
        let deadline = Instant::now() + self.guard;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(UnknownReason::WallClockGuard);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(e) => return Err(UnknownReason::Error(e.to_string())),
            }
        }
        reader
            .join()
            .map_err(|_| UnknownReason::Error("reader thread failed".into()))
    }
}

impl Solver for Z3Process {
    fn version(&self) -> String {
        self.version.clone()
    }

    fn check(&self, query: &Query) -> SolverAnswer {
        let mut script = String::new();
        script.push_str("(set-option :produce-models true)\n");
        script.push_str(&format!("(set-option :rlimit {})\n", query.rlimit.max(1)));
        script.push_str("(set-option :smt.random_seed 0)\n(set-option :sat.random_seed 0)\n");
        script.push_str(&query.script);
        script.push_str("\n(check-sat)\n(get-info :reason-unknown)\n");
        if !query.get.is_empty() {
            script.push_str(&format!("(get-value ({}))\n", query.get.join(" ")));
        }
        let output = match self.run(&script) {
            Ok(o) => o,
            Err(reason) => return SolverAnswer::Unknown(reason),
        };
        let parsed = match parse(&output) {
            Ok(p) => p,
            Err(e) => return SolverAnswer::Unknown(UnknownReason::Error(e.to_string())),
        };
        match parsed.first() {
            Some(Sexp::Atom(a)) if a == "unsat" => SolverAnswer::Unsat,
            Some(Sexp::Atom(a)) if a == "sat" => {
                let mut model = BTreeMap::new();
                if let Some(Sexp::List(pairs)) = parsed.get(2) {
                    for pair in pairs {
                        if let Sexp::List(kv) = pair
                            && let [Sexp::Atom(name), v] = kv.as_slice()
                        {
                            match value(v) {
                                Ok(val) => {
                                    model.insert(name.clone(), val);
                                }
                                Err(e) => {
                                    return SolverAnswer::Unknown(UnknownReason::Error(
                                        e.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                SolverAnswer::Sat(model)
            }
            Some(Sexp::Atom(a)) if a == "unknown" => {
                let reason = match parsed.get(1) {
                    Some(Sexp::List(items)) => match items.get(1) {
                        Some(Sexp::Str(r)) => r.clone(),
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                if reason.contains("resource")
                    || reason.contains("rlimit")
                    || reason.contains("canceled")
                {
                    SolverAnswer::Unknown(UnknownReason::ResourceLimit)
                } else {
                    SolverAnswer::Unknown(UnknownReason::SolverUnknown)
                }
            }
            other => SolverAnswer::Unknown(UnknownReason::Error(format!(
                "unexpected output: {other:?}"
            ))),
        }
    }
}
