//! SMT verification of admitted behavior modules, and the governance objects around it
//! (specs/002-smt-verification).
#![forbid(unsafe_code)]

pub mod cache;
pub mod checks;
pub mod confirm;
pub mod encode;
pub mod governance;
pub mod hashing;
pub mod migration;
pub mod reads;
pub mod smt;
pub mod solver;

use std::path::Path;

use serde_json::{Value as Json, json};

use behavior_core::canonical::to_canonical_string;
use behavior_core::semantic::module::Module;

use crate::checks::{CheckResult, Ctx};
use crate::hashing::{TAG_PROFILE, TAG_VERIFICATION, document_hash};
use crate::solver::Solver;

pub use migration::verify_migration;

/// Version of the verifier; part of every check key and attestation.
pub const VERIFIER_VERSION: &str = "0.6.0";

/// The kinds of checks a profile can select (research R6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckKind {
    DeadAction,
    EvaluationError,
    Postcondition,
    Preservation,
    Redundancy,
    Vacuity,
}

impl CheckKind {
    pub const ALL: [CheckKind; 6] = [
        CheckKind::DeadAction,
        CheckKind::EvaluationError,
        CheckKind::Postcondition,
        CheckKind::Preservation,
        CheckKind::Redundancy,
        CheckKind::Vacuity,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CheckKind::DeadAction => "dead_action",
            CheckKind::EvaluationError => "evaluation_error",
            CheckKind::Postcondition => "postcondition",
            CheckKind::Preservation => "preservation",
            CheckKind::Redundancy => "redundancy",
            CheckKind::Vacuity => "vacuity",
        }
    }

    pub fn parse(s: &str) -> Option<CheckKind> {
        CheckKind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// The profile kind of a check name as it appears in attestations.
    pub fn of_check(name: &str) -> Option<(&'static str, CheckKind)> {
        match name {
            "redundant_precondition" => Some(("redundant_precondition", CheckKind::Redundancy)),
            "always_true" => Some(("always_true", CheckKind::Vacuity)),
            "always_false" => Some(("always_false", CheckKind::Vacuity)),
            "referential_integrity" => Some(("referential_integrity", CheckKind::Preservation)),
            other => CheckKind::parse(other).map(|k| (k.as_str(), k)),
        }
    }
}

/// Which checks run and with what deterministic budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub checks: Vec<CheckKind>,
    /// Solver resource budget per query (deterministic).
    pub rlimit: u64,
    /// Safety cap only; results it produces are never cached or attested as verified.
    pub wall_clock_guard_ms: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            checks: CheckKind::ALL.to_vec(),
            rlimit: 20_000_000,
            wall_clock_guard_ms: 60_000,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("invalid profile: {0}")]
    Invalid(String),
}

impl Profile {
    fn normalized_checks(&self) -> Vec<CheckKind> {
        let mut c = self.checks.clone();
        c.sort();
        c.dedup();
        c
    }

    /// The profile as a document, with its hash.
    pub fn to_json(&self) -> Json {
        let mut doc = json!({
            "checks": self.normalized_checks().iter().map(|k| k.as_str()).collect::<Vec<_>>(),
            "rlimit": self.rlimit,
            "wall_clock_guard_ms": self.wall_clock_guard_ms,
        });
        let hash = document_hash(TAG_PROFILE, &doc).unwrap_or_default();
        if let Json::Object(m) = &mut doc {
            m.insert("hash".into(), json!(hash));
        }
        doc
    }

    pub fn hash(&self) -> String {
        self.to_json()["hash"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// Parses a profile document; missing fields take their defaults.
    pub fn from_json(text: &str) -> Result<Profile, ProfileError> {
        let v: Json =
            serde_json::from_str(text).map_err(|e| ProfileError::Invalid(e.to_string()))?;
        let mut p = Profile::default();
        if let Some(checks) = v.get("checks") {
            let items = checks
                .as_array()
                .ok_or_else(|| ProfileError::Invalid("`checks` must be a list".into()))?;
            p.checks = items
                .iter()
                .map(|c| {
                    c.as_str()
                        .and_then(CheckKind::parse)
                        .ok_or_else(|| ProfileError::Invalid(format!("unknown check {c}")))
                })
                .collect::<Result<_, _>>()?;
        }
        for (field, slot) in [
            ("rlimit", &mut p.rlimit),
            ("wall_clock_guard_ms", &mut p.wall_clock_guard_ms),
        ] {
            if let Some(n) = v.get(field) {
                *slot = n.as_u64().ok_or_else(|| {
                    ProfileError::Invalid(format!("`{field}` must be a positive integer"))
                })?;
            }
        }
        Ok(p)
    }
}

/// The verification attestation (contracts/verification-report.md).
#[derive(Debug, Clone)]
pub struct Attestation {
    /// The full document, including per-check `cached` flags and `hash`.
    pub value: Json,
    pub result: String,
    pub hash: String,
    pub checks: Vec<CheckResult>,
}

impl Attestation {
    pub fn to_json_string(&self) -> String {
        to_canonical_string(&self.value).unwrap_or_default()
    }

    pub fn findings(&self) -> Vec<Json> {
        self.value["findings"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

fn sort_key(c: &CheckResult) -> (String, &'static str, String, String) {
    (
        c.action
            .as_ref()
            .map(|(n, _)| n.clone())
            .unwrap_or_default(),
        c.check,
        c.subject.hash.clone(),
        c.subject.param.clone().unwrap_or_default(),
    )
}

/// Verifies `module` under `profile`. Every selected check is reported as proven, counterexample
/// (confirmed by evaluation), or inconclusive; the module is verified only without blocking
/// findings.
pub fn verify(
    module: &Module,
    profile: &Profile,
    cache: Option<&Path>,
    solver: &dyn Solver,
) -> Attestation {
    let ctx = Ctx {
        module,
        cache: cache.map(cache::Cache::new),
        solver,
        rlimit: profile.rlimit,
        profile_hash: profile.hash(),
        solver_version: solver.version(),
    };
    let kinds = profile.normalized_checks();
    let mut results = Vec::new();
    if kinds.contains(&CheckKind::Vacuity) {
        results.extend(checks::vacuity(&ctx));
    }
    for action in module.actions().keys() {
        let dead = kinds.contains(&CheckKind::DeadAction);
        let redundant = kinds.contains(&CheckKind::Redundancy);
        if dead || redundant {
            results.extend(checks::dead_and_redundant(&ctx, action, dead, redundant));
        }
        if kinds.contains(&CheckKind::Preservation) {
            results.extend(checks::preservation(&ctx, action));
        }
        if kinds.contains(&CheckKind::Postcondition) {
            results.extend(checks::postconditions(&ctx, action));
        }
        if kinds.contains(&CheckKind::EvaluationError) {
            results.extend(checks::evaluation_errors(&ctx, action));
        }
    }
    // Declared reads (feature 010): their evaluation errors.
    if kinds.contains(&CheckKind::EvaluationError) {
        for read in module.reads().keys() {
            results.extend(reads::evaluation_errors(&ctx, read));
        }
    }
    results.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    assemble(module, profile, &ctx.solver_version, results)
}

fn assemble(
    module: &Module,
    profile: &Profile,
    solver_version: &str,
    checks: Vec<CheckResult>,
) -> Attestation {
    let mut findings: Vec<Json> = checks.iter().filter_map(|c| c.finding.clone()).collect();
    findings.sort_by(|a, b| a["hash"].as_str().cmp(&b["hash"].as_str()));
    findings.dedup_by(|a, b| a["hash"] == b["hash"]);
    // Inconclusive checks carry blocking `inconclusive` findings (FR-009).
    let blocking = findings.iter().any(|f| f["severity"] == "blocking");
    let result = if blocking { "not_verified" } else { "verified" };
    let doc = |with_cached: bool| {
        json!({
            "attestation_version": "1",
            "behavior_version": module.behavior_version(),
            "profile": profile.to_json(),
            "verifier_version": VERIFIER_VERSION,
            "solver_version": solver_version,
            "result": result,
            "checks": checks.iter().map(|c| c.to_json(with_cached)).collect::<Vec<_>>(),
            "findings": findings,
        })
    };
    let hash = document_hash(TAG_VERIFICATION, &doc(false)).unwrap_or_default();
    let mut value = doc(true);
    if let Json::Object(m) = &mut value {
        m.insert("hash".into(), json!(hash));
    }
    Attestation {
        value,
        result: result.to_string(),
        hash,
        checks,
    }
}
