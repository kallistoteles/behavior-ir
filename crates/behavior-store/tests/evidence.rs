#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US4: every commit satisfies the store's evidence policy; records cite the policy and any
//! authorization; the guarantee is reported as structural (FR-013, FR-013a, FR-019, FR-023).

mod common;

use behavior_store::Backend as _;
use behavior_store::documents::{CommitBundle, Evidence, EvidencePolicy, Require};
use behavior_store::{InMemoryBackend, Store};
use behavior_verify::governance::{authorize, decode_policy};
use behavior_verify::solver::Z3Process;
use behavior_verify::{CheckKind, Profile, verify};
use common::*;
use serde_json::{Value, json};

const NOW: &str = "2026-09-27T12:00:00Z";

fn policy_json() -> String {
    read(&fixtures().join("governance/require_verified.json"))
}

fn policy_hash() -> String {
    decode_policy(&policy_json()).unwrap().hash
}

/// A verified attestation of the ledger's preservation checks.
fn attestation() -> Value {
    let profile = Profile {
        checks: vec![CheckKind::Preservation],
        ..Profile::default()
    };
    let a = verify(&ledger(), &profile, None, &Z3Process::from_env().unwrap());
    assert_eq!(a.result, "verified", "{}", a.to_json_string());
    a.value
}

/// Evidence for `bundle`: an authorization under `require_verified` with the attestation.
fn evidence_for(bundle: &CommitBundle, attestation: Option<&Value>) -> Evidence {
    let record = behavior_core::canonical::to_canonical_string(&bundle.record).unwrap();
    let att = attestation.map(|a| a.to_string());
    let auth = authorize(
        &policy_json(),
        &ledger(),
        &record,
        att.as_deref(),
        &[],
        &[],
        NOW,
    )
    .unwrap();
    Evidence {
        authorization: auth.value,
        execution_policy: Some(serde_json::from_str(&policy_json()).unwrap()),
        attestation: attestation.cloned(),
        waivers: vec![],
    }
}

fn strict(trusted: Option<Vec<String>>) -> EvidencePolicy {
    EvidencePolicy {
        require: Require::CommitAuthorization,
        trusted_execution_policies: trusted,
        ..EvidencePolicy::none()
    }
}

fn code(s: &mut Store<InMemoryBackend>, b: &CommitBundle) -> &'static str {
    s.commit(&ledger(), &b.evaluated_state.clone(), b)
        .unwrap_err()
        .code()
}

#[test]
fn a_store_without_requirements_cites_its_policy() {
    let mut s = store();
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    let c = commit(&mut s, &b);
    assert_eq!(c.evidence_trust, None);
    let r = s.backend().record(1).unwrap().unwrap();
    assert_eq!(r.evidence_policy, EvidencePolicy::none().hash().unwrap());
    assert_eq!(r.authorization, None);
}

#[test]
fn a_matching_authorization_is_bound_and_mismatches_are_refused() {
    let att = attestation();
    let mut s = store_with(InMemoryBackend::new(), strict(None));
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    assert_eq!(code(&mut s, &b), "EVIDENCE_REQUIRED");
    // An authorization for another transition.
    let other = transfer(&s, "a1", "a3", "2.00", T0).bundle.unwrap();
    assert_eq!(
        code(&mut s, &b.with_evidence(evidence_for(&other, Some(&att)))),
        "EVIDENCE_MISMATCH"
    );
    // A refused authorization (no attestation for a policy requiring verification).
    assert_eq!(
        code(&mut s, &b.with_evidence(evidence_for(&b, None))),
        "EVIDENCE_MISMATCH"
    );
    // A cited document that is not the one supplied.
    let mut e = evidence_for(&b, Some(&att));
    e.execution_policy = Some(json!({"policy_version": "1", "require": "verified_or_waived"}));
    assert_eq!(code(&mut s, &b.with_evidence(e)), "EVIDENCE_MISMATCH");
    let mut e = evidence_for(&b, Some(&att));
    e.authorization["now"] = json!("2030-01-01T00:00:00Z");
    assert_eq!(
        code(&mut s, &b.with_evidence(e)),
        "EVIDENCE_MISMATCH",
        "altered authorization"
    );
    // The matching authorization: bound, cited, structural.
    let e = evidence_for(&b, Some(&att));
    let auth_hash = e.authorization["hash"].as_str().unwrap().to_string();
    let bound = b.with_evidence(e);
    assert_eq!(
        bound.transition_hash, b.transition_hash,
        "evidence does not change the transition"
    );
    let c = commit(&mut s, &bound);
    assert_eq!(c.evidence_trust, Some("structural"));
    let r = s.backend().record(1).unwrap().unwrap();
    assert_eq!(r.authorization, Some(auth_hash));
    assert_eq!(r.evidence_policy, strict(None).hash().unwrap());
    // Idempotency still recognizes the transition, with or without the evidence re-attached.
    assert!(commit(&mut s, &bound).already);
}

#[test]
fn only_trusted_execution_policies_are_accepted() {
    let att = attestation();
    let mut s = store_with(
        InMemoryBackend::new(),
        strict(Some(vec!["sha256:00".into()])),
    );
    let b = transfer(&s, "a1", "a2", "1.00", T0).bundle.unwrap();
    assert_eq!(
        code(&mut s, &b.with_evidence(evidence_for(&b, Some(&att)))),
        "EVIDENCE_MISMATCH"
    );
    let mut t = store_with(InMemoryBackend::new(), strict(Some(vec![policy_hash()])));
    let b = transfer(&t, "a1", "a2", "1.00", T0).bundle.unwrap();
    assert!(!commit(&mut t, &b.with_evidence(evidence_for(&b, Some(&att)))).already);
}

#[test]
fn evidence_from_another_store_is_refused() {
    let att = attestation();
    // Identical content and behavior, a different evidence policy: a different store identity.
    let a = store_with(InMemoryBackend::new(), strict(None));
    let mut b = store_with(InMemoryBackend::new(), strict(Some(vec![policy_hash()])));
    assert_eq!(a.current().unwrap().state, b.current().unwrap().state);
    assert_ne!(a.store_id().unwrap(), b.store_id().unwrap());
    let foreign = transfer(&a, "a1", "a2", "1.00", T0).bundle.unwrap();
    let with = foreign.with_evidence(evidence_for(&foreign, Some(&att)));
    assert_eq!(code(&mut b, &with), "BUNDLE_INVALID");
    // B's own identical transition with A's authorization: another transition hash.
    let mine = transfer(&b, "a1", "a2", "1.00", T0).bundle.unwrap();
    assert_ne!(mine.transition_hash, foreign.transition_hash);
    let e = evidence_for(&foreign, Some(&att));
    assert_eq!(code(&mut b, &mine.with_evidence(e)), "EVIDENCE_MISMATCH");
}
