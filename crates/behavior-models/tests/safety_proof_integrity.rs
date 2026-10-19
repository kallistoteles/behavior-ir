#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/safety.rs"]
mod support;
use behavior_models::safety::certify::{ProofLimits, ProofToolchain, certify};
use behavior_models::safety::{FieldOwnershipInput, analyze};
use serde_json::json;
use std::path::PathBuf;
use std::sync::OnceLock;

fn positive_control() -> &'static (serde_json::Value, PathBuf) {
    static CONTROL: OnceLock<(serde_json::Value, PathBuf)> = OnceLock::new();
    CONTROL.get_or_init(|| {
        let m = support::fixture();
        let a = analysis(&m);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/safety-models/fo")
            .join(format!("safety-proof-control-{}", std::process::id()));
        let result = certify(
            &m,
            &a,
            &dir,
            &ProofToolchain::locked().unwrap(),
            &ProofLimits::default(),
        );
        assert!(result.certificate().is_some(), "{}", result.to_json());
        let work = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("proof-")
            })
            .unwrap();
        (result.to_json(), work)
    })
}

fn analysis(
    m: &behavior_engine::semantic::Module,
) -> behavior_models::safety::UncertifiedSafetyAnalysis {
    analyze(
        m,
        &[
            FieldOwnershipInput::from_json(&json!({"id":"notes","entity":"Item",
        "writers":{"note":["edit","copy"]},"creators":["create"],"removers":["remove"]}))
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn positive_control_requires_concrete_proof_axiom_audit_and_fresh_replay() {
    let evidence = &positive_control().0;
    for key in [
        "generic_proof",
        "concrete_proof",
        "theorem_types",
        "axiom_audit",
        "fresh_replay",
        "inventory_comparison",
    ] {
        assert_eq!(evidence["obligations"][key], "Passed", "{key}: {evidence}");
    }
}

#[test]
fn wrong_module_and_exhausted_budget_cannot_certify() {
    let m = support::fixture();
    let a = analysis(&m);
    let mut b = support::declarations();
    support::edit(&mut b, "extra", "x", false);
    let wrong = b.finish(None).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/safety-models/fo")
        .join(format!("safety-proof-refusal-{}", std::process::id()));
    let tools = ProofToolchain::locked().unwrap();
    assert!(
        certify(&wrong, &a, &dir, &tools, &ProofLimits::default())
            .certificate()
            .is_none()
    );
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("assessment.json")).unwrap())
            .unwrap();
    assert_eq!(report["outcome"], "Failed");
    let tiny = ProofLimits {
        max_heartbeats: 1,
        ..ProofLimits::default()
    };
    assert!(certify(&m, &a, &dir, &tools, &tiny).certificate().is_none());
}

#[test]
fn theorem_premise_and_integrity_mutations_have_a_positive_control() {
    // The fixture contains kernel-checked positive witnesses and fail_if_success controls.
    // It audits intentionally admitted sorry/custom/native axioms as rejected, too.
    let root = &positive_control().1;
    let tools = ProofToolchain::locked().unwrap();
    let source = root.join("OwnershipMutations.lean");
    std::fs::write(
        &source,
        include_str!("../proofs/safety/tests/OwnershipMutations.lean"),
    )
    .unwrap();
    let status = std::process::Command::new(tools.lean())
        .arg(source)
        .current_dir(root)
        .env("LEAN_PATH", root)
        .env_remove("LEAN_SRC_PATH")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn missing_or_fake_tools_do_not_become_proof_evidence() {
    assert!(ProofToolchain::from_root(std::path::Path::new("/nonexistent/safety-tools")).is_err());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/safety-models/fo/fake-tools");
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/lean"), "#!/bin/sh\necho Lean 4.30.0\n").unwrap();
    assert!(ProofToolchain::from_root(&root).is_err());
}

#[test]
fn zero_limits_cannot_select_leans_unlimited_mode() {
    assert!(ProofLimits::default().validate().is_ok());
    for bad in [
        ProofLimits {
            max_heartbeats: 0,
            ..ProofLimits::default()
        },
        ProofLimits {
            memory_mb: 0,
            ..ProofLimits::default()
        },
        ProofLimits {
            address_space_mb: 0,
            ..ProofLimits::default()
        },
        ProofLimits {
            wall_seconds: 0,
            ..ProofLimits::default()
        },
    ] {
        assert!(bad.validate().is_err());
    }
}

#[test]
fn wrong_concrete_identity_and_authority_fail_with_valid_control() {
    let root = &positive_control().1;
    let tools = ProofToolchain::locked().unwrap();
    let original = std::fs::read_to_string(root.join("Concrete.lean")).unwrap();
    // Alter the concrete policy, preserving otherwise well-formed finite data.
    let bad_authority = original.replace(
        "policies := [⟨0,[(1, [2,0])],[1],[3]⟩]",
        "policies := [⟨0,[(1, [])],[1],[3]⟩]",
    );
    // Do not let a stale textual control pass without changing the actual data.
    assert_ne!(
        bad_authority, original,
        "Inspect encoded policy before updating this mutation"
    );
    for (name, text) in [
        ("AuthorityMutation", bad_authority),
        (
            "IdentityMutation",
            original.replacen("behaviorHash := [115", "behaviorHash := [116", 1),
        ),
    ] {
        assert_ne!(text, original);
        std::fs::write(root.join(format!("{name}.lean")), text).unwrap();
        let status = std::process::Command::new(tools.lean())
            .arg(format!("{name}.lean"))
            .current_dir(root)
            .env("LEAN_PATH", root)
            .status()
            .unwrap();
        assert!(!status.success(), "{name} unexpectedly proved");
    }
}

#[test]
fn output_directory_cannot_inject_additional_lean_search_paths() {
    let m = support::fixture();
    let a = analysis(&m);
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/fo/path:injection-{}",
        std::process::id()
    ));
    let result = certify(
        &m,
        &a,
        &output,
        &ProofToolchain::locked().unwrap(),
        &ProofLimits {
            max_heartbeats: 1,
            ..ProofLimits::default()
        },
    );
    assert!(result.certificate().is_none());
    assert!(
        result.to_json()["detail"]
            .as_str()
            .unwrap()
            .contains("SAFETY_INVALID_PROOF_PATH"),
        "{}",
        result.to_json()
    );
    assert_eq!(result.to_json()["obligations"]["generic_proof"], "NotRun");
    assert!(!std::fs::read_dir(output).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("proof-")
    }));
}
