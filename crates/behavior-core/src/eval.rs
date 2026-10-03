//! Deterministic evaluation of a transition `T : S × I × C → Result<ΔS, O, Trace>`
//! (research R6, R7; contracts/engine-api.md).
//!
//! Order: input check → invariants on S → preconditions → effects (ΔS against S) →
//! `S' = apply(S, ΔS)` → postconditions and invariants on S'. Any failure returns no change.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json, json};

use crate::decimal::{Dec, NumError, fixed_text};
use crate::exact::Exact;
use crate::facts::{BoundEntity, EvaluationFacts, FactError, Facts, RefEdge, check_snapshot};
use crate::pretty;
use crate::record::DecisionRecord;
use crate::semantic::expr::{CANDIDATE, Expr, ExprKind};
use crate::semantic::module::{ActionItem, Module, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Hash, Type, fixed_scale, hash_display};
use crate::semantic::value::{Value, decode_scalar, encode};
use crate::wire::Loc;

/// Version of the decision record format (0.4: exact arithmetic closure, feature 004).
pub const RECORD_VERSION: &str = "0.4";
/// The record format with evaluation facts and lifecycle entries (feature 006); records without
/// either stay `0.4`, byte for byte.
pub const RECORD_VERSION_LIFECYCLE: &str = "0.5";
/// The record format with query and field facts (feature 007).
pub const RECORD_VERSION_QUERIES: &str = "0.6";

/// Checks a decoded fixed-scale value against its grid and range (FR-004).
fn check_fixed(ty: &Type, v: &Value, path: &str, problems: &mut Vec<InputProblem>) -> bool {
    let n = match ty {
        Type::Option(inner) => fixed_scale(inner),
        t => fixed_scale(t),
    };
    let (Some(n), Value::Dec(d)) = (n, v) else {
        return true;
    };
    let scale = n.scale.unwrap_or(0);
    if !d.on_grid(scale) {
        problems.push(InputProblem {
            code: "OFF_GRID",
            path: path.into(),
            message: format!("`{d}` has more than {scale} decimal places (`{}`)", n.name),
        });
        return false;
    }
    if !d.in_fixed_range(scale) {
        problems.push(InputProblem {
            code: "OUT_OF_RANGE",
            path: path.into(),
            message: format!(
                "`{d}` is outside the range of `{}` (at most {} integer digits)",
                n.name,
                28 - scale
            ),
        });
        return false;
    }
    true
}

/// JSON of a parameter value with its type, including the field types of entities.
pub(crate) fn encode_param(module: &Module, ty: &Type, v: &Value) -> Json {
    match (ty, v) {
        (Type::Entity(entity), Value::Entity(fields)) => match module.entity(entity) {
            Some(item) => Json::Object(
                fields
                    .iter()
                    .map(|(k, fv)| {
                        let fty = item.field_type(k).cloned().unwrap_or(Type::Bool);
                        (k.clone(), encode(&fty, fv))
                    })
                    .collect(),
            ),
            None => encode(ty, v),
        },
        _ => encode(ty, v),
    }
}

pub(crate) type Vals = BTreeMap<String, Value>;
type Reads = BTreeMap<String, Json>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    S,
    SPrime,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::S => "S",
            Phase::SPrime => "S'",
        }
    }
}

/// A problem with the request itself (INVALID_INPUT), sorted by `path`.
pub(crate) struct InputProblem {
    pub(crate) code: &'static str,
    pub(crate) path: String,
    pub(crate) message: String,
}

fn section_name(role: ParamRole) -> &'static str {
    match role {
        ParamRole::State | ParamRole::Read => "state",
        ParamRole::Input => "input",
        ParamRole::Context => "context",
    }
}

/// Replaces floating-point numbers with `{"$json_number": "<text>"}` so a record that echoes
/// invalid input stays float-free and replays to the same result.
pub(crate) fn sanitize_floats(v: &Json) -> Json {
    match v {
        Json::Number(n) if n.is_f64() => json!({"$json_number": n.to_string()}),
        Json::Array(items) => Json::Array(items.iter().map(sanitize_floats).collect()),
        Json::Object(map) => Json::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), sanitize_floats(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub(crate) fn decode_param(
    module: &Module,
    ty: &Type,
    raw: &Json,
    path: &str,
    problems: &mut Vec<InputProblem>,
) -> Option<Value> {
    let Type::Entity(entity) = ty else {
        return match decode_scalar(ty, raw) {
            Ok(v) if !check_fixed(ty, &v, path, problems) => None,
            Ok(v) => Some(v),
            Err(msg) => {
                problems.push(InputProblem {
                    code: "WRONG_TYPE",
                    path: path.into(),
                    message: msg,
                });
                None
            }
        };
    };
    let item = module.entity(entity)?;
    let Json::Object(obj) = raw else {
        problems.push(InputProblem {
            code: "WRONG_TYPE",
            path: path.into(),
            message: format!("expected a {entity} object"),
        });
        return None;
    };
    let mut fields = BTreeMap::new();
    let mut ok = true;
    for (name, fty) in item.fields() {
        let fpath = format!("{path}.{name}");
        match obj.get(name) {
            None => {
                problems.push(InputProblem {
                    code: "MISSING_FIELD",
                    path: fpath,
                    message: format!("missing field `{name}`"),
                });
                ok = false;
            }
            Some(v) => match decode_scalar(fty, v) {
                Ok(val) if !check_fixed(fty, &val, &fpath, problems) => {
                    ok = false;
                }
                Ok(val) => {
                    fields.insert(name.clone(), val);
                }
                Err(msg) => {
                    problems.push(InputProblem {
                        code: "WRONG_TYPE",
                        path: fpath,
                        message: msg,
                    });
                    ok = false;
                }
            },
        }
    }
    for key in obj.keys() {
        if item.field_type(key).is_none() {
            problems.push(InputProblem {
                code: "EXTRA_FIELD",
                path: format!("{path}.{key}"),
                message: format!("`{entity}` has no field `{key}`"),
            });
            ok = false;
        }
    }
    ok.then_some(Value::Entity(fields))
}

/// Decodes the three sections of a request against the action's parameters.
fn decode_sections(
    module: &Module,
    action: &ActionItem,
    sections: &BTreeMap<&'static str, Map<String, Json>>,
    problems: &mut Vec<InputProblem>,
) -> Option<Vals> {
    let mut vals = Vals::new();
    for p in action.params() {
        let section = section_name(p.role());
        let path = format!("{section}.{}", p.name());
        match sections.get(section).and_then(|s| s.get(p.name())) {
            None => problems.push(InputProblem {
                code: "MISSING_ARGUMENT",
                path,
                message: format!("missing parameter `{}`", p.name()),
            }),
            Some(raw) => {
                if let Some(v) = decode_param(module, p.ty(), raw, &path, problems) {
                    vals.insert(p.name().to_string(), v);
                }
            }
        }
    }
    for (section, map) in sections {
        for key in map.keys() {
            let declared = action
                .params()
                .iter()
                .any(|p| p.name() == key && section_name(p.role()) == *section);
            if !declared {
                problems.push(InputProblem {
                    code: "EXTRA_ARGUMENT",
                    path: format!("{section}.{key}"),
                    message: format!("`{key}` is not a {section} parameter of this action"),
                });
            }
        }
    }
    problems.is_empty().then_some(vals)
}

struct DerivedEntry {
    name: String,
    hash: Hash,
    phase: Phase,
    order: usize,
    args_key: String,
    value: Json,
}

/// Observed reads of one scope: `(param, field)` in that scope's parameter names, and how its
/// parameters map to the enclosing scope's names (feature 005, research R7).
#[derive(Default)]
struct ReadFrame {
    to_outer: BTreeMap<String, String>,
    reads: BTreeSet<(String, String)>,
}

type ObservedReads = BTreeSet<(String, String)>;

/// What an evaluation observed: the bound entities' fields (feature 005) and the evaluation facts
/// (feature 006: existence, identity and reference reads).
#[derive(Debug, Clone, Default)]
pub struct Observed {
    pub fields: ObservedReads,
    pub facts: Facts,
}

type Key = (String, String);

/// The entity universe as the transition sees it: the state-bound entities, and after the
/// effects, the created and removed identities and the bound entities' S' values.
#[derive(Default)]
struct World {
    /// State parameters by bound identity (they exist in S by binding).
    bound: BTreeMap<Key, String>,
    created: BTreeMap<Key, Value>,
    removed: BTreeSet<Key>,
    s_prime: Vals,
}

/// The entity type of an `Id<T>` or `Option<Id<T>>` expression.
fn id_entity(t: &Type) -> Option<&str> {
    match t {
        Type::Id(e) => Some(e),
        Type::Option(inner) => id_entity(inner),
        _ => None,
    }
}

fn id_text(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.clone()),
        _ => None,
    }
}

struct Evaluator<'a> {
    module: &'a Module,
    facts: &'a dyn EvaluationFacts,
    /// Facts obtained so far (each question is asked at most once).
    observed: Facts,
    /// Set when a fact could not be obtained: the evaluation error is `UNKNOWN_FACT`.
    fact_error: Option<String>,
    world: World,
    /// Derived values by (name, argument values, phase), with the reads their body made, in the
    /// derived value's own parameter names (re-attributed on every hit).
    memo: BTreeMap<(String, String, Phase), (Value, ObservedReads)>,
    /// Observed-read frames; the first is the action's scope.
    frames: Vec<ReadFrame>,
    derived: Vec<DerivedEntry>,
    /// Rescale entries of the step being evaluated (attached to its trace step).
    rescales: Vec<Json>,
    /// The member a lambda body is evaluated for (feature 007): its fields are read lazily.
    candidate: Option<Key>,
    /// While a module invariant is checked on S' (feature 007): the `unique` expressions known
    /// to hold on the valid S (its top-level conjuncts), decided by the delta rule.
    assume_valid: BTreeSet<Hash>,
}

/// The capture reads of a query (feature 007): enclosing field and parameter reads in its lambda
/// bodies, by canonical read name.
fn capture_reads(q: &crate::semantic::expr::QueryNode) -> BTreeMap<String, &Expr> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<&Expr> = q.bodies();
    while let Some(e) = stack.pop() {
        match &e.kind {
            ExprKind::Field { param, field } if param != CANDIDATE => {
                out.insert(format!("{param}.{field}"), e);
            }
            ExprKind::Param(name) if name != CANDIDATE => {
                out.insert(name.clone(), e);
            }
            _ => {}
        }
        stack.extend(e.children());
    }
    out
}

/// The `unique` expressions a module invariant guarantees on every valid state: its top-level
/// conjuncts (feature 007). Only these may be decided by the delta rule on S' (research R7); a
/// `unique` under `not` or `or` need not hold on S. The invariant is closed, so every occurrence
/// of the same expression has that value on S.
pub fn held_uniques(e: &Expr) -> BTreeSet<Hash> {
    let mut out = BTreeSet::new();
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        match &x.kind {
            ExprKind::And(xs) => stack.extend(xs),
            ExprKind::Fold {
                op: crate::semantic::expr::FoldOp::Unique,
                ..
            } => {
                out.insert(x.hash);
            }
            _ => {}
        }
    }
    out
}

/// Whether an action can affect a module invariant with this signature: it creates or removes an
/// entity of a queried type, or changes a field its lambdas read (feature 007, research R7).
pub fn affects(
    module: &Module,
    action: &ActionItem,
    sig: &crate::semantic::module::Signature,
) -> bool {
    let entity_of_param = |name: &str| {
        action
            .params()
            .iter()
            .find(|p| p.name() == name)
            .and_then(|p| match p.ty() {
                Type::Entity(e) => Some(e.clone()),
                _ => None,
            })
    };
    let _ = module;
    action
        .creates
        .iter()
        .any(|c| sig.types.contains_key(&c.entity))
        || action
            .removes
            .iter()
            .filter_map(|r| entity_of_param(&r.param))
            .any(|t| sig.types.contains_key(&t))
        || action.effects.iter().any(|e| {
            entity_of_param(&e.param)
                .and_then(|t| sig.types.get(&t))
                .is_some_and(|fields| fields.as_ref().is_none_or(|f| f.contains(&e.field)))
        })
}

impl Evaluator<'_> {
    /// The captured values of a query in the enclosing state, and the environment its predicates
    /// are evaluated with (feature 007, FR-013a). Captures of bound fields are observed reads.
    fn captures(
        &mut self,
        q: &crate::semantic::expr::QueryNode,
        vals: &Vals,
        phase: Phase,
    ) -> Result<(Vec<(String, Json)>, Vals), String> {
        let mut captures = Vec::new();
        let mut env = Vals::new();
        for (read, e) in capture_reads(q) {
            let v = self.eval(e, vals, phase, &mut None)?;
            captures.push((read, encode(&e.ty, &v)));
            match &e.kind {
                ExprKind::Field { param, field } => {
                    if let Value::Entity(fields) = env
                        .entry(param.clone())
                        .or_insert_with(|| Value::Entity(BTreeMap::new()))
                    {
                        fields.insert(field.clone(), v);
                    }
                }
                ExprKind::Param(name) => {
                    env.insert(name.clone(), v);
                }
                _ => {}
            }
        }
        Ok((captures, env))
    }

    /// The members of a query instance at S, observed once (feature 007).
    fn query_s(
        &mut self,
        q: &crate::semantic::expr::QueryNode,
        env: &Vals,
        captures: Vec<(String, Json)>,
    ) -> Result<Vec<String>, String> {
        let canonical = crate::canonical::to_canonical_string(&Json::Array(
            captures
                .iter()
                .map(|(r, v)| json!([r, v]))
                .collect::<Vec<_>>(),
        ))
        .unwrap_or_default();
        let instance = hash_display(&crate::admit::hash::query_instance(q.hash(), &canonical));
        if let Some(f) = self.observed.queries.get(&instance) {
            return Ok(f.members.clone());
        }
        let request = crate::facts::QueryRequest {
            module: self.module,
            node: q,
            definition: hash_display(q.hash()),
            instance: instance.clone(),
            captures: &captures,
            env,
        };
        let mut members = match self.facts.query(&request) {
            Ok(m) => m,
            Err(e) => return Err(self.fact_failed(e)),
        };
        members.sort();
        members.dedup();
        self.observed.queries.insert(
            instance,
            crate::facts::QueryFact {
                definition: hash_display(q.hash()),
                entity: q.entity().to_string(),
                captures,
                members: members.clone(),
            },
        );
        Ok(members)
    }

    /// The members of a query on the given state, in canonical identity order. On S' they are
    /// derived: `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS` (research R4).
    fn members(
        &mut self,
        q: &crate::semantic::expr::QueryNode,
        vals: &Vals,
        phase: Phase,
    ) -> Result<Vec<String>, String> {
        let (captures, env) = self.captures(q, vals, phase)?;
        let base = self.query_s(q, &env, captures)?;
        if phase == Phase::S {
            return Ok(base);
        }
        let t = q.entity();
        let mut out: BTreeSet<String> = base
            .into_iter()
            .filter(|id| {
                let k = (t.to_string(), id.clone());
                !self.world.removed.contains(&k) && !self.world.bound.contains_key(&k)
            })
            .collect();
        let bound: Vec<(String, Value)> = self
            .world
            .bound
            .iter()
            .filter(|((bt, bid), _)| {
                bt == t && !self.world.removed.contains(&(bt.clone(), bid.clone()))
            })
            .filter_map(|((_, bid), param)| {
                self.world
                    .s_prime
                    .get(param)
                    .map(|v| (bid.clone(), v.clone()))
            })
            .collect();
        let created: Vec<(String, Value)> = self
            .world
            .created
            .iter()
            .filter(|((ct, _), _)| ct == t)
            .map(|((_, cid), v)| (cid.clone(), v.clone()))
            .collect();
        for (id, value) in bound.into_iter().chain(created) {
            if self.matches_value(q, &env, &value)? {
                out.insert(id);
            }
        }
        Ok(out.into_iter().collect())
    }

    /// The members of a query on S' that the transition bound or created: the only members whose
    /// values can differ from S (the `unique` delta rule, research R7).
    fn touched_members(
        &mut self,
        q: &crate::semantic::expr::QueryNode,
        vals: &Vals,
    ) -> Result<Vec<String>, String> {
        let (_, env) = self.captures(q, vals, Phase::SPrime)?;
        let t = q.entity();
        let bound = self
            .world
            .bound
            .iter()
            .filter(|((bt, bid), _)| {
                bt == t && !self.world.removed.contains(&(bt.clone(), bid.clone()))
            })
            .filter_map(|((_, bid), param)| {
                self.world
                    .s_prime
                    .get(param)
                    .map(|v| (bid.clone(), v.clone()))
            });
        let created = self
            .world
            .created
            .iter()
            .filter(|((ct, _), _)| ct == t)
            .map(|((_, cid), v)| (cid.clone(), v.clone()));
        let candidates: Vec<(String, Value)> = bound.chain(created).collect();
        let mut out = BTreeSet::new();
        for (id, value) in candidates {
            if self.matches_value(q, &env, &value)? {
                out.insert(id);
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Whether a candidate value is a member of a query, given the captured environment.
    fn matches_value(
        &mut self,
        q: &crate::semantic::expr::QueryNode,
        env: &Vals,
        candidate: &Value,
    ) -> Result<bool, String> {
        use crate::semantic::expr::{QueryKind, SetOp};
        match &q.kind {
            QueryKind::Select => Ok(true),
            QueryKind::Where { base, body, .. } => {
                if !self.matches_value(base, env, candidate)? {
                    return Ok(false);
                }
                let mut vals = env.clone();
                vals.insert(CANDIDATE.to_string(), candidate.clone());
                let saved = self.candidate.take();
                let r = self.eval(body, &vals, Phase::S, &mut None);
                self.candidate = saved;
                r?.as_bool()
                    .ok_or_else(|| "internal: predicate is not Bool".to_string())
            }
            QueryKind::Set { op, a, b } => {
                let x = self.matches_value(a, env, candidate)?;
                let y = self.matches_value(b, env, candidate)?;
                Ok(match op {
                    SetOp::Union => x || y,
                    SetOp::Intersection => x && y,
                    SetOp::Difference => x && !y,
                })
            }
        }
    }

    /// A field of the current candidate: from its bound value, its created value, or an observed
    /// field fact (feature 007, FR-009a).
    fn candidate_field(&mut self, field: &str, vals: &Vals, phase: Phase) -> Result<Value, String> {
        let bad = || format!("internal: no candidate field `{field}`");
        let Some((t, id)) = self.candidate.clone() else {
            return Err(bad());
        };
        let k = (t.clone(), id.clone());
        if let Some(param) = self.world.bound.get(&k).cloned() {
            let src = if phase == Phase::SPrime {
                &self.world.s_prime
            } else {
                vals
            };
            let v = match src.get(&param) {
                Some(Value::Entity(fields)) => fields.get(field).cloned().ok_or_else(bad)?,
                _ => return Err(bad()),
            };
            self.observe(&param, field);
            return Ok(v);
        }
        if phase == Phase::SPrime
            && let Some(Value::Entity(fields)) = self.world.created.get(&k)
        {
            return fields.get(field).cloned().ok_or_else(bad);
        }
        let fk = (t.clone(), id.clone(), field.to_string());
        let raw = match self.observed.fields.get(&fk) {
            Some(v) => v.clone(),
            None => match self.facts.field(&t, &id, field) {
                Ok(v) => {
                    self.observed.fields.insert(fk, v.clone());
                    v
                }
                Err(e) => return Err(self.fact_failed(e)),
            },
        };
        let ty = self
            .module
            .entity(&t)
            .and_then(|item| item.field_type(field))
            .cloned()
            .ok_or_else(bad)?;
        decode_scalar(&ty, &raw).map_err(|e| format!("field fact {t} {id}.{field}: {e}"))
    }

    /// The whole value of the current candidate (for a derived value over it).
    fn candidate_value(&mut self, vals: &Vals, phase: Phase) -> Result<Value, String> {
        let Some((t, id)) = self.candidate.clone() else {
            return Err("internal: no candidate".into());
        };
        let names: Vec<String> = self
            .module
            .entity(&t)
            .map(|e| e.fields().iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let mut fields = BTreeMap::new();
        for n in names {
            let v = if n == "id" {
                Value::Str(id.clone())
            } else {
                self.candidate_field(&n, vals, phase)?
            };
            fields.insert(n, v);
        }
        Ok(Value::Entity(fields))
    }

    /// `any`, `all`, `sum`, `min`, `max` or `unique` over members in canonical order.
    #[allow(clippy::too_many_arguments)]
    fn fold(
        &mut self,
        op: crate::semantic::expr::FoldOp,
        q: &crate::semantic::expr::QueryNode,
        members: &[String],
        body: &Expr,
        ty: &Type,
        vals: &Vals,
        phase: Phase,
        delta: bool,
    ) -> Result<Value, String> {
        use crate::semantic::expr::FoldOp;
        let t = q.entity().to_string();
        let per = |ev: &mut Self, id: &str| -> Result<Value, String> {
            ev.candidate = Some((t.clone(), id.to_string()));
            ev.eval(body, vals, phase, &mut None)
        };
        match op {
            FoldOp::Any | FoldOp::All => {
                let want = op == FoldOp::Any;
                for id in members {
                    let b = per(self, id)?
                        .as_bool()
                        .ok_or_else(|| "internal: predicate is not Bool".to_string())?;
                    if b == want {
                        return Ok(Value::Bool(want));
                    }
                }
                Ok(Value::Bool(!want))
            }
            FoldOp::Sum => {
                let mut acc = Exact::from_i64(0);
                let mut int_acc: i64 = 0;
                for id in members {
                    let v = per(self, id)?;
                    if is_integer(ty) {
                        let Value::Int(i) = v else {
                            return Err("internal: integer sum".into());
                        };
                        int_acc = int_acc.checked_add(i).ok_or_else(|| {
                            format!("{} in {}", NumError::Overflow, pretty::text(body))
                        })?;
                    } else {
                        let x = to_exact(&v).ok_or_else(|| "internal: numeric sum".to_string())?;
                        acc = acc.add(&x).map_err(|err| num_err(err, body))?;
                        if let Some(n) = fixed_scale(ty) {
                            fixed_value(&acc, n).map_err(|err| num_err(err, body))?;
                        }
                    }
                }
                if is_integer(ty) {
                    Ok(Value::Int(int_acc))
                } else if let Some(n) = fixed_scale(ty) {
                    fixed_value(&acc, n).map_err(|err| num_err(err, body))
                } else if matches!(ty, Type::Exact(_)) {
                    Ok(Value::Exact(acc))
                } else {
                    acc.to_dec()
                        .map(Value::Dec)
                        .map_err(|err| num_err(err, body))
                }
            }
            FoldOp::Min | FoldOp::Max => {
                let mut best: Option<(Exact, Value)> = None;
                for id in members {
                    let v = per(self, id)?;
                    let x = to_exact(&v).ok_or_else(|| "internal: ordered value".to_string())?;
                    let better = match &best {
                        None => true,
                        Some((b, _)) if op == FoldOp::Min => x < *b,
                        Some((b, _)) => x > *b,
                    };
                    if better {
                        best = Some((x, v));
                    }
                }
                Ok(best.map_or(Value::None, |(_, v)| v))
            }
            FoldOp::Unique if delta => {
                // `unique` held on the valid S: only touched members can collide (research R7).
                let touched: Vec<String> = members
                    .iter()
                    .filter(|id| {
                        let k = (t.clone(), (*id).clone());
                        self.world.bound.contains_key(&k) || self.world.created.contains_key(&k)
                    })
                    .cloned()
                    .collect();
                for id in touched {
                    let key = per(self, &id)?;
                    self.candidate = None;
                    let loc = body.loc().clone();
                    let lit = Expr::new(ExprKind::Lit(key), body.ty.clone(), loc.clone());
                    let pred = Expr::new(
                        ExprKind::Cmp(CmpOp::Eq, Box::new(body.clone()), Box::new(lit)),
                        Type::Bool,
                        loc.clone(),
                    );
                    let same_key = crate::semantic::expr::QueryNode::new(
                        crate::semantic::expr::QueryKind::Where {
                            base: Box::new(q.clone()),
                            param: "key".into(),
                            body: Box::new(pred),
                        },
                        t.clone(),
                        loc,
                    );
                    let others = self.members(&same_key, vals, phase)?;
                    if others != [id] {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            FoldOp::Unique => {
                let mut seen = BTreeSet::new();
                for id in members {
                    let key = per(self, id)?;
                    if !seen.insert(crate::semantic::value::encode_untyped(&key).to_string()) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
        }
    }

    fn fact_failed(&mut self, e: FactError) -> String {
        self.fact_error.get_or_insert_with(|| e.0.clone());
        e.0
    }

    /// `exists` on S: bound entities exist by binding; anything else is an observed fact.
    fn exists_s(&mut self, entity: &str, id: &str) -> Result<bool, String> {
        let k = (entity.to_string(), id.to_string());
        if self.world.bound.contains_key(&k) {
            return Ok(true);
        }
        if let Some(b) = self.observed.existence.get(&k) {
            return Ok(*b);
        }
        match self.facts.exists(entity, id) {
            Ok(b) => {
                self.observed.existence.insert(k, b);
                Ok(b)
            }
            Err(e) => Err(self.fact_failed(e)),
        }
    }

    /// `exists` on the given state: on S' it is `(exists ∨ created) ∧ ¬removed` (research R4).
    fn exists_at(&mut self, phase: Phase, entity: &str, id: &str) -> Result<bool, String> {
        let k = (entity.to_string(), id.to_string());
        if phase == Phase::SPrime {
            if self.world.created.contains_key(&k) {
                return Ok(true);
            }
            if self.world.removed.contains(&k) {
                return Ok(false);
            }
        }
        self.exists_s(entity, id)
    }

    /// `used` (a history fact): bound entities are used by binding.
    fn used_s(&mut self, entity: &str, id: &str) -> Result<bool, String> {
        let k = (entity.to_string(), id.to_string());
        if self.world.bound.contains_key(&k) {
            return Ok(true);
        }
        if let Some(b) = self.observed.identities.get(&k) {
            return Ok(*b);
        }
        match self.facts.used(entity, id) {
            Ok(b) => {
                self.observed.identities.insert(k, b);
                Ok(b)
            }
            Err(e) => Err(self.fact_failed(e)),
        }
    }

    fn incoming_s(&mut self, entity: &str, id: &str) -> Result<Vec<RefEdge>, String> {
        let k = (entity.to_string(), id.to_string());
        if let Some(edges) = self.observed.references.get(&k) {
            return Ok(edges.clone());
        }
        match self.facts.incoming(entity, id) {
            Ok(mut edges) => {
                edges.sort();
                edges.dedup();
                self.observed.references.insert(k, edges.clone());
                Ok(edges)
            }
            Err(e) => Err(self.fact_failed(e)),
        }
    }

    /// `incoming'`: S's incoming references without removed sources and without the bound
    /// entities' old values, plus the bound entities' S' references and the created entities'.
    fn incoming_prime(&mut self, entity: &str, id: &str) -> Result<Vec<RefEdge>, String> {
        let base = self.incoming_s(entity, id)?;
        let mut out: BTreeSet<RefEdge> = base
            .into_iter()
            .filter(|e| {
                let k = (e.entity.clone(), e.id.clone());
                !self.world.removed.contains(&k) && !self.world.bound.contains_key(&k)
            })
            .collect();
        let points = |source_type: &str, fields: &BTreeMap<String, Value>| -> Vec<String> {
            self.module
                .entity(source_type)
                .map(|item| {
                    item.reference_fields()
                        .filter(|(f, t)| {
                            *t == entity && fields.get(*f) == Some(&Value::Str(id.to_string()))
                        })
                        .map(|(f, _)| f.to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        for ((source_type, source_id), param) in &self.world.bound {
            let k = (source_type.clone(), source_id.clone());
            if self.world.removed.contains(&k) {
                continue;
            }
            if let Some(Value::Entity(fields)) = self.world.s_prime.get(param) {
                for field in points(source_type, fields) {
                    out.insert(RefEdge {
                        entity: source_type.clone(),
                        id: source_id.clone(),
                        field,
                    });
                }
            }
        }
        for ((source_type, source_id), v) in &self.world.created {
            if let Value::Entity(fields) = v {
                for field in points(source_type, fields) {
                    out.insert(RefEdge {
                        entity: source_type.clone(),
                        id: source_id.clone(),
                        field,
                    });
                }
            }
        }
        Ok(out.into_iter().collect())
    }

    fn observe(&mut self, param: &str, field: &str) {
        if let Some(f) = self.frames.last_mut() {
            f.reads.insert((param.to_string(), field.to_string()));
        }
    }

    /// Adds reads made in an inner scope, renamed to the current scope's parameters.
    fn attribute(&mut self, reads: &ObservedReads, to_outer: &BTreeMap<String, String>) {
        for (p, f) in reads {
            if let Some(outer) = to_outer.get(p) {
                self.observe(&outer.clone(), f);
            }
        }
    }

    fn push_frame(&mut self, to_outer: BTreeMap<String, String>) {
        self.frames.push(ReadFrame {
            to_outer,
            reads: BTreeSet::new(),
        });
    }

    /// Pops the innermost frame, attributes its reads to the enclosing one, and returns them in
    /// the popped scope's own names.
    fn pop_frame(&mut self) -> ObservedReads {
        let f = self.frames.pop().unwrap_or_default();
        self.attribute(&f.reads, &f.to_outer);
        f.reads
    }
}

/// Moves the pending rescale entries onto the last trace step.
fn attach_rescales(ev: &mut Evaluator<'_>, trace: &mut [Json]) {
    let pending = std::mem::take(&mut ev.rescales);
    if pending.is_empty() {
        return;
    }
    if let Some(Json::Object(m)) = trace.last_mut() {
        m.insert("rescales".into(), Json::Array(pending));
    }
}

/// The exact value of a numeric runtime value.
fn to_exact(v: &Value) -> Option<Exact> {
    match v {
        Value::Dec(d) => Some(Exact::from_dec(d)),
        Value::Int(i) => Some(Exact::from_i64(*i)),
        Value::Exact(x) => Some(x.clone()),
        _ => None,
    }
}

/// Whether values of `t` are integers (`Int` or an integer nominal).
fn is_integer(t: &Type) -> bool {
    match t {
        Type::Int => true,
        Type::Nominal(n) => n.underlying == crate::semantic::types::Prim::Int,
        _ => false,
    }
}

/// A fixed-scale result of lossless arithmetic: on the grid by typing, range-checked here.
fn fixed_value(x: &Exact, n: &crate::semantic::types::NominalInfo) -> Result<Value, NumError> {
    let scale = n.scale.unwrap_or(0);
    // The value is on the grid, so any rounding mode returns it unchanged.
    let d = x.round_to_scale(scale, crate::exact::Rounding::Down)?;
    if Exact::from_dec(&d) != *x || !d.in_fixed_range(scale) {
        return Err(NumError::Overflow);
    }
    Ok(Value::Dec(d))
}

fn num_err(e: NumError, expr: &Expr) -> String {
    format!("{e} in {}", pretty::text(expr))
}

impl Evaluator<'_> {
    /// Evaluates a numeric expression in the exact domain: decimal arithmetic nodes recurse
    /// exactly; other nodes (fields, derived values, integer arithmetic, rescale) are evaluated
    /// normally and lifted (feature 004: all decimal arithmetic is exact).
    fn eval_exact(
        &mut self,
        e: &Expr,
        vals: &Vals,
        phase: Phase,
        reads: &mut Option<&mut Reads>,
    ) -> Result<Exact, String> {
        let bad = || format!("internal: ill-typed value in {}", pretty::text(e));
        match &e.kind {
            ExprKind::Arith(op, a, b) if !is_integer(&e.ty) => {
                let x = self.eval_exact(a, vals, phase, reads)?;
                let y = self.eval_exact(b, vals, phase, reads)?;
                let r = match op {
                    ArithOp::Add => x.add(&y),
                    ArithOp::Sub => x.sub(&y),
                    ArithOp::Mul => x.mul(&y),
                    ArithOp::Div => x.div(&y),
                };
                let r = r.map_err(|err| num_err(err, e))?;
                // A fixed-scale node (`F ± F`, `F × I`) is a value of `F` wherever it occurs, also
                // inside exact arithmetic or `underlying(...)`: its range is checked.
                if let Some(n) = fixed_scale(&e.ty) {
                    fixed_value(&r, n).map_err(|err| num_err(err, e))?;
                }
                Ok(r)
            }
            _ => {
                let v = self.eval(e, vals, phase, reads)?;
                to_exact(&v).ok_or_else(bad)
            }
        }
    }

    fn eval(
        &mut self,
        e: &Expr,
        vals: &Vals,
        phase: Phase,
        reads: &mut Option<&mut Reads>,
    ) -> Result<Value, String> {
        let bad = || format!("internal: ill-typed value in {}", pretty::text(e));
        match &e.kind {
            ExprKind::Lit(v) => Ok(v.clone()),
            ExprKind::Field { param, field } if param == CANDIDATE && self.candidate.is_some() => {
                let v = self.candidate_field(field, vals, phase)?;
                if let Some(r) = reads.as_deref_mut()
                    && let Some((_, id)) = &self.candidate
                {
                    r.insert(format!("{id}.{field}"), encode(&e.ty, &v));
                }
                Ok(v)
            }
            ExprKind::Field { param, field } => {
                let v = match vals.get(param) {
                    Some(Value::Entity(fields)) => fields.get(field).cloned().ok_or_else(bad)?,
                    _ => return Err(bad()),
                };
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(format!("{param}.{field}"), encode(&e.ty, &v));
                }
                self.observe(param, field);
                Ok(v)
            }
            ExprKind::Param(name) => {
                let v = vals.get(name).cloned().ok_or_else(bad)?;
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(name.clone(), encode(&e.ty, &v));
                }
                Ok(v)
            }
            ExprKind::DerivedRef { name, args, target } => {
                let d = self.module.derived(name).ok_or_else(bad)?;
                let mut inner = Vals::new();
                let mut arg_values = Vec::new();
                for (p, a) in d.params().iter().zip(args) {
                    let v = if a == CANDIDATE && self.candidate.is_some() {
                        self.candidate_value(vals, phase)?
                    } else {
                        vals.get(a).cloned().ok_or_else(bad)?
                    };
                    arg_values.push(crate::semantic::value::encode_untyped(&v));
                    inner.insert(p.name().to_string(), v);
                }
                let args_key = Json::Array(arg_values).to_string();
                let key = (name.clone(), args_key.clone(), phase);
                let to_outer: BTreeMap<String, String> = d
                    .params()
                    .iter()
                    .zip(args)
                    .map(|(p, a)| (p.name().to_string(), a.clone()))
                    .collect();
                let v = match self.memo.get(&key) {
                    Some((v, body_reads)) => {
                        let (v, body_reads) = (v.clone(), body_reads.clone());
                        self.attribute(&body_reads, &to_outer);
                        v
                    }
                    None => {
                        let body = d.body().clone();
                        self.push_frame(to_outer);
                        let r = self.eval(&body, &inner, phase, &mut None);
                        let body_reads = self.pop_frame();
                        let v = r?;
                        self.memo.insert(key, (v.clone(), body_reads));
                        let order = self
                            .module
                            .evaluation_order()
                            .iter()
                            .position(|n| n == name)
                            .unwrap_or(usize::MAX);
                        self.derived.push(DerivedEntry {
                            name: name.clone(),
                            hash: *target,
                            phase,
                            order,
                            args_key,
                            value: encode(&e.ty, &v),
                        });
                        v
                    }
                };
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(name.clone(), encode(&e.ty, &v));
                }
                Ok(v)
            }
            ExprKind::Cmp(op, a, b) => {
                let x = self.eval(a, vals, phase, reads)?;
                let y = self.eval(b, vals, phase, reads)?;
                let exact = matches!(x, Value::Exact(_)) || matches!(y, Value::Exact(_));
                let ord = || match (&x, &y) {
                    (Value::Int(p), Value::Int(q)) => Some(p.cmp(q)),
                    (Value::Dec(p), Value::Dec(q)) => Some(p.cmp(q)),
                    _ if exact => Some(to_exact(&x)?.cmp(&to_exact(&y)?)),
                    _ => None,
                };
                let r = match op {
                    CmpOp::Eq if exact => ord().ok_or_else(bad)?.is_eq(),
                    CmpOp::Ne if exact => !ord().ok_or_else(bad)?.is_eq(),
                    CmpOp::Eq => x == y,
                    CmpOp::Ne => x != y,
                    CmpOp::Lt => ord().ok_or_else(bad)?.is_lt(),
                    CmpOp::Le => ord().ok_or_else(bad)?.is_le(),
                    CmpOp::Gt => ord().ok_or_else(bad)?.is_gt(),
                    CmpOp::Ge => ord().ok_or_else(bad)?.is_ge(),
                };
                Ok(Value::Bool(r))
            }
            ExprKind::Arith(..) if matches!(e.ty, Type::Exact(_)) => {
                Ok(Value::Exact(self.eval_exact(e, vals, phase, reads)?))
            }
            ExprKind::Arith(..) if fixed_scale(&e.ty).is_some() => {
                let x = self.eval_exact(e, vals, phase, reads)?;
                let n = fixed_scale(&e.ty).ok_or_else(bad)?;
                fixed_value(&x, n).map_err(|err| num_err(err, e))
            }
            ExprKind::Rescale { arg, rounding } => {
                let n = fixed_scale(&e.ty).ok_or_else(bad)?;
                let scale = n.scale.unwrap_or(0);
                let x = self.eval_exact(arg, vals, phase, reads)?;
                let d = x
                    .round_to_scale(scale, *rounding)
                    .map_err(|err| num_err(err, e))?;
                if !d.in_fixed_range(scale) {
                    return Err(num_err(NumError::Overflow, e));
                }
                self.rescales.push(json!({
                    "rescale": pretty::text(arg),
                    "exact": x.to_text(),
                    "rounding": rounding.as_str(),
                    "scale": scale,
                    "result": fixed_text(&d, scale),
                }));
                Ok(Value::Dec(d))
            }
            ExprKind::Wrap(a) if fixed_scale(&e.ty).is_some() => {
                let v = self.eval(a, vals, phase, reads)?;
                let n = fixed_scale(&e.ty).ok_or_else(bad)?;
                let x = to_exact(&v).ok_or_else(bad)?;
                fixed_value(&x, n).map_err(|err| num_err(err, e))
            }
            // Integer arithmetic (plain or integer nominal); decimal arithmetic is exact above.
            ExprKind::Arith(op, a, b) => {
                let x = self.eval(a, vals, phase, reads)?;
                let y = self.eval(b, vals, phase, reads)?;
                let (Value::Int(p), Value::Int(q)) = (&x, &y) else {
                    return Err(bad());
                };
                let r = match op {
                    ArithOp::Add => p.checked_add(*q),
                    ArithOp::Sub => p.checked_sub(*q),
                    ArithOp::Mul => p.checked_mul(*q),
                    ArithOp::Div => return Err(bad()),
                };
                r.map(Value::Int)
                    .ok_or_else(|| num_err(NumError::Overflow, e))
            }
            ExprKind::And(xs) => {
                for x in xs {
                    if !self
                        .eval(x, vals, phase, reads)?
                        .as_bool()
                        .ok_or_else(bad)?
                    {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            ExprKind::Or(xs) => {
                for x in xs {
                    if self
                        .eval(x, vals, phase, reads)?
                        .as_bool()
                        .ok_or_else(bad)?
                    {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            ExprKind::Not(a) => Ok(Value::Bool(
                !self
                    .eval(a, vals, phase, reads)?
                    .as_bool()
                    .ok_or_else(bad)?,
            )),
            ExprKind::In(a, values) => {
                let v = self.eval(a, vals, phase, reads)?;
                Ok(Value::Bool(values.contains(&v)))
            }
            ExprKind::IsNone(a) => Ok(Value::Bool(
                self.eval(a, vals, phase, reads)? == Value::None,
            )),
            ExprKind::IsSome(a) => Ok(Value::Bool(
                self.eval(a, vals, phase, reads)? != Value::None,
            )),
            ExprKind::ValueOr(a, d) => match self.eval(a, vals, phase, reads)? {
                Value::None => self.eval(d, vals, phase, reads),
                v => Ok(v),
            },
            ExprKind::Some(a) | ExprKind::Wrap(a) | ExprKind::Unwrap(a) => {
                self.eval(a, vals, phase, reads)
            }
            ExprKind::ToDecimal(a) => match self.eval(a, vals, phase, reads)? {
                Value::Int(i) => Ok(Value::Dec(Dec::from_i64(i))),
                _ => Err(bad()),
            },
            // Narrowing sites of migrations (feature 009): a failure is an evaluation error.
            ExprKind::StrictUnwrap(a) => match self.eval(a, vals, phase, reads)? {
                Value::None => Err(format!(
                    "`{}` is absent: strict_unwrap needs a value",
                    pretty::text(a)
                )),
                v => Ok(v),
            },
            ExprKind::EnumMap { arg, mapping, .. } => match self.eval(arg, vals, phase, reads)? {
                Value::None => Ok(Value::None),
                Value::Str(v) => match mapping.iter().find(|(from, _)| *from == v) {
                    Some((_, to)) => Ok(Value::Str(to.clone())),
                    None => Err(format!(
                        "`{}` is {v}, which the strict enum map does not map",
                        pretty::text(arg)
                    )),
                },
                _ => Err(bad()),
            },
            ExprKind::Count(q) => {
                let n = self.members(q, vals, phase)?.len();
                let v = Value::Int(i64::try_from(n).map_err(|_| "internal: count".to_string())?);
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(pretty::text(e), encode(&e.ty, &v));
                }
                Ok(v)
            }
            ExprKind::Fold {
                op, query, body, ..
            } => {
                let delta = *op == crate::semantic::expr::FoldOp::Unique
                    && phase == Phase::SPrime
                    && self.assume_valid.contains(&e.hash);
                let members = if delta {
                    self.touched_members(query, vals)?
                } else {
                    self.members(query, vals, phase)?
                };
                let saved = self.candidate.take();
                let r = self.fold(*op, query, &members, body, &e.ty, vals, phase, delta);
                self.candidate = saved;
                let v = r?;
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(pretty::text(e), encode(&e.ty, &v));
                }
                Ok(v)
            }
            ExprKind::Exists(a) | ExprKind::Referenced(a) => {
                let v = self.eval(a, vals, phase, reads)?;
                let entity = id_entity(&a.ty).ok_or_else(bad)?.to_string();
                let b = match (&e.kind, v) {
                    (ExprKind::Exists(_), Value::None) => false,
                    (ExprKind::Exists(_), Value::Str(id)) => self.exists_at(phase, &entity, &id)?,
                    (ExprKind::Referenced(_), Value::Str(id)) => {
                        let edges = if phase == Phase::SPrime {
                            self.incoming_prime(&entity, &id)?
                        } else {
                            self.incoming_s(&entity, &id)?
                        };
                        !edges.is_empty()
                    }
                    _ => return Err(bad()),
                };
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(pretty::text(e), json!(b));
                }
                Ok(Value::Bool(b))
            }
        }
    }

    /// Evaluates a predicate, returning its trace outcome and reads.
    fn predicate(&mut self, e: &Expr, vals: &Vals, phase: Phase) -> (Result<bool, String>, Reads) {
        let mut reads = Reads::new();
        let r = self
            .eval(e, vals, phase, &mut Some(&mut reads))
            .and_then(|v| {
                v.as_bool()
                    .ok_or_else(|| "internal: predicate is not Bool".to_string())
            });
        (r, reads)
    }
}

fn step(
    phase: &str,
    name: Option<&str>,
    hash: &Hash,
    expr_text: String,
    reads: Reads,
    outcome: Json,
    loc: &Loc,
) -> Json {
    let mut s = json!({
        "phase": phase,
        "hash": hash_display(hash),
        "expr_text": expr_text,
        "reads": Json::Object(reads.into_iter().collect()),
        "outcome": outcome,
        "loc": {"file": loc.file, "line": loc.line},
    });
    if let (Some(n), Json::Object(m)) = (name, &mut s) {
        m.insert("name".into(), json!(n));
    }
    s
}

fn reason(code: &str, message: String, loc: Option<&Loc>) -> Json {
    let mut r = json!({"code": code, "message": message});
    if let (Some(l), Json::Object(m)) = (loc, &mut r) {
        m.insert("loc".into(), json!({"file": l.file, "line": l.line}));
    }
    r
}

/// Parsed request (possibly with problems).
struct Request {
    action: String,
    data_version: String,
    git_revision: Option<String>,
    sections: BTreeMap<&'static str, Map<String, Json>>,
    /// The supplied `facts` section (feature 006), as given.
    facts: Option<Json>,
}

impl<'a> Evaluator<'a> {
    fn new(module: &'a Module, facts: &'a dyn EvaluationFacts) -> Self {
        Evaluator {
            module,
            facts,
            observed: Facts::default(),
            fact_error: None,
            world: World::default(),
            memo: BTreeMap::new(),
            frames: vec![ReadFrame::default()],
            derived: Vec::new(),
            rescales: Vec::new(),
            candidate: None,
            assume_valid: BTreeSet::new(),
        }
    }

    /// The reason for a failed step: `UNKNOWN_FACT` when a fact was missing, otherwise
    /// `EVALUATION_ERROR`.
    fn error_reason(&mut self, msg: String, loc: &Loc) -> Json {
        match self.fact_error.take() {
            Some(m) => reason(
                "UNKNOWN_FACT",
                format!("{m}: the evaluation needs this fact"),
                Some(loc),
            ),
            None => reason("EVALUATION_ERROR", msg, Some(loc)),
        }
    }
}

fn parse_request(text: &str, problems: &mut Vec<InputProblem>) -> Request {
    let mut req = Request {
        action: String::new(),
        data_version: String::new(),
        git_revision: None,
        sections: BTreeMap::new(),
        facts: None,
    };
    let root: Json = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            problems.push(InputProblem {
                code: "DECODE_ERROR",
                path: "$".into(),
                message: format!("invalid JSON: {e}"),
            });
            return req;
        }
    };
    let Json::Object(obj) = root else {
        problems.push(InputProblem {
            code: "DECODE_ERROR",
            path: "$".into(),
            message: "expected an object".into(),
        });
        return req;
    };
    for (key, v) in &obj {
        match (key.as_str(), v) {
            ("action", Json::String(s)) => req.action = s.clone(),
            ("data_version", Json::String(s)) => req.data_version = s.clone(),
            ("git_revision", Json::String(s)) => req.git_revision = Some(s.clone()),
            ("facts", v) => req.facts = Some(v.clone()),
            (section @ ("state" | "input" | "context"), Json::Object(m)) => {
                let name = match section {
                    "state" => "state",
                    "input" => "input",
                    _ => "context",
                };
                req.sections.insert(name, m.clone());
            }
            _ => problems.push(InputProblem {
                code: "DECODE_ERROR",
                path: key.clone(),
                message: format!("unexpected or malformed key `{key}`"),
            }),
        }
    }
    for s in ["state", "input", "context"] {
        req.sections.entry(s).or_default();
    }
    if !obj.contains_key("action") {
        problems.push(InputProblem {
            code: "MISSING_ARGUMENT",
            path: "action".into(),
            message: "missing `action`".into(),
        });
    }
    if !obj.contains_key("data_version") {
        problems.push(InputProblem {
            code: "MISSING_ARGUMENT",
            path: "data_version".into(),
            message: "missing `data_version`".into(),
        });
    }
    req
}

fn echo_sections(req: &Request) -> (Json, Json, Json) {
    let get = |s: &str| {
        req.sections
            .get(s)
            .map(|m| sanitize_floats(&Json::Object(m.clone())))
            .unwrap_or_else(|| json!({}))
    };
    (get("state"), get("input"), get("context"))
}

struct Outcome {
    result: &'static str,
    reasons: Vec<Json>,
    trace: Vec<Json>,
    changes: Vec<Json>,
    /// Creations and removals of an allowed transition, in effect order (feature 006).
    lifecycle: Vec<Json>,
}

/// Decodes `raw` as an entity of type `entity` (field set, field types, fixed-scale grid and
/// range), checks the entity's constraints, and returns its canonical encoding as decision
/// records show it; otherwise the problems (feature 005: store genesis seeds). Constraints that
/// use `exists` or `referenced` need facts: see [`decode_entity`] and [`check_entity`].
pub fn canonical_entity(module: &Module, entity: &str, raw: &Json) -> Result<Json, Vec<String>> {
    let value = decode_entity(module, entity, raw)?;
    check_entity(module, entity, &value, &Facts::default())?;
    Ok(value)
}

pub(crate) fn decode_value(
    module: &Module,
    entity: &str,
    raw: &Json,
) -> Result<Value, Vec<String>> {
    if module.entity(entity).is_none() {
        return Err(vec![format!("unknown entity `{entity}`")]);
    }
    let mut problems = Vec::new();
    let value = decode_param(
        module,
        &Type::Entity(entity.into()),
        raw,
        entity,
        &mut problems,
    );
    value.filter(|_| problems.is_empty()).ok_or_else(|| {
        problems
            .into_iter()
            .map(|p| format!("{}: {}", p.path, p.message))
            .collect()
    })
}

/// Decodes `raw` as an entity of type `entity` (field set, field types, fixed-scale grid and
/// range) and returns its canonical encoding, without checking constraints.
pub fn decode_entity(module: &Module, entity: &str, raw: &Json) -> Result<Json, Vec<String>> {
    let value = decode_value(module, entity, raw)?;
    Ok(encode_param(module, &Type::Entity(entity.into()), &value))
}

/// Checks the entity constraints of a decoded entity value, answering `exists` and `referenced`
/// from `facts` (feature 006: a genesis answers them from its seed). Synthesized reference
/// constraints concern the universe, not one value: a genesis checks them over the whole seed.
pub fn check_entity(
    module: &Module,
    entity: &str,
    value: &Json,
    facts: &dyn EvaluationFacts,
) -> Result<(), Vec<String>> {
    let value = decode_value(module, entity, value)?;
    let mut ev = Evaluator::new(module, facts);
    let mut failed = Vec::new();
    for (name, c) in module
        .constraints_for(entity)
        .filter(|(_, c)| c.reference().is_none())
    {
        let vals = Vals::from([(c.param().to_string(), value.clone())]);
        match ev.predicate(c.body(), &vals, Phase::S).0 {
            Ok(true) => {}
            Ok(false) => failed.push(format!("constraint `{name}` is violated")),
            Err(e) => failed.push(format!("constraint `{name}`: {e}")),
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed)
    }
}

/// Whether a candidate entity (in its canonical encoding) is a member of a query, given the
/// captured environment (feature 007). Used by fact providers that answer queries themselves.
pub fn query_matches(
    module: &Module,
    q: &crate::semantic::expr::QueryNode,
    env: &Vals,
    candidate: &Json,
) -> Result<bool, String> {
    let value = decode_value(module, q.entity(), candidate).map_err(|p| p.join("; "))?;
    let no_facts = Facts::default();
    let mut ev = Evaluator::new(module, &no_facts);
    ev.matches_value(q, env, &value)
}

fn collect_query_node<'a>(
    q: &'a crate::semantic::expr::QueryNode,
    out: &mut BTreeMap<String, &'a crate::semantic::expr::QueryNode>,
) {
    use crate::semantic::expr::QueryKind;
    out.entry(hash_display(q.hash())).or_insert(q);
    match q.kind() {
        QueryKind::Select => {}
        QueryKind::Where { base, .. } => collect_query_node(base, out),
        QueryKind::Set { a, b, .. } => {
            collect_query_node(a, out);
            collect_query_node(b, out);
        }
    }
}

fn collect_query_nodes_expr<'a>(
    module: &'a Module,
    e: &'a Expr,
    out: &mut BTreeMap<String, &'a crate::semantic::expr::QueryNode>,
    seen_derived: &mut BTreeSet<String>,
) {
    match &e.kind {
        ExprKind::Count(q) | ExprKind::Fold { query: q, .. } => collect_query_node(q, out),
        ExprKind::DerivedRef { name, .. } => {
            if seen_derived.insert(name.clone())
                && let Some(d) = module.derived(name)
            {
                collect_query_nodes_expr(module, d.body(), out, seen_derived);
            }
        }
        _ => {}
    }
    for child in e.children() {
        collect_query_nodes_expr(module, child, out, seen_derived);
    }
}

fn collect_query_nodes(module: &Module) -> BTreeMap<String, &crate::semantic::expr::QueryNode> {
    let mut out = BTreeMap::new();
    let mut seen_derived = BTreeSet::new();
    for (name, d) in module.derived_items() {
        if seen_derived.insert(name.clone()) {
            collect_query_nodes_expr(module, d.body(), &mut out, &mut seen_derived);
        }
    }
    for i in module.invariants().values() {
        collect_query_nodes_expr(module, i.body(), &mut out, &mut seen_derived);
    }
    for g in module.global_invariants().values() {
        collect_query_nodes_expr(module, g.body(), &mut out, &mut seen_derived);
    }
    for c in module.constraints().values() {
        collect_query_nodes_expr(module, c.body(), &mut out, &mut seen_derived);
    }
    for a in module.actions().values() {
        for c in a.preconditions.iter().chain(&a.postconditions) {
            collect_query_nodes_expr(module, &c.expr, &mut out, &mut seen_derived);
        }
        for e in &a.effects {
            collect_query_nodes_expr(module, &e.value, &mut out, &mut seen_derived);
        }
        for c in &a.creates {
            collect_query_nodes_expr(module, &c.id, &mut out, &mut seen_derived);
            for (_, value) in &c.fields {
                collect_query_nodes_expr(module, value, &mut out, &mut seen_derived);
            }
        }
    }
    out
}

fn capture_env_from_fact(
    module: &Module,
    q: &crate::semantic::expr::QueryNode,
    fact: &crate::facts::QueryFact,
) -> Result<Vals, String> {
    let supplied: BTreeMap<&str, &Json> = fact
        .captures
        .iter()
        .map(|(read, value)| (read.as_str(), value))
        .collect();
    let reads = capture_reads(q);
    if supplied.len() != reads.len() {
        return Err("capture set differs from the query definition".into());
    }
    let mut env = Vals::new();
    for (read, e) in reads {
        let raw = supplied
            .get(read.as_str())
            .ok_or_else(|| format!("missing capture `{read}`"))?;
        let v = match &e.ty {
            Type::Entity(entity) => decode_value(module, entity, raw).map_err(|p| p.join("; "))?,
            ty => decode_scalar(ty, raw)?,
        };
        match &e.kind {
            ExprKind::Field { param, field } => {
                if let Value::Entity(fields) = env
                    .entry(param.clone())
                    .or_insert_with(|| Value::Entity(BTreeMap::new()))
                {
                    fields.insert(field.clone(), v);
                }
            }
            ExprKind::Param(name) => {
                env.insert(name.clone(), v);
            }
            _ => {}
        }
    }
    Ok(env)
}

fn check_query_universe_agreement(
    module: &Module,
    facts: &Facts,
) -> Result<(), crate::facts::FactsProblem> {
    let bad = |message: String| crate::facts::FactsProblem {
        code: "INCONSISTENT_FACTS",
        message,
    };
    if facts.queries.is_empty() || facts.universe.is_empty() {
        return Ok(());
    }
    let queries = collect_query_nodes(module);
    for (instance, fact) in &facts.queries {
        let Some(members) = facts.universe.get(&fact.entity) else {
            continue;
        };
        let Some(q) = queries.get(&fact.definition) else {
            return Err(bad(format!(
                "query fact for instance {instance} overlaps a complete universe but has an unknown definition"
            )));
        };
        if q.entity() != fact.entity {
            return Err(bad(format!(
                "query fact for instance {instance} names {}, but its definition queries {}",
                fact.entity,
                q.entity()
            )));
        }
        let env = capture_env_from_fact(module, q, fact)
            .map_err(|e| bad(format!("query fact for instance {instance}: {e}")))?;
        let mut expected = Vec::new();
        for (id, value) in members {
            if query_matches(module, q, &env, value).map_err(|e| {
                bad(format!(
                    "query fact for instance {instance} cannot be checked against the universe: {e}"
                ))
            })? {
                expected.push(id.clone());
            }
        }
        if expected != fact.members {
            return Err(bad(format!(
                "query fact for instance {instance} contradicts the supplied universe of {}",
                fact.entity
            )));
        }
    }
    Ok(())
}

/// The entity types an action's evaluation may query (feature 007): every query in its
/// conditions, effects and creations (through the derived values they use), and every module
/// invariant's queried types.
pub fn queried_types(module: &Module, action: &ActionItem) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<&Expr> = Vec::new();
    stack.extend(
        action
            .preconditions
            .iter()
            .chain(&action.postconditions)
            .map(|c| &c.expr),
    );
    stack.extend(action.effects.iter().map(|e| &e.value));
    for c in &action.creates {
        stack.push(&c.id);
        stack.extend(c.fields.iter().map(|(_, v)| v));
    }
    for g in module.global_invariants().values() {
        out.extend(g.signature().types.keys().cloned());
    }
    let mut seen = BTreeSet::new();
    while let Some(e) = stack.pop() {
        match &e.kind {
            ExprKind::Count(q) | ExprKind::Fold { query: q, .. } => {
                out.insert(q.entity().to_string());
            }
            ExprKind::DerivedRef { name, .. } => {
                if seen.insert(name.clone())
                    && let Some(d) = module.derived(name)
                {
                    stack.push(d.body());
                }
            }
            _ => {}
        }
        stack.extend(e.children());
    }
    out
}

/// Where a store may look for a query's candidates (feature 007): an indexable equality
/// `candidate.field == value` of a filter (a captured value preferred over a literal, the likely
/// more selective one), combined through set algebra. A store re-checks every candidate, so the
/// plan never changes a result.
pub fn index_hint(q: &crate::semantic::expr::QueryNode, env: &Vals) -> Option<IndexPlan> {
    use crate::semantic::expr::{QueryKind, SetOp};
    match &q.kind {
        QueryKind::Select => None,
        QueryKind::Where { base, body, .. } => {
            let conjuncts: Vec<&Expr> = match &body.kind {
                ExprKind::And(xs) => xs.iter().collect(),
                _ => vec![body],
            };
            let mut literal = None;
            for c in conjuncts {
                let ExprKind::Cmp(CmpOp::Eq, a, b) = &c.kind else {
                    continue;
                };
                for (field_side, value_side) in [(a, b), (b, a)] {
                    let ExprKind::Field { param, field } = &field_side.kind else {
                        continue;
                    };
                    if param != CANDIDATE {
                        continue;
                    }
                    let plan = |v: &Value| IndexPlan::Eq {
                        field: field.clone(),
                        value: encode(&field_side.ty, v),
                    };
                    match &value_side.kind {
                        // A captured value (an identity, a key) is the selective kind.
                        ExprKind::Field { param, field } if param != CANDIDATE => {
                            if let Some(Value::Entity(fields)) = env.get(param)
                                && let Some(v) = fields.get(field)
                            {
                                return Some(plan(v));
                            }
                        }
                        ExprKind::Param(name) if name != CANDIDATE => {
                            if let Some(v) = env.get(name) {
                                return Some(plan(v));
                            }
                        }
                        ExprKind::Lit(v) => {
                            literal.get_or_insert_with(|| plan(v));
                        }
                        _ => {}
                    }
                }
            }
            index_hint(base, env).or(literal)
        }
        QueryKind::Set { op, a, b } => match op {
            SetOp::Union => Some(IndexPlan::Union(
                Box::new(index_hint(a, env)?),
                Box::new(index_hint(b, env)?),
            )),
            SetOp::Intersection => index_hint(a, env).or_else(|| index_hint(b, env)),
            SetOp::Difference => index_hint(a, env),
        },
    }
}

/// Where to look for the candidates of a query (feature 007): a performance hint only. Every
/// candidate is re-checked against the query, so a plan may give too many, never too few.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexPlan {
    /// The entities whose `field` equals `value`.
    Eq { field: String, value: Json },
    /// The candidates of either plan.
    Union(Box<IndexPlan>, Box<IndexPlan>),
}

/// Evaluates one expression in `vals` against `facts` on S (feature 009: migration transforms and
/// requirements). A missing fact is reported as such.
pub(crate) fn eval_in(
    module: &Module,
    e: &Expr,
    vals: &Vals,
    facts: &dyn EvaluationFacts,
) -> Result<Value, String> {
    let mut ev = Evaluator::new(module, facts);
    let r = ev.eval(e, vals, Phase::S, &mut None);
    match (r, ev.fact_error.take()) {
        (Err(_), Some(m)) => Err(format!("{m}: the evaluation needs this fact")),
        (r, _) => r,
    }
}

/// Every entity rule of `entity` evaluated on a decoded value (feature 009): its constraints,
/// including the synthesized reference constraints (answered by `facts`), and its invariants.
/// Returns the violated rules as `(rule, message)`, in name order.
pub(crate) fn entity_rule_failures(
    module: &Module,
    entity: &str,
    value: &Value,
    facts: &dyn EvaluationFacts,
) -> Vec<(String, String)> {
    let mut ev = Evaluator::new(module, facts);
    let mut failed = Vec::new();
    for (name, c) in module.constraints_for(entity) {
        let vals = Vals::from([(c.param().to_string(), value.clone())]);
        match ev.predicate(c.body(), &vals, Phase::S).0 {
            Ok(true) => {}
            Ok(false) => failed.push((name.clone(), format!("constraint `{name}` is violated"))),
            Err(e) => failed.push((name.clone(), format!("constraint `{name}`: {e}"))),
        }
    }
    for (name, i) in module.invariants_for(entity) {
        let vals = Vals::from([(i.param().to_string(), value.clone())]);
        match ev.predicate(i.body(), &vals, Phase::S).0 {
            Ok(true) => {}
            Ok(false) => failed.push((name.clone(), format!("invariant `{name}` is violated"))),
            Err(e) => failed.push((name.clone(), format!("invariant `{name}`: {e}"))),
        }
    }
    failed
}

/// The members that violate a quantified requirement (feature 009): for `all(q, p)` the members
/// of `q` where `p` is false, for `not any(q, p)` those where `p` is true. Returns the entity
/// type and the sorted ids, or `None` for any other shape.
pub(crate) fn violators(
    module: &Module,
    e: &Expr,
    vals: &Vals,
    facts: &dyn EvaluationFacts,
) -> Option<Result<(String, Vec<String>), String>> {
    use crate::semantic::expr::FoldOp;
    let (query, body, violating) = match &e.kind {
        ExprKind::Fold {
            op: FoldOp::All,
            query,
            body,
            ..
        } => (query, body, false),
        ExprKind::Not(inner) => match &inner.kind {
            ExprKind::Fold {
                op: FoldOp::Any,
                query,
                body,
                ..
            } => (query, body, true),
            _ => return None,
        },
        _ => return None,
    };
    let mut ev = Evaluator::new(module, facts);
    let t = query.entity().to_string();
    let run = |ev: &mut Evaluator<'_>| -> Result<(String, Vec<String>), String> {
        let members = ev.members(query, vals, Phase::S)?;
        let mut out = Vec::new();
        for id in members {
            ev.candidate = Some((t.clone(), id.clone()));
            let b = ev
                .eval(body, vals, Phase::S, &mut None)?
                .as_bool()
                .ok_or_else(|| "internal: predicate is not Bool".to_string())?;
            if b == violating {
                out.push(id);
            }
        }
        out.sort();
        Ok((t.clone(), out))
    };
    Some(run(&mut ev))
}

/// Checks every module-level invariant on a state described by `facts` alone (feature 007: a
/// genesis seed). Returns the violated invariants, or the evaluation problems.
pub fn check_global_invariants(
    module: &Module,
    facts: &dyn EvaluationFacts,
) -> Result<(), Vec<String>> {
    let mut failed = Vec::new();
    for (name, g) in module.global_invariants() {
        let mut ev = Evaluator::new(module, facts);
        match ev.predicate(g.body(), &Vals::new(), Phase::S).0 {
            Ok(true) => {}
            Ok(false) => failed.push(format!("module invariant `{name}` is violated")),
            Err(e) => failed.push(format!("module invariant `{name}`: {e}")),
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed)
    }
}

/// Evaluates an EvaluationRequest (JSON) against an admitted module.
pub fn evaluate(module: &Module, request: &str) -> DecisionRecord {
    evaluate_observed(module, request).0
}

/// [`evaluate`], plus the entity fields the evaluation actually read, named by the action's
/// parameters: `(param, field)` for every field read, including reads inside derived values,
/// rules, invariants and constraints; operands skipped by short-circuiting are not read
/// (feature 005, research R7). The decision record is identical to [`evaluate`]'s.
pub fn evaluate_observed(module: &Module, request: &str) -> (DecisionRecord, ObservedReads) {
    let (record, observed) = evaluate_inner(module, request, None);
    (record, observed.fields)
}

/// Evaluates a request whose evaluation facts come from `facts` (feature 006: a store answering
/// as of the evaluated position, research R3) instead of a `facts` section, which the request must
/// not have. Returns the record and everything the evaluation observed.
pub fn evaluate_with(
    module: &Module,
    request: &str,
    facts: &dyn EvaluationFacts,
) -> (DecisionRecord, Observed) {
    evaluate_inner(module, request, Some(facts))
}

/// Supplied facts with a module (plain evaluation): incoming references not given explicitly are
/// derived from complete universes of every referencing type (feature 007).
struct Supplied<'a> {
    module: &'a Module,
    facts: &'a Facts,
}

impl EvaluationFacts for Supplied<'_> {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        self.facts.exists(entity, id)
    }
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        self.facts.used(entity, id)
    }
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError> {
        if let Ok(edges) = self.facts.incoming(entity, id) {
            return Ok(edges);
        }
        let mut out = Vec::new();
        for (source, field) in self.module.references_to(entity) {
            let Some(members) = self.facts.universe.get(&source) else {
                return self.facts.incoming(entity, id);
            };
            for (sid, value) in members {
                if value.get(&field).and_then(Json::as_str) == Some(id) {
                    out.push(RefEdge {
                        entity: source.clone(),
                        id: sid.clone(),
                        field: field.clone(),
                    });
                }
            }
        }
        out.sort();
        Ok(out)
    }
    fn query(&self, q: &crate::facts::QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        self.facts.query(q)
    }
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        self.facts.field(entity, id, field)
    }
}

/// Supplied universes, query and field facts must describe one valid state with the bound
/// entities (feature 007, research R5; closed under subsets).
fn check_query_snapshot(
    module: &Module,
    facts: &Facts,
    action: &ActionItem,
    vals: &Vals,
) -> Result<(), crate::facts::FactsProblem> {
    let bad = |message: String| crate::facts::FactsProblem {
        code: "INCONSISTENT_FACTS",
        message,
    };
    let absent = |t: &str, id: &str| {
        let k = (t.to_string(), id.to_string());
        facts.existence.get(&k) == Some(&false) || facts.identities.get(&k) == Some(&false)
    };
    // Bound state entities, by type and id, in their canonical encoding.
    let mut bound: BTreeMap<Key, Json> = BTreeMap::new();
    for p in action.params() {
        if p.role() != ParamRole::State {
            continue;
        }
        if let (Type::Entity(t), Some(v @ Value::Entity(fields))) = (p.ty(), vals.get(p.name()))
            && let Some(id) = fields.get("id").and_then(id_text)
        {
            bound.insert((t.clone(), id), encode_param(module, p.ty(), v));
        }
    }
    for (t, members) in &facts.universe {
        if module.entity(t).is_none() {
            return Err(bad(format!(
                "the universe names an unknown entity type `{t}`"
            )));
        }
        let no_facts = Facts::default();
        for (id, raw) in members {
            let value = decode_value(module, t, raw)
                .map_err(|p| bad(format!("{t} {id} in the universe: {}", p.join("; "))))?;
            if absent(t, id) {
                return Err(bad(format!(
                    "{t} {id} is in the universe but marked absent"
                )));
            }
            // Entity constraints hold for every existing entity (queries are not allowed in
            // them; reference constraints concern the universe and are skipped).
            let mut ev = Evaluator::new(module, &no_facts);
            for (name, c) in module
                .constraints_for(t)
                .filter(|(_, c)| c.reference().is_none())
            {
                let cv = Vals::from([(c.param().to_string(), value.clone())]);
                if ev.predicate(c.body(), &cv, Phase::S).0 == Ok(false) {
                    return Err(bad(format!(
                        "{t} {id} in the universe violates constraint `{name}`"
                    )));
                }
            }
            let canonical = encode_param(module, &Type::Entity(t.clone()), &value);
            if let Some(b) = bound.get(&(t.clone(), id.clone()))
                && *b != canonical
            {
                return Err(bad(format!(
                    "bound {t} {id} differs from its value in the universe"
                )));
            }
        }
        for ((bt, bid), _) in bound.iter().filter(|((bt, _), _)| bt == t) {
            if !members.contains_key(bid) {
                return Err(bad(format!(
                    "bound {bt} {bid} is missing from the universe"
                )));
            }
        }
        for ((et, eid), exists) in &facts.existence {
            if et == t && *exists && !members.contains_key(eid) {
                return Err(bad(format!(
                    "{et} {eid} exists but is missing from the universe"
                )));
            }
        }
    }
    for ((t, id, field), v) in &facts.fields {
        let known = bound
            .get(&(t.clone(), id.clone()))
            .or_else(|| facts.universe.get(t).and_then(|m| m.get(id)));
        if let Some(value) = known
            && value.get(field) != Some(v)
        {
            return Err(bad(format!(
                "the field {t} {id}.{field} contradicts its entity"
            )));
        }
    }
    for (instance, q) in &facts.queries {
        for m in &q.members {
            if absent(&q.entity, m) {
                return Err(bad(format!(
                    "{} {m} is a member of {instance} but marked absent",
                    q.entity
                )));
            }
        }
    }
    check_query_universe_agreement(module, facts)?;
    Ok(())
}

/// The bound state entities of an action with their reference-field values.
fn bound_entities(module: &Module, action: &ActionItem, vals: &Vals) -> Vec<BoundEntity> {
    action
        .params()
        .iter()
        .filter(|p| p.role() == ParamRole::State)
        .filter_map(|p| {
            let (Type::Entity(entity), Some(Value::Entity(fields))) = (p.ty(), vals.get(p.name()))
            else {
                return None;
            };
            let id = fields.get("id").and_then(id_text)?;
            let references = module
                .entity(entity)
                .map(|item| {
                    item.reference_fields()
                        .map(|(f, _)| (f.to_string(), fields.get(f).and_then(id_text)))
                        .collect()
                })
                .unwrap_or_default();
            Some(BoundEntity {
                entity: entity.clone(),
                id,
                references,
            })
        })
        .collect()
}

fn evaluate_inner(
    module: &Module,
    request: &str,
    provider: Option<&dyn EvaluationFacts>,
) -> (DecisionRecord, Observed) {
    let mut problems = Vec::new();
    let req = parse_request(request, &mut problems);
    let action = module.action(&req.action);
    if problems.is_empty() && action.is_none() {
        problems.push(InputProblem {
            code: "UNKNOWN_ACTION",
            path: "action".into(),
            message: format!("unknown action `{}`", req.action),
        });
    }
    let vals = match action {
        Some(a) if problems.is_empty() => decode_sections(module, a, &req.sections, &mut problems),
        _ => None,
    };
    // Supplied facts (plain evaluation) must parse and form a valid snapshot (FR-010g).
    let mut supplied = Facts::default();
    match (&req.facts, provider, action, &vals) {
        (Some(_), Some(_), _, _) => problems.push(InputProblem {
            code: "DECODE_ERROR",
            path: "facts".into(),
            message: "facts come from the evaluation's provider; the request cannot supply them"
                .into(),
        }),
        (Some(raw), None, Some(a), Some(v)) => {
            match Facts::from_json(raw)
                .and_then(|f| check_snapshot(module, &f, &bound_entities(module, a, v)).map(|_| f))
                .and_then(|f| check_query_snapshot(module, &f, a, v).map(|_| f))
            {
                Ok(f) => supplied = f,
                Err(p) => problems.push(InputProblem {
                    code: p.code,
                    path: "facts".into(),
                    message: p.message,
                }),
            }
        }
        _ => {}
    }

    let mut record = Map::new();
    record.insert("record_version".into(), json!(RECORD_VERSION));
    record.insert("behavior_version".into(), json!(module.behavior_version()));
    record.insert("data_version".into(), json!(req.data_version));
    if let Some(g) = &req.git_revision {
        record.insert("git_revision".into(), json!(g));
    }
    let mut action_json = json!({"name": req.action});
    if let (Some(a), Json::Object(m)) = (action, &mut action_json) {
        m.insert("hash".into(), json!(hash_display(a.hash())));
    }
    record.insert("action".into(), action_json);

    let (Some(action), Some(vals), true) = (action, vals, problems.is_empty()) else {
        problems.sort_by(|a, b| a.path.cmp(&b.path));
        let (s, i, c) = echo_sections(&req);
        record.insert("state".into(), s);
        record.insert("input".into(), i);
        record.insert("context".into(), c);
        // A refused request keeps its facts section, so replay refuses it the same way.
        if let Some(f) = &req.facts {
            record.insert("facts".into(), sanitize_floats(f));
            let queries = ["queries", "fields", "universe"]
                .iter()
                .any(|k| f.get(k).is_some());
            let version = if queries {
                RECORD_VERSION_QUERIES
            } else {
                RECORD_VERSION_LIFECYCLE
            };
            record.insert("record_version".into(), json!(version));
        }
        record.insert("result".into(), json!("INVALID_INPUT"));
        let reasons: Vec<Json> = problems
            .iter()
            .map(|p| reason(p.code, format!("{}: {}", p.path, p.message), None))
            .collect();
        record.insert("reasons".into(), Json::Array(reasons));
        record.insert("trace".into(), json!([]));
        record.insert("derived".into(), json!([]));
        record.insert("changes".into(), json!([]));
        return (
            DecisionRecord::new(Json::Object(record)),
            Observed::default(),
        );
    };

    // Normalized echo of the decoded request.
    let mut sections: BTreeMap<&str, Map<String, Json>> = [
        ("state", Map::new()),
        ("input", Map::new()),
        ("context", Map::new()),
    ]
    .into();
    for p in action.params() {
        if let (Some(v), Some(sec)) = (vals.get(p.name()), sections.get_mut(section_name(p.role())))
        {
            sec.insert(p.name().to_string(), encode_param(module, p.ty(), v));
        }
    }
    for (k, v) in sections {
        record.insert(k.into(), Json::Object(v));
    }

    // Binding: distinct state parameters must bind to distinct entity identities (FR-027).
    if let Some(message) = alias(action, &vals) {
        record.insert("result".into(), json!("INVALID_BINDING"));
        record.insert(
            "reasons".into(),
            json!([reason("STATE_ALIAS_NOT_ALLOWED", message, None)]),
        );
        record.insert("trace".into(), json!([]));
        record.insert("derived".into(), json!([]));
        record.insert("changes".into(), json!([]));
        return (
            DecisionRecord::new(Json::Object(record)),
            Observed::default(),
        );
    }

    let supplied = Supplied {
        module,
        facts: &supplied,
    };
    let mut ev = Evaluator::new(module, provider.unwrap_or(&supplied));
    let outcome = run_transition(&mut ev, action, vals);

    let mut derived = std::mem::take(&mut ev.derived);
    derived.sort_by(|a, b| (a.phase, a.order, &a.args_key).cmp(&(b.phase, b.order, &b.args_key)));
    let derived_json: Vec<Json> = derived
        .into_iter()
        .map(|d| {
            json!({"name": d.name, "hash": hash_display(&d.hash), "phase_state": d.phase.label(),
                   "value": d.value})
        })
        .collect();

    record.insert("result".into(), json!(outcome.result));
    record.insert("reasons".into(), Json::Array(outcome.reasons));
    record.insert("trace".into(), Json::Array(outcome.trace));
    record.insert("derived".into(), Json::Array(derived_json));
    record.insert("changes".into(), Json::Array(outcome.changes));
    let facts = std::mem::take(&mut ev.observed);
    if facts.has_query_facts() {
        record.insert("record_version".into(), json!(RECORD_VERSION_QUERIES));
    } else if !facts.is_empty() || !outcome.lifecycle.is_empty() {
        record.insert("record_version".into(), json!(RECORD_VERSION_LIFECYCLE));
    }
    if !facts.is_empty() {
        record.insert("facts".into(), facts.to_json());
    }
    if !outcome.lifecycle.is_empty() {
        record.insert("lifecycle".into(), Json::Array(outcome.lifecycle));
    }
    let fields = ev.frames.pop().map(|f| f.reads).unwrap_or_default();
    (
        DecisionRecord::new(Json::Object(record)),
        Observed { fields, facts },
    )
}

/// Converts an evaluated value for storage in a field of type `field_ty`: an `Exact<F>` stored
/// into `F` is on the grid by admission, converted and range-checked here.
fn stored_value(v: Value, field_ty: &Type, expr: &Expr) -> Result<Value, String> {
    match (&v, fixed_scale(field_ty)) {
        (Value::Exact(x), Some(n)) => fixed_value(x, n).map_err(|err| num_err(err, expr)),
        (Value::Exact(x), None) => x.to_dec().map(Value::Dec).map_err(|err| num_err(err, expr)),
        _ => Ok(v),
    }
}

fn entity_of(p: &crate::semantic::module::Param) -> Option<String> {
    match p.ty() {
        Type::Entity(e) => Some(e.clone()),
        _ => None,
    }
}

fn param_id(vals: &Vals, param: &str) -> Option<String> {
    match vals.get(param) {
        Some(Value::Entity(fields)) => fields.get("id").and_then(id_text),
        _ => None,
    }
}

/// Stops the transition with a lifecycle result (`ENTITY_ID_ALREADY_USED`, `LIFECYCLE_CONFLICT`).
fn refuse(out: &mut Outcome, code: &'static str, message: String, loc: &Loc) {
    out.result = code;
    out.reasons.push(reason(code, message, Some(loc)));
}

fn run_transition(ev: &mut Evaluator<'_>, action: &ActionItem, s: Vals) -> Outcome {
    let module = ev.module;
    let mut out = Outcome {
        result: "ALLOW",
        reasons: Vec::new(),
        trace: Vec::new(),
        changes: Vec::new(),
        lifecycle: Vec::new(),
    };
    let mut stopped = false;

    // State parameters exist in S by binding (feature 006).
    for p in action.params() {
        if p.role() != ParamRole::State {
            continue;
        }
        if let (Some(entity), Some(id)) = (entity_of(p), param_id(&s, p.name())) {
            ev.world.bound.insert((entity, id), p.name().to_string());
        }
    }

    // Single-entity rules in runtime order (research R11): entity constraints on every incoming
    // entity, then state invariants on S; after the effects, state invariants and entity
    // constraints on S'. A reference constraint holds in S for state entities (the store keeps
    // referential integrity), so it is checked only where S' can break it (feature 006).
    let mut incoming = Vec::new();
    let mut state_invariants = Vec::new();
    for p in action.params() {
        let Some(entity) = entity_of(p) else { continue };
        for (name, c) in module.constraints_for(&entity) {
            if p.role() == ParamRole::State && c.reference().is_some() {
                continue;
            }
            incoming.push(RuleCheck::constraint(name, c, p.name(), p.role()));
        }
        if p.role() == ParamRole::State {
            for (name, i) in module.invariants_for(&entity) {
                state_invariants.push(RuleCheck::invariant(name, i, p.name()));
            }
        }
    }

    run_rules(
        ev,
        &mut out,
        &mut stopped,
        &incoming,
        &s,
        Phase::S,
        "constraint",
    );
    if stopped {
        return out;
    }
    run_rules(
        ev,
        &mut out,
        &mut stopped,
        &state_invariants,
        &s,
        Phase::S,
        "invariant_pre",
    );
    if stopped {
        return out;
    }

    let conditions = |ev: &mut Evaluator<'_>,
                      out: &mut Outcome,
                      stopped: &mut bool,
                      conds: &[crate::semantic::module::Condition],
                      vals: &Vals,
                      phase: Phase| {
        let (label, code) = if phase == Phase::S {
            ("precondition", "PRECONDITION_FAILED")
        } else {
            ("postcondition", "POSTCONDITION_FAILED")
        };
        for c in conds {
            let text = pretty::text(&c.expr);
            if *stopped {
                out.trace.push(step(
                    label,
                    None,
                    c.expr.hash(),
                    text,
                    Reads::new(),
                    json!("skipped"),
                    &c.loc,
                ));
                continue;
            }
            let (r, reads) = ev.predicate(&c.expr, vals, phase);
            let outcome = match &r {
                Ok(b) => json!(b),
                Err(msg) => json!({"error": msg}),
            };
            out.trace.push(step(
                label,
                None,
                c.expr.hash(),
                text.clone(),
                reads,
                outcome,
                &c.loc,
            ));
            attach_rescales(ev, &mut out.trace);
            match r {
                Ok(true) => {}
                Ok(false) => {
                    *stopped = true;
                    out.result = "DENY";
                    out.reasons.push(reason(
                        code,
                        format!("{label} failed: {text}"),
                        Some(&c.loc),
                    ));
                }
                Err(msg) => {
                    *stopped = true;
                    out.result = "ERROR";
                    let r = ev.error_reason(msg, &c.loc);
                    out.reasons.push(r);
                }
            }
        }
    };

    conditions(
        ev,
        &mut out,
        &mut stopped,
        &action.preconditions,
        &s,
        Phase::S,
    );
    if stopped {
        return out;
    }

    // Effects: all right-hand sides against S, giving ΔS.
    let mut delta: Vec<(String, String, Value, Json, Json)> = Vec::new();
    for e in &action.effects {
        // The field's type: the value's type, except for an `Exact<F>` stored into `F` (on the
        // grid by admission; converted and range-checked here).
        let field_ty = action
            .params
            .iter()
            .find(|p| p.name() == e.param)
            .and_then(|p| match p.ty() {
                Type::Entity(en) => ev.module.entity(en)?.field_type(&e.field).cloned(),
                _ => None,
            })
            .unwrap_or_else(|| e.value.ty().clone());
        let text = format!("{}.{} := {}", e.param, e.field, pretty::text(&e.value));
        let mut reads = Reads::new();
        let result = ev
            .eval(&e.value, &s, Phase::S, &mut Some(&mut reads))
            .and_then(|v| stored_value(v, &field_ty, &e.value));
        match result {
            Ok(v) => {
                let new = encode(&field_ty, &v);
                let old = match s.get(&e.param) {
                    Some(Value::Entity(f)) => f
                        .get(&e.field)
                        .map(|v| encode(&field_ty, v))
                        .unwrap_or(Json::Null),
                    _ => Json::Null,
                };
                out.trace.push(step(
                    "effect",
                    None,
                    &e.hash,
                    text,
                    reads,
                    json!({"assigned": new}),
                    &e.loc,
                ));
                attach_rescales(ev, &mut out.trace);
                delta.push((e.param.clone(), e.field.clone(), v, old, new));
            }
            Err(msg) => {
                out.trace.push(step(
                    "effect",
                    None,
                    &e.hash,
                    text,
                    reads,
                    json!({"error": msg}),
                    &e.loc,
                ));
                attach_rescales(ev, &mut out.trace);
                out.result = "ERROR";
                let r = ev.error_reason(msg, &e.loc);
                out.reasons.push(r);
                return out;
            }
        }
    }

    // Creations (feature 006): the identity and the complete initial value, against S.
    let mut created: Vec<(Key, Value, &crate::semantic::module::CreateEffect)> = Vec::new();
    for c in &action.creates {
        let args: Vec<String> = std::iter::once(format!("id={}", pretty::text(&c.id)))
            .chain(
                c.fields
                    .iter()
                    .map(|(f, v)| format!("{f}={}", pretty::text(v))),
            )
            .collect();
        let text = format!("create {}({})", c.entity, args.join(", "));
        let mut reads = Reads::new();
        let mut value = || -> Result<(String, Value), String> {
            let id = ev.eval(&c.id, &s, Phase::S, &mut Some(&mut reads))?;
            let id =
                id_text(&id).ok_or_else(|| "internal: identity is not a string".to_string())?;
            let item = ev
                .module
                .entity(&c.entity)
                .ok_or_else(|| "internal: unknown entity".to_string())?;
            let mut fields = BTreeMap::from([("id".to_string(), Value::Str(id.clone()))]);
            for (f, e) in &c.fields {
                let fty = item
                    .field_type(f)
                    .cloned()
                    .unwrap_or_else(|| e.ty().clone());
                let v = ev.eval(e, &s, Phase::S, &mut Some(&mut reads))?;
                fields.insert(f.clone(), stored_value(v, &fty, e)?);
            }
            Ok((id, Value::Entity(fields)))
        };
        match value() {
            Ok((id, v)) => {
                out.trace.push(step(
                    "create",
                    None,
                    &c.hash,
                    text,
                    reads,
                    json!({"created": {"entity": c.entity, "id": id}}),
                    &c.loc,
                ));
                attach_rescales(ev, &mut out.trace);
                created.push(((c.entity.clone(), id), v, c));
            }
            Err(msg) => {
                out.trace.push(step(
                    "create",
                    None,
                    &c.hash,
                    text,
                    reads,
                    json!({"error": msg}),
                    &c.loc,
                ));
                attach_rescales(ev, &mut out.trace);
                out.result = "ERROR";
                let r = ev.error_reason(msg, &c.loc);
                out.reasons.push(r);
                return out;
            }
        }
    }
    let mut removed: Vec<(Key, &str, &crate::semantic::module::RemoveEffect)> = Vec::new();
    for r in &action.removes {
        let entity = action
            .params()
            .iter()
            .find(|p| p.name() == r.param)
            .and_then(entity_of)
            .unwrap_or_default();
        let id = param_id(&s, &r.param).unwrap_or_default();
        out.trace.push(step(
            "remove",
            None,
            &r.hash,
            format!("remove {}", r.param),
            Reads::new(),
            json!({"removed": {"entity": entity, "id": id}}),
            &r.loc,
        ));
        removed.push(((entity, id), &r.param, r));
    }

    // At most one lifecycle operation per typed identity (FR-005, research R2).
    let mut touched: BTreeSet<&Key> = removed.iter().map(|(k, _, _)| k).collect();
    for (k, _, c) in &created {
        if !touched.insert(k) {
            refuse(
                &mut out,
                "LIFECYCLE_CONFLICT",
                format!(
                    "{} {} has more than one lifecycle operation in this transition",
                    k.0, k.1
                ),
                &c.loc,
            );
            return out;
        }
    }
    // An identity names one lifetime: a creation needs an identity the store never used (FR-007).
    for (k, _, c) in &created {
        match ev.used_s(&k.0, &k.1) {
            Ok(false) => {}
            Ok(true) => {
                refuse(
                    &mut out,
                    "ENTITY_ID_ALREADY_USED",
                    format!(
                        "{} {} was already used; an identity names one lifetime, create a new one",
                        k.0, k.1
                    ),
                    &c.loc,
                );
                return out;
            }
            Err(msg) => {
                out.result = "ERROR";
                let r = ev.error_reason(msg, &c.loc);
                out.reasons.push(r);
                return out;
            }
        }
    }

    // S' = apply(S, ΔS)
    let mut s_prime = s.clone();
    for (param, field, v, _, _) in &delta {
        if let Some(Value::Entity(fields)) = s_prime.get_mut(param) {
            fields.insert(field.clone(), v.clone());
        }
    }
    ev.world.s_prime = s_prime.clone();
    ev.world.removed = removed.iter().map(|(k, _, _)| k.clone()).collect();
    ev.world.created = created
        .iter()
        .map(|(k, v, _)| (k.clone(), v.clone()))
        .collect();

    // Outgoing single-entity rules: bound entities that survive, then created entities. A
    // reference constraint of a bound entity is checked only if an effect assigns its field
    // (statically known, so the verifier follows the same steps).
    let removed_params: BTreeSet<&str> = removed.iter().map(|(_, p, _)| *p).collect();
    let changed: BTreeSet<(&str, &str)> = action
        .effects
        .iter()
        .map(|e| (e.param.as_str(), e.field.as_str()))
        .collect();
    let mut post_invariants: Vec<RuleCheck> = state_invariants
        .iter()
        .filter(|c| !removed_params.contains(c.bound.as_str()))
        .cloned()
        .collect();
    let mut post_constraints = Vec::new();
    for p in action.params() {
        if p.role() != ParamRole::State || removed_params.contains(p.name()) {
            continue;
        }
        let Some(entity) = entity_of(p) else { continue };
        for (name, c) in module.constraints_for(&entity) {
            if let Some(f) = c.reference()
                && !changed.contains(&(p.name(), f))
            {
                continue;
            }
            post_constraints.push(RuleCheck::constraint(name, c, p.name(), p.role()));
        }
    }
    for ((entity, id), v, _) in &created {
        let label = format!("{entity}#{id}");
        s_prime.insert(label.clone(), v.clone());
        for (name, i) in module.invariants_for(entity) {
            let mut check = RuleCheck::invariant(name, i, &label);
            check.created = true;
            post_invariants.push(check);
        }
        for (name, c) in module.constraints_for(entity) {
            let mut check = RuleCheck::constraint(name, c, &label, ParamRole::State);
            check.created = true;
            post_constraints.push(check);
        }
    }

    conditions(
        ev,
        &mut out,
        &mut stopped,
        &action.postconditions,
        &s_prime,
        Phase::SPrime,
    );
    run_rules(
        ev,
        &mut out,
        &mut stopped,
        &post_invariants,
        &s_prime,
        Phase::SPrime,
        "invariant_post",
    );
    run_rules(
        ev,
        &mut out,
        &mut stopped,
        &post_constraints,
        &s_prime,
        Phase::SPrime,
        "constraint_post",
    );
    // Module-level invariants on S' (feature 007, research R7): every invariant the transition
    // can affect, by a sound dependency analysis; they hold on the valid S.
    for (name, g) in module.global_invariants() {
        if !affects(module, action, g.signature()) {
            continue;
        }
        let text = pretty::text(g.body());
        if stopped {
            out.trace.push(step(
                "invariant_global",
                Some(name),
                g.hash(),
                text,
                Reads::new(),
                json!("skipped"),
                g.loc(),
            ));
            continue;
        }
        ev.assume_valid = held_uniques(g.body());
        let (r, reads) = ev.predicate(g.body(), &s_prime, Phase::SPrime);
        ev.assume_valid.clear();
        let outcome = match &r {
            Ok(b) => json!(b),
            Err(msg) => json!({"error": msg}),
        };
        out.trace.push(step(
            "invariant_global",
            Some(name),
            g.hash(),
            text,
            reads,
            outcome,
            g.loc(),
        ));
        match r {
            Ok(true) => {}
            Ok(false) => {
                stopped = true;
                out.result = "DENY";
                out.reasons.push(reason(
                    "INVARIANT_VIOLATED",
                    format!("module invariant `{name}` does not hold on the resulting state"),
                    Some(g.loc()),
                ));
            }
            Err(msg) => {
                stopped = true;
                out.result = "ERROR";
                let why = ev.error_reason(msg, g.loc());
                out.reasons.push(why);
            }
        }
    }
    if stopped {
        return out;
    }

    // Referential integrity on S' (research R6): no surviving `Ref` to a removed identity.
    for ((entity, id), _, r) in &removed {
        if module.references_to(entity).is_empty() {
            continue;
        }
        match ev.incoming_prime(entity, id) {
            Ok(edges) if edges.is_empty() => {}
            Ok(edges) => {
                out.result = "DENY";
                let list: Vec<String> = edges
                    .iter()
                    .map(|e| format!("{}.{} of {} {}", e.entity, e.field, e.entity, e.id))
                    .collect();
                let mut why = reason(
                    "DANGLING_REFERENCE",
                    format!(
                        "removing {entity} {id} leaves references to it: {}",
                        list.join(", ")
                    ),
                    Some(&r.loc),
                );
                if let Json::Object(m) = &mut why {
                    m.insert(
                        "references".into(),
                        Json::Array(edges.iter().map(RefEdge::to_json).collect()),
                    );
                }
                out.reasons.push(why);
                return out;
            }
            Err(msg) => {
                out.result = "ERROR";
                let why = ev.error_reason(msg, &r.loc);
                out.reasons.push(why);
                return out;
            }
        }
    }

    out.changes = delta
        .into_iter()
        .map(|(param, field, _, old, new)| {
            let entity = action
                .params()
                .iter()
                .find(|p| p.name() == param)
                .and_then(entity_of)
                .unwrap_or_default();
            let id = match s.get(&param) {
                Some(Value::Entity(fields)) => fields
                    .get("id")
                    .map(crate::semantic::value::encode_untyped)
                    .unwrap_or(Json::Null),
                _ => Json::Null,
            };
            json!({"param": param, "entity": entity, "id": id, "field": field, "old": old, "new": new})
        })
        .collect();
    let ty = |entity: &str| Type::Entity(entity.to_string());
    for ((entity, id), v, _) in &created {
        out.lifecycle
            .push(json!({"op": "create", "entity": entity, "id": id,
                                  "value": encode_param(module, &ty(entity), v)}));
    }
    for ((entity, id), param, _) in &removed {
        let value = s
            .get(*param)
            .map(|v| encode_param(module, &ty(entity), v))
            .unwrap_or(Json::Null);
        out.lifecycle
            .push(json!({"op": "remove", "entity": entity, "id": id, "value": value}));
    }
    out
}

/// One single-entity rule (entity constraint or state invariant) bound to an action parameter,
/// or to an entity created by the transition.
#[derive(Clone)]
struct RuleCheck {
    name: String,
    hash: Hash,
    body: Expr,
    rule_param: String,
    bound: String,
    role: ParamRole,
    constraint: bool,
    /// Bound to a created entity: its field reads are not state reads.
    created: bool,
}

impl RuleCheck {
    fn constraint(
        name: &str,
        c: &crate::semantic::module::ConstraintItem,
        bound: &str,
        role: ParamRole,
    ) -> Self {
        RuleCheck {
            name: name.to_string(),
            hash: *c.hash(),
            body: c.body().clone(),
            rule_param: c.param().to_string(),
            bound: bound.to_string(),
            role,
            constraint: true,
            created: false,
        }
    }

    fn invariant(name: &str, i: &crate::semantic::module::InvariantItem, bound: &str) -> Self {
        RuleCheck {
            name: name.to_string(),
            hash: *i.hash(),
            body: i.body().clone(),
            rule_param: i.param().to_string(),
            bound: bound.to_string(),
            role: ParamRole::State,
            constraint: false,
            created: false,
        }
    }
}

fn role_name(role: ParamRole) -> &'static str {
    match role {
        ParamRole::State | ParamRole::Read => "state",
        ParamRole::Input => "input",
        ParamRole::Context => "context",
    }
}

/// Runs rule checks in order; the first failure stops evaluation (later checks are `skipped`).
#[allow(clippy::too_many_arguments)]
fn run_rules(
    ev: &mut Evaluator<'_>,
    out: &mut Outcome,
    stopped: &mut bool,
    checks: &[RuleCheck],
    state: &Vals,
    phase: Phase,
    label: &str,
) {
    for c in checks {
        let text = pretty::text(&c.body);
        let loc = c.body.loc().clone();
        let annotate = |mut step_json: Json| {
            if let Json::Object(m) = &mut step_json {
                if c.constraint {
                    m.insert("param".into(), json!(c.bound));
                }
                if label == "constraint" {
                    m.insert("role".into(), json!(role_name(c.role)));
                }
            }
            step_json
        };
        if *stopped {
            let skipped = step(
                label,
                Some(&c.name),
                &c.hash,
                text,
                Reads::new(),
                json!("skipped"),
                &loc,
            );
            out.trace.push(annotate(skipped));
            continue;
        }
        let mut vals = Vals::new();
        if let Some(v) = state.get(&c.bound) {
            vals.insert(c.rule_param.clone(), v.clone());
        }
        let to_outer = if c.created {
            BTreeMap::new()
        } else {
            BTreeMap::from([(c.rule_param.clone(), c.bound.clone())])
        };
        ev.push_frame(to_outer);
        let (r, reads) = ev.predicate(&c.body, &vals, phase);
        ev.pop_frame();
        let outcome = match &r {
            Ok(b) => json!(b),
            Err(msg) => json!({"error": msg}),
        };
        out.trace.push(annotate(step(
            label,
            Some(&c.name),
            &c.hash,
            text,
            reads,
            outcome,
            &loc,
        )));
        attach_rescales(ev, &mut out.trace);
        match r {
            Ok(true) => {}
            Ok(false) => {
                *stopped = true;
                out.result = match (label, c.role) {
                    ("constraint", ParamRole::Input) => "INVALID_INPUT",
                    ("constraint", ParamRole::Context) => "INVALID_CONTEXT",
                    ("constraint", _) | ("invariant_pre", _) => "INVALID_STATE",
                    _ => "DENY",
                };
                let (code, kind) = if c.constraint {
                    ("CONSTRAINT_VIOLATED", "constraint")
                } else {
                    ("INVARIANT_VIOLATED", "invariant")
                };
                out.reasons.push(reason(
                    code,
                    format!("{kind} `{}` does not hold for `{}`", c.name, c.bound),
                    Some(&loc),
                ));
            }
            Err(msg) => {
                *stopped = true;
                out.result = "ERROR";
                let r = ev.error_reason(msg, &loc);
                out.reasons.push(r);
            }
        }
    }
}

/// The first pair of state parameters bound to the same entity (type and id), if any.
fn alias(action: &ActionItem, vals: &Vals) -> Option<String> {
    let states: Vec<(&str, &str, &Value)> = action
        .params()
        .iter()
        .filter(|p| p.role() == ParamRole::State)
        .filter_map(|p| match (p.ty(), vals.get(p.name())) {
            (Type::Entity(e), Some(Value::Entity(fields))) => {
                fields.get("id").map(|id| (p.name(), e.as_str(), id))
            }
            _ => None,
        })
        .collect();
    for (i, (a, ea, ida)) in states.iter().enumerate() {
        for (b, eb, idb) in &states[i + 1..] {
            if ea == eb && ida == idb {
                let id = crate::semantic::value::encode_untyped(ida);
                return Some(format!(
                    "`{a}` and `{b}` are bound to the same {ea} {id}; state parameters must be distinct"
                ));
            }
        }
    }
    None
}
