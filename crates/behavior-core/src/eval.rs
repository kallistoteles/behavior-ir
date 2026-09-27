//! Deterministic evaluation of a transition `T : S × I × C → Result<ΔS, O, Trace>`
//! (research R6, R7; contracts/engine-api.md).
//!
//! Order: input check → invariants on S → preconditions → effects (ΔS against S) →
//! `S' = apply(S, ΔS)` → postconditions and invariants on S'. Any failure returns no change.

use std::collections::BTreeMap;

use serde_json::{Map, Value as Json, json};

use crate::decimal::{Dec, NumError, fixed_text};
use crate::exact::Exact;
use crate::pretty;
use crate::record::DecisionRecord;
use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::module::{ActionItem, Module, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Hash, Type, fixed_scale, hash_display};
use crate::semantic::value::{Value, decode_scalar, encode};
use crate::wire::Loc;

/// Version of the decision record format (0.4: exact arithmetic closure, feature 004).
pub const RECORD_VERSION: &str = "0.4";

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
fn encode_param(module: &Module, ty: &Type, v: &Value) -> Json {
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

type Vals = BTreeMap<String, Value>;
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

struct Evaluator<'a> {
    module: &'a Module,
    memo: BTreeMap<(String, String, Phase), Value>,
    derived: Vec<DerivedEntry>,
    /// Rescale entries of the step being evaluated (attached to its trace step).
    rescales: Vec<Json>,
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
            ExprKind::Field { param, field } => {
                let v = match vals.get(param) {
                    Some(Value::Entity(fields)) => fields.get(field).cloned().ok_or_else(bad)?,
                    _ => return Err(bad()),
                };
                if let Some(r) = reads.as_deref_mut() {
                    r.insert(format!("{param}.{field}"), encode(&e.ty, &v));
                }
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
                    let v = vals.get(a).cloned().ok_or_else(bad)?;
                    arg_values.push(crate::semantic::value::encode_untyped(&v));
                    inner.insert(p.name().to_string(), v);
                }
                let args_key = Json::Array(arg_values).to_string();
                let key = (name.clone(), args_key.clone(), phase);
                let v = match self.memo.get(&key) {
                    Some(v) => v.clone(),
                    None => {
                        let body = d.body().clone();
                        let v = self.eval(&body, &inner, phase, &mut None)?;
                        self.memo.insert(key, v.clone());
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
}

fn parse_request(text: &str, problems: &mut Vec<InputProblem>) -> Request {
    let mut req = Request {
        action: String::new(),
        data_version: String::new(),
        git_revision: None,
        sections: BTreeMap::new(),
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
}

/// Evaluates an EvaluationRequest (JSON) against an admitted module.
pub fn evaluate(module: &Module, request: &str) -> DecisionRecord {
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

    let (Some(action), Some(vals)) = (action, vals) else {
        problems.sort_by(|a, b| a.path.cmp(&b.path));
        let (s, i, c) = echo_sections(&req);
        record.insert("state".into(), s);
        record.insert("input".into(), i);
        record.insert("context".into(), c);
        record.insert("result".into(), json!("INVALID_INPUT"));
        let reasons: Vec<Json> = problems
            .iter()
            .map(|p| reason(p.code, format!("{}: {}", p.path, p.message), None))
            .collect();
        record.insert("reasons".into(), Json::Array(reasons));
        record.insert("trace".into(), json!([]));
        record.insert("derived".into(), json!([]));
        record.insert("changes".into(), json!([]));
        return DecisionRecord::new(Json::Object(record));
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
        return DecisionRecord::new(Json::Object(record));
    }

    let mut ev = Evaluator {
        module,
        memo: BTreeMap::new(),
        derived: Vec::new(),
        rescales: Vec::new(),
    };
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
    DecisionRecord::new(Json::Object(record))
}

fn run_transition(ev: &mut Evaluator<'_>, action: &ActionItem, s: Vals) -> Outcome {
    let module = ev.module;
    let mut out = Outcome {
        result: "ALLOW",
        reasons: Vec::new(),
        trace: Vec::new(),
        changes: Vec::new(),
    };
    let mut stopped = false;

    // Single-entity rules in runtime order (research R11): entity constraints on every incoming
    // entity, then state invariants on S; after the effects, state invariants and entity
    // constraints on S'.
    let entity_of = |p: &crate::semantic::module::Param| match p.ty() {
        Type::Entity(e) => Some(e.clone()),
        _ => None,
    };
    let mut incoming = Vec::new();
    let mut state_invariants = Vec::new();
    let mut state_constraints = Vec::new();
    for p in action.params() {
        let Some(entity) = entity_of(p) else { continue };
        for (name, c) in module.constraints_for(&entity) {
            let check = RuleCheck {
                name: name.clone(),
                hash: *c.hash(),
                body: c.body().clone(),
                rule_param: c.param().to_string(),
                bound: p.name().to_string(),
                role: p.role(),
                constraint: true,
            };
            if p.role() == ParamRole::State {
                state_constraints.push(check.clone());
            }
            incoming.push(check);
        }
        if p.role() == ParamRole::State {
            for (name, i) in module.invariants_for(&entity) {
                state_invariants.push(RuleCheck {
                    name: name.clone(),
                    hash: *i.hash(),
                    body: i.body().clone(),
                    rule_param: i.param().to_string(),
                    bound: p.name().to_string(),
                    role: p.role(),
                    constraint: false,
                });
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
                    out.reasons
                        .push(reason("EVALUATION_ERROR", msg, Some(&c.loc)));
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
            .and_then(|v| match (&v, fixed_scale(&field_ty)) {
                (Value::Exact(x), Some(n)) => {
                    fixed_value(x, n).map_err(|err| num_err(err, &e.value))
                }
                (Value::Exact(x), None) => x
                    .to_dec()
                    .map(Value::Dec)
                    .map_err(|err| num_err(err, &e.value)),
                _ => Ok(v),
            });
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
                out.reasons
                    .push(reason("EVALUATION_ERROR", msg, Some(&e.loc)));
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
        &state_invariants,
        &s_prime,
        Phase::SPrime,
        "invariant_post",
    );
    run_rules(
        ev,
        &mut out,
        &mut stopped,
        &state_constraints,
        &s_prime,
        Phase::SPrime,
        "constraint_post",
    );
    if stopped {
        return out;
    }
    out.changes = delta
        .into_iter()
        .map(|(param, field, _, old, new)| {
            let entity = action
                .params()
                .iter()
                .find(|p| p.name() == param)
                .and_then(|p| match p.ty() {
                    Type::Entity(e) => Some(e.clone()),
                    _ => None,
                })
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
    out
}

/// One single-entity rule (entity constraint or state invariant) bound to an action parameter.
#[derive(Clone)]
struct RuleCheck {
    name: String,
    hash: Hash,
    body: Expr,
    rule_param: String,
    bound: String,
    role: ParamRole,
    constraint: bool,
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
        let (r, reads) = ev.predicate(&c.body, &vals, phase);
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
                out.reasons
                    .push(reason("EVALUATION_ERROR", msg, Some(&loc)));
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
