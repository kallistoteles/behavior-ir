//! The proposal is constructed independently from the producer's balanced tree.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;

fn scope() -> (Module, UncertifiedSafetyAnalysis) {
    let m = test_support::fixture();
    let p=super::super::FieldOwnershipInput::from_json(&json!({"id":"notes","entity":"Item","writers":{"note":["edit","copy"]},"creators":["create"],"removers":["remove"]})).unwrap();
    let a = super::super::analyze(&m, &[p]).unwrap();
    (m, a)
}
fn skewed(keys: &[String]) -> Value {
    fn build(keys: &[String], nodes: &mut Vec<Value>) -> (usize, Vec<String>) {
        if keys.len() == 1 {
            let index = nodes.len();
            nodes.push(json!({"kind":"leaf"}));
            return (index, keys.to_vec());
        }
        let left = nodes.len();
        nodes.push(json!({"kind":"leaf"}));
        let (right, sorted) = build(&keys[1..], nodes);
        let mut choices = Vec::new();
        let mut result = Vec::new();
        let mut inserted = false;
        for key in sorted {
            if !inserted && keys[0] < key {
                choices.push(true);
                result.push(keys[0].clone());
                inserted = true;
            }
            choices.push(false);
            result.push(key);
        }
        if !inserted {
            choices.push(true);
            result.push(keys[0].clone());
        }
        let index = nodes.len();
        nodes.push(json!({"kind":"branch","left":left,"right":right,"choices":choices}));
        (index, result)
    }
    let mut nodes = Vec::new();
    let (root, _) = build(keys, &mut nodes);
    json!({"format":"behavior.safety_merge_trace.v1","nodes":nodes,"root":root})
}
#[test]
fn independently_constructed_valid_trace_passes_the_complete_checker() {
    let (m, a) = scope();
    let keys = a
        .inventory
        .actions()
        .values()
        .map(|v| v.hash.clone())
        .collect::<Vec<_>>();
    let witness = MergeWitness::from_json(&skewed(&keys)).unwrap();
    assert_ne!(
        witness.to_json(),
        MergeWitness::produce(&a.inventory).to_json()
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/scale-511/independent-witness-{}",
        std::process::id()
    ));
    let tools = ProofToolchain::locked().unwrap();
    let canonical = proof_identity(&m, &a, &tools).unwrap();
    let alternate = proof_identity_with_witness(&m, &a, &tools, &witness).unwrap();
    assert_ne!(canonical["concrete_source"], alternate["concrete_source"]);
    assert_eq!(canonical["checker_version"], alternate["checker_version"]);
    let result = certify_with_witness(&m, &a, &witness, &root, &tools, &ProofLimits::default());
    assert!(result.certificate().is_some(), "{}", result.to_json());
    assert_eq!(
        result.to_json()["proof"]["audit"].as_array().unwrap().len(),
        16
    );
    assert_eq!(result.to_json()["proof"]["witness"], witness.to_json());
    let transported = bundle(&behavior_engine::serialize::to_wire_json(&m), &a, &result).unwrap();
    let verified = verify_safety_evidence(
        &m,
        &a,
        &transported,
        &root.join("consumer"),
        &tools,
        &ProofLimits::default(),
    );
    assert!(verified.certificate().is_some(), "{}", verified.to_json());
    assert_eq!(
        verified.to_json()["evidence_hash"],
        result.to_json()["evidence_hash"]
    );

    let original = MergeWitness::produce(&a.inventory);
    let mut damaged = original.to_json();
    damaged["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|n| n["kind"] == "leaf");
    damaged["root"] = json!(damaged["nodes"].as_array().unwrap().len() - 1);
    assert!(MergeWitness::from_json(&damaged).is_err());
}

#[test]
fn malformed_or_incomplete_proposals_cannot_issue_certificates() {
    let (m, a) = scope();
    let tools = ProofToolchain::locked().unwrap();
    let mut variants =
        vec![json!({"format":"behavior.safety_merge_trace.v1","nodes":[{"kind":"leaf"}],"root":0})];
    let mut truncated = MergeWitness::produce(&a.inventory).to_json();
    truncated["nodes"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["choices"]
        .as_array_mut()
        .unwrap()
        .pop();
    variants.push(truncated);
    let mut wrong_order = MergeWitness::produce(&a.inventory).to_json();
    let first = &mut wrong_order["nodes"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["choices"][0];
    *first = json!(!first.as_bool().unwrap());
    variants.push(wrong_order);
    for (i, data) in variants.into_iter().enumerate() {
        let witness = MergeWitness::from_json(&data).unwrap();
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/safety-models/scale-511/bad-witness-{}-{i}",
            std::process::id()
        ));
        let result = certify_with_witness(&m, &a, &witness, &out, &tools, &ProofLimits::default());
        assert!(result.certificate().is_none(), "{}", result.to_json());
        assert_eq!(result.to_json()["obligations"]["fresh_replay"], "NotRun");
    }
}
