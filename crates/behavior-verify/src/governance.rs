//! Waivers, signed attestations, execution policies, and commit authorizations
//! (contracts/governance.md, research R9, R10).
//!
//! Governance never changes a verification result: the attestation stays what the verifier
//! said, and the policy decides whether a proposed transition may be committed. Identity claims
//! are data; authority requires evidence — a waiver counts only with a valid Ed25519 signature by
//! a key the policy trusts for that kind of finding.

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde_json::{Map, Value as Json, json};

use behavior_core::canonical::to_canonical_string;
use behavior_core::semantic::module::Module;

use crate::hashing::{
    TAG_AUTHORIZATION, TAG_POLICY, TAG_TRANSITION, TAG_VERIFICATION, TAG_WAIVER, document_hash,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GovernanceError {
    #[error("invalid {0}: {1}")]
    Invalid(&'static str, String),
}

type R<T> = Result<T, GovernanceError>;

fn bad<T>(what: &'static str, msg: impl Into<String>) -> R<T> {
    Err(GovernanceError::Invalid(what, msg.into()))
}

fn parse(what: &'static str, text: &str) -> R<Map<String, Json>> {
    match serde_json::from_str::<Json>(text) {
        Ok(Json::Object(m)) => Ok(m),
        Ok(_) => bad(what, "expected an object"),
        Err(e) => bad(what, e.to_string()),
    }
}

/// Rejects fields outside `allowed` and returns required string fields.
fn fields(
    what: &'static str,
    m: &Map<String, Json>,
    required: &[&str],
    optional: &[&str],
) -> R<()> {
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return bad(what, format!("unknown field `{k}`"));
        }
    }
    for r in required {
        if !m.contains_key(*r) {
            return bad(what, format!("missing field `{r}`"));
        }
    }
    Ok(())
}

fn string(what: &'static str, m: &Map<String, Json>, key: &str) -> R<String> {
    match m.get(key) {
        Some(Json::String(s)) => Ok(s.clone()),
        _ => bad(what, format!("`{key}` must be a string")),
    }
}

fn strings(what: &'static str, v: Option<&Json>, key: &str) -> R<Vec<String>> {
    match v {
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| {
                i.as_str().map(str::to_string).ok_or_else(|| {
                    GovernanceError::Invalid(what, format!("`{key}` must list strings"))
                })
            })
            .collect(),
        Some(_) => bad(what, format!("`{key}` must be a list")),
    }
}

fn hex(what: &'static str, s: &str, bytes: usize) -> R<Vec<u8>> {
    if s.len() != bytes * 2
        || !s
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return bad(what, format!("expected {bytes} bytes of lowercase hex"));
    }
    (0..bytes)
        .map(|i| {
            u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
                .map_err(|e| GovernanceError::Invalid(what, e.to_string()))
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_bytes(what: &'static str, display: &str) -> R<[u8; 32]> {
    let Some(h) = display.strip_prefix("sha256:") else {
        return bad(what, "expected a sha256: hash");
    };
    let v = hex(what, h, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Ok(out)
}

/// `YYYY-MM-DDTHH:MM:SSZ`: fixed width, so string order is time order.
fn timestamp(what: &'static str, s: &str) -> R<()> {
    let b = s.as_bytes();
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let ok = b.len() == 20
        && digits.iter().all(|&i| b[i].is_ascii_digit())
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z';
    if ok {
        Ok(())
    } else {
        bad(
            what,
            format!("`{s}` is not an RFC 3339 UTC time (YYYY-MM-DDTHH:MM:SSZ)"),
        )
    }
}

/// A waiver: governance evidence for one finding of one behavior version. It has no reviewer
/// field; who stands behind it is shown only by signatures.
#[derive(Debug, Clone)]
pub struct Waiver {
    pub behavior_version: String,
    pub finding_hash: String,
    pub profile_hash: String,
    pub verifier_version: String,
    pub expires_at: Option<String>,
    pub hash: String,
}

pub fn decode_waiver(text: &str) -> R<Waiver> {
    const W: &str = "waiver";
    let mut m = parse(W, text)?;
    m.remove("hash");
    fields(
        W,
        &m,
        &[
            "behavior_version",
            "finding_hash",
            "profile_hash",
            "verifier_version",
            "rationale",
        ],
        &["expires_at"],
    )?;
    let expires_at = match m.get("expires_at") {
        None => None,
        Some(_) => {
            let t = string(W, &m, "expires_at")?;
            timestamp(W, &t)?;
            Some(t)
        }
    };
    string(W, &m, "rationale")?;
    let w = Waiver {
        behavior_version: string(W, &m, "behavior_version")?,
        finding_hash: string(W, &m, "finding_hash")?,
        profile_hash: string(W, &m, "profile_hash")?,
        verifier_version: string(W, &m, "verifier_version")?,
        expires_at,
        hash: document_hash(TAG_WAIVER, &Json::Object(m.clone()))
            .map_err(|e| GovernanceError::Invalid(W, e.to_string()))?,
    };
    for h in [&w.behavior_version, &w.finding_hash, &w.profile_hash] {
        hash_bytes(W, h)?;
    }
    Ok(w)
}

/// The content hash of a waiver (the value signatures are made over).
pub fn waiver_hash(text: &str) -> R<String> {
    Ok(decode_waiver(text)?.hash)
}

fn message(waiver_hash: &str) -> R<Vec<u8>> {
    let mut msg = TAG_WAIVER.as_bytes().to_vec();
    msg.push(0);
    msg.extend_from_slice(&hash_bytes("signed attestation", waiver_hash)?);
    Ok(msg)
}

fn signing_key(seed_hex: &str) -> R<SigningKey> {
    let seed = hex("key seed", seed_hex.trim(), 32)?;
    let mut s = [0u8; 32];
    s.copy_from_slice(&seed);
    Ok(SigningKey::from_bytes(&s))
}

/// `ed25519:<public key hex>` of a 32-byte seed.
pub fn key_id(seed_hex: &str) -> R<String> {
    Ok(format!(
        "ed25519:{}",
        to_hex(signing_key(seed_hex)?.verifying_key().as_bytes())
    ))
}

/// A detached signed attestation over a waiver's hash.
pub fn sign_waiver(seed_hex: &str, waiver_text: &str) -> R<Json> {
    let key = signing_key(seed_hex)?;
    let hash = waiver_hash(waiver_text)?;
    let sig = key.sign(&message(&hash)?);
    Ok(json!({
        "waiver_hash": hash,
        "key_id": format!("ed25519:{}", to_hex(key.verifying_key().as_bytes())),
        "signature": to_hex(&sig.to_bytes()),
    }))
}

#[derive(Debug, Clone)]
struct Signed {
    waiver_hash: String,
    key_id: String,
    signature: String,
}

fn decode_signed(text: &str) -> R<Signed> {
    const S: &str = "signed attestation";
    let m = parse(S, text)?;
    fields(S, &m, &["waiver_hash", "key_id", "signature"], &[])?;
    let s = Signed {
        waiver_hash: string(S, &m, "waiver_hash")?,
        key_id: string(S, &m, "key_id")?,
        signature: string(S, &m, "signature")?,
    };
    hash_bytes(S, &s.waiver_hash)?;
    Ok(s)
}

/// Whether `s` is a valid Ed25519 signature by its key over its waiver hash.
fn signature_valid(s: &Signed) -> bool {
    let check = || -> R<bool> {
        let Some(pk) = s.key_id.strip_prefix("ed25519:") else {
            return Ok(false);
        };
        let pk: [u8; 32] = hex("key id", pk, 32)?
            .try_into()
            .map_err(|_| GovernanceError::Invalid("key id", "length".into()))?;
        let sig: [u8; 64] = hex("signature", &s.signature, 64)?
            .try_into()
            .map_err(|_| GovernanceError::Invalid("signature", "length".into()))?;
        let Ok(vk) = VerifyingKey::from_bytes(&pk) else {
            return Ok(false);
        };
        Ok(vk
            .verify_strict(&message(&s.waiver_hash)?, &Signature::from_bytes(&sig))
            .is_ok())
    };
    check().unwrap_or(false)
}

#[derive(Debug, Clone)]
struct TrustedKey {
    principal: String,
    roles: BTreeSet<String>,
}

/// A declarative, content-addressed execution policy.
#[derive(Debug, Clone)]
pub struct Policy {
    pub require: String,
    pub required_profiles: Vec<String>,
    pub waivable: BTreeSet<String>,
    pub forbidden: BTreeSet<String>,
    trusted_keys: BTreeMap<String, TrustedKey>,
    pub required_roles: BTreeMap<String, Vec<String>>,
    pub accept_expired: bool,
    pub hash: String,
}

pub fn decode_policy(text: &str) -> R<Policy> {
    const P: &str = "policy";
    let mut m = parse(P, text)?;
    m.remove("hash");
    fields(
        P,
        &m,
        &["policy_version", "require"],
        &[
            "required_profiles",
            "waivable",
            "forbidden",
            "trusted_keys",
            "required_roles",
            "accept_expired",
        ],
    )?;
    if string(P, &m, "policy_version")? != "1" {
        return bad(P, "unsupported `policy_version` (supported: \"1\")");
    }
    let require = string(P, &m, "require")?;
    if require != "verified" && require != "verified_or_waived" {
        return bad(
            P,
            "`require` must be \"verified\" or \"verified_or_waived\"",
        );
    }
    let mut trusted_keys = BTreeMap::new();
    match m.get("trusted_keys") {
        None => {}
        Some(Json::Array(items)) => {
            for k in items {
                let Json::Object(km) = k else {
                    return bad(P, "`trusted_keys` entries must be objects");
                };
                fields(P, km, &["key_id", "principal"], &["roles"])?;
                let id = string(P, km, "key_id")?;
                let Some(pk) = id.strip_prefix("ed25519:") else {
                    return bad(P, "key ids must start with ed25519:");
                };
                hex(P, pk, 32)?;
                let key = TrustedKey {
                    principal: string(P, km, "principal")?,
                    roles: strings(P, km.get("roles"), "roles")?.into_iter().collect(),
                };
                if trusted_keys.insert(id.clone(), key).is_some() {
                    return bad(P, format!("duplicate trusted key {id}"));
                }
            }
        }
        Some(_) => return bad(P, "`trusted_keys` must be a list"),
    }
    let mut required_roles = BTreeMap::new();
    match m.get("required_roles") {
        None => {}
        Some(Json::Object(rm)) => {
            for (kind, roles) in rm {
                required_roles.insert(kind.clone(), strings(P, Some(roles), "required_roles")?);
            }
        }
        Some(_) => return bad(P, "`required_roles` must be an object"),
    }
    let accept_expired = match m.get("accept_expired") {
        None => false,
        Some(Json::Bool(b)) => *b,
        Some(_) => return bad(P, "`accept_expired` must be a boolean"),
    };
    let waivable = if m.contains_key("waivable") {
        strings(P, m.get("waivable"), "waivable")?
    } else {
        vec!["inconclusive".into()]
    };
    Ok(Policy {
        require,
        required_profiles: strings(P, m.get("required_profiles"), "required_profiles")?,
        waivable: waivable.into_iter().collect(),
        forbidden: strings(P, m.get("forbidden"), "forbidden")?
            .into_iter()
            .collect(),
        trusted_keys,
        required_roles,
        accept_expired,
        hash: document_hash(TAG_POLICY, &Json::Object(m))
            .map_err(|e| GovernanceError::Invalid(P, e.to_string()))?,
    })
}

/// Parses an attestation and checks that its hash matches its content (without `cached`
/// flags).
fn decode_attestation(text: &str) -> R<Json> {
    const A: &str = "attestation";
    let m = parse(A, text)?;
    let mut v = Json::Object(m);
    let claimed = v["hash"].as_str().map(str::to_string);
    if let Some(Json::Array(checks)) = v.get_mut("checks") {
        for c in checks {
            if let Json::Object(cm) = c {
                cm.remove("cached");
            }
        }
    }
    let actual = document_hash(TAG_VERIFICATION, &v)
        .map_err(|e| GovernanceError::Invalid(A, e.to_string()))?;
    if claimed.as_deref() != Some(actual.as_str()) {
        return bad(A, "hash does not match its content");
    }
    let blocking = v["findings"]
        .as_array()
        .is_some_and(|f| f.iter().any(|f| f["severity"] == "blocking"));
    let verified = v["result"] == "verified";
    if verified == blocking {
        return bad(A, "`result` contradicts its findings");
    }
    Ok(v)
}

/// A commit authorization (contracts/governance.md).
#[derive(Debug, Clone)]
pub struct Authorization {
    pub value: Json,
    pub decision: String,
    pub hash: String,
}

impl Authorization {
    pub fn to_json_string(&self) -> String {
        to_canonical_string(&self.value).unwrap_or_default()
    }
}

fn reason(code: &str, message: String, finding: Option<&str>) -> Json {
    let mut r = json!({"code": code, "message": message});
    if let (Some(f), Json::Object(m)) = (finding, &mut r) {
        m.insert("finding_hash".into(), json!(f));
    }
    r
}

/// Failure precedence when no waiver covers a finding: the most specific explanation first.
const FAILURES: [&str; 5] = [
    "invalid_signature",
    "missing_role",
    "untrusted_key",
    "missing_signature",
    "waiver_expired",
];

/// Judges an attestation of `subject` (the attestation's `subject_key`) under an execution
/// policy: verified, or every blocking finding waived as the policy allows. Adds the refusal
/// reasons and the waivers used. Shared by actions (`behavior_version`) and migrations
/// (`migration_hash`; a migration waiver names the migration hash as its subject).
#[allow(clippy::too_many_arguments)]
fn judge(
    policy: &Policy,
    attestation: &Option<Json>,
    subject_key: &str,
    subject: &str,
    waivers: &BTreeMap<String, Waiver>,
    signed: &[Signed],
    now: &str,
    reasons: &mut Vec<Json>,
    used: &mut Vec<Json>,
) {
    match &attestation {
        _ if !reasons.is_empty() => {}
        None => reasons.push(reason(
            "unverified",
            "no verification attestation".into(),
            None,
        )),
        Some(a) if a[subject_key] != subject => reasons.push(reason(
            "unverified",
            format!(
                "the attestation is for another {}",
                subject_key.replace('_', " ")
            ),
            None,
        )),
        Some(a)
            if !policy.required_profiles.is_empty()
                && !policy
                    .required_profiles
                    .iter()
                    .any(|p| a["profile"]["hash"] == p.as_str()) =>
        {
            reasons.push(reason(
                "profile_not_accepted",
                "the attestation's profile is not required by the policy".into(),
                None,
            ))
        }
        Some(a) if a["result"] == "verified" => {}
        Some(a) => {
            let mut blocking: Vec<&Json> = a["findings"]
                .as_array()
                .map(|f| f.iter().filter(|f| f["severity"] == "blocking").collect())
                .unwrap_or_default();
            blocking.sort_by(|x, y| x["hash"].as_str().cmp(&y["hash"].as_str()));
            for f in blocking {
                let fh = f["hash"].as_str().unwrap_or_default();
                let kind = f["kind"].as_str().unwrap_or_default();
                if policy.require != "verified_or_waived" {
                    reasons.push(reason(
                        "not_verified",
                        format!("blocking {kind} finding; the policy requires verification"),
                        Some(fh),
                    ));
                    continue;
                }
                if policy.forbidden.contains(kind) || !policy.waivable.contains(kind) {
                    reasons.push(reason(
                        "finding_not_waivable",
                        format!("the policy does not allow waiving {kind} findings"),
                        Some(fh),
                    ));
                    continue;
                }
                let candidates: Vec<&Waiver> = waivers
                    .values()
                    .filter(|w| {
                        w.finding_hash == fh
                            && a[subject_key] == w.behavior_version.as_str()
                            && a["profile"]["hash"] == w.profile_hash.as_str()
                            && a["verifier_version"] == w.verifier_version.as_str()
                    })
                    .collect();
                if candidates.is_empty() {
                    reasons.push(reason(
                        "not_verified",
                        format!("blocking {kind} finding without a waiver"),
                        Some(fh),
                    ));
                    continue;
                }
                let roles = policy.required_roles.get(kind);
                let mut failures = BTreeSet::new();
                let mut accepted = None;
                'waivers: for w in candidates {
                    if !policy.accept_expired && w.expires_at.as_deref().is_some_and(|t| t <= now) {
                        failures.insert("waiver_expired");
                        continue;
                    }
                    let sigs: Vec<&Signed> =
                        signed.iter().filter(|s| s.waiver_hash == w.hash).collect();
                    if sigs.is_empty() {
                        failures.insert("missing_signature");
                    }
                    for s in sigs {
                        let Some(key) = policy.trusted_keys.get(&s.key_id) else {
                            failures.insert("untrusted_key");
                            continue;
                        };
                        if roles.is_some_and(|r| !r.iter().any(|r| key.roles.contains(r))) {
                            failures.insert("missing_role");
                            continue;
                        }
                        if !signature_valid(s) {
                            failures.insert("invalid_signature");
                            continue;
                        }
                        accepted = Some(json!({
                            "waiver_hash": w.hash, "finding_hash": fh, "key_id": s.key_id,
                            "signature": s.signature, "principal": key.principal,
                        }));
                        break 'waivers;
                    }
                }
                match accepted {
                    Some(u) => used.push(u),
                    None => {
                        let code = FAILURES
                            .iter()
                            .find(|c| failures.contains(**c))
                            .copied()
                            .unwrap_or("not_verified");
                        reasons.push(reason(
                            code,
                            format!("no acceptable waiver for the blocking {kind} finding"),
                            Some(fh),
                        ));
                    }
                }
            }
        }
    }
}

/// Decides whether the transition in `record` may be committed under `policy`.
#[allow(clippy::too_many_arguments)]
pub fn authorize(
    policy_json: &str,
    module: &Module,
    record_json: &str,
    attestation_json: Option<&str>,
    waivers_json: &[String],
    signed_json: &[String],
    now: &str,
) -> R<Authorization> {
    let policy = decode_policy(policy_json)?;
    timestamp("time", now)?;
    let record = Json::Object(parse("record", record_json)?);
    let attestation = attestation_json.map(decode_attestation).transpose()?;
    let mut waivers: BTreeMap<String, Waiver> = BTreeMap::new();
    for w in waivers_json {
        let w = decode_waiver(w)?;
        waivers.insert(w.hash.clone(), w);
    }
    let mut signed: Vec<Signed> = signed_json
        .iter()
        .map(|s| decode_signed(s))
        .collect::<R<_>>()?;
    signed.sort_by(|a, b| (&a.key_id, &a.signature).cmp(&(&b.key_id, &b.signature)));

    let behavior_version = module.behavior_version();
    let transition_hash = document_hash(TAG_TRANSITION, &record)
        .map_err(|e| GovernanceError::Invalid("record", e.to_string()))?;
    let mut reasons = Vec::new();
    let mut used = Vec::new();

    if record["behavior_version"] != behavior_version.as_str() {
        reasons.push(reason(
            "behavior_mismatch",
            "the record was made by another behavior version".into(),
            None,
        ));
    } else if !behavior_core::replay(module, record_json).matches {
        reasons.push(reason(
            "record_not_reproducible",
            "replaying the record gives a different outcome".into(),
            None,
        ));
    } else if record["result"] != "ALLOW" {
        reasons.push(reason(
            "nothing_to_commit",
            format!("the record's result is {}", record["result"]),
            None,
        ));
    }
    judge(
        &policy,
        &attestation,
        "behavior_version",
        &behavior_version,
        &waivers,
        &signed,
        now,
        &mut reasons,
        &mut used,
    );
    let decision = if reasons.is_empty() {
        "allow"
    } else {
        "refuse"
    };
    if decision == "refuse" {
        used.clear();
    }
    let mut value = json!({
        "authorization_version": "1",
        "behavior_version": behavior_version,
        "transition_hash": transition_hash,
        "policy_hash": policy.hash,
        "verification": attestation.as_ref().map(|a| json!({"attestation_hash": a["hash"], "result": a["result"]})),
        "waivers_used": used,
        "now": now,
        "decision": decision,
        "reasons": reasons,
    });
    let hash = document_hash(TAG_AUTHORIZATION, &value)
        .map_err(|e| GovernanceError::Invalid("authorization", e.to_string()))?;
    if let Json::Object(m) = &mut value {
        m.insert("hash".into(), json!(hash));
    }
    Ok(Authorization {
        value,
        decision: decision.to_string(),
        hash,
    })
}

/// Decides whether a migration may be committed on one store state under an execution policy
/// (feature 009, research R8). The authorization binds the migration (its hash, source and target
/// schemas) and the store state it is applied to (`data_version`); like an action's, it needs a
/// verified attestation of that migration, or the policy's waivers for its blocking findings.
/// Runtime validation at application is never replaced by it.
pub fn authorize_migration(
    policy_json: &str,
    migration: &behavior_core::migration::Migration,
    data_version: &str,
    attestation_json: Option<&str>,
    waivers_json: &[String],
    signed_json: &[String],
    now: &str,
) -> R<Authorization> {
    let policy = decode_policy(policy_json)?;
    timestamp("time", now)?;
    let attestation = attestation_json.map(decode_attestation).transpose()?;
    let mut waivers: BTreeMap<String, Waiver> = BTreeMap::new();
    for w in waivers_json {
        let w = decode_waiver(w)?;
        waivers.insert(w.hash.clone(), w);
    }
    let mut signed: Vec<Signed> = signed_json
        .iter()
        .map(|s| decode_signed(s))
        .collect::<R<_>>()?;
    signed.sort_by(|a, b| (&a.key_id, &a.signature).cmp(&(&b.key_id, &b.signature)));
    let migration_hash = migration.hash();
    let mut reasons = Vec::new();
    let mut used = Vec::new();
    judge(
        &policy,
        &attestation,
        "migration_hash",
        &migration_hash,
        &waivers,
        &signed,
        now,
        &mut reasons,
        &mut used,
    );
    let decision = if reasons.is_empty() {
        "allow"
    } else {
        "refuse"
    };
    if decision == "refuse" {
        used.clear();
    }
    let mut value = json!({
        "authorization_version": "1",
        "kind": "migration",
        "migration_hash": migration_hash,
        "source": migration.source_schema(),
        "target": migration.target_schema(),
        "data_version": data_version,
        "policy_hash": policy.hash,
        "verification": attestation.as_ref().map(|a| json!({"attestation_hash": a["hash"], "result": a["result"]})),
        "waivers_used": used,
        "now": now,
        "decision": decision,
        "reasons": reasons,
    });
    let hash = document_hash(TAG_AUTHORIZATION, &value)
        .map_err(|e| GovernanceError::Invalid("authorization", e.to_string()))?;
    if let Json::Object(m) = &mut value {
        m.insert("hash".into(), json!(hash));
    }
    Ok(Authorization {
        value,
        decision: decision.to_string(),
        hash,
    })
}
