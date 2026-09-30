//! Canonical wire JSON of an admitted module (research R17; contracts/ir-encoding.md).
//!
//! Only admitted semantic IR can be serialized. The output shows every conversion explicitly
//! (`some`, `to_decimal`), keeps folded literals, and lists declarations by name, so admitting
//! it again yields the same behavior version and serializing again yields the same bytes.

use serde_json::{Map, Value as Json, json};

use crate::canonical;
use crate::semantic::expr::{CANDIDATE, Expr, ExprKind, QueryKind, QueryNode};
use crate::semantic::module::{Module, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Type, ops};
use crate::semantic::value::encode;
use crate::wire::{DerivedKind, IR_VERSION_EXACT, IR_VERSION_LIFECYCLE, IR_VERSION_QUERIES, Loc};

fn loc(l: &Loc) -> Json {
    json!({"file": l.file, "line": l.line})
}

fn op_node(op: &str, args: Vec<Json>, l: &Loc) -> Json {
    json!({"op": op, "args": args, "loc": loc(l)})
}

fn cmp_name(c: CmpOp) -> &'static str {
    match c {
        CmpOp::Eq => "eq",
        CmpOp::Ne => "ne",
        CmpOp::Lt => "lt",
        CmpOp::Le => "le",
        CmpOp::Gt => "gt",
        CmpOp::Ge => "ge",
    }
}

fn arith_name(a: ArithOp) -> &'static str {
    match a {
        ArithOp::Add => "add",
        ArithOp::Sub => "sub",
        ArithOp::Mul => "mul",
        ArithOp::Div => "div",
    }
}

fn expr(e: &Expr) -> Json {
    expr_in(e, None)
}

/// The wire form of a query (feature 007).
fn query(q: &QueryNode) -> Json {
    let l = q.loc();
    match q.kind() {
        QueryKind::Select => json!({"op": "select", "entity": q.entity(), "loc": loc(l)}),
        QueryKind::Where { base, param, body } => json!({
            "op": "where", "args": [query(base)], "param": param,
            "body": expr_in(body, Some(param)), "loc": loc(l),
        }),
        QueryKind::Set { op, a, b } => op_node(op.as_str(), vec![query(a), query(b)], l),
    }
}

/// [`expr`] inside a lambda body whose candidate is written as `cand`.
fn expr_in(e: &Expr, cand: Option<&str>) -> Json {
    let l = e.loc();
    match e.kind() {
        ExprKind::Lit(v) => {
            json!({"op": "lit", "type": e.ty().to_wire_json(), "value": encode(e.ty(), v), "loc": loc(l)})
        }
        ExprKind::Field { param, field } => {
            let param = if param == CANDIDATE {
                cand.unwrap_or(CANDIDATE)
            } else {
                param
            };
            json!({"op": "field", "param": param, "field": field, "loc": loc(l)})
        }
        ExprKind::Param(p) => json!({"op": "param", "param": p, "loc": loc(l)}),
        ExprKind::DerivedRef { name, args, .. } => {
            let args: Vec<&str> = args
                .iter()
                .map(|a| {
                    if a == CANDIDATE {
                        cand.unwrap_or(CANDIDATE)
                    } else {
                        a.as_str()
                    }
                })
                .collect();
            json!({"op": "derived", "name": name, "args": args, "loc": loc(l)})
        }
        ExprKind::Cmp(c, a, b) => {
            op_node(cmp_name(*c), vec![expr_in(a, cand), expr_in(b, cand)], l)
        }
        ExprKind::Arith(o, a, b) => {
            op_node(arith_name(*o), vec![expr_in(a, cand), expr_in(b, cand)], l)
        }
        ExprKind::And(xs) => op_node("and", xs.iter().map(|x| expr_in(x, cand)).collect(), l),
        ExprKind::Or(xs) => op_node("or", xs.iter().map(|x| expr_in(x, cand)).collect(), l),
        ExprKind::Not(a) => op_node("not", vec![expr_in(a, cand)], l),
        ExprKind::In(a, values) => json!({
            "op": "in",
            "args": [expr_in(a, cand)],
            "values": values.iter().map(|v| encode(a.ty(), v)).collect::<Vec<_>>(),
            "loc": loc(l),
        }),
        ExprKind::IsNone(a) => op_node("is_none", vec![expr_in(a, cand)], l),
        ExprKind::IsSome(a) => op_node("is_some", vec![expr_in(a, cand)], l),
        ExprKind::ValueOr(a, d) => op_node("value_or", vec![expr_in(a, cand), expr_in(d, cand)], l),
        ExprKind::Some(a) => op_node("some", vec![expr_in(a, cand)], l),
        ExprKind::ToDecimal(a) => op_node("to_decimal", vec![expr_in(a, cand)], l),
        ExprKind::Unwrap(a) => op_node("unwrap", vec![expr_in(a, cand)], l),
        ExprKind::Rescale { arg, rounding } => json!({
            "op": "rescale",
            "nominal": match e.ty() {
                Type::Nominal(n) => n.name.clone(),
                other => other.to_string(),
            },
            "rounding": rounding.as_str(),
            "args": [expr_in(arg, cand)],
            "loc": loc(l),
        }),
        ExprKind::Wrap(a) => {
            let nominal = match e.ty() {
                Type::Nominal(n) => n.name.clone(),
                other => other.to_string(),
            };
            json!({"op": "wrap", "nominal": nominal, "args": [expr_in(a, cand)], "loc": loc(l)})
        }
        ExprKind::Exists(a) => op_node("exists", vec![expr_in(a, cand)], l),
        ExprKind::Referenced(a) => op_node("referenced", vec![expr_in(a, cand)], l),
        ExprKind::Count(q) => op_node("count", vec![query(q)], l),
        ExprKind::Fold {
            op,
            query: q,
            param,
            body,
        } => json!({
            "op": op.as_str(), "args": [query(q)], "param": param,
            "body": expr_in(body, Some(param)), "loc": loc(l),
        }),
    }
}

/// The wire type of a field: `Ref<T>` for a reference field (feature 006).
fn field_type(ty: &Type, reference: bool) -> Json {
    match (ty, reference) {
        (Type::Id(e), true) => json!({"t": "ref", "entity": e}),
        (Type::Option(inner), true) => json!({"t": "option", "of": field_type(inner, true)}),
        (t, _) => t.to_wire_json(),
    }
}

fn params(ps: &[crate::semantic::module::Param], with_role: bool) -> Json {
    Json::Array(
        ps.iter()
            .map(|p| {
                let mut o = Map::new();
                o.insert("name".into(), json!(p.name()));
                o.insert("type".into(), p.ty().to_wire_json());
                let role = match p.role() {
                    ParamRole::State => Some("state"),
                    ParamRole::Input => Some("input"),
                    ParamRole::Context => Some("context"),
                    ParamRole::Read => None,
                };
                if let (true, Some(r)) = (with_role, role) {
                    o.insert("role".into(), json!(r));
                }
                Json::Object(o)
            })
            .collect(),
    )
}

fn op_names(bits: u8) -> Vec<&'static str> {
    // Sorted by name, as in canonical emission.
    [
        ("add", ops::ADD),
        ("order", ops::ORDER),
        ("ratio", ops::RATIO),
        ("scale", ops::SCALE),
    ]
    .into_iter()
    .filter(|(_, b)| bits & b != 0)
    .map(|(n, _)| n)
    .collect()
}

/// The wire JSON value of an admitted module.
pub fn to_wire_value(m: &Module) -> Json {
    let unknown = Loc {
        file: "<unknown>".into(),
        line: 1,
    };
    let enums: Vec<Json> = m
        .enums
        .values()
        .map(|e| {
            let l = m.enum_locs.get(&e.name).unwrap_or(&unknown);
            json!({"name": e.name, "values": e.values, "loc": loc(l)})
        })
        .collect();
    let nominals: Vec<Json> = m
        .nominals
        .values()
        .map(|n| {
            let l = m.nominal_locs.get(&n.name).unwrap_or(&unknown);
            let mut o = json!({"name": n.name, "underlying": n.underlying.to_type().to_wire_json(),
                   "ops": op_names(n.ops), "loc": loc(l)});
            if let (Some(sc), Json::Object(map)) = (n.scale, &mut o) {
                map.insert("scale".into(), json!(sc));
            }
            o
        })
        .collect();
    let entities: Vec<Json> = m
        .entities
        .values()
        .map(|e| {
            let fields: Vec<Json> = e
                .fields
                .iter()
                .skip(1) // the implicit `id`
                .zip(&e.field_locs)
                .map(|((name, ty), l)| {
                    json!({"name": name, "type": field_type(ty, e.is_reference(name)), "loc": loc(l)})
                })
                .collect();
            json!({"name": e.name, "fields": fields, "loc": loc(&e.loc)})
        })
        .collect();
    let derived: Vec<Json> = m
        .derived
        .iter()
        .map(|(name, d)| {
            let kind = if d.kind == DerivedKind::Rule {
                "rule"
            } else {
                "derived"
            };
            let mut o = json!({"name": name, "kind": kind, "params": params(&d.params, false),
                   "body": expr(&d.body), "loc": loc(&d.loc)});
            if let (Some(t), Json::Object(map)) = (&d.declared, &mut o) {
                map.insert("type".into(), t.to_wire_json());
            }
            o
        })
        .collect();
    let mut invariants: Vec<Json> = m
        .invariants
        .iter()
        .map(|(name, i)| {
            json!({"name": name, "entity": i.entity, "param": i.param,
                   "body": expr(&i.body), "loc": loc(&i.loc)})
        })
        .collect();
    // Module-level invariants (feature 007) have no entity or parameter.
    invariants.extend(
        m.global_invariants
            .iter()
            .map(|(name, g)| json!({"name": name, "body": expr(&g.body), "loc": loc(&g.loc)})),
    );
    let constraints: Vec<Json> = m
        .constraints
        .iter()
        .filter(|(_, c)| c.reference.is_none())
        .map(|(name, c)| {
            json!({"name": name, "entity": c.entity, "param": c.param,
                   "body": expr(&c.body), "loc": loc(&c.loc)})
        })
        .collect();
    let actions: Vec<Json> = m
        .actions
        .iter()
        .map(|(name, a)| {
            let cond = |c: &crate::semantic::module::Condition| {
                json!({"expr": expr(&c.expr), "loc": loc(&c.loc)})
            };
            let mut effects: Vec<Json> = a
                .effects
                .iter()
                .map(|e| {
                    json!({
                        "target": {"param": e.param, "field": e.field},
                        "value": expr(&e.value),
                        "loc": loc(&e.loc),
                    })
                })
                .collect();
            effects.extend(a.creates.iter().map(|c| {
                let fields: Map<String, Json> =
                    c.fields.iter().map(|(f, v)| (f.clone(), expr(v))).collect();
                json!({"create": c.entity, "id": expr(&c.id), "fields": fields, "loc": loc(&c.loc)})
            }));
            effects.extend(
                a.removes
                    .iter()
                    .map(|r| json!({"remove": r.param, "loc": loc(&r.loc)})),
            );
            json!({
                "name": name,
                "params": params(&a.params, true),
                "preconditions": a.preconditions.iter().map(cond).collect::<Vec<_>>(),
                "effects": effects,
                "postconditions": a.postconditions.iter().map(cond).collect::<Vec<_>>(),
                "loc": loc(&a.loc),
            })
        })
        .collect();
    let doc = json!({
        "ir_version": if uses_queries(m) {
            IR_VERSION_QUERIES
        } else if uses_lifecycle(m) {
            IR_VERSION_LIFECYCLE
        } else {
            IR_VERSION_EXACT
        },
        "enums": enums,
        "nominals": nominals,
        "entities": entities,
        "derived": derived,
        "invariants": invariants,
        "actions": actions,
        "constraints": constraints,
    });
    doc
}

fn expr_uses_fixed_scale(e: &Expr) -> bool {
    matches!(e.ty(), Type::Exact(_))
        || matches!(e.kind(), ExprKind::Rescale { .. })
        || e.children().into_iter().any(expr_uses_fixed_scale)
}

/// Whether the module uses a wire 0.3 feature (fixed scale, exact types, rescale, declared types).
pub fn uses_fixed_scale(m: &Module) -> bool {
    m.nominals.values().any(|n| n.scale.is_some())
        || m.derived
            .values()
            .any(|d| d.declared.is_some() || expr_uses_fixed_scale(&d.body))
        || m.invariants
            .values()
            .any(|i| expr_uses_fixed_scale(&i.body))
        || m.constraints
            .values()
            .any(|c| expr_uses_fixed_scale(&c.body))
        || m.actions.values().any(|a| {
            a.preconditions
                .iter()
                .chain(&a.postconditions)
                .any(|c| expr_uses_fixed_scale(&c.expr))
                || a.effects.iter().any(|e| expr_uses_fixed_scale(&e.value))
        })
}

fn expr_uses_lifecycle(e: &Expr) -> bool {
    matches!(e.kind(), ExprKind::Exists(_) | ExprKind::Referenced(_))
        || e.children().into_iter().any(expr_uses_lifecycle)
}

fn expr_uses_queries(e: &Expr) -> bool {
    matches!(e.kind(), ExprKind::Count(_) | ExprKind::Fold { .. })
        || e.children().into_iter().any(expr_uses_queries)
}

/// Whether the module uses a form introduced by wire 0.6 (feature 007): relational forms or
/// module-level invariants. Such a module serializes as `"0.6"`.
pub fn uses_queries(m: &Module) -> bool {
    !m.global_invariants.is_empty()
        || m.derived.values().any(|d| expr_uses_queries(&d.body))
        || m.invariants.values().any(|i| expr_uses_queries(&i.body))
        || m.constraints.values().any(|c| expr_uses_queries(&c.body))
        || m.actions.values().any(|a| {
            a.preconditions
                .iter()
                .chain(&a.postconditions)
                .any(|c| expr_uses_queries(&c.expr))
                || a.effects.iter().any(|e| expr_uses_queries(&e.value))
                || a.creates.iter().any(|c| {
                    expr_uses_queries(&c.id) || c.fields.iter().any(|(_, v)| expr_uses_queries(v))
                })
        })
}

/// Whether the module uses a form introduced by wire 0.5 (feature 006): reference fields,
/// lifecycle effects, `exists`, `referenced`. Such a module serializes as `"0.5"`.
pub fn uses_lifecycle(m: &Module) -> bool {
    m.entities.values().any(|e| !e.references.is_empty())
        || m.derived.values().any(|d| expr_uses_lifecycle(&d.body))
        || m.invariants.values().any(|i| expr_uses_lifecycle(&i.body))
        || m.constraints
            .values()
            .filter(|c| c.reference.is_none())
            .any(|c| expr_uses_lifecycle(&c.body))
        || m.actions.values().any(|a| {
            a.has_lifecycle()
                || a.preconditions
                    .iter()
                    .chain(&a.postconditions)
                    .any(|c| expr_uses_lifecycle(&c.expr))
                || a.effects.iter().any(|e| expr_uses_lifecycle(&e.value))
        })
}

/// Canonical wire JSON of an admitted module.
pub fn to_wire_json(m: &Module) -> String {
    // Values are strings, booleans, integers, and nulls only; canonicalization cannot fail.
    canonical::to_canonical_string(&to_wire_value(m)).unwrap_or_default()
}
