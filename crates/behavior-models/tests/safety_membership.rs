//! Independent public-Core C2 fixtures; authored IR is never normalized by Safety.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_engine::{
    builder::{Builder, ScopeSite, SemanticProfile},
    semantic::Module,
    wire::{WField, WType},
};
use behavior_models::safety::{
    FieldOwnershipInput, LifecycleDependencyInput, analyze_with_dependencies,
    certify::{ProofLimits, ProofToolchain, certify},
};
use serde_json::json;
use support::{loc, state};

fn fixture(
    values: &[String],
    required: &[String],
    nested: bool,
    variant: &str,
) -> (Module, Vec<FieldOwnershipInput>, LifecycleDependencyInput) {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    let mut members = required.to_vec();
    members.push("Active".into());
    members.push("Extra".into());
    b.declare_enum("Phase", members.clone(), loc()).unwrap();
    if variant == "enum" {
        let mut other = members;
        other.push("DifferentEnum".into());
        b.declare_enum("OtherPhase", other, loc()).unwrap();
    }
    for (name, fields) in [
        (
            "Parent",
            vec![
                ("phase", WType::Enum("Phase".into())),
                (
                    "alternate",
                    WType::Enum(if variant == "enum" {
                        "OtherPhase".into()
                    } else {
                        "Phase".into()
                    }),
                ),
            ],
        ),
        (
            "Child",
            vec![
                ("phase", WType::Enum("Phase".into())),
                ("owner", WType::Ref("Parent".into())),
            ],
        ),
    ] {
        b.declare_entity(
            name,
            fields
                .into_iter()
                .map(|(n, ty)| WField {
                    name: n.into(),
                    ty,
                    loc: loc(),
                })
                .collect(),
            loc(),
        )
        .unwrap();
    }
    let mut params = vec![state("p", "Parent"), state("q", "Parent")];
    if !nested {
        params.insert(0, state("c", "Child"));
    }
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let parent = if variant == "binding" { "q" } else { "p" };
    let mut guard = if nested {
        let query = b.select("Child", loc()).unwrap();
        b.push_lambda("authored", "Child").unwrap();
        let owner = b.field("authored", "owner", loc()).unwrap();
        let id = b.field(parent, "id", loc()).unwrap();
        let belongs = b.op("eq", vec![owner, id], loc()).unwrap();
        let phase = b.field("authored", "phase", loc()).unwrap();
        let membership = b
            .in_(phase, values.iter().map(|v| json!(v)).collect(), loc())
            .unwrap();
        let nonterminal = b.op("not", vec![membership], loc()).unwrap();
        let predicate = b.op("and", vec![belongs, nonterminal], loc()).unwrap();
        b.pop_scope();
        let any = b
            .lambda("any", query, "authored", predicate, loc())
            .unwrap();
        b.op("not", vec![any], loc()).unwrap()
    } else {
        let phase = b
            .field(
                parent,
                if variant == "field" || variant == "enum" {
                    "alternate"
                } else {
                    "phase"
                },
                loc(),
            )
            .unwrap();
        let membership = b
            .in_(phase, values.iter().map(|v| json!(v)).collect(), loc())
            .unwrap();
        b.op("not", vec![membership], loc()).unwrap()
    };
    if variant == "polarity" {
        guard = b.op("not", vec![guard], loc()).unwrap();
    }
    if variant == "or" {
        let yes = b.lit(WType::Bool, json!(true), loc()).unwrap();
        guard = b.op("or", vec![guard, yes], loc()).unwrap();
    }
    let mut roots = if variant == "missing" {
        vec![]
    } else {
        vec![(guard, loc())]
    };
    if !nested {
        let owner = b.field("c", "owner", loc()).unwrap();
        let id = b.field("p", "id", loc()).unwrap();
        roots.push((b.op("eq", vec![owner, id], loc()).unwrap(), loc()));
    }
    let target = if nested {
        required[0].as_str()
    } else {
        "Active"
    };
    let value = b
        .lit(WType::Enum("Phase".into()), json!(target), loc())
        .unwrap();
    b.add_action(
        "change",
        params,
        roots,
        vec![(
            if nested { "p" } else { "c" }.into(),
            "phase".into(),
            value,
            loc(),
        )],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let m = b.finish(None).unwrap();
    let policies=vec![FieldOwnershipInput::from_json(&json!({"id":"parents","entity":"Parent","writers":{"phase":["change"]},"creators":[],"removers":[]})).unwrap(),
    FieldOwnershipInput::from_json(&json!({"id":"children","entity":"Child","writers":{"phase":["change"],"owner":[]},"creators":[],"removers":[]})).unwrap()];
    let d=LifecycleDependencyInput::from_json(&json!({"id":"dependency","parent":"Parent","child":"Child","owner_field":"owner","parent_state":"phase","child_state":"phase","prohibited":required,"terminal":required,"ownership":["parents","children"]})).unwrap();
    (m, policies, d)
}
fn permutations(values: &[String]) -> Vec<Vec<String>> {
    if values.is_empty() {
        return vec![vec![]];
    }
    let mut out = vec![];
    for i in 0..values.len() {
        let mut rest = values.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            out.push(tail)
        }
    }
    out
}
#[test]
fn all_small_membership_permutations_have_clean_analysis() {
    for nested in [false, true] {
        for size in 2..=4 {
            let required = (0..size).map(|i| format!("M{i:02}")).collect::<Vec<_>>();
            for order in permutations(&required) {
                let (m, p, d) = fixture(&order, &required, nested, "canonical");
                let before = behavior_engine::serialize::to_wire_json(&m);
                let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
                assert_eq!(
                    a.to_json()["issues"],
                    json!([]),
                    "nested={nested} {order:?}: {}",
                    a.to_json()
                );
                assert_eq!(before, behavior_engine::serialize::to_wire_json(&m));
            }
        }
    }
}
#[test]
fn membership_negative_contexts_never_certify_by_hint_alone() {
    let tools = ProofToolchain::locked().unwrap();
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/release-readiness/negative-proofs-{}",
        std::process::id()
    ));
    let refuse =
        |m: &Module, a: &behavior_models::safety::UncertifiedSafetyAnalysis, label: &str| {
            let result = certify(m, a, &output.join(label), &tools, &ProofLimits::default());
            assert!(result.certificate().is_none(), "negative {label} certified");
            assert_eq!(
                result.to_json()["outcome"],
                "Unsupported",
                "{label}: {}",
                result.to_json()
            );
        };
    for variant in ["field", "enum"] {
        let required = vec!["M00".into(), "M01".into()];
        let (m, p, d) = fixture(&required, &required, false, variant);
        let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
        assert!(!a.issues().is_empty(), "wrong {variant} accepted");
        refuse(&m, &a, variant);
    }

    let required = vec!["M00".into(), "M01".into()];
    for nested in [false, true] {
        for variant in [
            "binding",
            "polarity",
            "or",
            "missing",
            "missing_member",
            "extra_member",
        ] {
            let mut order = required.clone();
            if variant == "missing_member" {
                order.pop();
            }
            if variant == "extra_member" {
                order.push("Extra".into());
            }
            let (m, p, d) = fixture(&order, &required, nested, variant);
            let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
            assert!(
                !a.to_json()["issues"].as_array().unwrap().is_empty(),
                "{nested} {variant}"
            );
            refuse(&m, &a, &format!("{nested}-{variant}"));
        }
    }
}
#[test]
fn every_small_permutation_duplicates_and_large_representatives_receive_actual_proofs() {
    let tools = ProofToolchain::locked().unwrap();
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/release-readiness/membership-proofs-{}",
        std::process::id()
    ));
    let mut cases = std::collections::VecDeque::new();
    for nested in [false, true] {
        for size in [2, 3, 4, 8, 16, 32] {
            let required = (0..size).map(|i| format!("M{i:02}")).collect::<Vec<_>>();
            let mut orders = if size <= 4 {
                permutations(&required)
            } else {
                let mut v = required.clone();
                v.reverse();
                vec![v]
            };
            let mut duplicates = required.clone();
            duplicates.reverse();
            duplicates.push(required[0].clone());
            orders.push(duplicates);
            for order in orders {
                cases.push_back((nested, required.clone(), order));
            }
        }
    }
    let count = cases.len();
    let queue = std::sync::Mutex::new(cases);
    // Independent proof processes, bounded concurrency; no proof cache or shared authority.
    std::thread::scope(|scope| {
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        for _ in 0..workers {
            let (queue, tools, output) = (&queue, &tools, &output);
            scope.spawn(move || {
                loop {
                    let Some((nested, required, order)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let (m, p, d) = fixture(&order, &required, nested, "canonical");
                    let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
                    let case_output = output.join(m.behavior_version().replace(':', "-"));
                    let result = certify(&m, &a, &case_output, tools, &ProofLimits::default());
                    assert!(
                        result.certificate().is_some(),
                        "nested={nested},order={order:?}: {}",
                        result.to_json()
                    );
                    println!(
                        "certified nested={nested} members={} actual_order={order:?}",
                        required.len()
                    );
                }
            });
        }
    });
    println!("all {count} exact-module instances passed full proof integrity");
}

#[test]
fn actual_core_membership_truth_and_seeded_larger_orders_agree_without_rewriting() {
    use behavior_engine::store::documents::SeedEntity;
    for nested in [false, true] {
        for size in [2, 3, 4, 8, 16, 32] {
            for seed in [1u64, 17, 510] {
                let required = (0..size).map(|i| format!("M{i:02}")).collect::<Vec<_>>();
                let mut order = required.clone();
                let mut random = seed;
                for i in (1..order.len()).rev() {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    order.swap(i, (random as usize) % (i + 1));
                }
                let (m, p, d) = fixture(&order, &required, nested, "canonical");
                let before = behavior_engine::serialize::to_wire_json(&m);
                let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
                assert_eq!(a.to_json()["issues"], json!([]));
                for phase in required.iter().map(String::as_str).chain(["Active"]) {
                    let parent_phase = if nested { "Active" } else { phase };
                    let child_phase = if nested { phase } else { "Active" };
                    let store = support::store(
                        &m,
                        vec![
                            SeedEntity {
                                entity: "Parent".into(),
                                value: json!({"id":"p1","phase":parent_phase,"alternate":"Active"}),
                            },
                            SeedEntity {
                                entity: "Parent".into(),
                                value: json!({"id":"p2","phase":"Active","alternate":"Active"}),
                            },
                            SeedEntity {
                                entity: "Child".into(),
                                value: json!({"id":"c1","phase":child_phase,"owner":"p1"}),
                            },
                        ],
                    );
                    let bindings = if nested {
                        json!({"p":{"entity":"Parent","id":"p1"},"q":{"entity":"Parent","id":"p2"}})
                    } else {
                        json!({"p":{"entity":"Parent","id":"p1"},"q":{"entity":"Parent","id":"p2"},"c":{"entity":"Child","id":"c1"}})
                    };
                    let result = support::invoke(&store, &m, "change", bindings, json!({}));
                    assert_eq!(
                        result.bundle.is_some(),
                        if nested {
                            phase != "Active"
                        } else {
                            phase == "Active"
                        },
                        "nested={nested} seed={seed} phase={phase}"
                    );
                }
                assert_eq!(before, behavior_engine::serialize::to_wire_json(&m));
            }
        }
    }
}

#[test]
fn record_exact_membership_case_inventory_for_release_evidence() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/safety-models/release-readiness/membership-inputs");
    std::fs::create_dir_all(&root).unwrap();
    let mut records = vec![];
    for nested in [false, true] {
        for size in [2, 3, 4, 8, 16, 32] {
            let required = (0..size).map(|i| format!("M{i:02}")).collect::<Vec<_>>();
            let mut orders = if size <= 4 {
                permutations(&required)
            } else {
                let mut v = required.clone();
                v.reverse();
                vec![v]
            };
            let mut duplicates = required.clone();
            duplicates.reverse();
            duplicates.push(required[0].clone());
            orders.push(duplicates);
            for order in orders {
                let (m, p, d) = fixture(&order, &required, nested, "canonical");
                let analysis = analyze_with_dependencies(&m, &p, &[d]).unwrap();
                assert!(analysis.issues().is_empty());
                let key = m.behavior_version().replace(':', "-");
                std::fs::write(
                    root.join(format!("{key}.json")),
                    behavior_engine::serialize::to_wire_json(&m),
                )
                .unwrap();
                records.push(json!({"nested":nested,"required":required,"ordered":order,"behavior_hash":m.behavior_version(),"policy_set_hash":analysis.policy_set_hash(),"module_file":format!("{key}.json")}));
            }
        }
    }
    assert_eq!(records.len(), 82);
    let text = behavior_engine::canonical::to_canonical_string(
        &json!({"format":"behavior.safety_membership_cases.v1","cases":records}),
    )
    .unwrap();
    std::fs::write(root.join("manifest.json"), text).unwrap();
}

#[test]
fn repeated_membership_evidence_is_identical_across_output_directories() {
    let required = vec!["M00".into(), "M01".into()];
    let order = vec!["M01".into(), "M00".into(), "M01".into()];
    let (m, p, d) = fixture(&order, &required, true, "canonical");
    let a = analyze_with_dependencies(&m, &p, &[d]).unwrap();
    let tools = ProofToolchain::locked().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/safety-models/release-readiness/membership-repeat-{}",
        std::process::id()
    ));
    let one = certify(&m, &a, &root.join("one"), &tools, &ProofLimits::default());
    let two = certify(
        &m,
        &a,
        &root.join("elsewhere"),
        &tools,
        &ProofLimits::default(),
    );
    assert!(one.certificate().is_some(), "{}", one.to_json());
    assert!(two.certificate().is_some(), "{}", two.to_json());
    assert_eq!(one.to_json(), two.to_json());
}
