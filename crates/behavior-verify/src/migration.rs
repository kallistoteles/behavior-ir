//! Migration verification (feature 009, FR-020, FR-021, research R9).
//!
//! **Local**, per migrated entity type `T`: one symbolic source entity `old`, assumed to satisfy
//! the source module's constraints and invariants of `T`, the source module invariants and every
//! source requirement, with `old` a known member of every query over `T` (so `all(select(T), p)`
//! gives `p(old)`). Under these assumptions the verifier proves that every narrowing succeeds,
//! that no other evaluation error occurs, and that the transformed value satisfies every target
//! constraint and invariant of `T`. A satisfiable violation counts only once running the
//! transform on the reported entity reproduces it. A proof that needs the requirements says so
//! (`under`).
//!
//! **Whole-state**: referential integrity is proven only when every target reference of a
//! migrated type is carried over from a source reference to the same type (or absent); a target
//! module invariant only when the source module states the same invariant and the migration
//! copies every field it reads. Everything else is inconclusive: precision debt, never assumed,
//! and always checked in full when the migration is applied.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value as Json, json};

use behavior_core::migration::{FieldSource, Migration, OLD, transform_one};
use behavior_core::semantic::expr::{Expr, ExprKind};
use behavior_core::semantic::module::{Module, Param, ParamRole};
use behavior_core::semantic::types::{Type, hash_display};
use behavior_core::wire::Loc;

use crate::checks::{CheckResult, Outcome, Subject, loc_json};
use crate::encode::{Binding, Encoder, Env, ErrKind, Nice, Obligation, Term, and_all, not};
use crate::hashing::{TAG_VERIFICATION, check_key, document_hash, finding_hash};
use crate::solver::{Query, Solver, SolverAnswer};
use crate::{Attestation, CheckKind, Profile, VERIFIER_VERSION};

struct Ctx<'a> {
    migration: &'a Migration,
    source: &'a Module,
    target: &'a Module,
    solver: &'a dyn Solver,
    rlimit: u64,
    profile_hash: String,
    solver_version: String,
}

/// One decided property with the requirements its proof relies on.
struct Decided {
    result: CheckResult,
    under: Vec<String>,
}

/// What evaluation must show for a counterexample to count.
enum Expect {
    /// The transform of `field` fails at runtime.
    TransformError { field: String },
    /// The target value breaks the named rule.
    Breaks { rule: String },
}

fn noerr(obs: &[Obligation]) -> String {
    and_all(
        &obs.iter()
            .map(|o| not(&and_all(&[o.guard.clone(), o.cond.clone()])))
            .collect::<Vec<_>>(),
    )
}

/// The encoded universe of one migrated entity type: the encoder, the environment (`old` and
/// the constants), the assumptions with and without the requirements, and the target value.
struct Local<'m> {
    enc: Encoder<'m>,
    env: Env,
    base: Vec<String>,
    requirements: Vec<String>,
    fields: BTreeMap<String, Term>,
    obligations: Vec<(String, Obligation)>,
}

fn setup<'m>(
    ctx: &Ctx<'m>,
    entity: &str,
    transform: &[(String, FieldSource)],
) -> Result<Local<'m>, String> {
    let err = |e: crate::encode::EncodeError| e.to_string();
    let mut enc = Encoder::new(ctx.source);
    let old = Param::new(OLD, ParamRole::State, Type::Entity(entity.to_string()));
    let b = enc.declare_param("state", &old).map_err(err)?;
    let mut env: Env = BTreeMap::from([(OLD.to_string(), b.clone())]);
    for c in ctx.migration.constants() {
        let term = Encoder::lit(c.ty(), c.value()).map_err(err)?;
        env.insert(c.name().to_string(), Binding::Scalar(term));
    }
    enc.world.bound = vec![(OLD.to_string(), entity.to_string())];
    enc.world.env_s = env.clone();
    let Binding::Entity {
        fields: old_fields, ..
    } = &b
    else {
        return Err("the old entity is not an entity".into());
    };
    // References of a valid source entity point at existing entities.
    if let Some(item) = ctx.source.entity(entity) {
        for (f, target) in item.reference_fields() {
            match old_fields.get(f) {
                Some(Term::Plain(v)) => {
                    let ex = enc.fact_free_exists(target, v);
                    enc.assert_axiom(ex);
                }
                Some(Term::Opt { some, val }) => {
                    let ex = enc.fact_free_exists(target, val);
                    enc.assert_axiom(format!("(=> {some} {ex})"));
                }
                None => {}
            }
        }
    }
    // Valid_A: the source rules of `T` on `old`, and the source module invariants.
    let mut base = Vec::new();
    let rules: Vec<(&str, &Expr)> = ctx
        .source
        .constraints_for(entity)
        .filter(|(_, c)| c.reference().is_none())
        .map(|(_, c)| (c.param(), c.body()))
        .chain(
            ctx.source
                .invariants_for(entity)
                .map(|(_, i)| (i.param(), i.body())),
        )
        .collect();
    for (param, body) in rules {
        let mut e = env.clone();
        e.insert(param.to_string(), b.clone());
        if let Ok(r) = enc.encode(body, &e)
            && let Term::Plain(term) = &r.term
        {
            base.push(and_all(&[term.clone(), noerr(&r.obligations)]));
        }
    }
    for g in ctx.source.global_invariants().values() {
        if let Ok(r) = enc.encode(g.body(), &env)
            && let Term::Plain(term) = &r.term
        {
            base.push(and_all(&[term.clone(), noerr(&r.obligations)]));
        }
    }
    // Requirements_M: applicability conditions the runtime establishes before transforming.
    let mut requirements = Vec::new();
    for r in ctx.migration.requirements() {
        let enc_r = enc.encode(r.body(), &env).map_err(err)?;
        let Term::Plain(term) = &enc_r.term else {
            return Err(format!("requirement `{}` is not a Bool", r.name()));
        };
        requirements.push(and_all(&[term.clone(), noerr(&enc_r.obligations)]));
    }
    // The transformed value.
    let mut fields = BTreeMap::new();
    let mut obligations = Vec::new();
    for (name, src) in transform {
        match src {
            FieldSource::Copy(from) => {
                let term = old_fields
                    .get(from)
                    .cloned()
                    .ok_or_else(|| format!("`{entity}.{from}` is not encoded"))?;
                fields.insert(name.clone(), term);
            }
            FieldSource::Expr(e) => {
                let r = enc.encode(e, &env).map_err(err)?;
                for o in r.obligations {
                    obligations.push((name.clone(), o));
                }
                fields.insert(name.clone(), r.term);
            }
        }
    }
    Ok(Local {
        enc,
        env,
        base,
        requirements,
        fields,
        obligations,
    })
}

enum Answer {
    Unsat,
    Unknown(String),
    Models(Vec<BTreeMap<String, crate::smt::SmtValue>>),
}

fn ask(ctx: &Ctx<'_>, enc: &Encoder<'_>, assertions: &[String]) -> Answer {
    let get = enc.input_symbols();
    let query = |nice, witness| Query {
        script: enc.script_with(assertions, nice, witness),
        get: get.clone(),
        rlimit: ctx.rlimit,
    };
    match ctx.solver.check(&query(Nice::Raw, false)) {
        SolverAnswer::Unsat => Answer::Unsat,
        SolverAnswer::Unknown(r) => Answer::Unknown(r.as_str().to_string()),
        SolverAnswer::Sat(raw) => {
            let witness = enc.has_slots();
            let mut models = Vec::new();
            for level in [Nice::Full, Nice::Scale] {
                if let SolverAnswer::Sat(m) = ctx.solver.check(&query(level, witness)) {
                    models.push(m);
                    break;
                }
            }
            if !witness {
                models.push(raw);
            } else if let SolverAnswer::Sat(m) = ctx.solver.check(&query(Nice::Raw, true)) {
                models.push(m);
            }
            Answer::Models(models)
        }
    }
}

/// The old entity of a model, as a source value.
fn old_value(enc: &Encoder<'_>, model: &BTreeMap<String, crate::smt::SmtValue>) -> Option<Json> {
    let [state, _, _] = crate::confirm::request_sections(enc, model)?;
    state.get(OLD).cloned()
}

struct Property<'a> {
    kind: CheckKind,
    check: &'static str,
    subject: Subject,
    explanation: String,
    expect: Expect,
    /// The violation, besides the assumptions.
    violation: Vec<String>,
    entity: &'a str,
}

fn key(ctx: &Ctx<'_>, check: &str, subject: &str) -> String {
    check_key(
        check,
        &[ctx.migration.hash(), subject.to_string()],
        &ctx.profile_hash,
        VERIFIER_VERSION,
        &ctx.solver_version,
    )
}

fn cites(ctx: &Ctx<'_>, subject: &Subject) -> Vec<(&'static str, String)> {
    vec![
        ("migration", ctx.migration.hash()),
        ("subject", subject.hash.clone()),
    ]
}

fn cites_json(cites: &[(&'static str, String)]) -> Json {
    Json::Object(
        cites
            .iter()
            .map(|(r, h)| (r.to_string(), json!(h)))
            .collect(),
    )
}

fn inconclusive(ctx: &Ctx<'_>, check: &str, subject: &Subject, reason: &str) -> Json {
    let mut c = vec![("check", check.to_string())];
    c.extend(cites(ctx, subject));
    json!({
        "hash": finding_hash("inconclusive", &c),
        "kind": "inconclusive",
        "check": check,
        "severity": "blocking",
        "cites": cites_json(&c[1..]),
        "locs": [loc_json(&subject.loc)],
        "explanation": format!("{check} of {} is inconclusive ({reason})", subject.name),
    })
}

fn decide(ctx: &Ctx<'_>, local: &Local<'_>, p: Property<'_>) -> Decided {
    let mut with_requirements = local.base.clone();
    with_requirements.extend(local.requirements.iter().cloned());
    with_requirements.extend(p.violation.iter().cloned());
    let k = key(ctx, p.check, &p.subject.hash);
    let mut under = Vec::new();
    let (outcome, finding) = match ask(ctx, &local.enc, &with_requirements) {
        Answer::Unsat => {
            if !local.requirements.is_empty() {
                let mut without = local.base.clone();
                without.extend(p.violation.iter().cloned());
                if !matches!(ask(ctx, &local.enc, &without), Answer::Unsat) {
                    under = ctx
                        .migration
                        .requirements()
                        .iter()
                        .map(|r| r.name().to_string())
                        .collect();
                }
            }
            (Outcome::Proven, None)
        }
        Answer::Unknown(r) => {
            let f = inconclusive(ctx, p.check, &p.subject, &r);
            (Outcome::Inconclusive(r), Some(f))
        }
        Answer::Models(models) => {
            let confirmed = models.iter().find_map(|m| {
                let value = old_value(&local.enc, m)?;
                let refusal =
                    transform_one(ctx.migration, ctx.source, ctx.target, p.entity, &value).err()?;
                let ok = match &p.expect {
                    Expect::TransformError { field } => {
                        refusal.code == "MIGRATION_TRANSFORM_ERROR"
                            && refusal.message.contains(&format!(".{field}:"))
                    }
                    Expect::Breaks { rule } => {
                        refusal.code == "MIGRATION_INVALID_RESULT"
                            && refusal.rule.as_deref() == Some(rule)
                    }
                };
                ok.then_some((value, refusal))
            });
            match confirmed {
                Some((value, refusal)) => {
                    let c = cites(ctx, &p.subject);
                    let f = json!({
                        "hash": finding_hash(p.check, &c),
                        "kind": p.check,
                        "severity": "blocking",
                        "cites": cites_json(&c),
                        "locs": [loc_json(&p.subject.loc)],
                        "explanation": p.explanation,
                        "counterexample": {
                            "entity": p.entity,
                            "value": value,
                            "refusal": {"code": refusal.code, "message": refusal.message,
                                        "rule": refusal.rule},
                        },
                    });
                    (Outcome::Counterexample, Some(f))
                }
                None => {
                    let r = "counterexample_not_reproduced";
                    let f = inconclusive(ctx, p.check, &p.subject, r);
                    (Outcome::Inconclusive(r.into()), Some(f))
                }
            }
        }
    };
    Decided {
        result: CheckResult {
            kind: p.kind,
            check: p.check,
            action: None,
            subject: p.subject,
            key: k,
            outcome,
            cached: false,
            finding,
        },
        under,
    }
}

/// A check that could not be encoded: inconclusive, never assumed.
fn unencodable(
    ctx: &Ctx<'_>,
    kind: CheckKind,
    check: &'static str,
    subject: Subject,
    reason: &str,
) -> Decided {
    let finding = inconclusive(ctx, check, &subject, reason);
    Decided {
        result: CheckResult {
            kind,
            check,
            action: None,
            key: key(ctx, check, &subject.hash),
            subject,
            outcome: Outcome::Inconclusive(reason.to_string()),
            cached: false,
            finding: Some(finding),
        },
        under: Vec::new(),
    }
}

/// The local checks of one entity type. For a type the migration leaves unchanged (`only_new`),
/// only the target rules the source module does not state identically are checked: the value is
/// carried over, so nothing else can change.
fn local_checks(
    ctx: &Ctx<'_>,
    entity: &str,
    transform: &[(String, FieldSource)],
    kinds: &[CheckKind],
    only_new: bool,
) -> Vec<Decided> {
    let mut out = Vec::new();
    let transform_loc = Loc {
        file: "<migration>".into(),
        line: 1,
    };
    let mut local = match setup(ctx, entity, transform) {
        Ok(l) => l,
        Err(e) => {
            let subject = Subject {
                kind: "transform".into(),
                name: entity.to_string(),
                hash: ctx.migration.hash(),
                loc: transform_loc,
                param: None,
            };
            out.push(unencodable(
                ctx,
                CheckKind::Preservation,
                "migration_constraint",
                subject,
                &format!("unsupported: {e}"),
            ));
            return out;
        }
    };
    // Narrowing sites and other evaluation errors of the transforms.
    if kinds.contains(&CheckKind::EvaluationError) {
        let obligations = local.obligations.clone();
        for (j, (field, o)) in obligations.iter().enumerate() {
            let mut violation: Vec<String> = obligations[..j]
                .iter()
                .filter(|(f, _)| f == field)
                .map(|(_, e)| not(&and_all(&[e.guard.clone(), e.cond.clone()])))
                .collect();
            violation.push(and_all(&[o.guard.clone(), o.cond.clone()]));
            let check = if o.kind == ErrKind::Narrowing {
                "migration_narrowing"
            } else {
                "evaluation_error"
            };
            let subject = Subject {
                kind: "transform".into(),
                name: format!("{entity}.{field}"),
                hash: hash_display(&o.hash),
                loc: o.loc.clone(),
                param: None,
            };
            let explanation = format!(
                "migrating a {entity} can fail at {}: {} in {}",
                field,
                o.kind.message(),
                o.text
            );
            out.push(decide(
                ctx,
                &local,
                Property {
                    kind: CheckKind::EvaluationError,
                    check,
                    subject,
                    explanation,
                    expect: Expect::TransformError {
                        field: field.clone(),
                    },
                    violation,
                    entity,
                },
            ));
        }
    }
    // Target constraints and invariants of `T` on the transformed value.
    if kinds.contains(&CheckKind::Preservation) {
        let target_value = Binding::Entity {
            entity: entity.to_string(),
            fields: local.fields.clone(),
        };
        let transforms_ok = noerr(
            &local
                .obligations
                .iter()
                .map(|(_, o)| o.clone())
                .collect::<Vec<_>>(),
        );
        let rules: Vec<(String, &str, &str, &Expr, String, Loc)> = ctx
            .target
            .constraints_for(entity)
            .filter(|(_, c)| c.reference().is_none())
            .map(|(n, c)| {
                (
                    n.clone(),
                    "constraint",
                    c.param(),
                    c.body(),
                    hash_display(c.hash()),
                    c.loc().clone(),
                )
            })
            .chain(ctx.target.invariants_for(entity).map(|(n, i)| {
                (
                    n.clone(),
                    "invariant",
                    i.param(),
                    i.body(),
                    hash_display(i.hash()),
                    i.loc().clone(),
                )
            }))
            .filter(|(.., hash, _)| !(only_new && source_rules(ctx.source, entity).contains(hash)))
            .collect();
        local.enc.module = ctx.target;
        for (name, kind, param, body, hash, loc) in rules {
            let subject = Subject {
                kind: kind.into(),
                name: format!("{entity}.{name}"),
                hash,
                loc,
                param: None,
            };
            let mut env = local.env.clone();
            env.insert(param.to_string(), target_value.clone());
            let encoded = match local.enc.encode(body, &env) {
                Ok(r) => r,
                Err(e) => {
                    out.push(unencodable(
                        ctx,
                        CheckKind::Preservation,
                        "migration_constraint",
                        subject,
                        &format!("unsupported: {e}"),
                    ));
                    continue;
                }
            };
            let Term::Plain(term) = &encoded.term else {
                continue;
            };
            let holds = and_all(&[term.clone(), noerr(&encoded.obligations)]);
            out.push(decide(
                ctx,
                &local,
                Property {
                    kind: CheckKind::Preservation,
                    check: "migration_constraint",
                    explanation: format!(
                        "a migrated {entity} can break the target {kind} `{name}`"
                    ),
                    subject,
                    expect: Expect::Breaks { rule: name.clone() },
                    violation: vec![transforms_ok.clone(), not(&holds)],
                    entity,
                },
            ));
        }
        local.enc.module = ctx.source;
    }
    out
}

/// The hashes of the source module's constraints and invariants of `entity`.
fn source_rules(source: &Module, entity: &str) -> BTreeSet<String> {
    source
        .constraints_for(entity)
        .map(|(_, c)| hash_display(c.hash()))
        .chain(
            source
                .invariants_for(entity)
                .map(|(_, i)| hash_display(i.hash())),
        )
        .collect()
}

/// Whether a transform field carries a source reference of `old` to `target_entity` over
/// unchanged (or is absent).
fn carries_reference(e: &FieldSource, source: &Module, entity: &str, target_entity: &str) -> bool {
    let is_ref = |field: &str| {
        source.entity(entity).is_some_and(|item| {
            item.reference_fields()
                .any(|(f, t)| f == field && t == target_entity)
        })
    };
    match e {
        FieldSource::Copy(from) => is_ref(from),
        FieldSource::Expr(x) => {
            fn walk(x: &Expr, is_ref: &dyn Fn(&str) -> bool) -> bool {
                match x.kind() {
                    ExprKind::Field { param, field } if param == OLD => is_ref(field),
                    ExprKind::Some(inner) => walk(inner, is_ref),
                    ExprKind::Lit(behavior_core::semantic::value::Value::None) => true,
                    _ => false,
                }
            }
            walk(x, &is_ref)
        }
    }
}

fn referential_integrity(ctx: &Ctx<'_>) -> Vec<Decided> {
    let mut out = Vec::new();
    for (entity, t) in ctx.migration.transforms() {
        let Some(item) = ctx.target.entity(entity) else {
            continue;
        };
        for (field, target_entity) in item.reference_fields() {
            let src = t.fields().iter().find(|(f, _)| f == field).map(|(_, s)| s);
            let subject = Subject {
                kind: "reference".into(),
                name: format!("{entity}.{field}"),
                hash: format!("{entity}.{field}->{target_entity}"),
                loc: Loc {
                    file: "<migration>".into(),
                    line: 1,
                },
                param: None,
            };
            let proven =
                src.is_some_and(|s| carries_reference(s, ctx.source, entity, target_entity));
            if proven {
                out.push(Decided {
                    result: CheckResult {
                        kind: CheckKind::Preservation,
                        check: "referential_integrity",
                        action: None,
                        key: key(ctx, "referential_integrity", &subject.hash),
                        subject,
                        outcome: Outcome::Proven,
                        cached: false,
                        finding: None,
                    },
                    under: Vec::new(),
                });
            } else {
                out.push(unencodable(
                    ctx,
                    CheckKind::Preservation,
                    "referential_integrity",
                    subject,
                    "the reference is not carried over from a source reference",
                ));
            }
        }
    }
    out
}

fn module_invariants(ctx: &Ctx<'_>) -> Vec<Decided> {
    let mut out = Vec::new();
    let source_bodies: BTreeSet<String> = ctx
        .source
        .global_invariants()
        .values()
        .map(|g| hash_display(g.body().hash()))
        .collect();
    for (name, g) in ctx.target.global_invariants() {
        let subject = Subject {
            kind: "module_invariant".into(),
            name: name.clone(),
            hash: hash_display(g.hash()),
            loc: g.loc().clone(),
            param: None,
        };
        // Every read field of every migrated type it queries is copied under its own name.
        let copies = g.signature().types.iter().all(|(entity, fields)| {
            let Some(t) = ctx.migration.transforms().get(entity) else {
                return true;
            };
            let copied = |f: &str| {
                t.fields()
                    .iter()
                    .any(|(n, s)| n == f && matches!(s, FieldSource::Copy(from) if from == f))
            };
            match fields {
                Some(fs) => fs.iter().all(|f| copied(f)),
                None => t.fields().iter().all(|(n, _)| copied(n)),
            }
        });
        if copies && source_bodies.contains(&hash_display(g.body().hash())) {
            out.push(Decided {
                result: CheckResult {
                    kind: CheckKind::Preservation,
                    check: "module_invariant",
                    action: None,
                    key: key(ctx, "module_invariant", &subject.hash),
                    subject,
                    outcome: Outcome::Proven,
                    cached: false,
                    finding: None,
                },
                under: Vec::new(),
            });
        } else {
            out.push(unencodable(
                ctx,
                CheckKind::Preservation,
                "module_invariant",
                subject,
                "precision_debt: a module invariant over migrated values is checked when the \
                 migration is applied",
            ));
        }
    }
    out
}

/// Verifies `migration` from `source` to `target` under `profile`: every selected check is
/// proven, a confirmed counterexample, or inconclusive; the migration is verified only without
/// blocking findings. The attestation's subject is the migration.
pub fn verify_migration(
    migration: &Migration,
    source: &Module,
    target: &Module,
    profile: &Profile,
    cache: Option<&Path>,
    solver: &dyn Solver,
) -> Attestation {
    let _ = cache;
    let ctx = Ctx {
        migration,
        source,
        target,
        solver,
        rlimit: profile.rlimit,
        profile_hash: profile.hash(),
        solver_version: solver.version(),
    };
    let kinds = profile.checks.clone();
    let mut decided = Vec::new();
    for (entity, t) in migration.transforms() {
        decided.extend(local_checks(&ctx, entity, t.fields(), &kinds, false));
    }
    // A type the migration carries over unchanged can still meet a rule the target adds.
    if kinds.contains(&CheckKind::Preservation) {
        for (entity, item) in target.entities() {
            if migration.transforms().contains_key(entity) || source.entity(entity).is_none() {
                continue;
            }
            let known = source_rules(source, entity);
            let new_rules = target
                .constraints_for(entity)
                .filter(|(_, c)| c.reference().is_none())
                .map(|(_, c)| hash_display(c.hash()))
                .chain(
                    target
                        .invariants_for(entity)
                        .map(|(_, i)| hash_display(i.hash())),
                )
                .any(|h| !known.contains(&h));
            if new_rules {
                let copies: Vec<(String, FieldSource)> = item
                    .fields()
                    .iter()
                    .map(|(f, _)| (f.clone(), FieldSource::Copy(f.clone())))
                    .collect();
                decided.extend(local_checks(&ctx, entity, &copies, &kinds, true));
            }
        }
    }
    if kinds.contains(&CheckKind::Preservation) {
        decided.extend(referential_integrity(&ctx));
        decided.extend(module_invariants(&ctx));
    }
    decided.sort_by(|a, b| {
        (
            a.result.check,
            &a.result.subject.name,
            &a.result.subject.hash,
        )
            .cmp(&(
                b.result.check,
                &b.result.subject.name,
                &b.result.subject.hash,
            ))
    });
    let mut findings: Vec<Json> = decided
        .iter()
        .filter_map(|d| d.result.finding.clone())
        .collect();
    findings.sort_by(|a, b| a["hash"].as_str().cmp(&b["hash"].as_str()));
    findings.dedup_by(|a, b| a["hash"] == b["hash"]);
    let blocking = findings.iter().any(|f| f["severity"] == "blocking");
    let result = if blocking { "not_verified" } else { "verified" };
    let checks_json = |with_cached: bool| {
        decided
            .iter()
            .map(|d| {
                let mut c = d.result.to_json(with_cached);
                if let Json::Object(m) = &mut c {
                    m.insert("under".into(), json!(d.under));
                }
                c
            })
            .collect::<Vec<_>>()
    };
    let doc = |with_cached: bool| {
        json!({
            "attestation_version": "1",
            "subject": "migration",
            "migration_hash": migration.hash(),
            "source": migration.source_schema(),
            "target": migration.target_schema(),
            "profile": profile.to_json(),
            "verifier_version": VERIFIER_VERSION,
            "solver_version": ctx.solver_version,
            "result": result,
            "checks": checks_json(with_cached),
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
        checks: decided.into_iter().map(|d| d.result).collect(),
    }
}
