//! Evidence is a build artifact. Importing JSON never constructs a verified certificate.
use super::{SafetyError, UncertifiedSafetyAnalysis, identity::tagged};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyOutcome {
    CertifiedUnderAssumptions,
    Inconclusive,
    Unsupported,
    Failed,
}
impl SafetyOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertifiedUnderAssumptions => "Certified under assumptions",
            Self::Inconclusive => "Inconclusive",
            Self::Unsupported => "Unsupported",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Debug)]
pub struct SafetyCertificate {
    pub(crate) record: Value,
}
impl SafetyCertificate {
    pub fn to_json(&self) -> Value {
        self.record.clone()
    }
}
#[derive(Debug)]
pub struct SafetyAssessment {
    pub(crate) record: Value,
    pub(crate) diagnostics: Vec<Value>,
    pub(crate) certificate: Option<SafetyCertificate>,
}
impl SafetyAssessment {
    pub fn certificate(&self) -> Option<&SafetyCertificate> {
        self.certificate.as_ref()
    }
    /// Diagnostic envelope. evidence.json contains only the canonical semantic record.
    pub fn to_json(&self) -> Value {
        let mut v = self.record.clone();
        if !self.diagnostics.is_empty() {
            v["diagnostics"] = json!(self.diagnostics);
            v["detail"] = json!(format!(
                "{}: {}",
                self.diagnostics[0]["code"]
                    .as_str()
                    .unwrap_or("SAFETY_FAILURE"),
                self.diagnostics[0]["detail"].as_str().unwrap_or("")
            ));
        }
        v
    }
}

pub(crate) fn dependency_identity() -> Result<Value, SafetyError> {
    // The distribution already carries the workspace lock and toolchain files.
    // Their contents are embedded at build time; installed verification reads no checkout.
    Ok(
        json!({"core_release":{"version":crate::CORE_RELEASE.version,"commit":crate::CORE_RELEASE.commit},
        "cargo_lock":tagged("behavior.safety_dependency.v1",&json!(include_str!("../../../../Cargo.lock")))?,
        "rust_toolchain":tagged("behavior.safety_dependency.v1",&json!(include_str!("../../../../rust-toolchain.toml")))?,
        "proof_toolchain_manifest":tagged("behavior.safety_dependency.v1",&json!(include_str!("../../proofs/toolchain.json")))?}),
    )
}

pub(crate) fn assessment(
    a: &UncertifiedSafetyAnalysis,
    provided_behavior: &str,
    outcome: SafetyOutcome,
    obligations: Value,
    proof: Value,
    error: Option<SafetyError>,
) -> SafetyAssessment {
    let (dependencies, dependency_error) = match dependency_identity() {
        Ok(d) => (d, None),
        Err(e) => (Value::Null, Some(e)),
    };
    let successful =
        outcome == SafetyOutcome::CertifiedUnderAssumptions && dependency_error.is_none();
    let outcome = if dependency_error.is_some() {
        SafetyOutcome::Inconclusive
    } else {
        outcome
    };
    let proved = |key: &str| obligations[key].as_str().unwrap_or("NotRun").to_owned();
    let mut evidence = serde_json::Map::new();
    for (key, class) in [
        ("inventory_comparison", "Structural correspondence check"),
        ("generic_proof", "Machine-checked theorem"),
        ("concrete_proof", "Kernel-checked finite premises"),
        ("theorem_types", "Kernel type comparison"),
        ("axiom_audit", "Transitive axiom audit"),
        ("fresh_replay", "Fresh kernel replay"),
    ] {
        evidence.insert(key.into(), json!({"class":class,"result":proved(key)}));
    }
    let mut trusted = vec![
        json!({"id":"core_semantics","status":"Assumed","class":"Trusted semantic contract","statement":"Pinned Core admission, ordinary atomic transition semantics, freshness and complete resulting-state Ref integrity"}),
        json!({"id":"backend_contract","status":"Assumed","class":"Host/backend obligation","statement":"Backend supplies complete consistent typed states and honors Core query and commit contracts"}),
        json!({"id":"inventory_extraction","status":"Assumed","class":"Trusted correspondence","statement":"Rust extracts every final Core action/effect and compares the full inventory independently"}),
        json!({"id":"proof_encoding","status":"Assumed","class":"Trusted correspondence","statement":"Rust resolves policy meaning and faithfully encodes Core identities, bindings, values and effect projections into the checked finite model"}),
        json!({"id":"core_hashes","status":"Assumed","class":"Cryptographic identity assumption","statement":"Core semantic hashing and collision resistance correctly bind the admitted declarations and module"}),
        json!({"id":"proof_toolchain","status":"Assumed","class":"Trusted proof infrastructure","statement":"Pinned Lean kernel, auditor, immutable Nix store, Rust/compiler dependencies and build environment are sound"}),
    ];
    if !a.dependencies.is_empty() {
        trusted.push(json!({"id":"canonical_probes","status":"Assumed","class":"Trusted correspondence","statement":"Public Core canonical probe construction and hash associations represent the declared guard meanings and complete effect projections"}));
    }
    let action_obligations=a.inventory.actions().iter().map(|(name,action)|{
        let issues=a.issues.iter().filter(|e|e.action.as_deref()==Some(name.as_str())).map(|e|e.code()).collect::<Vec<_>>();
        json!({"action_hash":action.hash,"operations":action.operations.iter().map(|o|o.to_json()).collect::<Vec<_>>(),
            "ownership":{"class":"Kernel-checked finite premises","result":proved("concrete_proof"),"proof_integrity":if successful{"Complete"}else{"Unestablished"}},
            "lifecycle":{"class":"Machine-checked theorem instantiated by finite checking","result":if issues.contains(&"SAFETY_GUARD_SCAN_EXHAUSTED"){"Inconclusive"}else if !issues.is_empty(){"Unsupported"}else if a.dependencies.is_empty(){"NotApplicable"}else{obligations["lifecycle_preservation"].as_str().unwrap_or("NotRun")},
                "dependencies":a.dependencies.iter().map(|d|d.model_hash()).collect::<Vec<_>>(),"issues":issues},
            "effect_correspondence":"Assumed; irrelevant projections use the proved frame lemmas"})
    }).collect::<Vec<_>>();
    let mut errors = a.issues.clone();
    for e in error.into_iter().chain(dependency_error) {
        if !errors.contains(&e) {
            errors.push(e);
        }
    }
    let mut diagnostics = errors.iter().map(|e| e.to_json()).collect::<Vec<_>>();
    let mut record = json!({"format":"behavior.safety_evidence.v1","analysis_version":1,"checker_version":super::SAFETY_CHECKER_VERSION,
        "scope":"OrdinaryTransitions","outcome":outcome.as_str(),"behavior_hash":a.inventory.behavior_hash(),
        "provided_behavior_hash":provided_behavior,"core_release":{"version":crate::CORE_RELEASE.version,"commit":crate::CORE_RELEASE.commit},
        "inventory":a.inventory.to_json(),"policy_set_hash":a.policy_set_hash(),"policies":a.policy_records(),
        "dependency_identity":dependencies,"proof":proof,"obligations":obligations,"obligation_evidence":evidence,
        "action_obligations":action_obligations,"initial_validity":"Unestablished","trusted_correspondence":trusted,
        "external_obligations":[{"id":"initial_validity","status":"Unestablished","class":"Separate state-validation obligation"},
            {"id":"approved_modules","status":"Host obligation","class":"Exact BehaviorHash and complete policy set, exact validated activation anchor and enforced continuity"},
            {"id":"migration_safety","status":"Excluded","class":"Separate resulting-state validation or preservation proof required"}],
        "diagnostic_codes":errors.iter().map(|e|e.code()).collect::<Vec<_>>(),
        "assumptions":["Core semantic and backend contracts","Rust extraction/probe/encoding correspondence","Pinned kernel/toolchain/build integrity","Valid initial property and host-enforced approved deployment segment"],
        "exclusions":["Genesis validity","Migration safety","Alternative modules","Physical host execution","End-to-end formal verification"],
        "runtime_tests":{"class":"Tests","status":"NotRunByCertification","statement":"Independent Core correspondence tests are separate evidence, not universal proofs"}});
    let identity = tagged("behavior.safety_evidence.v1", &record);
    let certificate = match identity {
        Ok(hash) => {
            record["evidence_hash"] = json!(hash);
            successful.then(|| SafetyCertificate {
                record: record.clone(),
            })
        }
        Err(error) => {
            record["outcome"] = json!("Inconclusive");
            record["evidence_hash"] = Value::Null;
            diagnostics.push(error.to_json());
            None
        }
    };
    SafetyAssessment {
        record,
        diagnostics,
        certificate,
    }
}

/// Transported data has no certificate/proof authority. No supplied Lean/compiled
/// objects are accepted in its fixed schema or executed by the verifier.
#[derive(Debug, Clone)]
pub struct UnverifiedSafetyBundle {
    pub(crate) document: Value,
}
impl UnverifiedSafetyBundle {
    pub fn from_json(document: &Value) -> Result<Self, SafetyError> {
        let invalid = || {
            SafetyError::new(
                "SAFETY_INVALID_BUNDLE",
                "Expected the fixed versioned Safety bundle schema; proof code is not an input",
            )
        };
        let object = document.as_object().ok_or_else(invalid)?;
        if object.len() != 5
            || !["format", "module", "inventory", "policies", "evidence"]
                .iter()
                .all(|k| object.contains_key(*k))
            || document["format"] != "behavior.safety_bundle.v1"
            || !document["module"].is_string()
            || !document["inventory"].is_object()
            || !document["policies"].is_array()
        {
            return Err(invalid());
        }
        let evidence = &document["evidence"];
        if evidence["format"] != "behavior.safety_evidence.v1"
            || !evidence["obligations"].is_object()
            || !evidence["proof"].is_object()
            || !evidence["behavior_hash"].is_string()
            || !evidence["evidence_hash"].is_string()
            || !evidence["policy_set_hash"].is_string()
        {
            return Err(invalid());
        }
        // Unverified data may claim success. Its identities/premises still require
        // fresh re-admission, extraction, proof checking, auditing and replay.
        Ok(Self {
            document: document.clone(),
        })
    }
    pub fn from_text(text: &str) -> Result<Self, SafetyError> {
        let value = behavior_engine::canonical::decode_strict(text)
            .map_err(|e| SafetyError::new("SAFETY_INVALID_BUNDLE", e.to_string()))?;
        Self::from_json(&value)
    }
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }
    pub fn is_verified(&self) -> bool {
        false
    }
}

/// Ephemeral exact-state evidence. JSON transport cannot construct this authority.
#[derive(Debug, Clone)]
pub struct StateValidationRecord {
    record: Value,
}
impl StateValidationRecord {
    pub(crate) fn new(mut record: Value) -> Self {
        match tagged("behavior.safety_state_validation.v1", &record) {
            Ok(hash) => record["validation_hash"] = json!(hash),
            Err(_) => {
                record["status"] = json!("Unestablished");
                record["validation_hash"] = Value::Null;
            }
        }
        Self { record }
    }
    pub fn to_json(&self) -> Value {
        self.record.clone()
    }
    pub fn status(&self) -> &str {
        self.record["status"].as_str().unwrap_or("Unestablished")
    }
    /// A necessary boundary check, not synchronization or activation. The host must
    /// enforce continuity while checking this and activating the incoming segment.
    pub fn matches_anchor(
        &self,
        module: &behavior_engine::semantic::Module,
        analysis: &UncertifiedSafetyAnalysis,
        history: &behavior_engine::store::documents::HistoryRef,
    ) -> bool {
        self.status() == "Valid"
            && self.record["behavior_hash"]
                == behavior_engine::semantic::types::hash_display(module.hash())
            && self.record["policy_set_hash"] == analysis.policy_set_hash()
            && self.record["history"] == json!(history)
            && analysis.inventory.verify_module(module).is_ok()
    }
}
