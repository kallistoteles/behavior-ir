//! Admission-time representation analysis (feature 004, research R4).
//!
//! For every numeric expression, bounds computed from types, literals, and type-level bounds
//! only — never from entity constraints, invariants, or preconditions (admission proves
//! representational validity; verification proves reachable behavioral safety):
//!
//! - `nb`, `db`: upper bounds on the bit lengths of the reduced numerator and denominator, so no
//!   exact value can exceed the runtime's representation (`MAX_BITS`);
//! - `scale`: `Some(s)` if the value is always a finite decimal with at most `s` fractional
//!   digits;
//! - `cd`: an upper bound on the digits of the value's coefficient at that scale
//!   (`v = c · 10^−scale`); the normalized coefficient has at most that many digits.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use crate::admit::AdmissionError;
use crate::decimal::Dec;
use crate::exact::Exact;
use crate::pretty;
use crate::semantic::expr::{Expr, ExprKind};
use crate::semantic::module::Module;
use crate::semantic::types::{ArithOp, Prim, Type, fixed_scale};
use crate::semantic::value::Value;

/// Largest bit length of an exact numerator or denominator the runtime supports.
pub const MAX_BITS: u32 = 511;

/// Coefficient digits of a general decimal input (28 significant digits at an unknown scale
/// ≤ 28, viewed at scale 28).
const DEC_INPUT_CD: u32 = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub nb: u32,
    pub db: u32,
    pub scale: Option<u32>,
    pub cd: Option<u32>,
}

fn pow10_bits(k: u32) -> u32 {
    u32::try_from(num_traits::pow(BigInt::from(10), k as usize).bits()).unwrap_or(u32::MAX)
}

fn digits(n: u128) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

const NON_NUMERIC: Facts = Facts {
    nb: 0,
    db: 0,
    scale: Some(0),
    cd: Some(0),
};

/// Facts of a value of type `t` about which nothing but its type is known.
fn leaf(t: &Type) -> Facts {
    let decimal_input = Facts {
        nb: pow10_bits(28),
        db: pow10_bits(28),
        scale: Some(28),
        cd: Some(DEC_INPUT_CD),
    };
    match t {
        Type::Int => Facts {
            nb: 63,
            db: 1,
            scale: Some(0),
            cd: Some(19),
        },
        Type::Decimal => decimal_input,
        Type::Nominal(n) => match (n.underlying, n.scale) {
            (Prim::Decimal, Some(s)) => Facts {
                nb: pow10_bits(28),
                db: pow10_bits(u32::from(s)),
                scale: Some(u32::from(s)),
                cd: Some(28),
            },
            (Prim::Decimal, None) => decimal_input,
            (Prim::Int, _) => leaf(&Type::Int),
            _ => NON_NUMERIC,
        },
        Type::Option(inner) => leaf(inner),
        Type::Exact(_) => Facts {
            nb: pow10_bits(28),
            db: pow10_bits(28),
            scale: None,
            cd: None,
        },
        _ => NON_NUMERIC,
    }
}

/// `Int` or an integer nominal (the same test as the evaluator and the verifier).
fn is_integer(t: &Type) -> bool {
    match t {
        Type::Int => true,
        Type::Nominal(n) => n.underlying == Prim::Int,
        _ => false,
    }
}

/// `(mantissa, scale)` of a literal number in normalized form.
fn literal_parts(v: &Value) -> Option<(i128, u32)> {
    match v {
        Value::Int(i) => Some((i128::from(*i), 0)),
        Value::Dec(d) => {
            let n = Dec::parse_str(&d.to_normalized_string()).ok()?;
            Some(n.mantissa_scale())
        }
        _ => None,
    }
}

fn literal(v: &Value) -> Facts {
    let Some((m, s)) = literal_parts(v) else {
        return NON_NUMERIC;
    };
    let exact = match v {
        Value::Int(i) => Exact::from_i64(*i),
        Value::Dec(d) => Exact::from_dec(d),
        _ => return NON_NUMERIC,
    };
    let (nb, db) = exact.bits();
    Facts {
        nb: u32::try_from(nb.max(1)).unwrap_or(u32::MAX),
        db: u32::try_from(db.max(1)).unwrap_or(u32::MAX),
        scale: Some(s),
        cd: Some(digits(m.unsigned_abs())),
    }
}

/// `|m| = 2^i · 5^j` → `Some(max(i, j))`.
fn two_five_power(m: i128) -> Option<u32> {
    let mut n = m.unsigned_abs();
    if n == 0 {
        return None;
    }
    let (mut i, mut j) = (0u32, 0u32);
    while n.is_multiple_of(2) {
        n /= 2;
        i += 1;
    }
    while n.is_multiple_of(5) {
        n /= 5;
        j += 1;
    }
    (n == 1).then_some(i.max(j))
}

fn add(a: u32, b: u32) -> u32 {
    a.saturating_add(b)
}

fn combine(op: ArithOp, x: Facts, y: Facts, divisor: Option<&Value>) -> Facts {
    match op {
        ArithOp::Mul => Facts {
            nb: add(x.nb, y.nb),
            db: add(x.db, y.db),
            scale: x.scale.zip(y.scale).map(|(a, b)| a + b),
            cd: x.cd.zip(y.cd).map(|(a, b)| a + b),
        },
        ArithOp::Add | ArithOp::Sub => {
            let scale = x.scale.zip(y.scale).map(|(a, b)| a.max(b));
            let cd = match (scale, x.scale, y.scale, x.cd, y.cd) {
                (Some(s), Some(s1), Some(s2), Some(c1), Some(c2)) => {
                    Some((c1 + s - s1).max(c2 + s - s2) + 1)
                }
                _ => None,
            };
            Facts {
                nb: add(add(x.nb, y.db).max(add(y.nb, x.db)), 1),
                db: add(x.db, y.db),
                scale,
                cd,
            }
        }
        ArithOp::Div => {
            // Dividing by a literal m·10^−k with |m| = 2^i·5^j keeps a finite decimal:
            // scale + max(i, j), coefficient digits + max(i, j) + k.
            let finite = divisor
                .and_then(literal_parts)
                .and_then(|(m, k)| two_five_power(m).map(|p| (p, k)));
            let (scale, cd) = match finite {
                Some((p, k)) => (x.scale.map(|s| s + p), x.cd.map(|c| c + p + k)),
                None => (None, None),
            };
            Facts {
                nb: add(x.nb, y.db),
                db: add(x.db, y.nb),
                scale,
                cd,
            }
        }
    }
}

fn join(x: Facts, y: Facts) -> Facts {
    Facts {
        nb: x.nb.max(y.nb),
        db: x.db.max(y.db),
        scale: x.scale.zip(y.scale).map(|(a, b)| a.max(b)),
        cd: x.cd.zip(y.cd).map(|(a, b)| a.max(b)),
    }
}

/// The facts of `e`; `derived` holds the facts of derived bodies already analysed. The first
/// arithmetic node that exceeds `MAX_BITS` is reported in `err` (once per expression tree).
pub(crate) fn facts(
    e: &Expr,
    derived: &BTreeMap<String, Facts>,
    err: &mut Option<AdmissionError>,
) -> Facts {
    match e.kind() {
        ExprKind::Lit(v) => literal(v),
        ExprKind::Field { .. } | ExprKind::Param(_) => leaf(e.ty()),
        ExprKind::DerivedRef { name, .. } => {
            derived.get(name).copied().unwrap_or_else(|| leaf(e.ty()))
        }
        // Integer arithmetic (plain or integer nominal) never enters the exact domain: it is
        // bounded by the runtime's 64-bit overflow check.
        ExprKind::Arith(..) if is_integer(e.ty()) => leaf(&Type::Int),
        ExprKind::Arith(op, a, b) => {
            let x = facts(a, derived, err);
            let y = facts(b, derived, err);
            let divisor = match b.kind() {
                ExprKind::Lit(v) => Some(v),
                _ => None,
            };
            let r = combine(*op, x, y, divisor);
            if err.is_none() && (r.nb > MAX_BITS || r.db > MAX_BITS) {
                let (which, bits) = if r.nb > MAX_BITS {
                    ("numerator", r.nb)
                } else {
                    ("denominator", r.db)
                };
                *err = Some(AdmissionError::new(
                    "EXACT_BOUND_EXCEEDED",
                    format!(
                        "the exact value of `{}` may need a {bits}-bit {which}; the runtime supports \
                         {MAX_BITS} bits. Split the expression with an explicit rescale",
                        pretty::text(e)
                    ),
                    Some(e.loc()),
                ));
            }
            r
        }
        ExprKind::Wrap(_) | ExprKind::Rescale { .. } if fixed_scale(e.ty()).is_some() => {
            leaf(e.ty())
        }
        ExprKind::ToDecimal(a)
        | ExprKind::Unwrap(a)
        | ExprKind::Wrap(a)
        | ExprKind::Some(a)
        | ExprKind::StrictUnwrap(a) => facts(a, derived, err),
        ExprKind::Rescale { arg, .. } => {
            let _ = facts(arg, derived, err);
            leaf(e.ty())
        }
        ExprKind::ValueOr(a, d) => {
            let x = facts(a, derived, err);
            let y = facts(d, derived, err);
            join(x, y)
        }
        ExprKind::Cmp(_, a, b) => {
            let _ = facts(a, derived, err);
            let _ = facts(b, derived, err);
            NON_NUMERIC
        }
        ExprKind::And(xs) | ExprKind::Or(xs) => {
            for x in xs {
                let _ = facts(x, derived, err);
            }
            NON_NUMERIC
        }
        ExprKind::Not(a)
        | ExprKind::In(a, _)
        | ExprKind::IsNone(a)
        | ExprKind::IsSome(a)
        | ExprKind::Exists(a)
        | ExprKind::Referenced(a)
        | ExprKind::EnumMap { arg: a, .. } => {
            let _ = facts(a, derived, err);
            NON_NUMERIC
        }
        ExprKind::Count(q) => {
            for b in q.bodies() {
                let _ = facts(b, derived, err);
            }
            leaf(&Type::Int)
        }
        ExprKind::Fold {
            op, query, body, ..
        } => {
            for b in query.bodies() {
                let _ = facts(b, derived, err);
            }
            let f = facts(body, derived, err);
            match op {
                // A sum has the body's scale; its magnitude grows by at most 64 bits (no store
                // holds more than 2^64 members; the runtime checks overflow regardless).
                crate::semantic::expr::FoldOp::Sum if is_integer(e.ty()) => leaf(&Type::Int),
                crate::semantic::expr::FoldOp::Sum if fixed_scale(e.ty()).is_some() => leaf(e.ty()),
                crate::semantic::expr::FoldOp::Sum => Facts {
                    nb: f.nb.saturating_add(64).min(MAX_BITS),
                    db: f.db,
                    scale: f.scale,
                    cd: f.cd.map(|c| c + 20),
                },
                crate::semantic::expr::FoldOp::Min | crate::semantic::expr::FoldOp::Max => f,
                _ => NON_NUMERIC,
            }
        }
    }
}

/// Facts of every derived value of an admitted module, in evaluation order.
pub fn derived_facts(m: &Module) -> BTreeMap<String, Facts> {
    let mut out = BTreeMap::new();
    for name in m.evaluation_order() {
        if let Some(d) = m.derived(name) {
            let f = facts(d.body(), &out, &mut None);
            out.insert(name.clone(), f);
        }
    }
    out
}
