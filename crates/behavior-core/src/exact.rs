//! Exact quantities and rounding modes (specs/003-fixed-scale-decimals, research R4, R6, R7).
//!
//! An exact quantity is a reduced rational whose numerator and denominator are both below
//! 2^512; anything larger is a numeric overflow, never an approximation. Rounding to a grid of
//! `10^-scale` happens only through [`Exact::round_to_scale`] with an explicit [`Rounding`].

use std::cmp::Ordering;
use std::fmt;

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::decimal::{Dec, NumError};

/// Largest bit length of a reduced numerator or denominator.
pub const MAX_BITS: u64 = 511;

/// The six rounding modes (contracts/numeric-semantics.md → Rounding modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rounding {
    HalfEven,
    HalfUp,
    Down,
    Up,
    Floor,
    Ceiling,
}

impl Rounding {
    pub const ALL: [Rounding; 6] = [
        Rounding::HalfEven,
        Rounding::HalfUp,
        Rounding::Down,
        Rounding::Up,
        Rounding::Floor,
        Rounding::Ceiling,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Rounding::HalfEven => "half_even",
            Rounding::HalfUp => "half_up",
            Rounding::Down => "down",
            Rounding::Up => "up",
            Rounding::Floor => "floor",
            Rounding::Ceiling => "ceiling",
        }
    }

    pub fn parse(s: &str) -> Option<Rounding> {
        Rounding::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// The byte used in content hashes.
    pub fn code(self) -> u8 {
        match self {
            Rounding::HalfEven => 0,
            Rounding::HalfUp => 1,
            Rounding::Down => 2,
            Rounding::Up => 3,
            Rounding::Floor => 4,
            Rounding::Ceiling => 5,
        }
    }
}

/// An exact rational quantity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Exact(BigRational);

fn pow10(n: u32) -> BigInt {
    num_traits::pow(BigInt::from(10), n as usize)
}

impl Exact {
    fn checked(r: BigRational) -> Result<Exact, NumError> {
        if r.numer().bits() > MAX_BITS || r.denom().bits() > MAX_BITS {
            return Err(NumError::ExactBound);
        }
        Ok(Exact(r))
    }

    pub fn zero() -> Exact {
        Exact(BigRational::zero())
    }

    /// Parses plain decimal notation (`-12.50`) or a fraction `n/d` (integers, `d > 0`).
    pub fn parse(s: &str) -> Result<Exact, NumError> {
        if let Some((n, d)) = s.split_once('/') {
            let n: BigInt = n.parse().map_err(|_| NumError::Invalid)?;
            let d: BigInt = d.parse().map_err(|_| NumError::Invalid)?;
            if !d.is_positive() {
                return Err(NumError::Invalid);
            }
            return Exact::checked(BigRational::new(n, d));
        }
        let (neg, body) = match s.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, s),
        };
        let (int, frac) = body.split_once('.').unwrap_or((body, ""));
        let ok = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty() || !ok(int) || !ok(frac) || (body.contains('.') && frac.is_empty()) {
            return Err(NumError::Invalid);
        }
        let digits: BigInt = format!("{int}{frac}")
            .parse()
            .map_err(|_| NumError::Invalid)?;
        let scale = u32::try_from(frac.len()).map_err(|_| NumError::Invalid)?;
        let n = if neg { -digits } else { digits };
        Exact::checked(BigRational::new(n, pow10(scale)))
    }

    /// Exact value of a decimal (a `Dec` has at most 96 mantissa bits: always within the bound).
    pub fn from_dec(d: &Dec) -> Exact {
        let (mantissa, scale) = d.mantissa_scale();
        Exact(BigRational::new(BigInt::from(mantissa), pow10(scale)))
    }

    pub fn from_i64(i: i64) -> Exact {
        Exact(BigRational::from_integer(BigInt::from(i)))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Bit lengths of the reduced numerator (absolute value) and denominator.
    pub fn bits(&self) -> (u64, u64) {
        (self.0.numer().bits(), self.0.denom().bits())
    }

    pub fn add(&self, o: &Exact) -> Result<Exact, NumError> {
        Exact::checked(&self.0 + &o.0)
    }

    pub fn sub(&self, o: &Exact) -> Result<Exact, NumError> {
        Exact::checked(&self.0 - &o.0)
    }

    pub fn mul(&self, o: &Exact) -> Result<Exact, NumError> {
        Exact::checked(&self.0 * &o.0)
    }

    pub fn div(&self, o: &Exact) -> Result<Exact, NumError> {
        if o.0.is_zero() {
            return Err(NumError::DivisionByZero);
        }
        Exact::checked(&self.0 / &o.0)
    }

    /// The finite decimal expansion, if the reduced denominator is `2^a · 5^b`.
    fn finite_decimal(&self) -> Option<String> {
        let mut d = self.0.denom().clone();
        let (two, five) = (BigInt::from(2), BigInt::from(5));
        let (mut twos, mut fives) = (0u32, 0u32);
        while d.is_even() {
            d /= &two;
            twos += 1;
        }
        while (&d % &five).is_zero() {
            d /= &five;
            fives += 1;
        }
        if !d.is_one() {
            return None;
        }
        let scale = twos.max(fives);
        let scaled = self.0.numer() * (pow10(scale) / self.0.denom());
        Some(digits_with_scale(&scaled, scale))
    }

    /// Normalized finite decimal if one exists, otherwise `n/d` in lowest terms.
    pub fn to_text(&self) -> String {
        match self.finite_decimal() {
            Some(s) => s,
            None => format!("{}/{}", self.0.numer(), self.0.denom()),
        }
    }

    /// The same value as a decimal (`c · 10^−s`, `|c| < 10^28`, `s ≤ 28`), if it is one.
    pub fn to_dec(&self) -> Result<Dec, NumError> {
        let text = self.finite_decimal().ok_or(NumError::NotRepresentable)?;
        let d = Dec::parse_str(&text).map_err(|_| NumError::NotRepresentable)?;
        let (m, scale) = d.mantissa_scale();
        if scale > 28 || m.unsigned_abs() >= 10u128.pow(28) || Exact::from_dec(&d) != *self {
            return Err(NumError::NotRepresentable);
        }
        Ok(d)
    }

    /// Rounds to the grid `10^-scale` with `mode`; the result must fit the engine's decimals.
    pub fn round_to_scale(&self, scale: u8, mode: Rounding) -> Result<Dec, NumError> {
        let p = pow10(u32::from(scale));
        let y = &self.0 * BigRational::from_integer(p);
        let fl = y.floor().to_integer();
        let k = if y.is_integer() {
            fl
        } else {
            let ce = &fl + BigInt::one();
            let positive = y.is_positive();
            let twice_frac = (&y - BigRational::from_integer(fl.clone()))
                * BigRational::from_integer(BigInt::from(2));
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
                    match twice_frac.cmp(&BigRational::one()) {
                        Ordering::Less => fl,
                        Ordering::Greater => ce,
                        Ordering::Equal if mode == Rounding::HalfUp => {
                            if positive {
                                ce
                            } else {
                                fl
                            }
                        }
                        Ordering::Equal => {
                            if fl.is_even() {
                                fl
                            } else {
                                ce
                            }
                        }
                    }
                }
            }
        };
        let text = digits_with_scale(&k, u32::from(scale));
        Dec::parse_str(&text).map_err(|_| NumError::Overflow)
    }
}

/// `k / 10^scale` in plain notation, normalized (no trailing zeros, `-0` → `0`).
fn digits_with_scale(k: &BigInt, scale: u32) -> String {
    let neg = k.sign() == Sign::Minus;
    let digits = k.abs().to_string();
    let s = scale as usize;
    let text = if s == 0 {
        digits
    } else {
        let padded = format!("{digits:0>width$}", width = s + 1);
        let (int, frac) = padded.split_at(padded.len() - s);
        let frac = frac.trim_end_matches('0');
        if frac.is_empty() {
            int.to_string()
        } else {
            format!("{int}.{frac}")
        }
    };
    if neg && text.chars().any(|c| c != '0' && c != '.') {
        format!("-{text}")
    } else {
        text
    }
}

impl PartialOrd for Exact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Exact {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl fmt::Display for Exact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}
