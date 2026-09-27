#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US2 scenarios 5–12: waivers are signed governance evidence; the policy decides commits;
//! verification results never change.

mod common;

use behavior_core::semantic::module::Module;
use behavior_verify::governance::{Authorization, authorize, sign_waiver, waiver_hash};
use behavior_verify::solver::Z3Process;
use behavior_verify::{Attestation, CheckKind, Profile, verify};
use serde_json::{Value, json};

const NOW: &str = "2026-09-25T12:00:00Z";

fn gov(name: &str) -> String {
    common::read(&common::fixtures().join("governance").join(name))
}

fn seed(key: &str) -> String {
    let keys: Value = serde_json::from_str(&gov("keys.json")).unwrap();
    keys[key]["seed"].as_str().unwrap().to_string()
}

fn key_id(key: &str) -> String {
    let keys: Value = serde_json::from_str(&gov("keys.json")).unwrap();
    keys[key]["key_id"].as_str().unwrap().to_string()
}

fn module(name: &str) -> Module {
    behavior_core::admit(&common::read(&common::fixtures().join("verify").join(name))).unwrap()
}

fn record(m: &Module) -> String {
    behavior_core::evaluate(m, &gov("approve_request.json")).to_json_string()
}

/// Forced inconclusive: `purchase_remaining` is easy for the solver (every check is decided under
/// the default budget); a resource limit of 1 makes every check inconclusive so waivers can be
/// tested. Since feature 004 no fixture is inconclusive on its own.
fn forced_inconclusive() -> (Module, Attestation) {
    let m = module("purchase_remaining.json");
    let a = attest(&m, 1);
    assert!(
        a.findings().iter().all(|f| f["kind"] == "inconclusive"),
        "{}",
        a.to_json_string()
    );
    // Only the forced budget may make these checks inconclusive.
    for c in a.value["checks"].as_array().unwrap() {
        assert_eq!(c["reason"], "resource_limit", "{c}");
    }
    (m, a)
}

fn attest(m: &Module, rlimit: u64) -> Attestation {
    let profile = Profile {
        checks: vec![CheckKind::Preservation],
        rlimit,
        ..Profile::default()
    };
    verify(m, &profile, None, &Z3Process::from_env().unwrap())
}

/// A waiver for `finding` of `a`, optionally overriding fields.
fn waiver(a: &Attestation, finding: &Value, patch: Value) -> String {
    let mut w = json!({
        "behavior_version": a.value["behavior_version"],
        "finding_hash": finding["hash"],
        "profile_hash": a.value["profile"]["hash"],
        "verifier_version": a.value["verifier_version"],
        "rationale": "the solver budget is too small for this test; reviewed by hand",
    });
    for (k, v) in patch.as_object().unwrap() {
        w[k] = v.clone();
    }
    w.to_string()
}

fn sign(w: &str, key: &str) -> String {
    sign_waiver(&seed(key), w).unwrap().to_string()
}

/// Waivers for every finding of `a`, each signed by `keys`.
fn waive_all(a: &Attestation, patch: Value, keys: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut ws = Vec::new();
    let mut ss = Vec::new();
    for f in a.findings() {
        let w = waiver(a, &f, patch.clone());
        for k in keys {
            ss.push(sign(&w, k));
        }
        ws.push(w);
    }
    (ws, ss)
}

fn run(
    policy: &str,
    m: &Module,
    a: Option<&Attestation>,
    ws: &[String],
    ss: &[String],
) -> Authorization {
    let text = a.map(|a| a.to_json_string());
    authorize(policy, m, &record(m), text.as_deref(), ws, ss, NOW).unwrap()
}

fn codes(auth: &Authorization) -> Vec<String> {
    let mut c: Vec<String> = auth.value["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["code"].as_str().unwrap().to_string())
        .collect();
    c.dedup();
    c
}

#[test]
fn verified_allows_and_unverified_refuses() {
    let m = module("purchase_fixed.json");
    let ok = attest(&m, Profile::default().rlimit);
    assert_eq!(ok.result, "verified");
    assert_eq!(
        run(&gov("require_verified.json"), &m, Some(&ok), &[], &[]).decision,
        "allow"
    );
    let none = run(&gov("require_verified.json"), &m, None, &[], &[]);
    assert_eq!(
        (none.decision.as_str(), codes(&none)),
        ("refuse", vec!["unverified".to_string()])
    );
    let (m2, pending) = forced_inconclusive();
    let auth = run(&gov("require_verified.json"), &m2, Some(&pending), &[], &[]);
    assert_eq!(
        (auth.decision.as_str(), codes(&auth)),
        ("refuse", vec!["not_verified".to_string()])
    );
}

#[test]
fn signed_waivers_allow_forced_inconclusive_without_changing_verification() {
    let (m, a) = forced_inconclusive();
    assert_eq!(a.result, "not_verified");
    let (ws, ss) = waive_all(&a, json!({}), &["A"]);
    let auth = run(&gov("verified_or_waived.json"), &m, Some(&a), &ws, &ss);
    assert_eq!(auth.decision, "allow", "{}", auth.to_json_string());
    assert_eq!(auth.value["verification"]["result"], "not_verified");
    assert_eq!(
        auth.value["verification"]["attestation_hash"],
        a.hash.as_str()
    );
    let used = auth.value["waivers_used"].as_array().unwrap();
    assert_eq!(used.len(), a.findings().len());
    for u in used {
        assert_eq!(u["key_id"], key_id("A").as_str());
        assert_eq!(u["principal"], "test-reviewer-a");
        assert!(
            ws.iter()
                .any(|w| waiver_hash(w).unwrap() == u["waiver_hash"].as_str().unwrap())
        );
        assert_eq!(u["signature"].as_str().unwrap().len(), 128);
    }
}

#[test]
fn waivers_for_other_versions_or_findings_are_ignored() {
    let (m, a) = forced_inconclusive();
    let other = module("purchase.json").behavior_version();
    for patch in [
        json!({"behavior_version": other}),
        json!({"finding_hash": format!("sha256:{}", "ab".repeat(32))}),
    ] {
        let (ws, ss) = waive_all(&a, patch, &["A"]);
        let auth = run(&gov("verified_or_waived.json"), &m, Some(&a), &ws, &ss);
        assert_eq!(
            (auth.decision.as_str(), codes(&auth)),
            ("refuse", vec!["not_verified".to_string()])
        );
    }
}

#[test]
fn forbidden_and_non_waivable_kinds_are_refused() {
    let m = module("purchase.json");
    let a = attest(&m, Profile::default().rlimit);
    assert_eq!(a.findings()[0]["kind"], "preservation");
    let (ws, ss) = waive_all(&a, json!({}), &["A"]);
    let auth = run(&gov("verified_or_waived.json"), &m, Some(&a), &ws, &ss);
    assert_eq!(codes(&auth), vec!["finding_not_waivable".to_string()]);
}

#[test]
fn expired_untrusted_and_corrupted_waivers_are_refused() {
    let (m, a) = forced_inconclusive();
    let policy = gov("verified_or_waived.json");
    let (ws, ss) = waive_all(&a, json!({"expires_at": "2026-01-01T00:00:00Z"}), &["A"]);
    assert_eq!(
        codes(&run(&policy, &m, Some(&a), &ws, &ss)),
        vec!["waiver_expired".to_string()]
    );
    let (ws, ss) = waive_all(&a, json!({}), &["B"]);
    assert_eq!(
        codes(&run(&policy, &m, Some(&a), &ws, &ss)),
        vec!["untrusted_key".to_string()]
    );
    let (ws, ss) = waive_all(&a, json!({}), &["A"]);
    let corrupted: Vec<String> = ss
        .iter()
        .map(|s| {
            let mut v: Value = serde_json::from_str(s).unwrap();
            let sig = v["signature"].as_str().unwrap();
            let flipped = if sig.starts_with('0') { "1" } else { "0" };
            v["signature"] = json!(format!("{flipped}{}", &sig[1..]));
            v.to_string()
        })
        .collect();
    assert_eq!(
        codes(&run(&policy, &m, Some(&a), &ws, &corrupted)),
        vec!["invalid_signature".to_string()]
    );
}

#[test]
fn several_signatures_keep_the_waiver_hash_and_record_the_smallest_satisfying_key() {
    let (m, a) = forced_inconclusive();
    let (ws, ss) = waive_all(&a, json!({}), &["A", "B"]);
    for s in &ss {
        let v: Value = serde_json::from_str(s).unwrap();
        assert!(
            ws.iter()
                .any(|w| waiver_hash(w).unwrap() == v["waiver_hash"].as_str().unwrap())
        );
    }
    // Only A is trusted: A is recorded although B's key id is smaller.
    assert!(key_id("B") < key_id("A"));
    let auth = run(&gov("verified_or_waived.json"), &m, Some(&a), &ws, &ss);
    assert!(
        auth.value["waivers_used"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["key_id"] == key_id("A").as_str())
    );
    // Both trusted: the smallest key id is recorded.
    let mut both: Value = serde_json::from_str(&gov("verified_or_waived.json")).unwrap();
    both["trusted_keys"].as_array_mut().unwrap().push(
        json!({"key_id": key_id("B"), "principal": "test-reviewer-b", "roles": ["risk_reviewer"]}),
    );
    let auth = run(&both.to_string(), &m, Some(&a), &ws, &ss);
    assert!(
        auth.value["waivers_used"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["key_id"] == key_id("B").as_str())
    );
}

#[test]
fn authorizations_cite_their_policy_and_are_deterministic() {
    let (m, a) = forced_inconclusive();
    let (ws, ss) = waive_all(&a, json!({}), &["A"]);
    let p1 = gov("verified_or_waived.json");
    let first = run(&p1, &m, Some(&a), &ws, &ss);
    let p1_hash = behavior_verify::governance::decode_policy(&p1)
        .unwrap()
        .hash;
    assert_eq!(first.value["policy_hash"], p1_hash.as_str());
    // A new policy P2 does not change what was authorized under P1.
    let p2 = gov("require_verified.json");
    let under_p2 = run(&p2, &m, Some(&a), &ws, &ss);
    assert_eq!(under_p2.decision, "refuse");
    assert_ne!(under_p2.value["policy_hash"], p1_hash.as_str());
    let again = run(&p1, &m, Some(&a), &ws, &ss);
    assert_eq!(first.to_json_string(), again.to_json_string());
    assert_eq!(
        first.value["transition_hash"],
        again.value["transition_hash"]
    );
}

#[test]
fn tampered_inputs_are_rejected() {
    let (m, a) = forced_inconclusive();
    let mut forged = a.value.clone();
    forged["result"] = json!("verified");
    let err = authorize(
        &gov("require_verified.json"),
        &m,
        &record(&m),
        Some(&forged.to_string()),
        &[],
        &[],
        NOW,
    );
    assert!(err.is_err());
    let mut rec: Value = serde_json::from_str(&record(&m)).unwrap();
    rec["changes"] = json!([]);
    let auth = authorize(
        &gov("require_verified.json"),
        &m,
        &rec.to_string(),
        Some(&a.to_json_string()),
        &[],
        &[],
        NOW,
    )
    .unwrap();
    assert_eq!(codes(&auth), vec!["record_not_reproducible".to_string()]);
}

#[test]
fn forced_inconclusive_module_is_decided_under_the_default_budget() {
    // The waiver tests' inconclusive findings come from the budget, not from the model.
    let m = module("purchase_remaining.json");
    let a = attest(&m, Profile::default().rlimit);
    assert!(
        a.findings().iter().all(|f| f["kind"] != "inconclusive"),
        "{}",
        a.to_json_string()
    );
}
