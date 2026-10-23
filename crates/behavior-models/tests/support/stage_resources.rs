//! Real pinned-process controls, not a mathematical certificate fixture.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;

fn workspace(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/safety-models/scale-511/stage-controls")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            NEXT_RUN.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn stage_resource_timeout_persists_actual_limits_and_exhaustion() {
    let tools = ProofToolchain::locked().unwrap();
    let work = workspace("timeout");
    fs::write(
        work.join("Slow.lean"),
        "def main : IO Unit := IO.sleep 3000\n",
    )
    .unwrap();
    let limits = ProofLimits {
        wall_seconds: 1,
        ..ProofLimits::default()
    };
    let args = vec![
        "-DmaxHeartbeats=17".into(),
        "-M128".into(),
        "-DmaxRecDepth=32".into(),
        "--run".into(),
        "Slow.lean".into(),
    ];
    for label in ["Concrete", "Verify"] {
        let error = run(&tools, &limits, &work, &tools.lean(), &args, label).unwrap_err();
        assert_eq!(error.code(), "SAFETY_PROOF_EXHAUSTED");
        let resources: Value = serde_json::from_str(
            &fs::read_to_string(work.join(format!("{label}.resources.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(resources["status"], "Exhausted");
        assert_eq!(resources["limits"]["heartbeats"], 17);
        assert_eq!(resources["limits"]["heap_mb"], 128);
        assert_eq!(resources["limits"]["recursion_depth"], 32);
        assert_eq!(resources["actual_heartbeats"], Value::Null);
        assert_eq!(resources["actual_recursion_depth"], Value::Null);
        assert!(resources["measurement_limits"].is_array());
    }
}

#[test]
fn stage_resource_fresh_does_not_claim_lean_elaboration_limits() {
    let tools = ProofToolchain::locked().unwrap();
    let work = workspace("fresh");
    let limits = ProofLimits::default();
    assert!(
        run(
            &tools,
            &limits,
            &work,
            &tools.root.join("bin/leanchecker"),
            &["--fresh".into(), "AbsentConcrete".into()],
            "Fresh"
        )
        .is_err()
    );
    let resources: Value =
        serde_json::from_str(&fs::read_to_string(work.join("Fresh.resources.json")).unwrap())
            .unwrap();
    assert_eq!(resources["status"], "Failed");
    assert_eq!(resources["limits"]["heartbeats"], Value::Null);
    assert_eq!(resources["limits"]["heap_mb"], Value::Null);
    assert_eq!(resources["limits"]["recursion_depth"], Value::Null);
    assert_eq!(
        resources["limits"]["address_space_mb"],
        limits.address_space_mb
    );
    assert_eq!(resources["limits"]["wall_seconds"], limits.wall_seconds);
}
