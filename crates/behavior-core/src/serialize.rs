//! Canonical wire JSON of an admitted module (research R17; contracts/ir-encoding.md).
//!
//! Only admitted semantic IR can be serialized. The output shows every conversion explicitly
//! (`some`, `to_decimal`), keeps folded literals, and lists declarations by name, so admitting
//! it again yields the same behavior version and serializing again yields the same bytes.

use serde_json::{Map, Value as Json, json};

use crate::canonical;
use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::module::{Module, ParamRole};
use crate::semantic::types::{ArithOp, CmpOp, Type, ops};
use crate::semantic::value::encode;
use crate::wire::{DerivedKind, IR_VERSION, IR_VERSION_CONSTRAINTS, Loc};

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
    let l = e.loc();
    match e.kind() {
        ExprKind::Lit(v) => {
            json!({"op": "lit", "type": e.ty().to_wire_json(), "value": encode(e.ty(), v), "loc": loc(l)})
        }
        ExprKind::Field { param, field } => {
            json!({"op": "field", "param": param, "field": field, "loc": loc(l)})
        }
        ExprKind::Param(p) => json!({"op": "param", "param": p, "loc": loc(l)}),
        ExprKind::DerivedRef { name, args, .. } => {
            json!({"op": "derived", "name": name, "args": args, "loc": loc(l)})
        }
        ExprKind::Cmp(c, a, b) => op_node(cmp_name(*c), vec![expr(a), expr(b)], l),
        ExprKind::Arith(o, a, b) => op_node(arith_name(*o), vec![expr(a), expr(b)], l),
        ExprKind::And(xs) => op_node("and", xs.iter().map(expr).collect(), l),
        ExprKind::Or(xs) => op_node("or", xs.iter().map(expr).collect(), l),
        ExprKind::Not(a) => op_node("not", vec![expr(a)], l),
        ExprKind::In(a, values) => json!({
            "op": "in",
            "args": [expr(a)],
            "values": values.iter().map(|v| encode(a.ty(), v)).collect::<Vec<_>>(),
            "loc": loc(l),
        }),
        ExprKind::IsNone(a) => op_node("is_none", vec![expr(a)], l),
        ExprKind::IsSome(a) => op_node("is_some", vec![expr(a)], l),
        ExprKind::ValueOr(a, d) => op_node("value_or", vec![expr(a), expr(d)], l),
        ExprKind::Some(a) => op_node("some", vec![expr(a)], l),
        ExprKind::ToDecimal(a) => op_node("to_decimal", vec![expr(a)], l),
        ExprKind::Unwrap(a) => op_node("unwrap", vec![expr(a)], l),
        ExprKind::Wrap(a) => {
            let nominal = match e.ty() {
                Type::Nominal(n) => n.name.clone(),
                other => other.to_string(),
            };
            json!({"op": "wrap", "nominal": nominal, "args": [expr(a)], "loc": loc(l)})
        }
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
            json!({"name": n.name, "underlying": n.underlying.to_type().to_wire_json(),
                   "ops": op_names(n.ops), "loc": loc(l)})
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
                .map(|((name, ty), l)| json!({"name": name, "type": ty.to_wire_json(), "loc": loc(l)}))
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
            json!({"name": name, "kind": kind, "params": params(&d.params, false),
                   "body": expr(&d.body), "loc": loc(&d.loc)})
        })
        .collect();
    let invariants: Vec<Json> = m
        .invariants
        .iter()
        .map(|(name, i)| {
            json!({"name": name, "entity": i.entity, "param": i.param,
                   "body": expr(&i.body), "loc": loc(&i.loc)})
        })
        .collect();
    let constraints: Vec<Json> = m
        .constraints
        .iter()
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
            json!({
                "name": name,
                "params": params(&a.params, true),
                "preconditions": a.preconditions.iter().map(cond).collect::<Vec<_>>(),
                "effects": a.effects.iter().map(|e| json!({
                    "target": {"param": e.param, "field": e.field},
                    "value": expr(&e.value),
                    "loc": loc(&e.loc),
                })).collect::<Vec<_>>(),
                "postconditions": a.postconditions.iter().map(cond).collect::<Vec<_>>(),
                "loc": loc(&a.loc),
            })
        })
        .collect();
    let mut doc = json!({
        "ir_version": IR_VERSION,
        "enums": enums,
        "nominals": nominals,
        "entities": entities,
        "derived": derived,
        "invariants": invariants,
        "actions": actions,
    });
    // Modules without constraints keep the 0.1 form, so feature 001 documents are unchanged.
    if !constraints.is_empty()
        && let Json::Object(map) = &mut doc
    {
        map.insert("ir_version".into(), json!(IR_VERSION_CONSTRAINTS));
        map.insert("constraints".into(), Json::Array(constraints));
    }
    doc
}

/// Canonical wire JSON of an admitted module.
pub fn to_wire_json(m: &Module) -> String {
    // Values are strings, booleans, integers, and nulls only; canonicalization cannot fail.
    canonical::to_canonical_string(&to_wire_value(m)).unwrap_or_default()
}
