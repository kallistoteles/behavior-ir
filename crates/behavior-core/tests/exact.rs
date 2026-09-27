#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The exact domain and the six rounding modes (contracts/numeric-semantics.md).

use std::path::Path;

use behavior_core::decimal::NumError;
use behavior_core::exact::{Exact, Rounding};
use proptest::prelude::*;
use serde_json::Value;

fn e(s: &str) -> Exact {
    Exact::parse(s).unwrap()
}

#[test]
fn parses_reduces_and_computes_exactly() {
    assert_eq!(e("20/3").to_text(), "20/3");
    assert_eq!(e("40/6").to_text(), "20/3");
    assert_eq!(e("-0.50").to_text(), "-0.5");
    assert_eq!(e("12").to_text(), "12");
    assert_eq!(e("20/3").mul(&e("2")).unwrap().to_text(), "40/3");
    assert_eq!(
        e("20.00").div(&e("3")).unwrap().mul(&e("2")).unwrap(),
        e("40/3")
    );
    assert_eq!(e("1/3").add(&e("1/6")).unwrap().to_text(), "0.5");
    assert_eq!(e("1").sub(&e("7/6")).unwrap().to_text(), "-1/6");
    assert_eq!(e("1").div(&e("0")), Err(NumError::DivisionByZero));
    assert!(e("25.0125") > e("25.01"));
    assert!(e("25.0125") < e("25.02"));
}

#[test]
fn text_form_is_finite_decimal_or_fraction() {
    assert_eq!(e("2001/400").to_text(), "5.0025");
    assert_eq!(e("40/3").to_text(), "40/3");
    assert_eq!(e("-7/6").to_text(), "-7/6");
    assert_eq!(e("0").to_text(), "0");
    assert_eq!(e("-0").to_text(), "0");
    assert_eq!(e("1/8").to_text(), "0.125");
}

#[test]
fn overflow_beyond_512_bits() {
    let big = Exact::from_dec(&"9999999999999999999999999999".parse().unwrap());
    let mut x = big.clone();
    let mut overflowed = false;
    for _ in 0..40 {
        match x.mul(&big) {
            Ok(y) => x = y,
            Err(err) => {
                assert_eq!(err, NumError::Overflow);
                overflowed = true;
                break;
            }
        }
    }
    assert!(overflowed);
    // Denominators count too.
    let mut d = e("1");
    let three = e("3");
    let mut hit = false;
    for _ in 0..400 {
        match d.div(&three) {
            Ok(y) => d = y,
            Err(err) => {
                assert_eq!(err, NumError::Overflow);
                hit = true;
                break;
            }
        }
    }
    assert!(hit);
}

#[test]
fn rounding_table() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/numeric/rounding.json");
    let rows: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for row in rows.as_array().unwrap() {
        let x = e(row["value"].as_str().unwrap());
        let s = u8::try_from(row["scale"].as_u64().unwrap()).unwrap();
        for mode in Rounding::ALL {
            let got = x.round_to_scale(s, mode).unwrap();
            assert_eq!(
                behavior_core::decimal::fixed_text(&got, s),
                row[mode.as_str()].as_str().unwrap(),
                "{} at scale {s} with {}",
                row["value"],
                mode.as_str()
            );
        }
    }
}

#[test]
fn rounding_names_and_codes() {
    let names: Vec<&str> = Rounding::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        names,
        ["half_even", "half_up", "down", "up", "floor", "ceiling"]
    );
    for (i, r) in Rounding::ALL.iter().enumerate() {
        assert_eq!(usize::from(r.code()), i);
        assert_eq!(Rounding::parse(r.as_str()), Some(*r));
    }
    assert_eq!(Rounding::parse("bankers"), None);
}

/// Reference: floor/ceil of y = x·10^s with the tie rules, on i128 numerators.
fn reference(num: i64, den: i64, s: u32, mode: Rounding) -> i128 {
    let (n, d) = (i128::from(num) * 10i128.pow(s), i128::from(den));
    let fl = n.div_euclid(d);
    let exact = n.rem_euclid(d) == 0;
    if exact {
        return fl;
    }
    let ce = fl + 1;
    let twice_rem = 2 * n.rem_euclid(d);
    let positive = n > 0;
    match mode {
        Rounding::Floor => fl,
        Rounding::Ceiling => ce,
        Rounding::Down => {
            if positive {
                fl
            } else {
                ce
            }
        }
        Rounding::Up => {
            if positive {
                ce
            } else {
                fl
            }
        }
        Rounding::HalfUp | Rounding::HalfEven => {
            if twice_rem < d {
                fl
            } else if twice_rem > d {
                ce
            } else if mode == Rounding::HalfUp {
                if positive { ce } else { fl }
            } else if fl % 2 == 0 {
                fl
            } else {
                ce
            }
        }
    }
}

proptest! {
    #[test]
    fn rounding_matches_reference(num in -1_000_000i64..1_000_000, den in 1i64..10_000, s in 0u32..4) {
        let x = e(&format!("{num}/{den}"));
        for mode in Rounding::ALL {
            let got = x.round_to_scale(u8::try_from(s).unwrap(), mode).unwrap();
            let k = reference(num, den, s, mode);
            let expected = Exact::parse(&format!("{k}/{}", 10i128.pow(s))).unwrap();
            prop_assert_eq!(Exact::from_dec(&got), expected.clone());
            // On the grid and within one grid step.
            let step = Exact::parse(&format!("1/{}", 10i128.pow(s))).unwrap();
            let diff = Exact::from_dec(&got).sub(&x).unwrap();
            let abs = if diff < Exact::parse("0").unwrap() { Exact::parse("0").unwrap().sub(&diff).unwrap() } else { diff };
            prop_assert!(abs < step);
        }
    }
}
