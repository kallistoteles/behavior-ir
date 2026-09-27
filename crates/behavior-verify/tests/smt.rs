#![allow(clippy::unwrap_used, clippy::expect_used)]

use behavior_verify::smt::{Sexp, SmtValue, parse, value};

#[test]
fn parses_answers_and_values() {
    let out = parse("sat\n((x 5)\n (y (- 7))\n (z 3.0)\n (w (/ 1.0 100.0))\n (v (- (/ 3.0 2.0)))\n (s \"a\"\"b\")\n (b true))").unwrap();
    assert_eq!(out[0], Sexp::Atom("sat".into()));
    let Sexp::List(pairs) = &out[1] else { panic!() };
    let vals: Vec<SmtValue> = pairs
        .iter()
        .map(|p| match p {
            Sexp::List(kv) => value(&kv[1]).unwrap(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        vals,
        vec![
            SmtValue::Int(5),
            SmtValue::Int(-7),
            SmtValue::Rational(3, 1),
            SmtValue::Rational(1, 100),
            SmtValue::Rational(-3, 2),
            SmtValue::Str("a\"b".into()),
            SmtValue::Bool(true),
        ]
    );
}

#[test]
fn parses_errors_and_unknown() {
    let out = parse("unknown\n(:reason-unknown \"(resource limits reached)\")\n(error \"line 3: model is not available\")").unwrap();
    assert_eq!(out[0], Sexp::Atom("unknown".into()));
    assert_eq!(out.len(), 3);
    assert!(matches!(&out[2], Sexp::List(items) if items[0] == Sexp::Atom("error".into())));
}

#[test]
fn rationals_convert_to_normalized_decimals() {
    assert_eq!(
        SmtValue::Rational(1, 100).to_decimal_string().as_deref(),
        Some("0.01")
    );
    assert_eq!(
        SmtValue::Rational(-3, 2).to_decimal_string().as_deref(),
        Some("-1.5")
    );
    assert_eq!(
        SmtValue::Rational(50000, 1).to_decimal_string().as_deref(),
        Some("50000")
    );
    assert_eq!(SmtValue::Int(7).to_decimal_string().as_deref(), Some("7"));
    assert_eq!(SmtValue::Rational(1, 3).to_decimal_string(), None);
}

#[test]
fn unicode_escapes_in_strings() {
    let out = parse("(\"x\\u{41}y\")").unwrap();
    let Sexp::List(items) = &out[0] else { panic!() };
    assert_eq!(value(&items[0]).unwrap(), SmtValue::Str("xAy".into()));
}
