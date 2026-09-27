//! Hashes of verification and governance objects (research R8, R9).
//!
//! Finding hashes and check keys use a small length-prefixed encoding under domain tags;
//! governance documents are hashed over their canonical JSON (without their `hash` field).

use serde_json::Value;
use sha2::{Digest, Sha256};

use behavior_core::canonical::{CanonicalError, to_canonical_string};

pub const TAG_FINDING: &str = "behavior.finding.v1";
pub const TAG_CHECK: &str = "behavior.check.v1";
pub const TAG_VERIFICATION: &str = "behavior.verification.v1";
pub const TAG_WAIVER: &str = "behavior.waiver.v1";
pub const TAG_POLICY: &str = "behavior.policy.v1";
pub const TAG_AUTHORIZATION: &str = "behavior.authorization.v1";
pub const TAG_TRANSITION: &str = "behavior.transition.v1";
pub const TAG_PROFILE: &str = "behavior.profile.v1";

/// `SHA-256(tag ‖ 0x00 ‖ bytes)`.
pub fn tagged(tag: &str, bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(tag.as_bytes());
    h.update([0u8]);
    h.update(bytes);
    h.finalize().into()
}

pub fn display(h: &[u8; 32]) -> String {
    let mut s = String::from("sha256:");
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn push_str(buf: &mut Vec<u8>, s: &str) {
    let n = u32::try_from(s.len()).unwrap_or(u32::MAX);
    buf.extend_from_slice(&n.to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
}

/// Identity of a finding: its kind and the content hashes it cites, never counterexample values
/// or messages (FR-019).
pub fn finding_hash(kind: &str, cites: &[(&str, String)]) -> String {
    let mut buf = Vec::new();
    push_str(&mut buf, kind);
    let n = u32::try_from(cites.len()).unwrap_or(u32::MAX);
    buf.extend_from_slice(&n.to_be_bytes());
    for (role, hash) in cites {
        push_str(&mut buf, role);
        push_str(&mut buf, hash);
    }
    display(&tagged(TAG_FINDING, &buf))
}

/// Cache key of a check: everything its outcome depends on.
pub fn check_key(
    kind: &str,
    subject_hashes: &[String],
    profile_hash: &str,
    verifier_version: &str,
    solver_version: &str,
) -> String {
    let mut buf = Vec::new();
    push_str(&mut buf, kind);
    let n = u32::try_from(subject_hashes.len()).unwrap_or(u32::MAX);
    buf.extend_from_slice(&n.to_be_bytes());
    for h in subject_hashes {
        push_str(&mut buf, h);
    }
    push_str(&mut buf, profile_hash);
    push_str(&mut buf, verifier_version);
    push_str(&mut buf, solver_version);
    display(&tagged(TAG_CHECK, &buf))
}

/// Hash of a governance document: canonical JSON without its `hash` field, under `tag`.
pub fn document_hash(tag: &str, doc: &Value) -> Result<String, CanonicalError> {
    let mut v = doc.clone();
    if let Value::Object(map) = &mut v {
        map.remove("hash");
    }
    let text = to_canonical_string(&v)?;
    Ok(display(&tagged(tag, text.as_bytes())))
}
