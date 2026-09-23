//! Canonical JSON output: keys sorted by byte value, compact, UTF-8, no floats.
//!
//! This is a serialization concern only; identity is computed separately (see `admit::hash`).

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("floating-point number in output at {0}")]
    Float(String),
    #[error("serialization failed: {0}")]
    Serialize(String),
}

fn check_no_floats(v: &Value, path: &str) -> Result<(), CanonicalError> {
    match v {
        Value::Number(n) if n.is_f64() => Err(CanonicalError::Float(path.to_string())),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .try_for_each(|(i, c)| check_no_floats(c, &format!("{path}[{i}]"))),
        Value::Object(map) => map
            .iter()
            .try_for_each(|(k, c)| check_no_floats(c, &format!("{path}.{k}"))),
        _ => Ok(()),
    }
}

/// Canonical string of a JSON value. `serde_json`'s map is ordered by key (the
/// `preserve_order` feature is not enabled), which gives byte-order key sorting.
pub fn to_canonical_string(v: &Value) -> Result<String, CanonicalError> {
    check_no_floats(v, "$")?;
    serde_json::to_string(v).map_err(|e| CanonicalError::Serialize(e.to_string()))
}

/// Canonical string of any serializable value.
pub fn canonical<T: Serialize>(t: &T) -> Result<String, CanonicalError> {
    let v = serde_json::to_value(t).map_err(|e| CanonicalError::Serialize(e.to_string()))?;
    to_canonical_string(&v)
}
