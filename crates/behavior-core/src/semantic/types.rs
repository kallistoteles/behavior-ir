//! Semantic types and the typing rules of data-model.md → Typing rules.
//!
//! The engine provides type construction; domains provide types. Nominal types carry their
//! declared operations, so `Money + Money` can be valid while `Money + Decimal` is not.

use std::fmt;
use std::sync::Arc;

use serde_json::{Value, json};

/// A 32-byte content hash.
pub type Hash = [u8; 32];

/// `"sha256:" + lowercase hex`.
pub fn hash_display(h: &Hash) -> String {
    let mut s = String::with_capacity(71);
    s.push_str("sha256:");
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Operation bits a nominal type may declare (equality is always available).
pub mod ops {
    pub const ORDER: u8 = 1;
    pub const ADD: u8 = 2;
    pub const SCALE: u8 = 4;
    pub const RATIO: u8 = 8;

    pub fn parse(name: &str) -> Option<u8> {
        Some(match name {
            "order" => ORDER,
            "add" => ADD,
            "scale" => SCALE,
            "ratio" => RATIO,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    Bool,
    Int,
    Decimal,
    String,
}

impl Prim {
    pub fn is_numeric(self) -> bool {
        matches!(self, Prim::Int | Prim::Decimal)
    }

    pub fn to_type(self) -> Type {
        match self {
            Prim::Bool => Type::Bool,
            Prim::Int => Type::Int,
            Prim::Decimal => Type::Decimal,
            Prim::String => Type::String,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo {
    pub name: String,
    pub values: Vec<String>,
    pub hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalInfo {
    pub name: String,
    pub underlying: Prim,
    pub ops: u8,
    /// Fixed scale (0–28) of a decimal-based nominal: its values lie on the grid `10^-scale`.
    pub scale: Option<u8>,
    pub hash: Hash,
}

impl NominalInfo {
    pub fn has(&self, op: u8) -> bool {
        self.ops & op != 0
    }
}

/// The fixed-scale nominal of a type, if it is one.
pub fn fixed_scale(t: &Type) -> Option<&Arc<NominalInfo>> {
    match t {
        Type::Nominal(n) if n.scale.is_some() => Some(n),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bool,
    Int,
    Decimal,
    String,
    Option(Box<Type>),
    Enum(Arc<EnumInfo>),
    Nominal(Arc<NominalInfo>),
    /// An exact quantity of a fixed-scale nominal: never stored, becomes the nominal only
    /// through `rescale`.
    Exact(Arc<NominalInfo>),
    Id(String),
    /// Entity instances; only valid as parameter types.
    Entity(String),
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Decimal)
    }

    /// The wire form of the type (names instead of declaration hashes).
    pub fn to_wire_type(&self) -> crate::wire::WType {
        use crate::wire::WType;
        match self {
            Type::Bool => WType::Bool,
            Type::Int => WType::Int,
            Type::Decimal => WType::Decimal,
            Type::String => WType::String,
            Type::Option(inner) => WType::Option(Box::new(inner.to_wire_type())),
            Type::Enum(e) => WType::Enum(e.name.clone()),
            Type::Nominal(n) => WType::Nominal(n.name.clone()),
            Type::Exact(n) => WType::Exact(n.name.clone()),
            Type::Id(e) => WType::Id(e.clone()),
            Type::Entity(e) => WType::Entity(e.clone()),
        }
    }

    /// The wire JSON form of the type (`{"t": ...}`).
    pub fn to_wire_json(&self) -> Value {
        match self {
            Type::Bool => json!({"t": "bool"}),
            Type::Int => json!({"t": "int"}),
            Type::Decimal => json!({"t": "decimal"}),
            Type::String => json!({"t": "string"}),
            Type::Option(inner) => json!({"t": "option", "of": inner.to_wire_json()}),
            Type::Enum(e) => json!({"t": "enum", "name": e.name}),
            Type::Nominal(n) => json!({"t": "nominal", "name": n.name}),
            Type::Exact(n) => json!({"t": "exact", "name": n.name}),
            Type::Id(e) => json!({"t": "id", "entity": e}),
            Type::Entity(e) => json!({"t": "entity", "name": e}),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Bool => f.write_str("Bool"),
            Type::Int => f.write_str("Int"),
            Type::Decimal => f.write_str("Decimal"),
            Type::String => f.write_str("String"),
            Type::Option(inner) => write!(f, "Option<{inner}>"),
            Type::Enum(e) => f.write_str(&e.name),
            Type::Nominal(n) => f.write_str(&n.name),
            Type::Exact(n) => write!(f, "Exact<{}>", n.name),
            Type::Id(e) => write!(f, "Id<{e}>"),
            Type::Entity(e) => f.write_str(e),
        }
    }
}

/// Implicit conversion applied to an operand; made explicit in the semantic IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conv {
    Keep,
    ToDecimal,
    Some,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// The shape of an operation for typing purposes.
#[derive(Debug, Clone)]
pub enum OpSig {
    Cmp(CmpOp),
    Arith(ArithOp),
    And,
    Or,
    Not,
    In { count: usize },
    IsNone,
    IsSome,
    ValueOr,
    Some,
    ToDecimal,
    Wrap(Arc<NominalInfo>),
    Unwrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCode {
    TypeMismatch,
    OpNotAllowed,
    EmptyIn,
    NestedOption,
    /// A lossy narrowing to a fixed-scale type without `rescale`.
    LossyConversion,
}

impl TypeCode {
    pub fn as_str(self) -> &'static str {
        match self {
            TypeCode::TypeMismatch => "TYPE_MISMATCH",
            TypeCode::OpNotAllowed => "OP_NOT_ALLOWED",
            TypeCode::EmptyIn => "EMPTY_IN",
            TypeCode::NestedOption => "NESTED_OPTION",
            TypeCode::LossyConversion => "LOSSY_CONVERSION",
        }
    }
}

pub type Typed = Result<(Type, Vec<Conv>), TypeCode>;

/// Checks that a type is well formed (no `Option<Option<T>>`).
pub fn check_type(t: &Type) -> Result<(), TypeCode> {
    match t {
        Type::Option(inner) if matches!(**inner, Type::Option(_)) => Err(TypeCode::NestedOption),
        Type::Option(inner) => check_type(inner),
        _ => Ok(()),
    }
}

/// How a value of type `actual` can be used where `expected` is required.
pub fn coerce(expected: &Type, actual: &Type) -> Option<Conv> {
    if expected == actual {
        return Some(Conv::Keep);
    }
    match (expected, actual) {
        (Type::Decimal, Type::Int) => Some(Conv::ToDecimal),
        (Type::Option(inner), a) if **inner == *a => Some(Conv::Some),
        _ => None,
    }
}

fn promote(a: &Type, b: &Type) -> Vec<Conv> {
    let c = |t: &Type| {
        if *t == Type::Int {
            Conv::ToDecimal
        } else {
            Conv::Keep
        }
    };
    if *a == Type::Int && *b == Type::Int {
        vec![Conv::Keep, Conv::Keep]
    } else {
        vec![c(a), c(b)]
    }
}

fn nominal_pair<'a>(a: &'a Type, b: &Type) -> Option<&'a Arc<NominalInfo>> {
    match (a, b) {
        (Type::Nominal(x), Type::Nominal(y)) if x == y => Some(x),
        _ => None,
    }
}

fn scale_conv(n: &NominalInfo, scalar: &Type) -> Conv {
    if n.underlying == Prim::Decimal && *scalar == Type::Int {
        Conv::ToDecimal
    } else {
        Conv::Keep
    }
}

/// The fixed-scale nominal behind `T` or `Exact<T>`, and whether it is exact.
fn fixed_or_exact(t: &Type) -> Option<(&Arc<NominalInfo>, bool)> {
    match t {
        Type::Nominal(n) if n.scale.is_some() => Some((n, false)),
        Type::Exact(n) => Some((n, true)),
        _ => None,
    }
}

/// Typing of fixed-scale nominals and exact quantities (contracts/numeric-semantics.md → Typing);
/// `None` when no operand is one (the general rules apply).
fn fixed_scale_op(sig: &OpSig, operands: &[Type]) -> Option<Typed> {
    use TypeCode::*;
    let keep2 = || vec![Conv::Keep, Conv::Keep];
    match (sig, operands) {
        (OpSig::Cmp(c), [a, b]) => {
            let (x, _) = fixed_or_exact(a)?;
            let (y, _) = fixed_or_exact(b)?;
            if x != y {
                return Some(Err(TypeMismatch));
            }
            if !matches!(c, CmpOp::Eq | CmpOp::Ne) && !x.has(ops::ORDER) {
                return Some(Err(OpNotAllowed));
            }
            Some(Ok((Type::Bool, keep2())))
        }
        (OpSig::Arith(ArithOp::Add | ArithOp::Sub), [a, b]) => {
            let fa = fixed_or_exact(a);
            let fb = fixed_or_exact(b);
            let ((x, ea), (y, eb)) = match (fa, fb) {
                (Some(p), Some(q)) => (p, q),
                (None, None) => return None,
                _ => return Some(Err(TypeMismatch)),
            };
            if x != y {
                return Some(Err(TypeMismatch));
            }
            if !x.has(ops::ADD) {
                return Some(Err(OpNotAllowed));
            }
            let t = if ea || eb {
                Type::Exact(x.clone())
            } else {
                Type::Nominal(x.clone())
            };
            Some(Ok((t, keep2())))
        }
        (OpSig::Arith(ArithOp::Mul), [a, b]) => {
            let (n, exact, scalar) = match (fixed_or_exact(a), fixed_or_exact(b)) {
                (Some((n, e)), None) => (n, e, b),
                (None, Some((n, e))) => (n, e, a),
                (None, None) => return None,
                (Some(_), Some(_)) => return Some(Err(TypeMismatch)),
            };
            if !scalar.is_numeric() {
                return Some(Err(TypeMismatch));
            }
            if !n.has(ops::SCALE) {
                return Some(Err(OpNotAllowed));
            }
            let lossless = !exact && *scalar == Type::Int;
            let t = if lossless {
                Type::Nominal(n.clone())
            } else {
                Type::Exact(n.clone())
            };
            Some(Ok((t, keep2())))
        }
        (OpSig::Arith(ArithOp::Div), [a, b]) => match (fixed_or_exact(a), fixed_or_exact(b)) {
            (None, None) => None,
            (Some((x, false)), Some((y, false))) => Some(if x != y {
                Err(TypeMismatch)
            } else if x.has(ops::RATIO) {
                Ok((Type::Decimal, keep2()))
            } else {
                Err(OpNotAllowed)
            }),
            (Some((n, _)), None) if b.is_numeric() => Some(if n.has(ops::SCALE) {
                Ok((Type::Exact(n.clone()), keep2()))
            } else {
                Err(OpNotAllowed)
            }),
            _ => Some(Err(TypeMismatch)),
        },
        (OpSig::Wrap(n), [a]) if n.scale.is_some() => Some(match a {
            // An integer is on every grid (range-checked at runtime).
            Type::Int => Ok((Type::Nominal(n.clone()), vec![Conv::ToDecimal])),
            Type::Decimal | Type::Exact(_) => Err(LossyConversion),
            _ => Err(TypeMismatch),
        }),
        (OpSig::Unwrap, [Type::Exact(_)]) => Some(Err(LossyConversion)),
        (OpSig::Some, [Type::Exact(_)]) => Some(Err(LossyConversion)),
        (OpSig::ValueOr, [_, Type::Exact(_)]) => Some(Err(LossyConversion)),
        (_, ops) if ops.iter().any(|t| matches!(t, Type::Exact(_))) => Some(Err(TypeMismatch)),
        _ => None,
    }
}

/// The result type of an operation and the conversions to apply to its operands.
pub fn type_of_op(sig: &OpSig, operands: &[Type]) -> Typed {
    use TypeCode::*;
    let two = || match operands {
        [a, b] => Ok((a, b)),
        _ => Err(TypeMismatch),
    };
    let one = || match operands {
        [a] => Ok(a),
        _ => Err(TypeMismatch),
    };
    if let Some(r) = fixed_scale_op(sig, operands) {
        return r;
    }
    match sig {
        OpSig::Cmp(CmpOp::Eq | CmpOp::Ne) => {
            let (a, b) = two()?;
            if matches!(a, Type::Entity(_)) || matches!(b, Type::Entity(_)) {
                return Err(TypeMismatch);
            }
            if a == b {
                return Ok((Type::Bool, vec![Conv::Keep, Conv::Keep]));
            }
            if a.is_numeric() && b.is_numeric() {
                return Ok((Type::Bool, promote(a, b)));
            }
            match (a, b) {
                (Type::Option(inner), other) if **inner == *other => {
                    Ok((Type::Bool, vec![Conv::Keep, Conv::Some]))
                }
                (other, Type::Option(inner)) if **inner == *other => {
                    Ok((Type::Bool, vec![Conv::Some, Conv::Keep]))
                }
                _ => Err(TypeMismatch),
            }
        }
        OpSig::Cmp(_) => {
            let (a, b) = two()?;
            if a.is_numeric() && b.is_numeric() {
                return Ok((Type::Bool, promote(a, b)));
            }
            match nominal_pair(a, b) {
                Some(n) if n.has(ops::ORDER) => Ok((Type::Bool, vec![Conv::Keep, Conv::Keep])),
                Some(_) => Err(OpNotAllowed),
                None => Err(TypeMismatch),
            }
        }
        OpSig::Arith(ArithOp::Add | ArithOp::Sub) => {
            let (a, b) = two()?;
            if *a == Type::Int && *b == Type::Int {
                return Ok((Type::Int, vec![Conv::Keep, Conv::Keep]));
            }
            if a.is_numeric() && b.is_numeric() {
                return Ok((Type::Decimal, promote(a, b)));
            }
            match nominal_pair(a, b) {
                Some(n) if n.has(ops::ADD) => Ok((a.clone(), vec![Conv::Keep, Conv::Keep])),
                Some(_) => Err(OpNotAllowed),
                None => Err(TypeMismatch),
            }
        }
        OpSig::Arith(ArithOp::Mul) => {
            let (a, b) = two()?;
            if *a == Type::Int && *b == Type::Int {
                return Ok((Type::Int, vec![Conv::Keep, Conv::Keep]));
            }
            if a.is_numeric() && b.is_numeric() {
                return Ok((Type::Decimal, promote(a, b)));
            }
            let (n, scalar, nominal_first) = match (a, b) {
                (Type::Nominal(n), s) if s.is_numeric() => (n, s, true),
                (s, Type::Nominal(n)) if s.is_numeric() => (n, s, false),
                _ => return Err(TypeMismatch),
            };
            if !n.has(ops::SCALE) {
                return Err(OpNotAllowed);
            }
            if *scalar == Type::Decimal && n.underlying != Prim::Decimal {
                return Err(TypeMismatch);
            }
            let c = scale_conv(n, scalar);
            let convs = if nominal_first {
                vec![Conv::Keep, c]
            } else {
                vec![c, Conv::Keep]
            };
            Ok((Type::Nominal(n.clone()), convs))
        }
        OpSig::Arith(ArithOp::Div) => {
            let (a, b) = two()?;
            if a.is_numeric() && b.is_numeric() {
                let c = |t: &Type| {
                    if *t == Type::Int {
                        Conv::ToDecimal
                    } else {
                        Conv::Keep
                    }
                };
                return Ok((Type::Decimal, vec![c(a), c(b)]));
            }
            if let Some(n) = nominal_pair(a, b) {
                return if n.has(ops::RATIO) {
                    Ok((Type::Decimal, vec![Conv::Keep, Conv::Keep]))
                } else {
                    Err(OpNotAllowed)
                };
            }
            match (a, b) {
                (Type::Nominal(n), s) if s.is_numeric() => {
                    if !n.has(ops::SCALE) {
                        Err(OpNotAllowed)
                    } else if n.underlying != Prim::Decimal {
                        Err(TypeMismatch)
                    } else {
                        Ok((a.clone(), vec![Conv::Keep, scale_conv(n, s)]))
                    }
                }
                _ => Err(TypeMismatch),
            }
        }
        OpSig::And | OpSig::Or => {
            if operands.len() >= 2 && operands.iter().all(|t| *t == Type::Bool) {
                Ok((Type::Bool, vec![Conv::Keep; operands.len()]))
            } else {
                Err(TypeMismatch)
            }
        }
        OpSig::Not => match one()? {
            Type::Bool => Ok((Type::Bool, vec![Conv::Keep])),
            _ => Err(TypeMismatch),
        },
        OpSig::In { count } => {
            let a = one()?;
            if *count == 0 {
                return Err(EmptyIn);
            }
            match a {
                Type::Option(_) | Type::Entity(_) => Err(TypeMismatch),
                _ => Ok((Type::Bool, vec![Conv::Keep])),
            }
        }
        OpSig::IsNone | OpSig::IsSome => match one()? {
            Type::Option(_) => Ok((Type::Bool, vec![Conv::Keep])),
            _ => Err(TypeMismatch),
        },
        OpSig::ValueOr => {
            let (a, b) = two()?;
            match a {
                Type::Option(inner) => match coerce(inner, b) {
                    Some(c @ (Conv::Keep | Conv::ToDecimal)) => {
                        Ok(((**inner).clone(), vec![Conv::Keep, c]))
                    }
                    _ => Err(TypeMismatch),
                },
                _ => Err(TypeMismatch),
            }
        }
        OpSig::Some => match one()? {
            Type::Option(_) => Err(NestedOption),
            Type::Entity(_) => Err(TypeMismatch),
            t => Ok((Type::Option(Box::new(t.clone())), vec![Conv::Keep])),
        },
        OpSig::ToDecimal => match one()? {
            Type::Int => Ok((Type::Decimal, vec![Conv::Keep])),
            _ => Err(TypeMismatch),
        },
        OpSig::Wrap(n) => {
            let a = one()?;
            match coerce(&n.underlying.to_type(), a) {
                Some(c @ (Conv::Keep | Conv::ToDecimal)) => Ok((Type::Nominal(n.clone()), vec![c])),
                _ => Err(TypeMismatch),
            }
        }
        OpSig::Unwrap => match one()? {
            Type::Nominal(n) => Ok((n.underlying.to_type(), vec![Conv::Keep])),
            _ => Err(TypeMismatch),
        },
    }
}
