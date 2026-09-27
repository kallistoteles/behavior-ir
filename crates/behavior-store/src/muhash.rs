//! The state identity (research R3): a MuHash3072 multiset hash over the content hashes of the
//! current entities. Order independent and incremental: a commit multiplies in the new content
//! hashes and divides out the replaced ones, so its cost does not depend on the store size.

use num_bigint::BigUint;
use num_traits::{One, Zero};
use sha2::{Digest, Sha256};

use behavior_verify::hashing::{display, tagged};

use crate::documents::{R, StoreError, TAG_STATE};

const EXPAND_TAG: &str = "behavior.muhash3072.v1";
const BYTES: usize = 384;

/// `p = 2^3072 − 1103717`.
fn prime() -> BigUint {
    (BigUint::one() << 3072u32) - BigUint::from(1_103_717u32)
}

/// The 3072-bit group element of a content hash (`sha256:<hex>`): SHA-256 in counter mode.
fn element(content_hash: &str) -> R<BigUint> {
    let hex = content_hash
        .strip_prefix("sha256:")
        .ok_or_else(|| StoreError::BundleInvalid(format!("not a content hash: {content_hash}")))?;
    let mut bytes = Vec::with_capacity(BYTES);
    for i in 0..(BYTES / 32) {
        let mut h = Sha256::new();
        h.update(EXPAND_TAG.as_bytes());
        h.update([0u8]);
        h.update(hex.as_bytes());
        h.update([u8::try_from(i).unwrap_or(u8::MAX)]);
        bytes.extend_from_slice(&h.finalize());
    }
    let e = BigUint::from_bytes_be(&bytes) % prime();
    // Zero has no inverse; it occurs with negligible probability and maps to 1.
    Ok(if e.is_zero() { BigUint::one() } else { e })
}

/// A multiset accumulator kept as a fraction `num / den` (mod p), so removals need no inverse
/// until the identity is taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accumulator {
    num: BigUint,
    den: BigUint,
}

impl Default for Accumulator {
    fn default() -> Self {
        Accumulator::empty()
    }
}

impl Accumulator {
    /// The empty multiset.
    pub fn empty() -> Self {
        Accumulator {
            num: BigUint::one(),
            den: BigUint::one(),
        }
    }

    pub fn insert(&mut self, content_hash: &str) -> R<()> {
        self.num = (&self.num * element(content_hash)?) % prime();
        Ok(())
    }

    pub fn remove(&mut self, content_hash: &str) -> R<()> {
        self.den = (&self.den * element(content_hash)?) % prime();
        Ok(())
    }

    /// The normalized value `num · den⁻¹ mod p`.
    fn normalized(&self) -> R<BigUint> {
        let p = prime();
        let inv = self
            .den
            .modinv(&p)
            .ok_or_else(|| StoreError::BundleInvalid("state accumulator not invertible".into()))?;
        Ok((&self.num * inv) % p)
    }

    /// The state identity: `behavior.state.v1` over the normalized value as 384 big-endian bytes.
    pub fn state_id(&self) -> R<String> {
        let v = self.normalized()?.to_bytes_be();
        let mut bytes = vec![0u8; BYTES - v.len()];
        bytes.extend_from_slice(&v);
        Ok(display(&tagged(TAG_STATE, &bytes)))
    }

    /// Hex encodings of numerator and denominator (stored in the head).
    pub fn to_hex(&self) -> (String, String) {
        (self.num.to_str_radix(16), self.den.to_str_radix(16))
    }

    pub fn from_hex(num: &str, den: &str) -> R<Self> {
        let parse = |s: &str| {
            BigUint::parse_bytes(s.as_bytes(), 16)
                .ok_or_else(|| StoreError::Backend(format!("bad accumulator value `{s}`")))
        };
        Ok(Accumulator {
            num: parse(num)?,
            den: parse(den)?,
        })
    }

    /// The same value with denominator 1 (what a fresh computation of the set gives).
    pub fn normalize(&self) -> R<Self> {
        Ok(Accumulator {
            num: self.normalized()?,
            den: BigUint::one(),
        })
    }
}
