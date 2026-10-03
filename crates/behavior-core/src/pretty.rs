//! Readable text for semantic expressions (`expr_text` in decision records).
//!
//! Produced from the semantic IR, not from source, so explicit conversions are visible:
//! `invoice.approved_by == some(actor.id)`, `invoice.amount >= Money(0)`.
//!
//! Traces and check subjects are evidence, so the text must preserve the expression tree
//! unambiguously: binary operators are left-associative, a right operand of equal precedence is
//! parenthesized (`a * (b / c)`, `a - (b + c)`), and a nested `and`/`or` keeps its grouping.
//! `tests/pretty_roundtrip.rs` re-reads every rendered expression and compares trees.

use crate::semantic::expr::{CANDIDATE, Expr, ExprKind, QueryKind, QueryNode};
use crate::semantic::types::{ArithOp, CmpOp, Type, Unit};
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

fn wrap_if(e: &Expr, parens: bool, cand: Option<&str>) -> String {
    let s = text_in(e, cand);
    if parens { format!("({s})") } else { s }
}

/// A literal value of type `t`.
pub fn value_text(t: &Type, v: &Value) -> String {
    match (t, v) {
        (Type::Option(_), Value::None) => "none".to_string(),
        (Type::Option(inner), v) => format!("some({})", value_text(inner, v)),
        (Type::Enum(e), Value::Str(s)) => format!("{}.{s}", e.name),
        (Type::Nominal(n), Value::Dec(d)) if n.scale.is_some() => {
            format!(
                "{}({})",
                n.name,
                crate::decimal::fixed_text(d, n.scale.unwrap_or(0))
            )
        }
        (Type::Nominal(n), v) => format!("{}({})", n.name, value_text(&n.underlying.to_type(), v)),
        (_, Value::Str(s)) => serde_json::Value::String(s.clone()).to_string(),
        (_, Value::Bool(b)) => b.to_string(),
        (_, Value::Int(i)) => i.to_string(),
        (_, Value::Dec(d)) => d.to_normalized_string(),
        (_, Value::None) => "none".to_string(),
        (_, Value::Exact(x)) => x.to_text(),
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

/// The nominal a `wrap` attaches (`Money(e)` for both `Money` and `Exact<Money>` results).
fn wrap_name(t: &Type) -> String {
    match t {
        Type::Exact(Unit::Nominal(n)) => n.name.clone(),
        t => t.to_string(),
    }
}

/// The enum of an `Enum` or `Option<Enum>` type.
fn enum_name(t: &Type) -> String {
    match t {
        Type::Enum(e) => e.name.clone(),
        Type::Option(inner) => enum_name(inner),
        t => t.to_string(),
    }
}

/// Readable text of an expression with the fewest parentheses that keep its tree unambiguous.
pub fn text(e: &Expr) -> String {
    text_in(e, None)
}

/// Readable text of a query (feature 007): `select(T)`, `where(q, o, predicate)`, `union(a, b)`.
pub fn query_text(q: &QueryNode) -> String {
    match &q.kind {
        QueryKind::Select => format!("select({})", q.entity),
        QueryKind::Where { base, param, body } => format!(
            "where({}, {param}, {})",
            query_text(base),
            text_in(body, Some(param))
        ),
        QueryKind::Set { op, a, b } => {
            format!("{}({}, {})", op.as_str(), query_text(a), query_text(b))
        }
    }
}

/// [`text`] inside a lambda body whose candidate is displayed as `cand`.
fn text_in(e: &Expr, cand: Option<&str>) -> String {
    let p = prec(e);
    match &e.kind {
        ExprKind::Lit(v) => value_text(&e.ty, v),
        ExprKind::Field { param, field } if param == CANDIDATE => {
            format!("{}.{field}", cand.unwrap_or(CANDIDATE))
        }
        ExprKind::Field { param, field } => format!("{param}.{field}"),
        ExprKind::Param(name) => name.clone(),
        ExprKind::DerivedRef { name, args, .. } => {
            let shown: Vec<&str> = args
                .iter()
                .map(|a| {
                    if a == CANDIDATE {
                        cand.unwrap_or(CANDIDATE)
                    } else {
                        a.as_str()
                    }
                })
                .collect();
            format!("{name}({})", shown.join(", "))
        }
        ExprKind::Cmp(c, a, b) => format!(
            "{} {} {}",
            wrap_if(a, prec(a) <= p, cand),
            cmp_symbol(*c),
            wrap_if(b, prec(b) <= p, cand)
        ),
        // Left-associative: `a * b / c` is `(a * b) / c`; a right operand of equal precedence
        // keeps its parentheses even where the value would be the same (`a * (b / c)`).
        ExprKind::Arith(op, a, b) => format!(
            "{} {} {}",
            wrap_if(a, prec(a) < p, cand),
            arith_symbol(*op),
            wrap_if(b, prec(b) <= p, cand)
        ),
        // A nested `and` inside `and` (or `or` inside `or`) is a different tree: keep it grouped.
        ExprKind::And(xs) => xs
            .iter()
            .map(|x| wrap_if(x, prec(x) <= p, cand))
            .collect::<Vec<_>>()
            .join(" and "),
        ExprKind::Or(xs) => xs
            .iter()
            .map(|x| wrap_if(x, prec(x) <= p, cand))
            .collect::<Vec<_>>()
            .join(" or "),
        ExprKind::Not(a) => format!("not {}", wrap_if(a, prec(a) < p, cand)),
        ExprKind::In(a, values) => {
            let vs: Vec<String> = values.iter().map(|v| value_text(&a.ty, v)).collect();
            format!("{} in [{}]", wrap_if(a, prec(a) <= p, cand), vs.join(", "))
        }
        ExprKind::IsNone(a) => format!("{}.is_none()", wrap_if(a, prec(a) < 7, cand)),
        ExprKind::IsSome(a) => format!("{}.is_some()", wrap_if(a, prec(a) < 7, cand)),
        ExprKind::ValueOr(a, d) => format!(
            "{}.value_or({})",
            wrap_if(a, prec(a) < 7, cand),
            text_in(d, cand)
        ),
        ExprKind::Some(a) => format!("some({})", text_in(a, cand)),
        ExprKind::ToDecimal(a) => format!("decimal({})", text_in(a, cand)),
        ExprKind::Wrap(a) => format!("{}({})", wrap_name(&e.ty), text_in(a, cand)),
        ExprKind::Unwrap(a) => format!("underlying({})", text_in(a, cand)),
        ExprKind::Rescale { arg, rounding } => {
            format!(
                "rescale({}, {}, {})",
                text_in(arg, cand),
                e.ty,
                rounding.as_str()
            )
        }
        ExprKind::StrictUnwrap(a) => format!("strict_unwrap({})", text_in(a, cand)),
        ExprKind::EnumMap {
            arg,
            mapping,
            strict,
        } => {
            let (from, to) = (enum_name(&arg.ty), enum_name(&e.ty));
            let pairs: Vec<String> = mapping
                .iter()
                .map(|(f, t)| format!("{from}.{f}: {to}.{t}"))
                .collect();
            format!(
                "{}({}, {{{}}})",
                if *strict {
                    "strict_enum_map"
                } else {
                    "enum_map"
                },
                text_in(arg, cand),
                pairs.join(", ")
            )
        }
        ExprKind::Exists(a) => format!("exists({})", text_in(a, cand)),
        ExprKind::Referenced(a) => format!("referenced({})", text_in(a, cand)),
        ExprKind::Count(q) => format!("count({})", query_text(q)),
        ExprKind::Fold {
            op,
            query,
            param,
            body,
        } => format!(
            "{}({}, {param}, {})",
            op.as_str(),
            query_text(query),
            text_in(body, Some(param))
        ),
    }
}
