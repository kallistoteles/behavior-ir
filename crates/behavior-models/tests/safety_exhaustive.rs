//! Bounded correspondence testing of real Core commits; not a universal proof.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_models::safety::{
    analyze_with_dependencies,
    certify::{ProofLimits, ProofToolchain, certify},
};
use serde_json::{Value, json};
use support::*;

#[test]
fn enumerate_small_typed_states_against_actual_core_transitions() {
    let mut source_states = 0_u64;
    let mut valid_sources = 0_u64;
    let mut attempts = 0_u64;
    let mut commits = 0_u64;
    let mut kinds = std::collections::BTreeMap::<String, u64>::new();
    for family_count in [1, 2] {
        let m = module(&Options {
            two_children: family_count == 2,
            ..Options::default()
        });
        let deps = if family_count == 1 {
            vec![dependency("Child")]
        } else {
            vec![dependency("Child"), dependency("OtherChild")]
        };
        let a = analyze_with_dependencies(&m, &ownership(&m), &deps).unwrap();
        assert!(a.issues().is_empty());
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/safety-models/ld/exhaustive-certificate-{family_count}"
        ));
        let certificate = certify(
            &m,
            &a,
            &output,
            &ProofToolchain::locked().unwrap(),
            &ProofLimits::default(),
        );
        assert!(
            certificate.certificate().is_some(),
            "{}",
            certificate.to_json()
        );
        for np in 0_usize..=2 {
            eprintln!(
                "Safety enumeration: {family_count} child types, {np} parents; {attempts} transitions checked"
            );
            for n0 in 0_usize..=2 {
                for n1 in 0_usize..=if family_count == 2 { 2 } else { 0 } {
                    let nc = n0 + n1;
                    if np == 0 && nc > 0 {
                        continue;
                    }
                    for owner_code in 0..np.pow(nc as u32) {
                        for phase_bits in 0..(1_usize << (np + nc)) {
                            for variant in [0, 1] {
                                source_states += 1;
                                let pid = |i| {
                                    if variant == 0 {
                                        format!("p{i}")
                                    } else {
                                        format!("{i}")
                                    }
                                };
                                let cid = |family, i| {
                                    if variant == 0 {
                                        format!("c{family}_{i}")
                                    } else {
                                        format!("{i}")
                                    }
                                };
                                let mut seed = (0..np)
                                    .map(|i| {
                                        parent(
                                            &pid(i),
                                            if phase_bits & (1 << i) == 0 {
                                                "Open"
                                            } else {
                                                "Gone"
                                            },
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                let mut owners = owner_code;
                                let mut index = np;
                                let mut children = vec![];
                                for (family, count) in [(0, n0), (1, n1)] {
                                    for i in 0..count {
                                        let owner = owners % np;
                                        owners /= np;
                                        let entity =
                                            if family == 0 { "Child" } else { "OtherChild" };
                                        let id = cid(family, i);
                                        seed.push(child(
                                            entity,
                                            &id,
                                            &pid(owner),
                                            if phase_bits & (1 << index) == 0 {
                                                "Active"
                                            } else {
                                                "Terminal"
                                            },
                                        ));
                                        children.push((family, entity, id, pid(owner)));
                                        index += 1;
                                    }
                                }
                                if !property(&seed) {
                                    continue;
                                }
                                valid_sources += 1;
                                let binding =
                                    |entity: &str, id: &str| json!({"entity":entity,"id":id});
                                let mut operations: Vec<(String, Value, Value)> = vec![(
                                    "create_parent".into(),
                                    json!({}),
                                    json!({"new_id":"fresh_parent"}),
                                )];
                                for i in 0..np {
                                    for action in ["stop", "reopen", "remove_parent"] {
                                        operations.push((
                                            action.into(),
                                            json!({"p":binding("Parent",&pid(i))}),
                                            json!({}),
                                        ));
                                    }
                                    for family in 0..family_count {
                                        operations.push((
                                            if family == 0 {
                                                "create_child"
                                            } else {
                                                "other_create_child"
                                            }
                                            .into(),
                                            json!({"p":binding("Parent",&pid(i))}),
                                            json!({"new_id":"fresh_child"}),
                                        ));
                                    }
                                }
                                for (family, entity, id, owner) in &children {
                                    for action in ["activate", "finish", "remove_child"] {
                                        let mut bindings = json!({"c":binding(entity,id)});
                                        if action == "activate" {
                                            bindings["p"] = binding("Parent", owner);
                                        }
                                        operations.push((
                                            format!(
                                                "{}{action}",
                                                if *family == 0 { "" } else { "other_" }
                                            ),
                                            bindings,
                                            json!({}),
                                        ));
                                    }
                                }
                                for (action, bindings, input) in operations {
                                    attempts += 1;
                                    let mut st = store(&m, seed.clone());
                                    let result = invoke(&st, &m, &action, bindings, input);
                                    if let Some(bundle) = result.bundle {
                                        st.commit(&m, &st.current().unwrap(), &bundle).unwrap();
                                        commits += 1;
                                        *kinds.entry(action.clone()).or_default() += 1;
                                        assert!(
                                            property(&snapshot(&st, &m)),
                                            "unsafe commit {action}, seed {seed:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for action in [
        "stop",
        "reopen",
        "create_parent",
        "remove_parent",
        "activate",
        "finish",
        "create_child",
        "remove_child",
    ] {
        assert!(
            kinds.get(action).copied().unwrap_or(0) > 0,
            "No useful success for {action}"
        );
    }
    let record = json!({"format":"behavior.safety_exhaustive_coverage.v1","passed":true,
        "evidence_class":"Bounded actual-Core correspondence tests; not generic mathematical proof",
        "bounds":{"parents":[0,2],"children_per_type":[0,2],"child_types":[1,2],"parent_categories":["Open","Gone"],
        "child_categories":["Active","Terminal"],"owner_mappings":"all valid typed mappings","id_variants":2},
        "core_valid_source_states":source_states,"safety_valid_source_states":valid_sources,"attempts":attempts,"committed":commits,
        "unsafe_certified_commits":0,"commits_by_action":kinds});
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/safety-models/ld/coverage.json");
    std::fs::write(
        path,
        behavior_engine::canonical::to_canonical_string(&record).unwrap(),
    )
    .unwrap();
}
