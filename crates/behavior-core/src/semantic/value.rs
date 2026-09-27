//! Runtime and literal values, and their JSON encoding (data-model.md → Evaluation request).
//!
//! Enum values, ids, and strings are all `Str`; nominal values use their underlying value;
//! `Option<T>` is `None` or the inner value itself (options never nest).

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::decimal::{Dec, fixed_text};
use crate::exact::Exact;
use crate::semantic::types::{Prim, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Dec(Dec),
    Str(String),
    None,
    Entity(BTreeMap<String, Value>),
    /// An exact quantity; exists only while an expression is evaluated, never in state.
    Exact(Exact),
}

fn describe(t: &Type) -> String {
    match t {
        Type::Decimal => "a decimal (string or integer)".to_string(),
        Type::Int => "an integer".to_string(),
        Type::Bool => "a boolean".to_string(),
        Type::String => "a string".to_string(),
        Type::Id(e) => format!("an id of {e} (string)"),
        Type::Enum(e) => format!("one of {:?}", e.values),
        Type::Nominal(n) => match n.scale {
            Some(s) => format!("{} (a decimal with at most {s} decimal places)", n.name),
            None => format!("{} ({})", n.name, describe(&n.underlying.to_type())),
        },
        Type::Exact(_) => format!("an exact {t} value (never an input)"),
        Type::Option(inner) => format!("null or {}", describe(inner)),
        Type::Entity(e) => format!("a {e} object"),
    }
}

/// Decodes a non-entity value of type `t` from JSON; the error message says what was expected.
pub fn decode_scalar(t: &Type, v: &Json) -> Result<Value, String> {
    let wrong = || Err(format!("expected {}", describe(t)));
    match t {
        Type::Bool => match v {
            Json::Bool(b) => Ok(Value::Bool(*b)),
            _ => wrong(),
        },
        Type::Int => match v.as_i64() {
            Some(i) if v.is_i64() || v.is_u64() => Ok(Value::Int(i)),
            _ => wrong(),
        },
        Type::Decimal => match Dec::from_json(v) {
            Ok(d) => Ok(Value::Dec(d)),
            Err(_) => wrong(),
        },
        Type::String | Type::Id(_) => match v {
            Json::String(s) => Ok(Value::Str(s.clone())),
            _ => wrong(),
        },
        Type::Enum(e) => match v {
            Json::String(s) if e.values.contains(s) => Ok(Value::Str(s.clone())),
            _ => wrong(),
        },
        Type::Nominal(n) => decode_scalar(&prim_type(n.underlying), v).or_else(|_| wrong()),
        Type::Option(inner) => match v {
            Json::Null => Ok(Value::None),
            _ => decode_scalar(inner, v).or_else(|_| wrong()),
        },
        Type::Entity(_) | Type::Exact(_) => wrong(),
    }
}

fn prim_type(p: Prim) -> Type {
    p.to_type()
}

/// Encodes a value of type `t` as JSON (decimals as normalized strings; fixed-scale values with
/// exactly `scale` fractional digits; exact quantities as decimal or `n/d` text).
pub fn encode(t: &Type, v: &Value) -> Json {
    let fixed = match t {
        Type::Nominal(n) => n.scale,
        Type::Option(inner) => match &**inner {
            Type::Nominal(n) => n.scale,
            _ => None,
        },
        _ => None,
    };
    match v {
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Dec(d) => match fixed {
            Some(s) => json!(fixed_text(d, s)),
            None => json!(d.to_normalized_string()),
        },
        Value::Exact(x) => json!(x.to_text()),
        Value::Str(s) => json!(s),
        Value::None => Json::Null,
        Value::Entity(fields) => {
            let _ = t;
            Json::Object(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), encode_untyped(v)))
                    .collect(),
            )
        }
    }
}

/// Encoding that does not need the type (the value representation is unambiguous).
pub fn encode_untyped(v: &Value) -> Json {
    encode(&Type::Bool, v)
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
