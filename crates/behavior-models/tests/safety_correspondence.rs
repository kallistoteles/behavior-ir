//! Public Core feasibility only: no LD certification or property implementation.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use behavior_engine::builder::{Builder, Node, ScopeSite, SemanticProfile};
use behavior_engine::semantic::{Module, types::hash_display};
use behavior_engine::wire::{Loc, Role, WField, WLifecycle, WParam, WType};
use behavior_models::safety::guards::{CanonicalExpression as E, CanonicalProbes};
use serde_json::json;

fn loc() -> Loc {
    Loc {
        file: "safety_correspondence.rs".into(),
        line: 1,
    }
}
fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}
fn param(name: &str, entity: &str, role: Role) -> WParam {
    WParam {
        name: name.into(),
        ty: WType::Entity(entity.into()),
        role: Some(role),
    }
}
fn inputs() -> Vec<(String, String)> {
    vec![("p".into(), "Parent".into()), ("c".into(), "Child".into())]
}

fn member(b: &mut Builder, binding: &str, field: &str, values: &[&str], negated: bool) -> Node {
    if values.is_empty() {
        return b.lit(WType::Bool, json!(negated), loc()).unwrap();
    }
    let v = b.field(binding, field, loc()).unwrap();
    let m = b
        .in_(v, values.iter().map(|v| json!(v)).collect(), loc())
        .unwrap();
    if negated {
        b.op("not", vec![m], loc()).unwrap()
    } else {
        m
    }
}
fn no_children(b: &mut Builder, child: &str, owner: &str, status: &str, candidate: &str) -> Node {
    let q = b.select(child, loc()).unwrap();
    b.push_lambda(candidate, child).unwrap();
    let owner = b.field(candidate, owner, loc()).unwrap();
    let id = b.field("p", "id", loc()).unwrap();
    let belongs = b.op("eq", vec![owner, id], loc()).unwrap();
    let nonterminal = member(b, candidate, status, &["Terminal"], true);
    let body = b.op("and", vec![belongs, nonterminal], loc()).unwrap();
    b.pop_scope();
    let any = b.lambda("any", q, candidate, body, loc()).unwrap();
    b.op("not", vec![any], loc()).unwrap()
}
fn fixture() -> Module {
    fixture_with_reference(true)
}
fn fixture_with_reference(required: bool) -> Module {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum(
        "ParentPhase",
        vec!["Open".into(), "Gone".into(), "Abandoned".into()],
        loc(),
    )
    .unwrap();
    b.declare_enum(
        "ChildPhase",
        vec!["Active".into(), "Terminal".into()],
        loc(),
    )
    .unwrap();
    b.declare_nominal("Time", WType::Int, vec!["order".into()], None, loc())
        .unwrap();
    b.declare_nominal(
        "Money",
        WType::Decimal,
        vec!["order".into(), "add".into(), "scale".into(), "ratio".into()],
        Some(2),
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Parent",
        vec![
            field("phase", WType::Enum("ParentPhase".into())),
            field("note", WType::String),
        ],
        loc(),
    )
    .unwrap();
    for child in ["Child", "OtherChild"] {
        b.declare_entity(
            child,
            vec![
                field(
                    "owner",
                    if required {
                        WType::Ref("Parent".into())
                    } else {
                        WType::Id("Parent".into())
                    },
                ),
                field("phase", WType::Enum("ChildPhase".into())),
                field("alternate", WType::Enum("ChildPhase".into())),
            ],
            loc(),
        )
        .unwrap();
    }
    // Every actual expression is built independently of the probe implementation.
    for (name, source, owner, status, candidate, weaken) in [
        ("stop", "Child", "owner", "phase", "candidate", false),
        (
            "alpha_stop",
            "Child",
            "owner",
            "phase",
            "renamed_lambda",
            false,
        ),
        (
            "wrong_query",
            "OtherChild",
            "owner",
            "phase",
            "candidate",
            false,
        ),
        (
            "wrong_field",
            "Child",
            "owner",
            "alternate",
            "candidate",
            false,
        ),
        ("weakened", "Child", "owner", "phase", "candidate", true),
        ("nested", "Child", "owner", "phase", "candidate", false),
    ] {
        let params = vec![param("p", "Parent", Role::State)];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let mut guard = no_children(&mut b, source, owner, status, candidate);
        if weaken {
            let t = b.lit(WType::Bool, json!(true), loc()).unwrap();
            guard = b.op("or", vec![guard, t], loc()).unwrap();
        }
        if name == "nested" {
            guard = b.op("not", vec![guard], loc()).unwrap();
        }
        let gone = b
            .lit(WType::Enum("ParentPhase".into()), json!("Gone"), loc())
            .unwrap();
        b.add_action(
            name,
            params,
            vec![(guard, loc())],
            vec![("p".into(), "phase".into(), gone, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    for (name, parent, role, wrong_owner, computed) in [
        ("activate", "p", Role::State, false, false),
        ("wrong_binding", "p", Role::State, true, false),
        ("context_parent", "p", Role::Context, false, false),
        ("input_parent", "p", Role::Input, false, false),
        ("computed", "p", Role::State, false, true),
    ] {
        let params = vec![
            param("c", "Child", Role::State),
            param(parent, "Parent", role),
            param("q", "Parent", Role::State),
        ];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let owner = b.field("c", "owner", loc()).unwrap();
        let id = b
            .field(if wrong_owner { "q" } else { parent }, "id", loc())
            .unwrap();
        let owner_match = b.op("eq", vec![owner, id], loc()).unwrap();
        let permitted = member(&mut b, parent, "phase", &["Abandoned", "Gone"], true);
        let mut active = b
            .lit(WType::Enum("ChildPhase".into()), json!("Active"), loc())
            .unwrap();
        if computed {
            let some = b.op("some", vec![active.clone()], loc()).unwrap();
            active = b.op("value_or", vec![some, active], loc()).unwrap();
        }
        b.add_action(
            name,
            params,
            vec![(owner_match, loc()), (permitted, loc())],
            vec![("c".into(), "phase".into(), active, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    for (name, scalar) in [("create_child", false), ("create_scalar", true)] {
        let params = vec![
            param("p", "Parent", Role::State),
            WParam {
                name: "new_id".into(),
                ty: WType::Id("Child".into()),
                role: Some(Role::Input),
            },
            WParam {
                name: "owner_id".into(),
                ty: WType::Id("Parent".into()),
                role: Some(Role::Input),
            },
        ];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let permitted = member(&mut b, "p", "phase", &["Abandoned", "Gone"], true);
        let id = b.param("new_id", loc()).unwrap().into_wire();
        let owner = if scalar {
            b.param("owner_id", loc()).unwrap()
        } else {
            b.field("p", "id", loc()).unwrap()
        }
        .into_wire();
        let phase = b
            .lit(WType::Enum("ChildPhase".into()), json!("Active"), loc())
            .unwrap()
            .into_wire();
        b.add_action(
            name,
            params,
            vec![(permitted, loc())],
            vec![],
            vec![WLifecycle::Create {
                entity: "Child".into(),
                id,
                fields: vec![
                    ("owner".into(), owner),
                    ("phase".into(), phase.clone()),
                    ("alternate".into(), phase),
                ],
                loc: loc(),
            }],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    // Both empty-membership forms are explicit literals, not an empty `in`.
    let params = vec![param("p", "Parent", Role::State)];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let yes = member(&mut b, "p", "phase", &[], true);
    let no = member(&mut b, "p", "phase", &[], false);
    let note = b.field("p", "note", loc()).unwrap();
    b.add_action(
        "empty_membership",
        params,
        vec![(yes, loc()), (no, loc())],
        vec![("p".into(), "note".into(), note, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    b.finish(None).unwrap()
}
fn no_children_probe() -> E {
    E::NoNonterminalChildren {
        parent: "p".into(),
        child: "Child".into(),
        owner_field: "owner".into(),
        child_state: "phase".into(),
        terminal: vec!["Terminal".into()],
    }
}
fn owner_probe() -> E {
    E::OwnerEquality {
        child: "c".into(),
        owner_field: "owner".into(),
        parent: "p".into(),
    }
}
fn permitted_probe() -> E {
    E::Membership {
        binding: "p".into(),
        field: "phase".into(),
        values: vec!["Gone".into(), "Abandoned".into()],
        negated: true,
    }
}

#[test]
fn reconstructed_declarations_preserve_enum_nominal_entity_and_ref_identities() {
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    let scratch = probes.declarations();
    for ((kind, name), hash) in m.name_table() {
        if ["entity", "enum", "nominal"].contains(&kind.as_str()) {
            assert_eq!(
                scratch.name_table().get(&(*kind, name.clone())),
                Some(hash),
                "{name}"
            );
        }
    }
    assert!(scratch.entity("Child").unwrap().is_reference("owner"));
}

#[test]
fn whole_query_root_matches_but_wrong_sources_fields_and_weakened_guards_do_not() {
    let m = fixture();
    let original = m.behavior_version();
    let probes = CanonicalProbes::new(&m).unwrap();
    let expected = probes
        .expression(&[("p".into(), "Parent".into())], &no_children_probe())
        .unwrap();
    assert!(probes.mandatory_root("stop", &expected).unwrap());
    assert!(probes.mandatory_root("alpha_stop", &expected).unwrap());
    for action in ["wrong_query", "wrong_field", "weakened", "nested"] {
        assert!(
            !probes.mandatory_root(action, &expected).unwrap(),
            "{action}"
        );
    }
    assert_eq!(original, m.behavior_version());
}

#[test]
fn guard_bindings_and_actual_state_roles_are_both_required() {
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    let owner = probes.expression(&inputs(), &owner_probe()).unwrap();
    let permitted = probes.expression(&inputs(), &permitted_probe()).unwrap();
    assert!(probes.mandatory_root("activate", &owner).unwrap());
    assert!(probes.mandatory_root("activate", &permitted).unwrap());
    assert!(!probes.mandatory_root("wrong_binding", &owner).unwrap());
    assert!(probes.is_state_binding("activate", "p", "Parent").unwrap());
    for name in ["context_parent", "input_parent"] {
        // A hash match alone is insufficient: these values came from the caller.
        assert!(probes.mandatory_root(name, &owner).unwrap());
        assert!(!probes.is_state_binding(name, "p", "Parent").unwrap());
    }
    assert!(!probes.is_state_binding("activate", "p", "Child").unwrap());
}

#[test]
fn literal_rhs_and_bound_owner_initializer_are_exact_core_expressions() {
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    let literal = probes
        .expression(
            &[],
            &E::EnumLiteral {
                entity: "Child".into(),
                field: "phase".into(),
                member: "Active".into(),
            },
        )
        .unwrap();
    assert_eq!(
        hash_display(m.action("activate").unwrap().effects()[0].value().hash()),
        literal
    );
    assert_ne!(
        hash_display(m.action("computed").unwrap().effects()[0].value().hash()),
        literal
    );
    let other = probes
        .expression(
            &[],
            &E::EnumLiteral {
                entity: "Child".into(),
                field: "phase".into(),
                member: "Terminal".into(),
            },
        )
        .unwrap();
    assert_ne!(literal, other);
    assert!(
        probes
            .expression(
                &[],
                &E::EnumLiteral {
                    entity: "Child".into(),
                    field: "phase".into(),
                    member: "Missing".into()
                }
            )
            .is_err()
    );
    let gone = probes
        .expression(
            &[],
            &E::EnumLiteral {
                entity: "Parent".into(),
                field: "phase".into(),
                member: "Gone".into(),
            },
        )
        .unwrap();
    assert_eq!(
        hash_display(m.action("stop").unwrap().effects()[0].value().hash()),
        gone
    );
    let owner = probes
        .expression(
            &[("p".into(), "Parent".into())],
            &E::BoundId {
                binding: "p".into(),
            },
        )
        .unwrap();
    for (action, match_expected) in [("create_child", true), ("create_scalar", false)] {
        let value = &m.action(action).unwrap().creates()[0]
            .fields()
            .iter()
            .find(|(f, _)| f == "owner")
            .unwrap()
            .1;
        assert_eq!(hash_display(value.hash()) == owner, match_expected);
    }
    // Scalar owner_id may equal p.id at runtime; that cannot turn its Param
    // expression into the required bound State field expression.
}

#[test]
fn record_reviewable_public_core_preflight_identities() {
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    let declarations = m
        .name_table()
        .iter()
        .filter(|((k, _), _)| ["entity", "enum", "nominal"].contains(&k.as_str()))
        .map(|((k, n), h)| json!({"kind":k.as_str(),"name":n,"hash":hash_display(h)}))
        .collect::<Vec<_>>();
    let no_children = probes
        .expression(&[("p".into(), "Parent".into())], &no_children_probe())
        .unwrap();
    let owner = probes.expression(&inputs(), &owner_probe()).unwrap();
    let permitted = probes.expression(&inputs(), &permitted_probe()).unwrap();
    assert!(probes.mandatory_root("stop", &no_children).unwrap());
    assert!(probes.mandatory_root("activate", &owner).unwrap());
    assert!(probes.mandatory_root("activate", &permitted).unwrap());
    let report = json!({"format":"behavior.safety_public_core_preflight.v1","identity_probe_checks":"Passed",
        "evidence_class":"Public-facade correspondence tests; not a proof of Rust extraction",
        "core_release":{"version":behavior_models::CORE_RELEASE.version,"commit":behavior_models::CORE_RELEASE.commit},
        "behavior_hash":m.behavior_version(),"declarations":declarations,
        "probes":{"no_nonterminal_children":no_children,"owner_equality":owner,"permitted_parent":permitted},
        "public_operations":["Module::semantic_profile","Module::enums/nominals/entities/name_table/actions",
        "EntityItem::fields/field_type/is_reference","NominalInfo::underlying.to_type and declaration hash",
        "Builder::declare_enum/declare_nominal/declare_entity","Builder::push_scope/field/lit/in_/op/select/push_lambda/lambda",
        "Builder::add_derived/finish","DerivedItem::body().hash()","ActionItem::preconditions/effects/creates/params",
        "Param::ty().to_wire_type and role equality against public State anchor"],
        "negative_controls":["wrong source/field/binding/type/member","OR weakening and nested root",
        "Context/Input Entity despite matching guard hash","computed lifecycle RHS","plain Id versus Ref declaration",
        "scalar Input Id equal to parent id under actual Core invocation","explicit empty membership"],
        "trust_boundary":"Canonical probe construction and semantic hash correspondence remain explicit trusted Rust/Core assumptions"});
    if let Some(path) = std::env::var_os("BEHAVIOR_SAFETY_PREFLIGHT_REPORT") {
        std::fs::write(
            path,
            behavior_engine::canonical::to_canonical_string(&report).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn ref_marker_changes_the_declaration_contract_and_is_not_inferred_from_id_type() {
    let reference = fixture();
    let plain = fixture_with_reference(false);
    let r = CanonicalProbes::new(&reference).unwrap();
    let p = CanonicalProbes::new(&plain).unwrap();
    assert!(
        r.declarations()
            .entity("Child")
            .unwrap()
            .is_reference("owner")
    );
    assert!(
        !p.declarations()
            .entity("Child")
            .unwrap()
            .is_reference("owner")
    );
    let hash = |m: &Module| {
        hash_display(
            m.name_table()
                .iter()
                .find(|((kind, name), _)| kind.as_str() == "entity" && name == "Child")
                .unwrap()
                .1,
        )
    };
    assert_ne!(hash(r.declarations()), hash(p.declarations()));
}

#[test]
fn scalar_owner_can_overlap_actual_parent_identity_without_meeting_the_bound_id_pattern() {
    use behavior_engine::store::{InMemoryBackend, Store, documents::SeedEntity};
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    let policy=behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#).unwrap();
    let genesis = behavior_engine::store::store::genesis_v2_for(
        &m,
        policy,
        vec![SeedEntity {
            entity: "Parent".into(),
            value: json!({"id":"p1","phase":"Open","note":""}),
        }],
    )
    .unwrap();
    let store = Store::create(InMemoryBackend::default(), &m, genesis).unwrap();
    let (request, errors) = behavior_engine::invocation::RequestedInvocation::decode(
        &json!({
        "format":"behavior.invocation.v1","capability":"create_scalar",
        "bindings":{"p":{"entity":"Parent","id":"p1"}},
        "input":{"new_id":"c1","owner_id":"p1"},"context":{}})
        .to_string(),
    )
    .unwrap();
    let result = store
        .invoke(
            &m,
            &request.unwrap_or_else(|| panic!("{errors:?}")),
            "2026-10-18T00:00:00Z",
            None,
            None,
        )
        .unwrap();
    assert!(result.bundle.is_some());
    let expected = probes
        .expression(
            &[("p".into(), "Parent".into())],
            &E::BoundId {
                binding: "p".into(),
            },
        )
        .unwrap();
    let initializer = &m.action("create_scalar").unwrap().creates()[0]
        .fields()
        .iter()
        .find(|(f, _)| f == "owner")
        .unwrap()
        .1;
    assert_ne!(hash_display(initializer.hash()), expected);
}

#[test]
fn explicit_empty_membership_and_capture_names_are_checked() {
    let m = fixture();
    let probes = CanonicalProbes::new(&m).unwrap();
    for negated in [true, false] {
        let expected = probes
            .expression(
                &[("p".into(), "Parent".into())],
                &E::Membership {
                    binding: "p".into(),
                    field: "phase".into(),
                    values: vec![],
                    negated,
                },
            )
            .unwrap();
        assert!(
            probes
                .mandatory_root("empty_membership", &expected)
                .unwrap()
        );
    }
    let mut wrong = permitted_probe();
    if let E::Membership { binding, .. } = &mut wrong {
        *binding = "q".into();
    }
    let expected = probes
        .expression(&[("q".into(), "Parent".into())], &wrong)
        .unwrap();
    assert!(!probes.mandatory_root("activate", &expected).unwrap());
}

// Release-readiness regression: independently authored vectors retain order/multiplicity.
#[test]
fn ordered_membership_probes_preserve_actual_roots_and_duplicate_literals() {
    use behavior_engine::wire::DerivedKind;
    let mut records = vec![];
    for nested in [false, true] {
        for values in [
            vec!["Abandoned", "Gone"],
            vec!["Gone", "Abandoned"],
            vec!["Gone", "Abandoned", "Gone"],
        ] {
            let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
            b.declare_enum(
                "Phase",
                vec!["Open".into(), "Gone".into(), "Abandoned".into()],
                loc(),
            )
            .unwrap();
            b.declare_entity(
                "Parent",
                vec![field("phase", WType::Enum("Phase".into()))],
                loc(),
            )
            .unwrap();
            b.declare_entity(
                "Child",
                vec![
                    field("phase", WType::Enum("Phase".into())),
                    field("owner", WType::Ref("Parent".into())),
                ],
                loc(),
            )
            .unwrap();
            let params = vec![WParam {
                name: "p".into(),
                ty: WType::Entity("Parent".into()),
                role: None,
            }];
            b.push_scope(ScopeSite::Derived, params.clone(), loc())
                .unwrap();
            let body = if nested {
                let query = b.select("Child", loc()).unwrap();
                b.push_lambda("authored_child", "Child").unwrap();
                let owner = b.field("authored_child", "owner", loc()).unwrap();
                let id = b.field("p", "id", loc()).unwrap();
                let belongs = b.op("eq", vec![owner, id], loc()).unwrap();
                let nonterminal = member(&mut b, "authored_child", "phase", &values, true);
                let predicate = b.op("and", vec![belongs, nonterminal], loc()).unwrap();
                b.pop_scope();
                let any = b
                    .lambda("any", query, "authored_child", predicate, loc())
                    .unwrap();
                b.op("not", vec![any], loc()).unwrap()
            } else {
                member(&mut b, "p", "phase", &values, true)
            };
            b.add_derived("actual", DerivedKind::Derived, params, body, None, loc())
                .unwrap();
            b.pop_scope();
            let m = b.finish(None).unwrap();
            let expression = if nested {
                E::NoNonterminalChildren {
                    parent: "p".into(),
                    child: "Child".into(),
                    owner_field: "owner".into(),
                    child_state: "phase".into(),
                    terminal: values.iter().map(|v| (*v).into()).collect(),
                }
            } else {
                E::Membership {
                    binding: "p".into(),
                    field: "phase".into(),
                    values: values.iter().map(|v| (*v).into()).collect(),
                    negated: true,
                }
            };
            let probe = CanonicalProbes::new(&m)
                .unwrap()
                .expression_ordered(&[("p".into(), "Parent".into())], &expression)
                .unwrap();
            assert_eq!(
                probe,
                hash_display(m.derived("actual").unwrap().body().hash()),
                "nested={nested}, order={values:?}"
            );
            records.push(json!({"nested":nested,"ordered":values,"actual_root":probe,"behavior_hash":m.behavior_version()}));
        }
    }
    if let Some(path) = std::env::var_os("BEHAVIOR_SAFETY_RELEASE_PREFLIGHT_REPORT") {
        std::fs::write(
            path,
            behavior_engine::canonical::to_canonical_string(
                &json!({"cases":records,"passed":true}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn independent_proposals_are_data_and_have_strict_complete_tree_transport() {
    use behavior_models::safety::evidence::UnverifiedSafetyBundle;
    // Parsing grants no certificate; correspondence/proof checking is separate.
    let base = json!({"format":"behavior.safety_bundle.v2","module":"{}","inventory":{},"policies":[],
        "evidence":{"format":"behavior.safety_evidence.v2","obligations":{},"proof":{},"behavior_hash":"x","policy_set_hash":"p","evidence_hash":"e"},
        "witness":{"format":"behavior.safety_merge_trace.v1","nodes":[{"kind":"leaf"},{"kind":"leaf"},{"kind":"branch","left":0,"right":1,"choices":[false,true]}],"root":2}});
    assert!(
        !UnverifiedSafetyBundle::from_json(&base)
            .unwrap()
            .is_verified()
    );
    for mutation in [
        "extra_code",
        "boolean_index",
        "forward_edge",
        "orphan",
        "shared_node",
        "nonboolean_choice",
    ] {
        let mut value = base.clone();
        match mutation {
            "extra_code" => value["witness"]["lean_source"] = json!("untrusted"),
            "boolean_index" => value["witness"]["root"] = json!(false),
            "forward_edge" => value["witness"]["nodes"][2]["left"] = json!(2),
            "orphan" => value["witness"]["root"] = json!(1),
            "shared_node" => value["witness"]["nodes"][2]["right"] = json!(0),
            "nonboolean_choice" => value["witness"]["nodes"][2]["choices"][0] = json!(0),
            _ => unreachable!(),
        }
        assert!(
            UnverifiedSafetyBundle::from_json(&value).is_err(),
            "{mutation}"
        );
    }
}
