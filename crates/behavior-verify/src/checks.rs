//! The checks (research R6). Each check is one or two solver queries over an action's runtime
//! path; satisfiable answers are confirmed by evaluation before they count.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use behavior_core::semantic::module::{Kind, Module, ParamRole};
use behavior_core::semantic::types::{Type, hash_display};
use behavior_core::wire::Loc;

use crate::CheckKind;
use crate::confirm::{Confirmed, Expect, confirm};
use crate::encode::{ActionEncoding, Encoder, ErrKind, Nice, RuleEncoding, StepKind};
use crate::hashing::{check_key, finding_hash};

use crate::smt::SmtValue;
use crate::solver::{Query, Solver, SolverAnswer, UnknownReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Proven,
    Counterexample,
    Inconclusive(String),
}

impl Outcome {
    pub fn parse(outcome: &str, reason: Option<&str>) -> Option<Outcome> {
        match outcome {
            "proven" => Some(Outcome::Proven),
            "counterexample" => Some(Outcome::Counterexample),
            "inconclusive" => Some(Outcome::Inconclusive(reason?.to_string())),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Outcome::Proven => "proven",
            Outcome::Counterexample => "counterexample",
            Outcome::Inconclusive(_) => "inconclusive",
        }
    }
}

/// What a check is about: a rule, condition, or effect, possibly bound to an action parameter.
#[derive(Debug, Clone)]
pub struct Subject {
    pub kind: String,
    pub name: String,
    pub hash: String,
    pub loc: Loc,
    pub param: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    /// The profile entry that selected this check.
    pub kind: CheckKind,
    /// The check's name in the attestation (`always_true` and `always_false` are both
    /// `vacuity` checks).
    pub check: &'static str,
    /// Action name and hash (`None` for checks not tied to an action).
    pub action: Option<(String, String)>,
    pub subject: Subject,
    pub key: String,
    pub outcome: Outcome,
    pub cached: bool,
    pub finding: Option<Json>,
}

impl CheckResult {
    pub fn to_json(&self, with_cached: bool) -> Json {
        let mut subject = json!({
            "kind": self.subject.kind,
            "name": self.subject.name,
            "hash": self.subject.hash,
            "loc": loc_json(&self.subject.loc),
        });
        if let (Some(p), Json::Object(m)) = (&self.subject.param, &mut subject) {
            m.insert("param".into(), json!(p));
        }
        let mut c = json!({
            "kind": self.check,
            "action": self.action.as_ref().map(|(n, h)| json!({"name": n, "hash": h})),
            "subject": subject,
            "key": self.key,
            "outcome": self.outcome.as_str(),
        });
        if let Json::Object(m) = &mut c {
            if let Outcome::Inconclusive(reason) = &self.outcome {
                m.insert("reason".into(), json!(reason));
                if reason == UnknownReason::WallClockGuard.as_str() {
                    m.insert("reproducible".into(), json!(false));
                }
            }
            if with_cached {
                m.insert("cached".into(), json!(self.cached));
            }
        }
        c
    }
}

impl CheckResult {
    /// Rebuilds a result from its cache entry (`to_json(false)` plus the finding).
    pub fn from_json(check: &Json, finding: Option<Json>) -> Option<CheckResult> {
        let str_of = |v: &Json| v.as_str().map(str::to_string);
        let (check_name, kind) = CheckKind::of_check(check["kind"].as_str()?)?;
        let action = match &check["action"] {
            Json::Null => None,
            a => Some((str_of(&a["name"])?, str_of(&a["hash"])?)),
        };
        let s = &check["subject"];
        let subject = Subject {
            kind: str_of(&s["kind"])?,
            name: str_of(&s["name"])?,
            hash: str_of(&s["hash"])?,
            loc: Loc {
                file: str_of(&s["loc"]["file"])?,
                line: s["loc"]["line"].as_u64()?,
            },
            param: s.get("param").and_then(str_of),
        };
        let outcome = Outcome::parse(
            check["outcome"].as_str()?,
            check.get("reason").and_then(|r| r.as_str()),
        )?;
        Some(CheckResult {
            kind,
            check: check_name,
            action,
            subject,
            key: str_of(&check["key"])?,
            outcome,
            cached: true,
            finding,
        })
    }

    /// Only results that another run would reproduce may be cached.
    pub fn reproducible(&self) -> bool {
        self.outcome != Outcome::Inconclusive(UnknownReason::WallClockGuard.as_str().to_string())
            && !self.key.is_empty()
    }
}

pub fn loc_json(l: &Loc) -> Json {
    json!({"file": l.file, "line": l.line})
}

/// Shared inputs of every check in one verification run.
pub struct Ctx<'a> {
    pub module: &'a Module,
    pub cache: Option<crate::cache::Cache>,
    pub solver: &'a dyn Solver,
    pub rlimit: u64,
    pub profile_hash: String,
    pub solver_version: String,
}

enum Search {
    Unsat,
    Unknown(UnknownReason),
    /// Candidate models, most readable first.
    Models(Vec<BTreeMap<String, SmtValue>>),
}

/// Unrestricted query first (a proof covers every value); if satisfiable, further queries
/// restricted to readable decimals for a nicer counterexample (research R5).
fn search(ctx: &Ctx<'_>, enc: &Encoder<'_>, assertions: &[String]) -> Search {
    let get = enc.input_symbols();
    let query = |nice| Query {
        script: enc.script(assertions, nice),
        get: get.clone(),
        rlimit: ctx.rlimit,
    };
    match ctx.solver.check(&query(Nice::Raw)) {
        SolverAnswer::Unsat => Search::Unsat,
        SolverAnswer::Unknown(r) => Search::Unknown(r),
        // Feature 007: a model of the summaries is not yet a state; the witness query asks for
        // one whose summaries are folds over explicit unknown members (research R10).
        SolverAnswer::Sat(_) if enc.has_slots() => {
            let witness = |nice| Query {
                script: enc.script_with(assertions, nice, true),
                get: get.clone(),
                rlimit: ctx.rlimit,
            };
            let mut models = Vec::new();
            for level in [Nice::Full, Nice::Scale, Nice::Raw] {
                if let SolverAnswer::Sat(m) = ctx.solver.check(&witness(level)) {
                    models.push(m);
                    break;
                }
            }
            Search::Models(models)
        }
        SolverAnswer::Sat(raw) => {
            let mut models = Vec::new();
            for level in [Nice::Full, Nice::Scale] {
                if let SolverAnswer::Sat(m) = ctx.solver.check(&query(level)) {
                    models.push(m);
                    break;
                }
            }
            models.push(raw);
            Search::Models(models)
        }
    }
}

fn entity_hash(module: &Module, entity: &str) -> Option<String> {
    module
        .name_table()
        .get(&(Kind::Entity, entity.to_string()))
        .map(hash_display)
}

/// Everything an action check depends on besides the action itself: the definitions of its
/// entity types and every rule the runtime assumes for them.
fn action_dependencies(module: &Module, action: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(a) = module.action(action) else {
        return out;
    };
    // Feature 007: queried types and module invariants are assumptions too.
    for entity in behavior_core::queried_types(module, a) {
        out.extend(entity_hash(module, &entity));
        out.extend(
            module
                .constraints_for(&entity)
                .map(|(_, c)| hash_display(c.hash())),
        );
        out.extend(
            module
                .invariants_for(&entity)
                .map(|(_, i)| hash_display(i.hash())),
        );
    }
    out.extend(
        module
            .global_invariants()
            .values()
            .map(|g| hash_display(g.hash())),
    );
    for p in a.params() {
        let Type::Entity(entity) = p.ty() else {
            continue;
        };
        out.extend(entity_hash(module, entity));
        out.extend(
            module
                .constraints_for(entity)
                .map(|(_, c)| hash_display(c.hash())),
        );
        if p.role() == ParamRole::State {
            out.extend(
                module
                    .invariants_for(entity)
                    .map(|(_, i)| hash_display(i.hash())),
            );
        }
    }
    out
}

/// Runs `search` and confirmation for one property; returns the outcome and the confirmed
/// counterexample, if any.
fn decide(
    ctx: &Ctx<'_>,
    ae: &ActionEncoding<'_>,
    assertions: &[String],
    expect: &Expect,
) -> (Outcome, Option<Confirmed>) {
    match search(ctx, &ae.enc, assertions) {
        Search::Unsat => (Outcome::Proven, None),
        Search::Unknown(r) => (Outcome::Inconclusive(r.as_str().to_string()), None),
        Search::Models(models) => {
            for m in &models {
                if let Some(c) = confirm(ctx.module, &ae.enc, &ae.action, m, expect) {
                    return (Outcome::Counterexample, Some(c));
                }
            }
            (
                Outcome::Inconclusive("counterexample_not_reproduced".into()),
                None,
            )
        }
    }
}

fn counterexample_json(c: &Confirmed) -> Json {
    let mut j =
        json!({"state": c.state, "input": c.input, "context": c.context, "record": c.record});
    if let (Some(f), Json::Object(m)) = (&c.facts, &mut j) {
        m.insert("facts".into(), f.clone());
    }
    j
}

/// One property to decide for an action.
pub(crate) struct Spec {
    pub kind: CheckKind,
    pub check: &'static str,
    pub subject: Subject,
    /// Content hashes the outcome depends on besides the action's (part of the check key).
    pub depends: Vec<String>,
    /// Finding identity: roles and content hashes (research R8).
    pub cites: Vec<(&'static str, String)>,
    pub locs: Vec<Loc>,
}

/// Inputs shared by every check of one action.
pub(crate) struct ActionCtx<'a, 'm> {
    pub ae: &'a ActionEncoding<'m>,
    pub name: String,
    pub hash: String,
    pub loc: Loc,
    pub param_index: BTreeMap<String, usize>,
}

impl ActionCtx<'_, '_> {
    fn cites(&self, spec: &Spec) -> Vec<(&'static str, String)> {
        let mut c = vec![("action", self.hash.clone())];
        c.extend(spec.cites.iter().cloned());
        if let Some(i) = spec
            .subject
            .param
            .as_ref()
            .and_then(|p| self.param_index.get(p))
        {
            c.push(("param_index", i.to_string()));
        }
        c
    }
}

fn cites_json(cites: &[(&'static str, String)]) -> Json {
    Json::Object(
        cites
            .iter()
            .filter(|(role, _)| *role != "param_index")
            .map(|(role, h)| (role.to_string(), json!(h)))
            .collect(),
    )
}

/// A blocking finding for a check the solver or confirmation could not decide (FR-009).
fn inconclusive_finding(a: &ActionCtx<'_, '_>, spec: &Spec, reason: &str) -> Json {
    let mut cites = vec![("check", spec.check.to_string())];
    cites.extend(a.cites(spec));
    json!({
        "hash": finding_hash("inconclusive", &cites),
        "kind": "inconclusive",
        "check": spec.check,
        "severity": "blocking",
        "cites": cites_json(&cites[1..]),
        "locs": spec.locs.iter().map(loc_json).collect::<Vec<_>>(),
        "explanation": format!(
            "{} of {} in {} is inconclusive ({reason})",
            spec.check, spec.subject.name, a.name
        ),
    })
}

/// A blocking finding with a confirmed counterexample.
fn counterexample_finding(
    a: &ActionCtx<'_, '_>,
    spec: &Spec,
    explanation: String,
    c: &Confirmed,
) -> Json {
    let cites = a.cites(spec);
    json!({
        "hash": finding_hash(spec.check, &cites),
        "kind": spec.check,
        "severity": "blocking",
        "cites": cites_json(&cites),
        "locs": spec.locs.iter().map(loc_json).collect::<Vec<_>>(),
        "explanation": explanation,
        "counterexample": counterexample_json(c),
    })
}

/// Decides one safety property: the assertions describe a violation, `expect` what evaluation
/// must show. Cached results are reused; non-reproducible ones are never stored.
pub(crate) fn run_safety(
    ctx: &Ctx<'_>,
    a: &ActionCtx<'_, '_>,
    spec: Spec,
    assertions: &[String],
    expect: &Expect,
    explanation: String,
) -> CheckResult {
    let mut subjects = vec![a.hash.clone(), spec.subject.hash.clone()];
    subjects.extend(spec.depends.iter().cloned());
    if let Some(p) = &spec.subject.param {
        subjects.push(format!(
            "param:{}",
            a.param_index.get(p).copied().unwrap_or(0)
        ));
    }
    let key = check_key(
        spec.check,
        &subjects,
        &ctx.profile_hash,
        crate::VERIFIER_VERSION,
        &ctx.solver_version,
    );
    if let Some(hit) = ctx.cache.as_ref().and_then(|c| c.get(&key)) {
        return hit;
    }
    let (outcome, cx) = decide(ctx, a.ae, assertions, expect);
    let finding = match (&outcome, &cx) {
        (Outcome::Counterexample, Some(c)) => {
            Some(counterexample_finding(a, &spec, explanation, c))
        }
        (Outcome::Inconclusive(reason), _) => Some(inconclusive_finding(a, &spec, reason)),
        _ => None,
    };
    let result = CheckResult {
        kind: spec.kind,
        check: spec.check,
        action: Some((a.name.clone(), a.hash.clone())),
        subject: spec.subject,
        key,
        outcome,
        cached: false,
        finding,
    };
    if let Some(c) = &ctx.cache {
        c.put(&result);
    }
    result
}

/// Builds the action encoding and the shared context, or an inconclusive result if the action
/// cannot be encoded (a verifier bug for admitted modules, reported loudly).
pub(crate) fn with_action<'m>(
    ctx: &Ctx<'m>,
    kind: CheckKind,
    action: &str,
    f: impl FnOnce(&ActionCtx<'_, 'm>) -> Vec<CheckResult>,
) -> Vec<CheckResult> {
    let module = ctx.module;
    let Some(item) = module.action(action) else {
        return Vec::new();
    };
    let hash = hash_display(item.hash());
    match ActionEncoding::build(module, action) {
        Ok(ae) => {
            let a = ActionCtx {
                ae: &ae,
                name: action.to_string(),
                hash,
                loc: item.loc().clone(),
                param_index: item
                    .params()
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.name().to_string(), i))
                    .collect(),
            };
            f(&a)
        }
        Err(e) => {
            let cites = [
                ("check", kind.as_str().to_string()),
                ("action", hash.clone()),
            ];
            let finding = json!({
                "hash": finding_hash("inconclusive", &cites),
                "kind": "inconclusive",
                "check": kind.as_str(),
                "severity": "blocking",
                "cites": {"action": hash},
                "locs": [loc_json(item.loc())],
                "explanation": format!("{} of {action} is inconclusive (encoding_error: {e})", kind.as_str()),
            });
            vec![CheckResult {
                kind,
                check: kind.as_str(),
                action: Some((action.to_string(), hash.clone())),
                subject: Subject {
                    kind: "action".into(),
                    name: action.to_string(),
                    hash,
                    loc: item.loc().clone(),
                    param: None,
                },
                key: String::new(),
                outcome: Outcome::Inconclusive("encoding_error".into()),
                cached: false,
                finding: Some(finding),
            }]
        }
    }
}

/// Invariant and constraint preservation (FR-001): for every state invariant and every entity
/// constraint of every state parameter, can an allowed run of the action end in an S' that
/// violates it?
pub fn preservation(ctx: &Ctx<'_>, action: &str) -> Vec<CheckResult> {
    let module = ctx.module;
    let deps = action_dependencies(module, action);
    with_action(ctx, CheckKind::Preservation, action, |a| {
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut out = Vec::new();
        for (k, step) in a.ae.steps.iter().enumerate() {
            if step.kind == StepKind::Integrity {
                out.push(integrity(ctx, a, &deps, k, action));
                continue;
            }
            let (phase, subject_kind) = match step.kind {
                StepKind::InvariantPost => ("invariant_post", "invariant"),
                StepKind::ConstraintPost => ("constraint_post", "constraint"),
                StepKind::InvariantGlobal => ("invariant_global", "invariant_global"),
                _ => continue,
            };
            let index = {
                let n = counts.entry(phase).or_insert(0);
                *n += 1;
                *n - 1
            };
            let rule_hash = hash_display(&step.hash);
            let rule_loc =
                rule_loc(module, subject_kind, &step.name).unwrap_or_else(|| step.loc.clone());
            let param = step.bound.clone();
            let cond = step.cond.clone().unwrap_or_else(|| "true".into());
            let assertions = [a.ae.path(k), a.ae.noerr(k), format!("(not {cond})")];
            let explanation = match &param {
                Some(p) => format!("{action} can break {} for {p}", step.name),
                None => format!("{action} can break the module invariant {}", step.name),
            };
            let spec = Spec {
                kind: CheckKind::Preservation,
                check: CheckKind::Preservation.as_str(),
                subject: Subject {
                    kind: subject_kind.into(),
                    name: step.name.clone(),
                    hash: rule_hash.clone(),
                    loc: rule_loc.clone(),
                    param,
                },
                depends: deps.clone(),
                cites: vec![(subject_kind, rule_hash)],
                locs: vec![a.loc.clone(), rule_loc],
            };
            out.push(run_safety(
                ctx,
                a,
                spec,
                &assertions,
                &Expect::RuleFails { phase, index },
                explanation,
            ));
        }
        out
    })
}

/// Referential integrity (feature 006, research R12): can an allowed run remove an identity that
/// a surviving `Ref` field still points at? Part of preservation: it preserves the invariant "no
/// surviving reference to an absent entity".
fn integrity(
    ctx: &Ctx<'_>,
    a: &ActionCtx<'_, '_>,
    deps: &[String],
    k: usize,
    action: &str,
) -> CheckResult {
    let step = &a.ae.steps[k];
    let param = step.bound.clone().unwrap_or_default();
    let hash = hash_display(&step.hash);
    let cond = step.cond.clone().unwrap_or_else(|| "true".into());
    let assertions = [a.ae.path(k), a.ae.noerr(k), format!("(not {cond})")];
    let spec = Spec {
        kind: CheckKind::Preservation,
        check: "referential_integrity",
        subject: Subject {
            kind: "removal".into(),
            name: step.name.clone(),
            hash: hash.clone(),
            loc: step.loc.clone(),
            param: Some(param.clone()),
        },
        depends: deps.to_vec(),
        cites: vec![("removal", hash)],
        locs: vec![a.loc.clone(), step.loc.clone()],
    };
    run_safety(
        ctx,
        a,
        spec,
        &assertions,
        &Expect::Reason {
            code: "DANGLING_REFERENCE",
        },
        format!("{action} can remove {param} while a reference to it survives"),
    )
}

/// Postconditions (FR-002): can an allowed run reach S' where `ensures` fails? Rules checked
/// on S before the postconditions are part of the path; their failure would already deny.
pub fn postconditions(ctx: &Ctx<'_>, action: &str) -> Vec<CheckResult> {
    let deps = action_dependencies(ctx.module, action);
    with_action(ctx, CheckKind::Postcondition, action, |a| {
        let mut out = Vec::new();
        let mut index = 0;
        for (k, step) in a.ae.steps.iter().enumerate() {
            if step.kind != StepKind::Postcondition {
                continue;
            }
            let hash = hash_display(&step.hash);
            let cond = step.cond.clone().unwrap_or_else(|| "true".into());
            let assertions = [a.ae.path(k), a.ae.noerr(k), format!("(not {cond})")];
            let spec = Spec {
                kind: CheckKind::Postcondition,
                check: CheckKind::Postcondition.as_str(),
                subject: Subject {
                    kind: "postcondition".into(),
                    name: step.name.clone(),
                    hash: hash.clone(),
                    loc: step.loc.clone(),
                    param: None,
                },
                depends: deps.clone(),
                cites: vec![("postcondition", hash)],
                locs: vec![a.loc.clone(), step.loc.clone()],
            };
            let explanation = format!(
                "{action} can allow a transition that violates `ensures {}`",
                step.name
            );
            out.push(run_safety(
                ctx,
                a,
                spec,
                &assertions,
                &Expect::RuleFails {
                    phase: "postcondition",
                    index,
                },
                explanation,
            ));
            index += 1;
        }
        out
    })
}

/// Evaluation errors (FR-005): can division by zero or overflow occur in a precondition, an
/// effect, a postcondition, or a rule on S', on a path that reaches it from a valid state? The
/// first error in evaluation order stops evaluation, so earlier obligations of the same step are
/// assumed not to fire.
pub fn evaluation_errors(ctx: &Ctx<'_>, action: &str) -> Vec<CheckResult> {
    let deps = action_dependencies(ctx.module, action);
    with_action(ctx, CheckKind::EvaluationError, action, |a| {
        let mut out = Vec::new();
        for (k, step) in a.ae.steps.iter().enumerate() {
            if matches!(step.kind, StepKind::Constraint | StepKind::InvariantPre) {
                continue;
            }
            let step_hash = hash_display(&step.hash);
            for (j, o) in step.obligations.iter().enumerate() {
                let mut assertions = vec![a.ae.path(k)];
                for earlier in &step.obligations[..j] {
                    assertions.push(format!(
                        "(not {})",
                        crate::encode::and_all(&[earlier.guard.clone(), earlier.cond.clone()])
                    ));
                }
                assertions.push(crate::encode::and_all(&[o.guard.clone(), o.cond.clone()]));
                let hash = hash_display(&o.hash);
                let error = match o.kind {
                    ErrKind::DivisionByZero => "division_by_zero",
                    ErrKind::Overflow => "overflow",
                };
                let message = format!("{} in {}", o.kind.message(), o.text);
                let mut depends = deps.clone();
                depends.extend([
                    step_hash.clone(),
                    format!("error:{error}"),
                    format!("obligation:{j}"),
                ]);
                let spec = Spec {
                    kind: CheckKind::EvaluationError,
                    check: CheckKind::EvaluationError.as_str(),
                    subject: Subject {
                        kind: "expression".into(),
                        name: format!("{} ({})", o.text, o.kind.message()),
                        hash: hash.clone(),
                        loc: o.loc.clone(),
                        param: None,
                    },
                    depends,
                    cites: vec![
                        ("expression", hash),
                        ("step", step_hash.clone()),
                        ("error", error.to_string()),
                    ],
                    locs: vec![a.loc.clone(), o.loc.clone()],
                };
                let explanation = format!("{action} can fail with {message}");
                out.push(run_safety(
                    ctx,
                    a,
                    spec,
                    &assertions,
                    &Expect::Error { message },
                    explanation,
                ));
            }
        }
        out
    })
}

// --- warnings (US4) --------------------------------------------------------------------------

/// One warning property: "proven" means the property holds, which is the (warning) finding; a
/// satisfying assignment is a witness that it does not, and needs no confirmation (a spurious
/// witness can only hide a warning, never a blocking finding).
struct Warning {
    kind: CheckKind,
    check: &'static str,
    action: Option<(String, String)>,
    subject: Subject,
    depends: Vec<String>,
    cites: Vec<(&'static str, String)>,
    locs: Vec<Loc>,
    explanation: String,
}

fn run_warning(ctx: &Ctx<'_>, enc: &Encoder<'_>, w: Warning, assertions: &[String]) -> CheckResult {
    let mut subjects: Vec<String> = w.action.iter().map(|(_, h)| h.clone()).collect();
    subjects.push(w.subject.hash.clone());
    subjects.extend(w.depends.iter().cloned());
    let key = check_key(
        w.check,
        &subjects,
        &ctx.profile_hash,
        crate::VERIFIER_VERSION,
        &ctx.solver_version,
    );
    if let Some(hit) = ctx.cache.as_ref().and_then(|c| c.get(&key)) {
        return hit;
    }
    let query = Query {
        script: enc.script(assertions, Nice::Raw),
        get: Vec::new(),
        rlimit: ctx.rlimit,
    };
    let outcome = match ctx.solver.check(&query) {
        SolverAnswer::Unsat => Outcome::Proven,
        SolverAnswer::Sat(_) => Outcome::Counterexample,
        SolverAnswer::Unknown(r) => Outcome::Inconclusive(r.as_str().to_string()),
    };
    let locs: Vec<Json> = w.locs.iter().map(loc_json).collect();
    let finding = match &outcome {
        Outcome::Proven => Some(json!({
            "hash": finding_hash(w.check, &w.cites),
            "kind": w.check,
            "severity": "warning",
            "cites": cites_json(&w.cites),
            "locs": locs,
            "explanation": w.explanation,
        })),
        Outcome::Inconclusive(reason) => {
            let mut cites = vec![("check", w.check.to_string())];
            cites.extend(w.cites.iter().cloned());
            Some(json!({
                "hash": finding_hash("inconclusive", &cites),
                "kind": "inconclusive",
                "check": w.check,
                "severity": "blocking",
                "cites": cites_json(&w.cites),
                "locs": locs,
                "explanation": format!("{} of {} is inconclusive ({reason})", w.check, w.subject.name),
            }))
        }
        Outcome::Counterexample => None,
    };
    let result = CheckResult {
        kind: w.kind,
        check: w.check,
        action: w.action,
        subject: w.subject,
        key,
        outcome,
        cached: false,
        finding,
    };
    if let Some(c) = &ctx.cache {
        c.put(&result);
    }
    result
}

/// Dead actions (no valid state, input, and context passes every precondition) and redundant
/// preconditions (always true when reached) (FR-003, FR-004). The preconditions of a dead action
/// are not checked for redundancy: everything is vacuously implied there.
pub fn dead_and_redundant(
    ctx: &Ctx<'_>,
    action: &str,
    dead: bool,
    redundant: bool,
) -> Vec<CheckResult> {
    let deps = action_dependencies(ctx.module, action);
    let kind = if dead {
        CheckKind::DeadAction
    } else {
        CheckKind::Redundancy
    };
    with_action(ctx, kind, action, |a| {
        let mut out = Vec::new();
        let action_id = Some((a.name.clone(), a.hash.clone()));
        let end = a.ae.after_preconditions();
        let mut is_dead = false;
        if dead {
            let r = run_warning(
                ctx,
                &a.ae.enc,
                Warning {
                    kind: CheckKind::DeadAction,
                    check: "dead_action",
                    action: action_id.clone(),
                    subject: Subject {
                        kind: "action".into(),
                        name: action.to_string(),
                        hash: a.hash.clone(),
                        loc: a.loc.clone(),
                        param: None,
                    },
                    depends: deps.clone(),
                    cites: vec![("action", a.hash.clone())],
                    locs: vec![a.loc.clone()],
                    explanation: format!(
                        "{action} can never run: its preconditions cannot all hold for a valid state, input, and context"
                    ),
                },
                &[a.ae.path(end)],
            );
            is_dead = r.outcome == Outcome::Proven;
            out.push(r);
        }
        if redundant && !is_dead {
            for (k, step) in a.ae.steps.iter().enumerate().take(end) {
                if step.kind != StepKind::Precondition {
                    continue;
                }
                let hash = hash_display(&step.hash);
                let cond = step.cond.clone().unwrap_or_else(|| "true".into());
                out.push(run_warning(
                    ctx,
                    &a.ae.enc,
                    Warning {
                        kind: CheckKind::Redundancy,
                        check: "redundant_precondition",
                        action: action_id.clone(),
                        subject: Subject {
                            kind: "precondition".into(),
                            name: step.name.clone(),
                            hash: hash.clone(),
                            loc: step.loc.clone(),
                            param: None,
                        },
                        depends: deps.clone(),
                        cites: vec![("action", a.hash.clone()), ("precondition", hash)],
                        locs: vec![a.loc.clone(), step.loc.clone()],
                        explanation: format!(
                            "precondition `{}` of {action} always holds when it is checked",
                            step.name
                        ),
                    },
                    &[a.ae.path(k), a.ae.noerr(k), format!("(not {cond})")],
                ));
            }
        }
        out
    })
}

/// Rules that are always true or always false for valid instances of their parameters (only
/// type domains and entity constraints are assumed) (FR-004).
pub fn vacuity(ctx: &Ctx<'_>) -> Vec<CheckResult> {
    let module = ctx.module;
    let mut out = Vec::new();
    for (name, d) in module.derived_items() {
        if !d.is_rule() {
            continue;
        }
        let hash = hash_display(d.hash());
        let mut depends = Vec::new();
        for p in d.params() {
            if let Type::Entity(entity) = p.ty() {
                depends.extend(entity_hash(module, entity));
                depends.extend(
                    module
                        .constraints_for(entity)
                        .map(|(_, c)| hash_display(c.hash())),
                );
            }
        }
        let subject = Subject {
            kind: "rule".into(),
            name: name.clone(),
            hash: hash.clone(),
            loc: d.loc().clone(),
            param: None,
        };
        let re = match RuleEncoding::build(module, name) {
            Ok(re) => re,
            Err(e) => {
                let cites = vec![("check", "vacuity".to_string()), ("rule", hash.clone())];
                out.push(CheckResult {
                    kind: CheckKind::Vacuity,
                    check: "always_true",
                    action: None,
                    subject,
                    key: String::new(),
                    outcome: Outcome::Inconclusive("encoding_error".into()),
                    cached: false,
                    finding: Some(json!({
                        "hash": finding_hash("inconclusive", &cites),
                        "kind": "inconclusive",
                        "check": "vacuity",
                        "severity": "blocking",
                        "cites": {"rule": hash},
                        "locs": [loc_json(d.loc())],
                        "explanation": format!("vacuity of {name} is inconclusive (encoding_error: {e})"),
                    })),
                });
                continue;
            }
        };
        for (check, negate, what) in [
            ("always_true", true, "true"),
            ("always_false", false, "false"),
        ] {
            let mut assertions = re.assumptions.clone();
            assertions.push(re.noerr.clone());
            assertions.push(if negate {
                format!("(not {})", re.cond)
            } else {
                re.cond.clone()
            });
            out.push(run_warning(
                ctx,
                &re.enc,
                Warning {
                    kind: CheckKind::Vacuity,
                    check,
                    action: None,
                    subject: subject.clone(),
                    depends: depends.clone(),
                    cites: vec![("rule", hash.clone())],
                    locs: vec![d.loc().clone()],
                    explanation: format!("rule {name} is always {what} for valid entities"),
                },
                &assertions,
            ));
        }
    }
    out
}

fn rule_loc(module: &Module, kind: &str, name: &str) -> Option<Loc> {
    match kind {
        "invariant" => module.invariants().get(name).map(|i| i.loc().clone()),
        "constraint" => module.constraints().get(name).map(|c| c.loc().clone()),
        "invariant_global" => module
            .global_invariants()
            .get(name)
            .map(|g| g.loc().clone()),
        _ => None,
    }
}
