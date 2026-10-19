#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_models::safety::{
    analyze_with_dependencies,
    certify::{ProofLimits, ProofToolchain, certify},
};
#[test]
fn lifecycle_generic_and_exact_concrete_proofs_pass_with_premise_mutations() {
    let m = support::module(&support::Options::default());
    let a = analyze_with_dependencies(&m, &support::ownership(&m), &[support::dependency("Child")])
        .unwrap();
    let tools = ProofToolchain::locked().unwrap();
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/ld/proof-control-{}",
        std::process::id()
    ));
    let result = certify(&m, &a, &output, &tools, &ProofLimits::default());
    assert!(result.certificate().is_some(), "{}", result.to_json());
    let report = result.to_json();
    assert_eq!(report["obligations"]["lifecycle_preservation"], "Passed");
    assert!(
        report["proof"]["audit"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["declaration"] == "Safety.LifecycleDependency.sound")
    );
    let root = std::fs::read_dir(output)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir() && p.join("Concrete.olean").exists())
        .unwrap();
    let source = root.join("LifecycleMutations.lean");
    std::fs::write(
        &source,
        include_str!("../proofs/safety/tests/LifecycleMutations.lean"),
    )
    .unwrap();
    assert!(
        std::process::Command::new(tools.lean())
            .arg(source)
            .env("LEAN_PATH", &root)
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let original = std::fs::read_to_string(root.join("Concrete.lean")).unwrap();
    let lifecycle_start = original.find("lifecycle := ").unwrap();
    let tail = &original[lifecycle_start..];
    let roots = tail.find("roots");
    assert!(
        roots.is_none(),
        "The positional finite encoding changed; review mutations"
    );
    // Typed finite-data mutation: State bindings become Context/Input-like.
    // Lowering stays well formed; canonical parent/owner premises must fail.
    let changed = tail
        .replace("(0, true)", "(0, false)")
        .replace("(1, true)", "(1, false)")
        .replace("(0,true)", "(0,false)")
        .replace("(1,true)", "(1,false)");
    assert_ne!(changed, tail, "No actual role data was changed");
    let role_mutation = format!("{}{}", &original[..lifecycle_start], changed);
    std::fs::write(root.join("RoleMutation.lean"), role_mutation).unwrap();
    let failed = std::process::Command::new(tools.lean())
        .args(["-DmaxRecDepth=4096", "RoleMutation.lean"])
        .env("LEAN_PATH", &root)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let diagnostic = String::from_utf8_lossy(&failed.stdout);
    assert!(
        diagnostic.contains("decide") || diagnostic.contains("false"),
        "{diagnostic}"
    );

    let parent_index = m.entities().keys().position(|n| n == "Parent").unwrap();
    let stop = m.action("stop").unwrap();
    let hash =
        behavior_engine::semantic::types::hash_display(stop.preconditions()[0].expr().hash());
    let encoded = format!(
        "[{}]",
        hash.bytes()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let target = format!("⟨[({parent_index}, true)],[{encoded}],");
    let replacement = format!("⟨[({parent_index}, true)],[],");
    let changed = tail.replacen(&target, &replacement, 1);
    assert_ne!(changed, tail, "No actual mandatory guard root was removed");
    std::fs::write(
        root.join("GuardMutation.lean"),
        format!("{}{}", &original[..lifecycle_start], changed),
    )
    .unwrap();
    let failed = std::process::Command::new(tools.lean())
        .args(["-DmaxRecDepth=4096", "GuardMutation.lean"])
        .env("LEAN_PATH", &root)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let diagnostic = String::from_utf8_lossy(&failed.stdout);
    assert!(
        diagnostic.contains("decide") || diagnostic.contains("false"),
        "{diagnostic}"
    );
    let category = root.join("CategoryMembership.lean");
    std::fs::write(
        &category,
        include_str!("../proofs/safety/tests/CategoryMembership.lean"),
    )
    .unwrap();
    assert!(
        std::process::Command::new(tools.lean())
            .arg(category)
            .env("LEAN_PATH", &root)
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
}
