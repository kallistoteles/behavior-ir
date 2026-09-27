//! Exact decimal numbers with a normalized plain-string encoding (research R9).
//!
//! Decimals are never floats: they are parsed from plain strings (`"50000"`, `"-12.5"`) or JSON
//! integers and printed in normalized form (no exponent, no trailing zeros, `-0` → `0`).

use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;

/// Maximum number of significant digits a decimal may carry.
pub const MAX_DIGITS: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NumError {
    #[error("numeric overflow")]
    Overflow,
    #[error("division by zero")]
    DivisionByZero,
    #[error("invalid decimal")]
    Invalid,
}

/// An exact decimal value. Equality and ordering are numeric (`50000 == 50000.00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dec(Decimal);

impl Dec {
    /// Parses plain decimal notation: optional `-`, digits, optional `.` followed by digits.
    pub fn parse_str(s: &str) -> Result<Dec, NumError> {
        let body = s.strip_prefix('-').unwrap_or(s);
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (body, None),
        };
        let digits_ok = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
        if !digits_ok(int_part) || frac_part.is_some_and(|f| !digits_ok(f)) {
            return Err(NumError::Invalid);
        }
        let all: String = int_part
            .chars()
            .chain(frac_part.unwrap_or("").chars())
            .collect();
        let significant = all.trim_start_matches('0').len();
        if significant > MAX_DIGITS || frac_part.is_some_and(|f| f.len() > MAX_DIGITS) {
            return Err(NumError::Invalid);
        }
        Decimal::from_str_exact(s)
            .map(Dec)
            .map_err(|_| NumError::Invalid)
    }

    /// Accepts a JSON string in plain notation or a JSON integer; rejects fractional numbers.
    pub fn from_json(v: &serde_json::Value) -> Result<Dec, NumError> {
        match v {
            serde_json::Value::String(s) => Dec::parse_str(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Dec::from_i64(i))
                } else if let Some(u) = n.as_u64() {
                    Ok(Dec(Decimal::from(u)))
                } else {
                    Err(NumError::Invalid)
                }
            }
            _ => Err(NumError::Invalid),
        }
    }

    pub fn from_i64(i: i64) -> Dec {
        Dec(Decimal::from(i))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Normalized plain notation: no exponent, no trailing zeros, `-0` written as `0`.
    pub fn to_normalized_string(&self) -> String {
        if self.0.is_zero() {
            return "0".to_string();
        }
        self.0.normalize().to_string()
    }

    /// The value as `mantissa / 10^scale`.
    pub fn mantissa_scale(&self) -> (i128, u32) {
        (self.0.mantissa(), self.0.scale())
    }

    /// Whether the value lies on the grid `10^-scale` (no non-zero digit beyond `scale`).
    pub fn on_grid(&self, scale: u8) -> bool {
        self.0.normalize().scale() <= u32::from(scale)
    }

    /// Whether `|v| < 10^(28 − scale)`, the range of a fixed-scale type.
    pub fn in_fixed_range(&self, scale: u8) -> bool {
        let digits = 28u32.saturating_sub(u32::from(scale));
        // 10^28 < 2^96, so the limit is always representable.
        self.0.abs() < Decimal::from_i128_with_scale(10i128.pow(digits), 0)
    }

    pub fn checked_add(&self, o: &Dec) -> Result<Dec, NumError> {
        self.0.checked_add(o.0).map(Dec).ok_or(NumError::Overflow)
    }

    pub fn checked_sub(&self, o: &Dec) -> Result<Dec, NumError> {
        self.0.checked_sub(o.0).map(Dec).ok_or(NumError::Overflow)
    }

    pub fn checked_mul(&self, o: &Dec) -> Result<Dec, NumError> {
        self.0.checked_mul(o.0).map(Dec).ok_or(NumError::Overflow)
    }

    /// Division rounded to at most 28 significant digits (the `rust_decimal` rule).
    pub fn checked_div(&self, o: &Dec) -> Result<Dec, NumError> {
        if o.0.is_zero() {
            return Err(NumError::DivisionByZero);
        }
        self.0.checked_div(o.0).map(Dec).ok_or(NumError::Overflow)
    }
}

/// A fixed-scale value in its canonical text form: exactly `scale` fractional digits
/// (`100.50`, `0.00`, `-3.10`; scale 0 without a point).
pub fn fixed_text(d: &Dec, scale: u8) -> String {
    let mut v = d.0;
    v.rescale(u32::from(scale));
    if v.is_zero() {
        v.set_sign_positive(true);
    }
    v.to_string()
}

impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_normalized_string())
    }
}

impl FromStr for Dec {
    type Err = NumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Dec::parse_str(s)
    }
}
