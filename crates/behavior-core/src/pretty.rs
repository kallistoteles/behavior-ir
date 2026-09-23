//! Readable text for semantic expressions (`expr_text` in decision records).
//!
//! Produced from the semantic IR, not from source, so explicit conversions are visible:
//! `invoice.approved_by == some(actor.id)`, `invoice.amount >= Money(0)`.

use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::types::{ArithOp, CmpOp, Type};
use crate::semantic::value::Value;

fn prec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Or(_) => 1,
        ExprKind::And(_) => 2,
        ExprKind::Not(_) => 3,
        ExprKind::Cmp(..) | ExprKind::In(..) => 4,
        ExprKind::Arith(ArithOp::Add | ArithOp::Sub, ..) => 5,
        ExprKind::Arith(ArithOp::Mul | ArithOp::Div, ..) => 6,
        _ => 7,
    }
}

fn wrap_if(e: &Expr, parens: bool) -> String {
    let s = text(e);
    if parens { format!("({s})") } else { s }
}

/// A literal value of type `t`.
pub fn value_text(t: &Type, v: &Value) -> String {
    match (t, v) {
        (Type::Option(_), Value::None) => "none".to_string(),
        (Type::Option(inner), v) => format!("some({})", value_text(inner, v)),
        (Type::Enum(e), Value::Str(s)) => format!("{}.{s}", e.name),
        (Type::Nominal(n), v) => format!("{}({})", n.name, value_text(&n.underlying.to_type(), v)),
        (_, Value::Str(s)) => serde_json::Value::String(s.clone()).to_string(),
        (_, Value::Bool(b)) => b.to_string(),
        (_, Value::Int(i)) => i.to_string(),
        (_, Value::Dec(d)) => d.to_normalized_string(),
        (_, Value::None) => "none".to_string(),
        (_, Value::Entity(_)) => "<entity>".to_string(),
    }
}

fn cmp_symbol(c: CmpOp) -> &'static str {
    match c {
        CmpOp::Eq => "==",
        CmpOp::Ne => "!=",
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Gt => ">",
        CmpOp::Ge => ">=",
    }
}

fn arith_symbol(a: ArithOp) -> &'static str {
    match a {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    }
}

/// Readable text of an expression with minimal parentheses.
pub fn text(e: &Expr) -> String {
    let p = prec(e);
    match &e.kind {
        ExprKind::Lit(v) => value_text(&e.ty, v),
        ExprKind::Field { param, field } => format!("{param}.{field}"),
        ExprKind::Param(name) => name.clone(),
        ExprKind::DerivedRef { name, args, .. } => format!("{name}({})", args.join(", ")),
        ExprKind::Cmp(c, a, b) => format!(
            "{} {} {}",
            wrap_if(a, prec(a) <= p),
            cmp_symbol(*c),
            wrap_if(b, prec(b) <= p)
        ),
        ExprKind::Arith(op, a, b) => {
            let right_strict = matches!(op, ArithOp::Sub | ArithOp::Div);
            format!(
                "{} {} {}",
                wrap_if(a, prec(a) < p),
                arith_symbol(*op),
                wrap_if(
                    b,
                    if right_strict {
                        prec(b) <= p
                    } else {
                        prec(b) < p
                    }
                )
            )
        }
        ExprKind::And(xs) => xs
            .iter()
            .map(|x| wrap_if(x, prec(x) < p))
            .collect::<Vec<_>>()
            .join(" and "),
        ExprKind::Or(xs) => xs
            .iter()
            .map(|x| wrap_if(x, prec(x) < p))
            .collect::<Vec<_>>()
            .join(" or "),
        ExprKind::Not(a) => format!("not {}", wrap_if(a, prec(a) < p)),
        ExprKind::In(a, values) => {
            let vs: Vec<String> = values.iter().map(|v| value_text(&a.ty, v)).collect();
            format!("{} in [{}]", wrap_if(a, prec(a) <= p), vs.join(", "))
        }
        ExprKind::IsNone(a) => format!("{}.is_none()", wrap_if(a, prec(a) < 7)),
        ExprKind::IsSome(a) => format!("{}.is_some()", wrap_if(a, prec(a) < 7)),
        ExprKind::ValueOr(a, d) => format!("{}.value_or({})", wrap_if(a, prec(a) < 7), text(d)),
        ExprKind::Some(a) => format!("some({})", text(a)),
        ExprKind::ToDecimal(a) => format!("decimal({})", text(a)),
        ExprKind::Wrap(a) => format!("{}({})", e.ty, text(a)),
        ExprKind::Unwrap(a) => format!("underlying({})", text(a)),
    }
}
