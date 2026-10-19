//! Fixed proof generation and pinned, bounded build-time verification.
//! The extractor/encoder and immutable Nix store remain explicit trusted boundaries.
use super::{
    SafetyError, UncertifiedSafetyAnalysis,
    evidence::{SafetyAssessment, SafetyOutcome},
    identity::tagged,
    inventory::Operation,
};
use behavior_engine::semantic::Module;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const TOOLCHAIN: &str = include_str!("../../proofs/toolchain.json");
const SOURCES: [(&str, &str); 8] = [
    ("Data", include_str!("../../proofs/safety/Data.lean")),
    (
        "FieldOwnership",
        include_str!("../../proofs/safety/FieldOwnership.lean"),
    ),
    (
        "LifecycleData",
        include_str!("../../proofs/safety/LifecycleData.lean"),
    ),
    (
        "LifecycleDependency",
        include_str!("../../proofs/safety/LifecycleDependency.lean"),
    ),
    (
        "LifecycleCheck",
        include_str!("../../proofs/safety/LifecycleCheck.lean"),
    ),
    (
        "LifecycleSequence",
        include_str!("../../proofs/safety/LifecycleSequence.lean"),
    ),
    ("Check", include_str!("../../proofs/safety/Check.lean")),
    ("Audit", include_str!("../../proofs/safety/Audit.lean")),
];
static NEXT_RUN: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[path = "../../tests/support/safety.rs"]
mod test_support;

#[cfg(test)]
#[path = "../../tests/support/lifecycle.rs"]
mod lifecycle_test_support;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn matched_membership_witness_is_bound_to_its_exact_order_and_coordinates() {
        let m = lifecycle_test_support::module(&lifecycle_test_support::Options::default());
        let a = super::super::analyze_with_dependencies(
            &m,
            &lifecycle_test_support::ownership(&m),
            &[lifecycle_test_support::dependency("Child")],
        )
        .unwrap();
        assert!(proof_input(&m, &a).is_ok());
        for field in ["order", "enum", "field", "dependency", "parent", "root"] {
            let mut changed = a.clone();
            let matched = changed
                .membership_matches
                .iter_mut()
                .find(|m| m.family == "permitted_parent")
                .unwrap();
            match field {
                "order" => matched.ordered.reverse(),
                "enum" => matched.enumeration = "DifferentEnum".into(),
                "field" => matched.field = "DifferentField".into(),
                "dependency" => matched.dependency = "sha256:wrong".into(),
                "parent" => matched.parent = "not_a_binding".into(),
                "root" => matched.root = "sha256:wrong".into(),
                _ => unreachable!(),
            }
            assert!(
                proof_input(&m, &changed).is_err(),
                "forged {field} witness reached proof input"
            );
        }
    }
    #[test]
    fn omitted_action_or_effect_never_reaches_proof_certification() {
        let module = test_support::fixture();
        let input =
            super::super::FieldOwnershipInput::from_json(&json!({"id":"notes","entity":"Item",
            "writers":{"note":["edit","copy"]},"creators":["create"],"removers":["remove"]}))
            .unwrap();
        let analysis = super::super::analyze(&module, &[input]).unwrap();
        let tools = ProofToolchain::locked().unwrap();
        for action_omission in [true, false] {
            let mut data = analysis.inventory.to_json();
            if action_omission {
                data["actions"].as_array_mut().unwrap().remove(0);
            } else {
                data["actions"][0]["operations"]
                    .as_array_mut()
                    .unwrap()
                    .clear();
            }
            test_support::rehash(&mut data);
            let mut altered = analysis.clone();
            altered.inventory = super::super::Inventory::from_json(&data).unwrap();
            let output = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/safety-models/fo/omission-controls");
            let result = certify(&module, &altered, &output, &tools, &ProofLimits::default());
            assert!(result.certificate().is_none());
            assert_eq!(result.to_json()["outcome"], "Failed");
            assert_eq!(result.to_json()["obligations"]["generic_proof"], "NotRun");
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProofToolchain {
    root: PathBuf,
    identity: Value,
}
impl ProofToolchain {
    pub fn locked() -> Result<Self, SafetyError> {
        let pin: Value = serde_json::from_str(TOOLCHAIN).map_err(io_error)?;
        let platform = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
        let root = pin["profiles"][&platform]["outPath"]
            .as_str()
            .ok_or_else(|| {
                SafetyError::new("SAFETY_TOOLCHAIN_UNAVAILABLE", "Unsupported proof platform")
            })?;
        Self::from_root(Path::new(root))
    }
    pub fn from_root(root: &Path) -> Result<Self, SafetyError> {
        for variable in ["BEHAVIOR_LEAN_BINARY", "BEHAVIOR_LEANCHECKER_BINARY"] {
            if std::env::var_os(variable).is_some_and(|v| !v.is_empty()) {
                return Err(SafetyError::new(
                    "SAFETY_TOOLCHAIN_MISMATCH",
                    format!("Unsupported override {variable}"),
                ));
            }
        }
        let pin: Value = serde_json::from_str(TOOLCHAIN).map_err(io_error)?;
        let platform = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
        let profile = &pin["profiles"][&platform];
        if root.to_str() != profile["outPath"].as_str()
            || fs::canonicalize(root).ok().as_deref() != Some(root)
        {
            return Err(SafetyError::new(
                "SAFETY_TOOLCHAIN_MISMATCH",
                "Expected the locked immutable Nix output",
            ));
        }
        for exe in ["lean", "leanchecker"] {
            if !root.join("bin").join(exe).is_file() {
                return Err(SafetyError::new(
                    "SAFETY_TOOLCHAIN_UNAVAILABLE",
                    format!("Missing pinned {exe}"),
                ));
            }
        }
        let nix = Path::new("/run/current-system/sw/bin/nix-store");
        let nix = if nix.is_file() {
            nix
        } else {
            Path::new("/nix/var/nix/profiles/default/bin/nix-store")
        };
        let result = Command::new(nix)
            .args(["--query", "--deriver"])
            .arg(root)
            .output()
            .map_err(io_error)?;
        if !result.status.success()
            || Some(String::from_utf8_lossy(&result.stdout).trim()) != profile["drvPath"].as_str()
        {
            return Err(SafetyError::new(
                "SAFETY_TOOLCHAIN_MISMATCH",
                "Actual Nix derivation differs from the locked profile",
            ));
        }
        Ok(Self {
            root: root.into(),
            identity: json!({"platform":platform,"profile":profile,
            "flake_lock_sha256":pin["flake_lock_sha256"]}),
        })
    }
    pub fn lean(&self) -> PathBuf {
        self.root.join("bin/lean")
    }
}

#[derive(Debug, Clone)]
pub struct ProofLimits {
    pub max_heartbeats: u64,
    pub memory_mb: u64,
    /// Virtual address reservation, separate from Lean's heap budget (-M).
    pub address_space_mb: u64,
    pub wall_seconds: u64,
}
impl Default for ProofLimits {
    fn default() -> Self {
        Self {
            max_heartbeats: 200_000,
            memory_mb: 4096,
            address_space_mb: 8192,
            // Fresh replay measured ~113 s for the minimal module on the
            // supported host. Keep a bounded margin; this is outside quick CI.
            wall_seconds: 180,
        }
    }
}
impl ProofLimits {
    pub fn validate(&self) -> Result<(), SafetyError> {
        if [
            self.max_heartbeats,
            self.memory_mb,
            self.address_space_mb,
            self.wall_seconds,
        ]
        .contains(&0)
        {
            return Err(SafetyError::new(
                "SAFETY_INVALID_LIMITS",
                "Every proof limit must be positive; unlimited modes are forbidden",
            ));
        }
        Ok(())
    }
}
fn io_error(e: impl std::fmt::Display) -> SafetyError {
    SafetyError::new("SAFETY_PROOF_IO", e.to_string())
}
fn proof_search_path(path: &Path) -> Result<std::ffi::OsString, SafetyError> {
    // LEAN_PATH is a path list. An output directory containing its separator
    // must never select additional, untrusted import directories.
    std::env::join_paths([path]).map_err(|_| {
        SafetyError::new(
            "SAFETY_INVALID_PROOF_PATH",
            "Proof output must be representable as one Lean import directory",
        )
    })
}
fn canonical(value: &Value) -> Result<String, SafetyError> {
    behavior_engine::canonical::to_canonical_string(value).map_err(io_error)
}
pub(super) fn list(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}
pub(super) fn bytes(s: &str) -> String {
    list(s.bytes().map(|b| b.to_string()))
}

fn concrete(module: &Module, a: &UncertifiedSafetyAnalysis) -> Result<String, SafetyError> {
    let inventory = &a.inventory;
    let entities = inventory
        .entities()
        .keys()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let fields = inventory
        .entities()
        .iter()
        .map(|(n, e)| {
            (
                n.as_str(),
                e.fields
                    .keys()
                    .enumerate()
                    .map(|(i, f)| (f.as_str(), i))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actions = inventory
        .actions()
        .values()
        .enumerate()
        .map(|(i, act)| (act.hash.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let lookup = |map: &BTreeMap<&str, usize>, key: &str| {
        map.get(key).copied().ok_or_else(|| {
            SafetyError::new(
                "SAFETY_ENCODING_FAILED",
                format!("Unresolved identity {key}"),
            )
        })
    };
    let action_ids = |set: &std::collections::BTreeSet<String>| -> Result<String, SafetyError> {
        Ok(list(
            set.iter()
                .map(|h| lookup(&actions, h).map(|i| i.to_string()))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    };
    let mut encoded_actions = Vec::new();
    for (i, act) in inventory.actions().values().enumerate() {
        let mut ops = Vec::new();
        for op in &act.operations {
            let entity = lookup(&entities, op.entity())?;
            let (kind, field) = match op {
                Operation::Assign { entity, field, .. } => {
                    ("assign", lookup(&fields[entity.as_str()], field)?)
                }
                Operation::Create { .. } => ("create", 0),
                Operation::Remove { .. } => ("remove", 0),
            };
            ops.push(format!("⟨{entity},.{kind},{field}⟩"));
        }
        encoded_actions.push(format!("⟨{i},{}⟩", list(ops)));
    }
    let mut policies = Vec::new();
    for p in &a.policies {
        let entity = lookup(&entities, &p.entity)?;
        let mut writers = Vec::new();
        for (f, set) in &p.writers {
            writers.push(format!(
                "({}, {})",
                lookup(&fields[p.entity.as_str()], f)?,
                action_ids(set)?
            ));
        }
        policies.push(format!(
            "⟨{entity},{},{},{}⟩",
            list(writers),
            action_ids(&p.creators)?,
            action_ids(&p.removers)?
        ));
    }
    Ok(format!(
        "import safety.Check\n{}\nnamespace Concrete\ndef data : Safety.ModuleData := {{\nbehaviorHash := {}\ninventoryHash := {}\npolicySetHash := {}\ninventoryPayload := {}\npolicyPayload := {}\nactionHashes := {}\nentityFields := {}\nactions := {}\npolicies := {}\nlifecycle := {}\n}}\ntheorem checked : Safety.checkModule data = true := by decide\ntheorem certified : {} := ⟨rfl,rfl,rfl,rfl,rfl,Safety.module_preserves data checked,Safety.lifecycle_module_preserves data checked⟩\nend Concrete\n",
        anchors(a, "ConcreteAnchors")?,
        bytes(inventory.behavior_hash()),
        bytes(inventory.hash()),
        bytes(&a.policy_set_hash),
        "ConcreteAnchors.inventory",
        "ConcreteAnchors.policies",
        list(inventory.actions().values().map(|act| bytes(&act.hash))),
        list(
            inventory
                .entities()
                .values()
                .map(|e| e.fields.len().to_string())
        ),
        list(encoded_actions),
        list(policies),
        super::proof_data::lifecycle(module, a)?,
        instance_type(a, "data", "ConcreteAnchors")?
    ))
}

fn anchors(a: &UncertifiedSafetyAnalysis, namespace: &str) -> Result<String, SafetyError> {
    let mut out = format!("namespace {namespace}\n");
    for (name, payload) in [
        ("inventory", canonical(&a.inventory.to_json())?),
        ("policies", canonical(&json!(a.policy_documents()))?),
    ] {
        let mut names = Vec::new();
        for (index, chunk) in payload.as_bytes().chunks(64).enumerate() {
            let local = format!("{name}{index}");
            out.push_str(&format!(
                "def {local} : List Nat := {}\n",
                list(chunk.iter().map(u8::to_string))
            ));
            names.push(local);
        }
        out.push_str(&format!(
            "def {name} : List (List Nat) := {}\n",
            list(names)
        ));
    }
    out.push_str(&format!("end {namespace}\n"));
    Ok(out)
}

fn instance_type(
    a: &UncertifiedSafetyAnalysis,
    data: &str,
    anchor: &str,
) -> Result<String, SafetyError> {
    Ok(format!(
        "Safety.InstanceGuarantee {data} {} {} {} {anchor}.inventory {anchor}.policies",
        bytes(a.inventory.behavior_hash()),
        bytes(a.inventory.hash()),
        bytes(&a.policy_set_hash)
    ))
}
fn verification(a: &UncertifiedSafetyAnalysis) -> Result<String, SafetyError> {
    Ok(format!(
        "import safety.Audit\nimport Concrete\n{}\n#safety_audit Safety.FieldOwnership.sound : Safety.OwnershipGuarantee\n#safety_audit Safety.LifecycleDependency.sound : Safety.LifecycleGuarantee\n#safety_audit Safety.LifecycleDependency.conjunction : Safety.LifecycleConjunctionGuarantee\n#safety_audit Safety.LifecycleDependency.projection_frame : Safety.LifecycleDependency.ProjectionFrameGuarantee\n#safety_audit Safety.membership_order_equivalent : Safety.MembershipOrderGuarantee\n#safety_audit Safety.checkLifecycle_sound : Safety.LifecycleCheckerGuarantee\n#safety_audit Safety.classify_permitted : Safety.ClassificationGuarantee\n#safety_audit Safety.lifecycle_module_preserves : Safety.LifecycleModuleSoundness\n#safety_audit Safety.checkModule_sound : Safety.CheckedAuthority\n#safety_audit Safety.module_preserves : Safety.ModuleSoundness\n#safety_audit Concrete.checked : Safety.checkModule Concrete.data = true\n#safety_audit Concrete.certified : {}\n",
        anchors(a, "VerificationAnchors")?,
        instance_type(a, "Concrete.data", "VerificationAnchors")?
    ))
}

fn run(
    tools: &ProofToolchain,
    limits: &ProofLimits,
    work: &Path,
    exe: &Path,
    args: &[String],
    label: &str,
) -> Result<(), SafetyError> {
    let log = fs::File::create(work.join(format!("{label}.log"))).map_err(io_error)?;
    let error_log = log.try_clone().map_err(io_error)?;
    // Limits are numeric arguments, never interpolated shell code. Apply the
    // virtual-memory cap to leanchecker too; it has no Lean -M option.
    let mut child = Command::new("/bin/bash")
        .args([
            "-c",
            "ulimit -v \"$1\" || exit 125; shift; exec \"$@\"",
            "safety-proof",
        ])
        .arg(limits.address_space_mb.saturating_mul(1024).to_string())
        .arg(exe)
        .args(args)
        .current_dir(work)
        .env_clear()
        .env("PATH", tools.root.join("bin"))
        .env("LEAN_PATH", proof_search_path(work)?)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(error_log)
        .spawn()
        .map_err(io_error)?;
    let start = Instant::now();
    let mut resident_kb = 0_u64;
    let mut virtual_kb = 0_u64;
    loop {
        if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", child.id())) {
            for line in status.lines() {
                let mut parts = line.split_whitespace();
                let key = parts.next();
                let value = parts
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                match key {
                    Some("VmRSS:") => resident_kb = resident_kb.max(value),
                    Some("VmSize:") => virtual_kb = virtual_kb.max(value),
                    _ => {}
                }
            }
        }
        if let Some(status) = child.try_wait().map_err(io_error)? {
            fs::write(work.join(format!("{label}.resources.json")),json!({"elapsed_ms":start.elapsed().as_millis(),
                "peak_sampled_resident_kb":resident_kb,"peak_sampled_virtual_kb":virtual_kb,"exit_code":status.code(),
                "limits":{"heartbeats":limits.max_heartbeats,"heap_mb":limits.memory_mb,
                "address_space_mb":limits.address_space_mb,"wall_seconds":limits.wall_seconds}}).to_string()).map_err(io_error)?;
            if status.success() {
                return Ok(());
            }
            let text = fs::read_to_string(work.join(format!("{label}.log"))).unwrap_or_default();
            let exhausted = text.contains("heartbeats")
                || text.contains("recursion depth")
                || text.contains("memory")
                || status.code().is_none();
            return Err(SafetyError::new(
                if exhausted {
                    "SAFETY_PROOF_EXHAUSTED"
                } else {
                    "SAFETY_PROOF_REJECTED"
                },
                format!("{label}: {status}: {text}"),
            ));
        }
        if start.elapsed() >= Duration::from_secs(limits.wall_seconds) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SafetyError::new(
                "SAFETY_PROOF_EXHAUSTED",
                format!("{label}: wall-time limit"),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn outcome_for(error: &SafetyError) -> SafetyOutcome {
    if error.code().contains("UNSUPPORTED")
        || matches!(
            error.code(),
            "SAFETY_MIXED_LIFECYCLE_ROLES"
                | "SAFETY_MULTIPLE_LIFECYCLE_OPERATIONS"
                | "SAFETY_PARENT_BINDING_REQUIRED"
                | "SAFETY_OWNER_ASSOCIATION_REQUIRED"
                | "SAFETY_PARENT_GUARD_REQUIRED"
        )
    {
        SafetyOutcome::Unsupported
    } else if [
        "SAFETY_PROOF_IO",
        "SAFETY_PROOF_EXHAUSTED",
        "SAFETY_GUARD_SCAN_EXHAUSTED",
        "SAFETY_TOOLCHAIN_UNAVAILABLE",
    ]
    .contains(&error.code())
    {
        SafetyOutcome::Inconclusive
    } else {
        SafetyOutcome::Failed
    }
}

fn implementation_identity() -> Result<String, SafetyError> {
    tagged(
        "behavior.safety_implementation.v1",
        &json!({
            "inventory":include_str!("inventory.rs"),"input":include_str!("input.rs"),"resolve":include_str!("resolve.rs"),
            "identity":include_str!("identity.rs"),"lifecycle":include_str!("lifecycle_dependency.rs"),"probes":include_str!("guards.rs"),
            "membership_orders":include_str!("membership_orders.rs"),"lifecycle_encoder":include_str!("proof_data.rs"),"ownership":include_str!("field_ownership.rs"),
            "encoder_runner":include_str!("certify.rs"),"evidence":include_str!("evidence.rs"),"entrypoints":include_str!("mod.rs")
        }),
    )
}

fn proof_identity(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
    tools: &ProofToolchain,
) -> Result<Value, SafetyError> {
    let sources = SOURCES
        .iter()
        .map(|(name, source)| {
            Ok((
                *name,
                tagged("behavior.safety_proof_source.v1", &json!(source))?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, SafetyError>>()?;
    let input = proof_input(module, a).ok();
    let verifier = verification(a)?;
    Ok(
        json!({"statement_version":1,"checker_version":super::SAFETY_CHECKER_VERSION,"sources":sources,
        "concrete_source":input.map(|source|tagged("behavior.safety_proof_source.v1",&json!(source))).transpose()?,
        "audit_source":tagged("behavior.safety_proof_source.v1",&json!(verifier))?,"toolchain":tools.identity,"audit":[],
        "implementation_sources":implementation_identity()?}),
    )
}

/// Fixed generated input for inspection. This is data/source, not proof authority.
pub fn proof_input(module: &Module, a: &UncertifiedSafetyAnalysis) -> Result<String, SafetyError> {
    a.inventory.verify_module(module)?;
    concrete(module, a)
}

pub(crate) fn persist(output: &Path, result: &SafetyAssessment) -> Result<(), SafetyError> {
    fs::create_dir_all(output).map_err(io_error)?;
    fs::write(
        output.join("assessment.json"),
        canonical(&result.to_json())?,
    )
    .map_err(io_error)?;
    fs::write(output.join("evidence.json"), canonical(&result.record)?).map_err(io_error)?;
    fs::write(
        output.join("diagnostics.json"),
        canonical(&json!({"diagnostics":result.diagnostics}))?,
    )
    .map_err(io_error)?;
    Ok(())
}

pub fn certify(
    module: &Module,
    analysis: &UncertifiedSafetyAnalysis,
    output: &Path,
    tools: &ProofToolchain,
    limits: &ProofLimits,
) -> SafetyAssessment {
    let mut obligations = json!({"inventory_comparison":"NotRun","generic_proof":"NotRun","concrete_proof":"NotRun","theorem_types":"NotRun","axiom_audit":"NotRun","fresh_replay":"NotRun"});
    if !analysis.dependencies.is_empty() {
        obligations["lifecycle_preservation"] = json!("NotRun");
    }
    let metadata = proof_identity(module, analysis, tools);
    let mut proof=metadata.as_ref().cloned().unwrap_or_else(|_|json!({"statement_version":1,"checker_version":super::SAFETY_CHECKER_VERSION,"audit":[]}));
    let checked = metadata.and_then(|_| {
        certify_checked(
            module,
            analysis,
            output,
            tools,
            limits,
            &mut obligations,
            &mut proof,
        )
    });
    let (outcome, error) = match checked {
        Ok(()) => (SafetyOutcome::CertifiedUnderAssumptions, None),
        Err(e) => (outcome_for(&e), Some(e)),
    };
    let result = super::evidence::assessment(
        analysis,
        &module.behavior_version(),
        outcome,
        obligations,
        proof,
        error,
    );
    match persist(output, &result) {
        Ok(()) => result,
        Err(error) => super::evidence::assessment(
            analysis,
            &module.behavior_version(),
            SafetyOutcome::Inconclusive,
            result.record["obligations"].clone(),
            result.record["proof"].clone(),
            Some(error),
        ),
    }
}

fn certify_checked(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
    output: &Path,
    tools: &ProofToolchain,
    limits: &ProofLimits,
    obligations: &mut Value,
    proof: &mut Value,
) -> Result<(), SafetyError> {
    limits.validate()?;
    a.inventory.verify_module(module)?;
    obligations["inventory_comparison"] = json!("Passed");
    super::field_ownership::check(&a.inventory, &a.policies)?;
    if !a.issues.is_empty() {
        return Err(a
            .issues
            .iter()
            .find(|e| e.code() == "SAFETY_GUARD_SCAN_EXHAUSTED")
            .unwrap_or(&a.issues[0])
            .clone());
    }
    if a.policies.is_empty() {
        return Err(SafetyError::new(
            "SAFETY_EMPTY_POLICY_SET",
            "No safety property was requested",
        ));
    }
    // Revalidate at use time; an earlier tool-selection result is not proof evidence.
    ProofToolchain::from_root(&tools.root)?;
    fs::create_dir_all(output).map_err(io_error)?;
    let output = fs::canonicalize(output).map_err(io_error)?;
    proof_search_path(&output)?;
    let work = output.join(format!(
        "proof-{}-{}",
        std::process::id(),
        NEXT_RUN.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&work).map_err(io_error)?;
    fs::create_dir(work.join("safety")).map_err(io_error)?;
    let concrete = concrete(module, a)?;
    if proof["concrete_source"] != tagged("behavior.safety_proof_source.v1", &json!(concrete))? {
        return Err(SafetyError::new(
            "SAFETY_ENCODING_FAILED",
            "Concrete proof input identity is missing or changed",
        ));
    }
    let common = vec![
        format!("-DmaxHeartbeats={}", limits.max_heartbeats),
        format!("-M{}", limits.memory_mb),
        "-j1".into(),
        // The concrete LD input is larger than FO. Keep reduction bounded,
        // with a documented depth ceiling rather than Lean's unlimited mode.
        "-DmaxRecDepth=4096".into(),
    ];
    for (name, source) in SOURCES {
        fs::write(work.join(format!("safety/{name}.lean")), source).map_err(io_error)?;
        let mut args = common.clone();
        args.extend([
            "-o".into(),
            format!("safety/{name}.olean"),
            format!("safety/{name}.lean"),
        ]);
        run(tools, limits, &work, &tools.lean(), &args, name)?;
    }
    obligations["generic_proof"] = json!("Passed");
    fs::write(work.join("Concrete.lean"), &concrete).map_err(io_error)?;
    let mut args = common.clone();
    args.extend(["-o".into(), "Concrete.olean".into(), "Concrete.lean".into()]);
    run(tools, limits, &work, &tools.lean(), &args, "Concrete")?;
    obligations["concrete_proof"] = json!("Passed");
    if !a.dependencies.is_empty() {
        obligations["lifecycle_preservation"] = json!("Passed");
    }
    let verify = verification(a)?;
    fs::write(work.join("Verify.lean"), &verify).map_err(io_error)?;
    let mut args = common;
    args.push("Verify.lean".into());
    run(tools, limits, &work, &tools.lean(), &args, "Verify")?;
    let audit = fs::read_to_string(work.join("audit.jsonl"))
        .map_err(io_error)?
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(io_error))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = [
        "Safety.FieldOwnership.sound",
        "Safety.LifecycleDependency.sound",
        "Safety.LifecycleDependency.conjunction",
        "Safety.LifecycleDependency.projection_frame",
        "Safety.membership_order_equivalent",
        "Safety.checkLifecycle_sound",
        "Safety.classify_permitted",
        "Safety.lifecycle_module_preserves",
        "Safety.checkModule_sound",
        "Safety.module_preserves",
        "Concrete.checked",
        "Concrete.certified",
    ];
    if audit.len() != expected.len()
        || audit.iter().zip(expected).any(|(r, name)| {
            r["declaration"] != name
                || r["expected_type_checked"] != true
                || !r["axioms"].is_array()
                || r["axioms"].as_array().is_some_and(|ax| {
                    ax.iter().any(|a| {
                        !matches!(
                            a.as_str(),
                            Some("propext" | "Classical.choice" | "Quot.sound")
                        )
                    })
                })
        })
    {
        return Err(SafetyError::new(
            "SAFETY_PROOF_REJECTED",
            "Incomplete or invalid structured theorem audit",
        ));
    }
    obligations["theorem_types"] = json!("Passed");
    obligations["axiom_audit"] = json!("Passed");
    proof["audit"] = json!(audit);
    run(
        tools,
        limits,
        &work,
        &tools.root.join("bin/leanchecker"),
        &["--fresh".into(), "Concrete".into()],
        "Fresh",
    )?;
    obligations["fresh_replay"] = json!("Passed");
    fs::write(output.join("analysis.json"), canonical(&a.to_json())?).map_err(io_error)?;
    Ok(())
}

/// Bundle transport is supplied by Core's own serializer in the caller/binding.
/// Re-admission is the only consumer of that transport here; no expression AST
/// inspection or alternative Core parser is introduced.
pub fn bundle(
    module_transport: &str,
    a: &UncertifiedSafetyAnalysis,
    result: &SafetyAssessment,
) -> Result<super::evidence::UnverifiedSafetyBundle, SafetyError> {
    let module = behavior_engine::admit(module_transport).map_err(|e| {
        SafetyError::new(
            "SAFETY_INVALID_BUNDLE",
            format!("Core re-admission failed: {e:?}"),
        )
    })?;
    a.inventory.verify_module(&module)?;
    if result.record["behavior_hash"] != a.inventory.behavior_hash()
        || result.record["policy_set_hash"] != a.policy_set_hash()
    {
        return Err(SafetyError::new(
            "SAFETY_INVALID_BUNDLE",
            "Assessment scope differs from the transported module/policies",
        ));
    }
    super::evidence::UnverifiedSafetyBundle::from_json(
        &json!({"format":"behavior.safety_bundle.v1","module":module_transport,
        "inventory":a.inventory.to_json(),"policies":a.policy_records(),"evidence":result.record}),
    )
}

fn prepare_bundle(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
    bundle: &super::evidence::UnverifiedSafetyBundle,
    tools: &ProofToolchain,
) -> Result<UncertifiedSafetyAnalysis, SafetyError> {
    let invalid = |detail: &str| SafetyError::new("SAFETY_INVALID_BUNDLE", detail);
    a.inventory.verify_module(module)?;
    let data = &bundle.document;
    let transported = behavior_engine::admit(
        data["module"]
            .as_str()
            .ok_or_else(|| invalid("Missing module transport"))?,
    )
    .map_err(|e| invalid(&format!("Core re-admission failed: {e:?}")))?;
    if transported.behavior_version() != module.behavior_version() {
        return Err(invalid("Transported module identity differs"));
    }
    let inventory = super::Inventory::from_json(&data["inventory"])?;
    inventory.verify_module(&transported)?;
    let fresh = super::resolve::imported(&transported, &data["policies"])?;
    if fresh.policy_set_hash() != a.policy_set_hash()
        || fresh.inventory.to_json() != inventory.to_json()
        || fresh.policy_records() != a.policy_records()
    {
        return Err(invalid("Expected complete policy set or inventory differs"));
    }
    let report = &data["evidence"];
    if report["inventory"] != inventory.to_json()
        || report["policies"] != data["policies"]
        || report["behavior_hash"] != module.behavior_version()
        || report["policy_set_hash"] != fresh.policy_set_hash()
        || report["outcome"] != SafetyOutcome::CertifiedUnderAssumptions.as_str()
    {
        return Err(invalid("Claimed evidence scope/outcome differs"));
    }
    let required = [
        "inventory_comparison",
        "generic_proof",
        "concrete_proof",
        "theorem_types",
        "axiom_audit",
        "fresh_replay",
    ];
    if report["scope"] != "OrdinaryTransitions"
        || report["initial_validity"] != "Unestablished"
        || required
            .iter()
            .any(|key| report["obligations"][*key] != "Passed")
        || (!fresh.dependencies.is_empty()
            && report["obligations"]["lifecycle_preservation"] != "Passed")
    {
        return Err(invalid(
            "Claimed certification has missing/failed obligations or conflates initial-state validity",
        ));
    }
    let mut payload = report.clone();
    let hash = payload
        .as_object_mut()
        .ok_or_else(|| invalid("Expected semantic evidence object"))?
        .remove("evidence_hash");
    if hash != Some(json!(tagged("behavior.safety_evidence.v1", &payload)?)) {
        return Err(invalid("Evidence content identity differs"));
    }
    let mut expected = proof_identity(&transported, &fresh, tools)?;
    let mut supplied = report["proof"].clone();
    let audit = supplied
        .as_object_mut()
        .ok_or_else(|| invalid("Expected proof identity object"))?
        .remove("audit")
        .ok_or_else(|| invalid("Missing theorem audits"))?;
    expected
        .as_object_mut()
        .ok_or_else(|| invalid("Internal proof identity error"))?
        .remove("audit");
    if supplied != expected
        || report["dependency_identity"] != super::evidence::dependency_identity()?
    {
        return Err(invalid(
            "Stale/edited theorem, input, checker or dependency identity",
        ));
    }
    let roots = [
        "Safety.FieldOwnership.sound",
        "Safety.LifecycleDependency.sound",
        "Safety.LifecycleDependency.conjunction",
        "Safety.LifecycleDependency.projection_frame",
        "Safety.membership_order_equivalent",
        "Safety.checkLifecycle_sound",
        "Safety.classify_permitted",
        "Safety.lifecycle_module_preserves",
        "Safety.checkModule_sound",
        "Safety.module_preserves",
        "Concrete.checked",
        "Concrete.certified",
    ];
    let rows = audit
        .as_array()
        .ok_or_else(|| invalid("Invalid theorem audit data"))?;
    if rows.len() != roots.len()
        || rows.iter().zip(roots).any(|(row, name)| {
            row["declaration"] != name
                || row["expected_type_checked"] != true
                || row["axioms"].as_array().is_none_or(|axioms| {
                    axioms.iter().any(|ax| {
                        !matches!(
                            ax.as_str(),
                            Some("propext" | "Classical.choice" | "Quot.sound")
                        )
                    })
                })
        })
    {
        return Err(invalid("Forged or incomplete theorem type/axiom result"));
    }
    Ok(fresh)
}

/// Verify data by regenerating trusted wrappers and rerunning every proof check.
/// Supplied source code and cached compiled proof objects are never inputs.
pub fn verify_safety_evidence(
    module: &Module,
    expected: &UncertifiedSafetyAnalysis,
    bundle: &super::evidence::UnverifiedSafetyBundle,
    output: &Path,
    tools: &ProofToolchain,
    limits: &ProofLimits,
) -> SafetyAssessment {
    let rejected = |error: SafetyError| {
        super::evidence::assessment(
            expected,
            &module.behavior_version(),
            outcome_for(&error),
            json!({"inventory_comparison":"NotRun","generic_proof":"NotRun","concrete_proof":"NotRun","theorem_types":"NotRun","axiom_audit":"NotRun","fresh_replay":"NotRun"}),
            proof_identity(module, expected, tools).unwrap_or_else(|_| json!({})),
            Some(error),
        )
    };
    let mut result = match prepare_bundle(module, expected, bundle, tools) {
        Ok(fresh) => {
            let actual = certify(module, &fresh, output, tools, limits);
            if actual.certificate.is_some() && actual.record != bundle.document["evidence"] {
                rejected(SafetyError::new(
                    "SAFETY_INVALID_BUNDLE",
                    "Fresh checked semantic evidence differs from the supplied record",
                ))
            } else {
                actual
            }
        }
        Err(error) => rejected(error),
    };
    if let Err(error) = persist(output, &result) {
        result = super::evidence::assessment(
            expected,
            &module.behavior_version(),
            SafetyOutcome::Inconclusive,
            result.record["obligations"].clone(),
            result.record["proof"].clone(),
            Some(error),
        );
    }
    result
}

pub fn write_bundle(
    module_transport: &str,
    analysis: &UncertifiedSafetyAnalysis,
    result: &SafetyAssessment,
    output: &Path,
) -> Result<(), SafetyError> {
    let data = bundle(module_transport, analysis, result)?;
    fs::write(output.join("bundle.json"), canonical(&data.to_json())?).map_err(io_error)
}
