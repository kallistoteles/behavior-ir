//! Verification of declared reads (feature 010, FR-017, research R10): can a read fail with an
//! evaluation error on a valid state? A value read is checked like an action's expressions; a
//! projection's items are checked for one symbolic member that satisfies the query's
//! (candidate-local) filter, or for the bound entity. Counterexamples are confirmed by evaluating
//! the read.

use serde_json::{Value as Json, json};

use behavior_core::facts::Facts;
use behavior_core::read::{ReadSource, evaluate_read};
use behavior_core::semantic::expr::{CANDIDATE, QueryKind, QueryNode};
use behavior_core::semantic::module::{
    Module, Over, Param, ParamRole, Projection, ReadBody, ReadItem,
};
use behavior_core::semantic::types::{Hash, Type, hash_display};
use behavior_core::wire::Loc;

use crate::CheckKind;
use crate::checks::{CheckResult, Ctx, Outcome, Search, Subject, loc_json, search};
use crate::confirm::request_parts;
use crate::encode::{Binding, Encoder, Env, ErrKind, Obligation, Term, and_all, not};
use crate::hashing::{check_key, finding_hash};
use crate::smt::SmtValue;

type R<T> = Result<T, crate::encode::EncodeError>;

/// One step of a read's evaluation: the filter of a projection, an item, or the value.
struct Step {
    hash: Hash,
    obligations: Vec<Obligation>,
}

/// A read encoded over valid states: its assumptions (validity of every entity it binds or
/// projects, module invariants, the projection's filter) and its steps in evaluation order.
struct ReadEncoding<'m> {
    enc: Encoder<'m>,
    assumptions: Vec<String>,
    steps: Vec<Step>,
    /// The symbolic member of a query projection: its parameter name and entity type.
    member: Option<(String, String)>,
}

fn section(role: ParamRole) -> &'static str {
    match role {
        ParamRole::State | ParamRole::Read => "state",
        ParamRole::Input => "input",
        ParamRole::Context => "context",
    }
}

/// No obligation of `obligations` fires.
fn no_error(obligations: &[Obligation]) -> String {
    and_all(
        &obligations
            .iter()
            .map(|o| not(&and_all(&[o.guard.clone(), o.cond.clone()])))
            .collect::<Vec<_>>(),
    )
}

/// The validity the runtime guarantees for an entity bound to `binding`: its constraints, its
/// invariants, and the existence of what its references point at.
fn valid_entity(
    enc: &mut Encoder<'_>,
    module: &Module,
    entity: &str,
    binding: &Binding,
    out: &mut Vec<String>,
) -> R<()> {
    let rules = module
        .constraints_for(entity)
        .map(|(_, c)| (c.param(), c.body()))
        .chain(
            module
                .invariants_for(entity)
                .map(|(_, i)| (i.param(), i.body())),
        );
    for (param, body) in rules.collect::<Vec<_>>() {
        let mut inner = Env::new();
        inner.insert(param.to_string(), binding.clone());
        let r = enc.encode(body, &inner)?;
        out.push(r.term.plain()?.to_string());
        out.push(no_error(&r.obligations));
    }
    if let (Some(item), Binding::Entity { fields, .. }) = (module.entity(entity), binding) {
        for (f, target) in item.reference_fields() {
            match fields.get(f) {
                Some(Term::Plain(v)) => out.push(enc.fact_free_exists(target, v)),
                Some(Term::Opt { some, val }) => {
                    let ex = enc.fact_free_exists(target, val);
                    out.push(format!("(=> {some} {ex})"));
                }
                None => {}
            }
        }
    }
    Ok(())
}

/// The membership predicate of a candidate-local query for the member bound in `env` under
/// [`CANDIDATE`], and the obligations of evaluating it.
fn predicate(enc: &mut Encoder<'_>, q: &QueryNode, env: &Env) -> R<(String, Vec<Obligation>)> {
    Ok(match q.kind() {
        QueryKind::Select => ("true".to_string(), Vec::new()),
        QueryKind::Where { base, body, .. } => {
            let (b, mut obligations) = predicate(enc, base, env)?;
            let r = enc.encode(body, env)?;
            obligations.extend(r.obligations);
            (and_all(&[b, r.term.plain()?.to_string()]), obligations)
        }
        QueryKind::Set { op, a, b } => {
            let (pa, mut oa) = predicate(enc, a, env)?;
            let (pb, ob) = predicate(enc, b, env)?;
            oa.extend(ob);
            let p = match op {
                behavior_core::semantic::expr::SetOp::Union => format!("(or {pa} {pb})"),
                behavior_core::semantic::expr::SetOp::Intersection => and_all(&[pa, pb]),
                behavior_core::semantic::expr::SetOp::Difference => and_all(&[pa, not(&pb)]),
            };
            (p, oa)
        }
    })
}

/// A parameter name for the projected member that no read parameter uses.
fn member_name(read: &ReadItem, p: &Projection) -> String {
    let mut name = p.member().to_string();
    while read.params().iter().any(|x| x.name() == name) {
        name.push('_');
    }
    name
}

impl<'m> ReadEncoding<'m> {
    fn build(module: &'m Module, read: &ReadItem) -> R<Self> {
        let mut enc = Encoder::new(module);
        let mut env = Env::new();
        let mut assumptions = Vec::new();
        for p in read.params() {
            let b = enc.declare_param(section(p.role()), p)?;
            env.insert(p.name().to_string(), b);
        }
        enc.world.bound = read
            .params()
            .iter()
            .filter(|p| p.role() == ParamRole::State)
            .filter_map(|p| match p.ty() {
                Type::Entity(e) => Some((p.name().to_string(), e.clone())),
                _ => None,
            })
            .collect();
        for p in read.params() {
            if let (Type::Entity(entity), Some(b)) = (p.ty(), env.get(p.name()).cloned()) {
                valid_entity(&mut enc, module, entity, &b, &mut assumptions)?;
            }
        }
        let mut steps = Vec::new();
        let mut member = None;
        if let ReadBody::Project(p) = read.body()
            && let Over::Query(q) = p.over()
        {
            // One symbolic member of the query: valid, and a member by the filter.
            let name = member_name(read, p);
            let param = Param::new(&name, ParamRole::State, Type::Entity(p.entity().into()));
            let b = enc.declare_param("state", &param)?;
            valid_entity(&mut enc, module, p.entity(), &b, &mut assumptions)?;
            env.insert(name.clone(), b.clone());
            enc.world.bound.push((name.clone(), p.entity().to_string()));
            let mut cand = env.clone();
            cand.insert(CANDIDATE.to_string(), b);
            let (pred, obligations) = predicate(&mut enc, q, &cand)?;
            steps.push(Step {
                hash: *q.hash(),
                obligations,
            });
            assumptions.push(pred);
            member = Some((name, p.entity().to_string()));
        }
        enc.world.env_s = env.clone();
        // The runtime assumes every module invariant on the state it reads.
        for g in module.global_invariants().values() {
            let r = enc.encode(g.body(), &Env::new())?;
            assumptions.push(r.term.plain()?.to_string());
        }
        match read.body() {
            ReadBody::Value(body) => {
                let r = enc.encode(body, &env)?;
                steps.push(Step {
                    hash: *body.hash(),
                    obligations: r.obligations,
                });
            }
            ReadBody::Project(p) => {
                let param = match (p.over(), &member) {
                    (Over::Param { name, .. }, _) => name.clone(),
                    (Over::Query(_), Some((m, _))) => m.clone(),
                    (Over::Query(_), None) => {
                        return Err(crate::encode::EncodeError::Unsupported(
                            "projection member".into(),
                        ));
                    }
                };
                for item in p.items() {
                    let e = p.item_expr(module, item, &param).ok_or_else(|| {
                        crate::encode::EncodeError::Unsupported(format!("item {}", item.name()))
                    })?;
                    let r = enc.encode(&e, &env)?;
                    steps.push(Step {
                        hash: *e.hash(),
                        obligations: r.obligations,
                    });
                }
            }
        }
        Ok(ReadEncoding {
            enc,
            assumptions,
            steps,
            member,
        })
    }
}

/// A confirmed counterexample of a read: the request it was evaluated with and its record.
struct Confirmed {
    state: Json,
    input: Json,
    context: Json,
    facts: Option<Json>,
    record: Json,
}

/// Evaluates the model as a read; `Some` only if the read fails with `message` (for a
/// projection, `<T>#<id>.<item>: <message>`).
fn confirm(
    module: &Module,
    re: &ReadEncoding<'_>,
    read: &str,
    model: &std::collections::BTreeMap<String, SmtValue>,
    message: &str,
) -> Option<Confirmed> {
    let ([mut state, input, context], facts) = request_parts(&re.enc, model)?;
    let mut facts = facts;
    if let Some((m, entity)) = &re.member {
        // The projected member is an entity of the state, not a parameter of the read.
        let value = state.as_object_mut()?.remove(m)?;
        let id = value.get("id")?.as_str()?.to_string();
        let mut f = match &facts {
            Some(j) => Facts::from_json(j).ok()?,
            None => Facts::default(),
        };
        let members = f.universe.entry(entity.clone()).or_default();
        members.insert(id, value);
        // The read's bound entities of the same type exist too: a universe is complete.
        for p in module.read(read)?.params() {
            if p.role() == ParamRole::State
                && p.ty() == &Type::Entity(entity.clone())
                && let Some(bound) = state.get(p.name())
                && let Some(bid) = bound.get("id").and_then(Json::as_str)
            {
                members.insert(bid.to_string(), bound.clone());
            }
        }
        facts = Some(f.to_json());
    }
    let mut request = json!({
        "data_version": "verification",
        "state": state,
        "input": input,
        "context": context,
    });
    if let (Some(f), Json::Object(m)) = (&facts, &mut request) {
        m.insert("facts".into(), f.clone());
    }
    let x = evaluate_read(
        module,
        &ReadSource::Declared(read.to_string()),
        &request.to_string(),
    );
    let record = x.record.as_json().clone();
    let got = record["reasons"][0]["message"].as_str().unwrap_or_default();
    let reproduced = record["result"] == "EVALUATION_ERROR"
        && (got == message || got.ends_with(&format!(": {message}")));
    reproduced.then_some(Confirmed {
        state,
        input,
        context,
        facts,
        record,
    })
}

/// Evaluation errors of the declared read `name` (FR-017): one check per obligation of every
/// step, on the path that reaches it (earlier steps and earlier obligations did not fail).
pub fn evaluation_errors(ctx: &Ctx<'_>, name: &str) -> Vec<CheckResult> {
    let module = ctx.module;
    let Some(read) = module.read(name) else {
        return Vec::new();
    };
    let read_hash = hash_display(read.hash());
    let label = format!("read:{name}");
    let re = match ReadEncoding::build(module, read) {
        Ok(re) => re,
        Err(e) => return vec![encoding_failure(name, &label, &read_hash, read.loc(), &e)],
    };
    let mut out = Vec::new();
    for (k, step) in re.steps.iter().enumerate() {
        for (j, o) in step.obligations.iter().enumerate() {
            let mut assertions = re.assumptions.clone();
            for earlier in &re.steps[..k] {
                assertions.push(no_error(&earlier.obligations));
            }
            assertions.push(no_error(&step.obligations[..j]));
            assertions.push(and_all(&[o.guard.clone(), o.cond.clone()]));
            let error = match o.kind {
                ErrKind::DivisionByZero => "division_by_zero",
                ErrKind::Overflow => "overflow",
                ErrKind::Narrowing => "narrowing",
            };
            let message = format!("{} in {}", o.kind.message(), o.text);
            let hash = hash_display(&o.hash);
            let step_hash = hash_display(&step.hash);
            let key = check_key(
                CheckKind::EvaluationError.as_str(),
                &[
                    read_hash.clone(),
                    hash.clone(),
                    module.behavior_version(),
                    step_hash.clone(),
                    format!("error:{error}"),
                    format!("obligation:{k}:{j}"),
                ],
                &ctx.profile_hash,
                crate::VERIFIER_VERSION,
                &ctx.solver_version,
            );
            if let Some(hit) = ctx.cache.as_ref().and_then(|c| c.get(&key)) {
                out.push(hit);
                continue;
            }
            let cites: Vec<(&'static str, String)> = vec![
                ("read", read_hash.clone()),
                ("expression", hash.clone()),
                ("step", step_hash),
                ("error", error.to_string()),
            ];
            let locs = [read.loc().clone(), o.loc.clone()];
            let (outcome, finding) = match search(ctx, &re.enc, &assertions) {
                Search::Unsat => (Outcome::Proven, None),
                Search::Unknown(r) => {
                    let reason = r.as_str().to_string();
                    let f = inconclusive(&cites, &locs, &label, &o.text, &reason);
                    (Outcome::Inconclusive(reason), Some(f))
                }
                Search::Models(models) => {
                    match models
                        .iter()
                        .find_map(|m| confirm(module, &re, name, m, &message))
                    {
                        Some(c) => {
                            let mut cx = json!({"state": c.state, "input": c.input,
                                                "context": c.context, "record": c.record});
                            if let (Some(f), Json::Object(m)) = (&c.facts, &mut cx) {
                                m.insert("facts".into(), f.clone());
                            }
                            let f = json!({
                                "hash": finding_hash(CheckKind::EvaluationError.as_str(), &cites),
                                "kind": CheckKind::EvaluationError.as_str(),
                                "severity": "blocking",
                                "cites": cites_json(&cites),
                                "locs": locs.iter().map(loc_json).collect::<Vec<_>>(),
                                "explanation": format!("{label} can fail with {message}"),
                                "counterexample": cx,
                            });
                            (Outcome::Counterexample, Some(f))
                        }
                        None => {
                            let reason = "counterexample_not_reproduced".to_string();
                            let f = inconclusive(&cites, &locs, &label, &o.text, &reason);
                            (Outcome::Inconclusive(reason), Some(f))
                        }
                    }
                }
            };
            let result = CheckResult {
                kind: CheckKind::EvaluationError,
                check: CheckKind::EvaluationError.as_str(),
                action: Some((label.clone(), read_hash.clone())),
                subject: Subject {
                    kind: "expression".into(),
                    name: format!("{} ({})", o.text, o.kind.message()),
                    hash,
                    loc: o.loc.clone(),
                    param: None,
                },
                key,
                outcome,
                cached: false,
                finding,
            };
            if let Some(c) = &ctx.cache {
                c.put(&result);
            }
            out.push(result);
        }
    }
    out
}

fn cites_json(cites: &[(&'static str, String)]) -> Json {
    Json::Object(
        cites
            .iter()
            .map(|(role, h)| (role.to_string(), json!(h)))
            .collect(),
    )
}

fn inconclusive(
    cites: &[(&'static str, String)],
    locs: &[Loc],
    label: &str,
    text: &str,
    reason: &str,
) -> Json {
    let mut all = vec![("check", CheckKind::EvaluationError.as_str().to_string())];
    all.extend(cites.iter().cloned());
    json!({
        "hash": finding_hash("inconclusive", &all),
        "kind": "inconclusive",
        "check": CheckKind::EvaluationError.as_str(),
        "severity": "blocking",
        "cites": cites_json(cites),
        "locs": locs.iter().map(loc_json).collect::<Vec<_>>(),
        "explanation": format!("evaluation_error of {text} in {label} is inconclusive ({reason})"),
    })
}

/// A blocking, inconclusive result for a read the verifier cannot encode (a verifier gap,
/// reported loudly).
fn encoding_failure(
    name: &str,
    label: &str,
    read_hash: &str,
    loc: &Loc,
    e: &crate::encode::EncodeError,
) -> CheckResult {
    let cites = [
        ("check", CheckKind::EvaluationError.as_str().to_string()),
        ("read", read_hash.to_string()),
    ];
    let finding = json!({
        "hash": finding_hash("inconclusive", &cites),
        "kind": "inconclusive",
        "check": CheckKind::EvaluationError.as_str(),
        "severity": "blocking",
        "cites": {"read": read_hash},
        "locs": [loc_json(loc)],
        "explanation": format!("evaluation_error of {label} is inconclusive (encoding_error: {e})"),
    });
    CheckResult {
        kind: CheckKind::EvaluationError,
        check: CheckKind::EvaluationError.as_str(),
        action: Some((label.to_string(), read_hash.to_string())),
        subject: Subject {
            kind: "read".into(),
            name: name.to_string(),
            hash: read_hash.to_string(),
            loc: loc.clone(),
            param: None,
        },
        key: String::new(),
        outcome: Outcome::Inconclusive("encoding_error".into()),
        cached: false,
        finding: Some(finding),
    }
}
