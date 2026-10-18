//! Evidence identifies both checked mathematics and its remaining trust boundary.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/safety.rs"]
mod support;
use behavior_models::safety::certify::{ProofLimits, ProofToolchain, certify};
use behavior_models::safety::evidence::UnverifiedSafetyBundle;
use behavior_models::safety::{FieldOwnershipInput, analyze};
use serde_json::json;

fn analysis(
    m: &behavior_engine::semantic::Module,
) -> behavior_models::safety::UncertifiedSafetyAnalysis {
    analyze(m,&[FieldOwnershipInput::from_json(&json!({"id":"notes","entity":"Item","writers":{"note":["edit","copy"]},"creators":["create"],"removers":["remove"]})).unwrap()]).unwrap()
}
#[test]
fn lowered_analysis_is_complete_data_and_never_a_certificate() {
    let m = support::fixture();
    let a = analysis(&m);
    let data = a.to_json();
    assert_eq!(data["status"], "Lowered");
    assert_eq!(data["is_certified"], false);
    assert_eq!(data["analysis_version"], 1);
    assert_eq!(data["checker_version"], 1);
    assert_eq!(data["policies"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["inventory"]["behavior_hash"],
        a.inventory().behavior_hash()
    );
}
#[test]
fn proof_and_exhaustion_assessments_have_scope_identities_classes_and_external_boundaries() {
    let m = support::fixture();
    let a = analysis(&m);
    let tools = ProofToolchain::locked().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/evidence/contracts-{}",
        std::process::id()
    ));
    for (name, limits, expected) in [
        (
            "positive",
            ProofLimits::default(),
            "Certified under assumptions",
        ),
        (
            "exhausted",
            ProofLimits {
                max_heartbeats: 1,
                ..ProofLimits::default()
            },
            "Inconclusive",
        ),
    ] {
        let result = certify(&m, &a, &root.join(name), &tools, &limits);
        let report = result.to_json();
        assert_eq!(report["outcome"], expected, "{report}");
        assert_eq!(report["format"], "behavior.safety_evidence.v1");
        assert_eq!(report["scope"], "OrdinaryTransitions");
        assert_eq!(report["checker_version"], 1);
        assert_eq!(report["core_release"]["version"], "0.15.0");
        assert_eq!(report["initial_validity"], "Unestablished");
        assert_eq!(report["behavior_hash"], a.inventory().behavior_hash());
        assert_eq!(report["policy_set_hash"], a.policy_set_hash());
        assert_eq!(
            report["action_obligations"].as_array().unwrap().len(),
            m.actions().len()
        );
        let trusted = report["trusted_correspondence"].as_array().unwrap();
        assert!(trusted.iter().all(|r| r["status"] == "Assumed"));
        for id in ["core_semantics", "inventory_extraction", "proof_encoding"] {
            assert!(
                trusted.iter().any(|r| r["id"] == id),
                "Missing named trust boundary {id}"
            );
        }
        assert!(
            report["external_obligations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == "initial_validity" && r["status"] == "Unestablished")
        );
        assert_eq!(
            report["obligation_evidence"]["generic_proof"]["class"],
            "Machine-checked theorem"
        );
        assert_eq!(
            report["obligation_evidence"]["concrete_proof"]["class"],
            "Kernel-checked finite premises"
        );
        assert!(
            report["evidence_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        if expected == "Certified under assumptions" {
            assert!(result.certificate().is_some());
            let mut payload = report.clone();
            payload.as_object_mut().unwrap().remove("evidence_hash");
            payload.as_object_mut().unwrap().remove("diagnostics");
            assert_eq!(
                behavior_engine::canonical::tagged_hash("behavior.safety_evidence.v1", &payload)
                    .unwrap(),
                report["evidence_hash"].as_str().unwrap()
            );
        } else {
            assert!(result.certificate().is_none());
        }
    }
}
#[test]
fn importing_claimed_success_is_not_proof_checking() {
    let forged = json!({"format":"behavior.safety_bundle.v1","module":"{}","inventory":{},"policies":[],"evidence":{"outcome":"Certified under assumptions"}});
    assert!(UnverifiedSafetyBundle::from_json(&forged).is_err());
    for supplied in ["lean_source", "compiled_proof", "proof_files"] {
        let mut value = forged.clone();
        value[supplied] = json!("untrusted");
        assert!(UnverifiedSafetyBundle::from_json(&value).is_err());
    }
}

#[path = "support/lifecycle.rs"]
mod lifecycle_support;
#[test]
fn failed_and_unsupported_assessments_keep_the_same_normative_contract() {
    let tools = ProofToolchain::locked().unwrap();
    let mut builder = support::declarations();
    support::edit(&mut builder, "extra", "subject", false);
    let wrong = builder.finish(None).unwrap();
    let original = support::fixture();
    let a = analysis(&original);
    let ld = lifecycle_support::module(&lifecycle_support::Options {
        missing_guard: true,
        ..Default::default()
    });
    let unsupported = behavior_models::safety::analyze_with_dependencies(
        &ld,
        &lifecycle_support::ownership(&ld),
        &[lifecycle_support::dependency("Child")],
    )
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/evidence/refused-{}",
        std::process::id()
    ));
    for (module, analysis, outcome) in [(&wrong, &a, "Failed"), (&ld, &unsupported, "Unsupported")]
    {
        let result = certify(
            module,
            analysis,
            &root.join(outcome),
            &tools,
            &ProofLimits::default(),
        );
        let report = result.to_json();
        assert_eq!(report["outcome"], outcome);
        assert!(result.certificate().is_none());
        for field in [
            "core_release",
            "behavior_hash",
            "inventory",
            "policy_set_hash",
            "policies",
            "proof",
            "obligations",
            "action_obligations",
            "trusted_correspondence",
            "external_obligations",
            "obligation_evidence",
            "evidence_hash",
        ] {
            assert!(
                !report[field].is_null(),
                "{outcome} is missing {field}: {report}"
            );
        }
        assert_eq!(report["initial_validity"], "Unestablished");
        assert_eq!(report["obligations"]["generic_proof"], "NotRun");
    }
}
