#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_engine::wire::WType;
use behavior_models::safety::certify::{ProofLimits, ProofToolchain, certify};
use behavior_models::safety::{LifecycleDependencyInput, analyze_with_dependencies};
use serde_json::json;
use support::*;

#[test]
fn supported_operations_and_shared_parent_are_all_checked() {
    for two_children in [false, true] {
        let m = module(&Options {
            two_children,
            ..Options::default()
        });
        let deps = if two_children {
            vec![dependency("Child"), dependency("OtherChild")]
        } else {
            vec![dependency("Child")]
        };
        let a = analyze_with_dependencies(&m, &ownership(&m), &deps).unwrap();
        assert!(a.issues().is_empty(), "{:?}", a.issues());
        assert_eq!(a.dependencies().len(), deps.len());
        assert_eq!(a.lifecycle_actions()["stop"].relevant_operations, 1);
        assert_eq!(a.lifecycle_actions()["stop"].dependencies.len(), deps.len());
        assert!(!a.is_certified());
    }
}
#[test]
fn owner_must_be_required_ref_not_plain_or_optional_id() {
    for owner in [
        WType::Id("Parent".into()),
        WType::Option(Box::new(WType::Ref("Parent".into()))),
    ] {
        let m = declarations(owner, false).finish(None).unwrap();
        assert!(analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).is_err());
    }
}
#[test]
fn owner_assignments_are_structural_build_errors_including_noops() {
    for options in [
        Options {
            owner_assignment: true,
            ..Options::default()
        },
        Options {
            owner_noop: true,
            ..Options::default()
        },
    ] {
        let m = module(&options);
        let e = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap_err();
        assert_eq!(e.code(), "SAFETY_UNAUTHORIZED_WRITE");
        assert_eq!(e.to_json()["field"], "owner");
    }
}
#[test]
fn missing_ownership_and_invalid_enums_fail_before_certification() {
    let m = module(&Options::default());
    let mut controls = ownership(&m);
    controls
        .iter_mut()
        .find(|p| p.entity == "Child")
        .unwrap()
        .writers
        .remove("owner");
    assert!(analyze_with_dependencies(&m, &controls, &[dependency("Child")]).is_err());
    for mutation in [
        json!({"terminal":["Missing"]}),
        json!({"ownership":["stale"]}),
        json!({"parent_state":"note"}),
    ] {
        let mut p = json!({"id":"children","parent":"Parent","child":"Child","owner_field":"owner","parent_state":"phase","child_state":"phase",
            "prohibited":["Gone"],"terminal":["Terminal"],"ownership":["parent_control","child_control"]});
        p.as_object_mut()
            .unwrap()
            .extend(mutation.as_object().unwrap().clone());
        let input = LifecycleDependencyInput::from_json(&p).unwrap();
        assert!(analyze_with_dependencies(&m, &ownership(&m), &[input]).is_err());
    }
}
#[test]
fn computed_missing_guards_and_mixed_effects_are_uncertified_unsupported() {
    for options in [
        Options {
            computed: true,
            ..Options::default()
        },
        Options {
            missing_guard: true,
            ..Options::default()
        },
        Options {
            double_relevant: true,
            ..Options::default()
        },
        Options {
            mixed_creation: true,
            ..Options::default()
        },
        Options {
            two_children: true,
            missing_second_guard: true,
            ..Options::default()
        },
    ] {
        let m = module(&options);
        let mut deps = vec![dependency("Child")];
        if options.two_children {
            deps.push(dependency("OtherChild"));
        }
        let a = analyze_with_dependencies(&m, &ownership(&m), &deps).unwrap();
        assert!(!a.issues().is_empty(), "{options:?}");
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/safety-models/ld/unsupported");
        let result = certify(
            &m,
            &a,
            &output,
            &ProofToolchain::locked().unwrap(),
            &ProofLimits::default(),
        );
        assert_eq!(result.to_json()["outcome"], "Unsupported");
        assert!(result.certificate().is_none());
    }
}
#[test]
fn irrelevant_field_effect_does_not_consume_lifecycle_limit() {
    let m = module(&Options {
        unrelated_write: true,
        ..Options::default()
    });
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    assert!(a.issues().is_empty());
    assert_eq!(a.lifecycle_actions()["stop"].relevant_operations, 1);
    assert_eq!(a.inventory().actions()["stop"].operations.len(), 2);
}

#[test]
fn one_primitive_operation_cannot_hide_simultaneous_parent_and_child_roles() {
    use behavior_engine::builder::{Builder, ScopeSite};
    use behavior_engine::wire::{Role, WField, WParam};
    let mut b = Builder::new();
    b.declare_enum("Phase", vec!["Open".into(), "Gone".into()], loc())
        .unwrap();
    b.declare_entity(
        "Node",
        vec![
            WField {
                name: "phase".into(),
                ty: WType::Enum("Phase".into()),
                loc: loc(),
            },
            WField {
                name: "owner".into(),
                ty: WType::Ref("Node".into()),
                loc: loc(),
            },
        ],
        loc(),
    )
    .unwrap();
    let params = vec![WParam {
        name: "n".into(),
        role: Some(Role::State),
        ty: WType::Entity("Node".into()),
    }];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let gone = b
        .lit(WType::Enum("Phase".into()), json!("Gone"), loc())
        .unwrap();
    b.add_action(
        "change",
        params,
        vec![],
        vec![("n".into(), "phase".into(), gone, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let m = b.finish(None).unwrap();
    let fo = behavior_models::safety::FieldOwnershipInput::from_json(
        &json!({"id":"nodes","entity":"Node",
        "writers":{"phase":["change"],"owner":[]}}),
    )
    .unwrap();
    let dep=LifecycleDependencyInput::from_json(&json!({"id":"self_relation","parent":"Node","child":"Node",
        "owner_field":"owner","parent_state":"phase","child_state":"phase","prohibited":["Gone"],"terminal":["Gone"],"ownership":["nodes"]})).unwrap();
    let a = analyze_with_dependencies(&m, &[fo], &[dep]).unwrap();
    assert!(
        a.issues()
            .iter()
            .any(|e| e.code() == "SAFETY_MIXED_LIFECYCLE_ROLES")
    );
}

#[test]
fn mixed_roles_across_two_distinct_dependencies_are_also_unsupported() {
    use behavior_engine::{
        builder::{Builder, ScopeSite},
        wire::{Role, WField, WParam},
    };
    let mut b = Builder::new();
    b.declare_enum("Phase", vec!["Open".into(), "Terminal".into()], loc())
        .unwrap();
    for (entity, owner) in [
        ("Top", None),
        ("Middle", Some("Top")),
        ("Leaf", Some("Middle")),
    ] {
        let mut fields = vec![WField {
            name: "phase".into(),
            ty: WType::Enum("Phase".into()),
            loc: loc(),
        }];
        if let Some(parent) = owner {
            fields.push(WField {
                name: "owner".into(),
                ty: WType::Ref(parent.into()),
                loc: loc(),
            });
        }
        b.declare_entity(entity, fields, loc()).unwrap();
    }
    let params = vec![WParam {
        name: "p".into(),
        role: Some(Role::State),
        ty: WType::Entity("Middle".into()),
    }];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let guard = no_children(&mut b, "Leaf");
    let terminal = b
        .lit(WType::Enum("Phase".into()), json!("Terminal"), loc())
        .unwrap();
    b.add_action(
        "change",
        params,
        vec![(guard, loc())],
        vec![("p".into(), "phase".into(), terminal, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let m = b.finish(None).unwrap();
    let controls = ["Top", "Middle", "Leaf"].map(|entity| {
        behavior_models::safety::FieldOwnershipInput::from_json(&json!({
        "id":entity.to_lowercase(),"entity":entity,"writers":if entity=="Top"{json!({"phase":[]})}
            else{json!({"phase":if entity=="Middle"{vec!["change"]}else{vec![]},"owner":[]})}}))
        .unwrap()
    });
    let deps=[("top_middle","Top","Middle"),("middle_leaf","Middle","Leaf")].map(|(id,parent,child)|
        LifecycleDependencyInput::from_json(&json!({"id":id,"parent":parent,"child":child,"owner_field":"owner",
            "parent_state":"phase","child_state":"phase","prohibited":["Terminal"],"terminal":["Terminal"],
            "ownership":[parent.to_lowercase(),child.to_lowercase()]})).unwrap());
    let a = analyze_with_dependencies(&m, &controls, &deps).unwrap();
    assert!(
        a.issues()
            .iter()
            .any(|e| e.code() == "SAFETY_MIXED_LIFECYCLE_ROLES")
    );
}

fn prerequisite_fixture(mode: &str) -> behavior_engine::semantic::Module {
    use behavior_engine::{
        builder::ScopeSite,
        wire::{Role, WLifecycle, WParam},
    };
    let mut b = declarations(WType::Ref("Parent".into()), false);
    let creating = mode.starts_with("terminal_create");
    let no_parent = matches!(
        mode,
        "binding" | "terminal_assign" | "terminal_create_no_binding"
    );
    let mut params = if creating {
        vec![]
    } else {
        vec![state("c", "Child")]
    };
    if !no_parent {
        params.extend([state("p", "Parent"), state("q", "Parent")]);
    }
    if creating {
        params.extend([
            WParam {
                name: "new_id".into(),
                role: Some(Role::Input),
                ty: WType::Id("Child".into()),
            },
            WParam {
                name: "owner_id".into(),
                role: Some(Role::Input),
                ty: WType::Id("Parent".into()),
            },
        ]);
    }
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let mut guards = vec![];
    if matches!(mode, "guard" | "cross" | "good" | "later") {
        let owner = b.field("c", "owner", loc()).unwrap();
        let id = b
            .field(if mode == "later" { "q" } else { "p" }, "id", loc())
            .unwrap();
        guards.push((b.op("eq", vec![owner, id], loc()).unwrap(), loc()));
    }
    if matches!(mode, "association" | "cross" | "good" | "later") {
        let phase = b
            .field(
                if matches!(mode, "cross" | "later") {
                    "q"
                } else {
                    "p"
                },
                "phase",
                loc(),
            )
            .unwrap();
        let membership = b
            .in_(phase, vec![json!("Abandoned"), json!("Gone")], loc())
            .unwrap();
        guards.push((b.op("not", vec![membership], loc()).unwrap(), loc()));
    }
    let terminal = creating || mode == "terminal_assign";
    let value = b
        .lit(
            WType::Enum("ChildPhase".into()),
            json!(if terminal { "Terminal" } else { "Active" }),
            loc(),
        )
        .unwrap();
    let (effects, creates) = if creating {
        let id = b.param("new_id", loc()).unwrap().into_wire();
        let owner = if matches!(
            mode,
            "terminal_create_no_assoc" | "terminal_create_no_binding"
        ) {
            b.param("owner_id", loc()).unwrap()
        } else {
            b.field("p", "id", loc()).unwrap()
        };
        (
            vec![],
            vec![WLifecycle::Create {
                entity: "Child".into(),
                id,
                fields: vec![
                    ("owner".into(), owner.into_wire()),
                    ("phase".into(), value.into_wire()),
                ],
                loc: loc(),
            }],
        )
    } else {
        (vec![("c".into(), "phase".into(), value, loc())], vec![])
    };
    b.add_action("change", params, guards, effects, creates, vec![], loc())
        .unwrap();
    b.pop_scope();
    b.finish(None).unwrap()
}

#[test]
fn prerequisite_diagnostics_are_staged_typed_and_do_not_combine_different_parents() {
    for (mode, code, entity, field) in [
        (
            "binding",
            "SAFETY_PARENT_BINDING_REQUIRED",
            "Parent",
            "phase",
        ),
        (
            "association",
            "SAFETY_OWNER_ASSOCIATION_REQUIRED",
            "Child",
            "owner",
        ),
        ("guard", "SAFETY_PARENT_GUARD_REQUIRED", "Parent", "phase"),
        (
            "combined",
            "SAFETY_OWNER_ASSOCIATION_REQUIRED",
            "Child",
            "owner",
        ),
        ("cross", "SAFETY_PARENT_GUARD_REQUIRED", "Parent", "phase"),
        (
            "terminal_create_no_assoc",
            "SAFETY_OWNER_ASSOCIATION_REQUIRED",
            "Child",
            "owner",
        ),
        (
            "terminal_create_no_binding",
            "SAFETY_PARENT_BINDING_REQUIRED",
            "Parent",
            "phase",
        ),
    ] {
        let m = prerequisite_fixture(mode);
        let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
        assert_eq!(a.issues().len(), 1, "{mode}: {}", a.to_json());
        let issue = a.issues()[0].to_json();
        assert_eq!(issue["code"], code, "{mode}");
        assert_eq!(issue["action"], "change");
        assert_eq!(issue["dependency"], a.dependencies()[0].model_hash());
        assert_eq!(issue["entity"], entity);
        assert_eq!(issue["field"], field);
        let repeat = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
        assert_eq!(a.to_json(), repeat.to_json());
        let tools = ProofToolchain::locked().unwrap();
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/safety-models/release-readiness/diagnostics-{}-{mode}",
            std::process::id()
        ));
        let result = certify(&m, &a, &output, &tools, &ProofLimits::default());
        assert!(result.certificate().is_none());
        assert_eq!(result.to_json()["outcome"], "Unsupported");
        assert_eq!(result.to_json()["diagnostics"], json!([issue]));
    }
    for mode in ["later", "good", "terminal_assign", "terminal_create"] {
        let m = prerequisite_fixture(mode);
        let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
        assert!(a.issues().is_empty(), "{mode}: {}", a.to_json());
    }
}

#[test]
fn independent_dependency_diagnostics_are_ordered_by_semantic_action_and_policy() {
    let m = module(&Options {
        two_children: true,
        missing_second_guard: true,
        computed: true,
        ..Options::default()
    });
    let deps = [dependency("Child"), dependency("OtherChild")];
    let a = analyze_with_dependencies(&m, &ownership(&m), &deps).unwrap();
    let keys = a
        .issues()
        .iter()
        .map(|e| {
            let v = e.to_json();
            let name = v["action"].as_str().unwrap();
            assert!(v["dependency"].is_string());
            (
                a.inventory().actions()[name].hash.clone(),
                v["dependency"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
    let fixed = module(&Options {
        two_children: true,
        missing_second_guard: true,
        ..Options::default()
    });
    let remaining = analyze_with_dependencies(&fixed, &ownership(&fixed), &deps).unwrap();
    assert_eq!(remaining.issues().len(), 1);
    assert_eq!(remaining.issues()[0].to_json()["action"], "stop");
}
