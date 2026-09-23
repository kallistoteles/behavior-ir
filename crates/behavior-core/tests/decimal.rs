#![allow(clippy::unwrap_used, clippy::expect_used)]

use behavior_core::decimal::{Dec, NumError};
use serde_json::json;

fn d(s: &str) -> Dec {
    Dec::parse_str(s).unwrap()
}

#[test]
fn parses_and_normalizes() {
    assert_eq!(d("50000.00").to_normalized_string(), "50000");
    assert_eq!(d("0.050").to_normalized_string(), "0.05");
    assert_eq!(d("-0").to_normalized_string(), "0");
    assert_eq!(d("-0.00").to_normalized_string(), "0");
    assert_eq!(d("-12.50").to_normalized_string(), "-12.5");
    assert_eq!(d("0").to_normalized_string(), "0");
    assert_eq!(d("100").to_normalized_string(), "100");
}

#[test]
fn rejects_non_plain_forms() {
    for bad in [
        "1E+3", "1e3", "", ".5", "5.", "+1", "1,5", "abc", "1.2.3", " 1", "NaN",
    ] {
        assert!(Dec::parse_str(bad).is_err(), "{bad:?} must be rejected");
    }
    // More than 28 significant digits.
    assert!(Dec::parse_str("12345678901234567890123456789").is_err());
}

#[test]
fn json_accepts_strings_and_integers_only() {
    assert_eq!(Dec::from_json(&json!("43200")).unwrap(), d("43200"));
    assert_eq!(Dec::from_json(&json!(43200)).unwrap(), d("43200"));
    assert!(Dec::from_json(&json!(43200.5)).is_err());
    assert!(Dec::from_json(&json!(1e3)).is_err());
    assert!(Dec::from_json(&json!(true)).is_err());
    assert!(Dec::from_json(&json!("1E+3")).is_err());
}

#[test]
fn equality_is_numeric() {
    assert_eq!(d("50000"), d("50000.00"));
    assert!(d("49999.99") < d("50000"));
}

#[test]
fn arithmetic_is_exact_and_checked() {
    assert_eq!(d("0.1").checked_add(&d("0.2")).unwrap(), d("0.3"));
    assert_eq!(d("43200").checked_sub(&d("50000")).unwrap(), d("-6800"));
    assert_eq!(
        d("2").checked_div(&d("3")).unwrap().to_normalized_string(),
        "0.6666666666666666666666666667"
    );
    assert_eq!(d("1").checked_div(&d("0")), Err(NumError::DivisionByZero));
    // 28 significant digits is the maximum; multiplying past the representable range fails.
    let big = d("9999999999999999999999999999");
    assert_eq!(big.checked_mul(&d("10")), Err(NumError::Overflow));
    assert_eq!(big.checked_mul(&big), Err(NumError::Overflow));
}
