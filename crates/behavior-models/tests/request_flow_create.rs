//! Creation lowering, the initializer rules and the equivalence pairs (data model §5; T014,
//! T015, T026).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::{Role, WField, WLifecycle, WParam, WType};
use behavior_models::{CreationFragments, CreationScope, ModelInput, ModelSpec, compose_models};
use common::rf::*;
use common::{codes, item_names, loc};
use serde_json::json;

#[test]
fn creation_action_layout() {
    let out = build_reference();
    let a = &out.module.actions()["purchase_request__create"];
    let params: Vec<(String, String)> = a
        .params()
        .iter()
        .map(|p| (p.name().to_string(), format!("{:?}", p.role())))
        .collect();
    assert_eq!(
        params,
        vec![
            ("request_id".to_string(), "Input".to_string()),
            ("amount".to_string(), "Input".to_string()),
            ("title".to_string(), "Input".to_string()),
            ("actor".to_string(), "Context".to_string()),
        ]
    );
    assert_eq!(
        a.creates().len(),
        1,
        "exactly one creation per creation action"
    );
    let created = &a.creates()[0];
    assert_eq!(created.entity(), "PurchaseRequest");
    let fields: Vec<&str> = created.fields().iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        fields,
        vec!["amount", "assignee", "requester", "status", "title"]
    );
    assert!(
        a.preconditions().is_empty() && a.effects().is_empty() && a.postconditions().is_empty()
    );
}

#[test]
fn the_module_holds_only_ordinary_items() {
    let out = build_reference();
    let mut generated: Vec<String> = item_names(&out.module)
        .into_iter()
        .filter(|n| n.starts_with("purchase_request__") || n == "PurchaseRequest")
        .collect();
    generated.sort();
    assert_eq!(
        generated,
        vec![
            "PurchaseRequest",
            "purchase_request__approve",
            "purchase_request__cancel",
            "purchase_request__create",
            "purchase_request__import_submitted",
            "purchase_request__reject",
            "purchase_request__return_for_changes",
            "purchase_request__start_review",
            "purchase_request__submit",
        ]
    );
}

fn machine_over_entity() -> ModelInput {
    let f = flow();
    ModelInput {
        model_id: "purchase_request".into(),
        subject_entity: "PurchaseRequest".into(),
        state_field: "status".into(),
        subject_param: Some("purchase_request".into()),
        entry_states: vec!["Draft".into(), "Submitted".into()],
        transitions: f.transitions,
        display: Default::default(),
        source: None,
    }
}

fn machine_spec() -> ModelSpec<'static> {
    let s = source();
    ModelSpec {
        input: machine_over_entity(),
        fragments: Box::new(s.transitions),
    }
}

/// The equivalent hand-written module: authored entity, hand-written creations, standalone
/// StateMachine.
fn hand_written() -> behavior_models::BuildOutput {
    let mut b = authored();
    let fields = [
        ("amount", WType::Int),
        (
            "assignee",
            WType::Option(Box::new(WType::Id("User".into()))),
        ),
        ("requester", WType::Id("User".into())),
        ("status", WType::Enum("PurchaseRequestStatus".into())),
        ("title", WType::String),
    ];
    b.declare_entity(
        "PurchaseRequest",
        fields
            .iter()
            .map(|(n, t)| WField {
                name: n.to_string(),
                ty: t.clone(),
                loc: loc(),
            })
            .collect(),
        loc(),
    )
    .unwrap();
    commands(&mut b);
    for (name, state) in [
        ("purchase_request__create", "Draft"),
        ("purchase_request__import_submitted", "Submitted"),
    ] {
        let ps = vec![
            WParam {
                name: "request_id".into(),
                role: Some(Role::Input),
                ty: WType::Id("PurchaseRequest".into()),
            },
            WParam {
                name: "amount".into(),
                role: Some(Role::Input),
                ty: WType::Int,
            },
            WParam {
                name: "title".into(),
                role: Some(Role::Input),
                ty: WType::String,
            },
            WParam {
                name: "actor".into(),
                role: Some(Role::Context),
                ty: WType::Id("User".into()),
            },
        ];
        b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
        let id = b.param("request_id", loc()).unwrap();
        let v = |b: &mut Builder, n: &str| b.param(n, loc()).unwrap().into_wire();
        let amount = v(&mut b, "amount");
        let actor = v(&mut b, "actor");
        let title = v(&mut b, "title");
        let none = b
            .lit(
                WType::Option(Box::new(WType::Id("User".into()))),
                json!(null),
                loc(),
            )
            .unwrap()
            .into_wire();
        let status = b
            .lit(
                WType::Enum("PurchaseRequestStatus".into()),
                json!(state),
                loc(),
            )
            .unwrap()
            .into_wire();
        let create = WLifecycle::Create {
            entity: "PurchaseRequest".into(),
            id: id.into_wire(),
            fields: vec![
                ("amount".into(), amount),
                ("assignee".into(), none),
                ("requester".into(), actor),
                ("status".into(), status),
                ("title".into(), title),
            ],
            loc: loc(),
        };
        b.add_action(name, ps, vec![], vec![], vec![create], vec![], loc())
            .unwrap();
        b.pop_scope();
    }
    compose_models(&mut b, vec![machine_spec().into()], None).unwrap()
}

/// SC-006: the flow's transitions are exactly the standalone StateMachine's.
#[test]
fn transitions_equal_a_standalone_state_machine() {
    let flow_out = build_reference();
    let hand = hand_written();
    for t in flow().transitions {
        let name = format!("purchase_request__{}", t.transition_id);
        assert_eq!(
            hash_display(flow_out.module.actions()[&name].hash()),
            hash_display(hand.module.actions()[&name].hash()),
            "{name}"
        );
    }
}

/// The constitution's equivalence pair: the whole module equals the hand-written one.
#[test]
fn the_flow_module_equals_the_hand_written_module() {
    assert_eq!(
        build_reference().module.behavior_version(),
        hand_written().module.behavior_version()
    );
}

fn with_initializer(
    edit: impl Fn(&mut Vec<(String, behavior_engine::builder::Node)>, &mut Builder) + 'static,
) -> Vec<&'static str> {
    let mut s = source();
    s.creations.insert(
        "create".into(),
        Box::new(move |b: &mut Builder, sc: &CreationScope| {
            let mut f: CreationFragments = standard(b, sc)?;
            edit(&mut f.initializer, b);
            Ok(f)
        }),
    );
    codes(&build_with(|_| {}, vec![spec(flow(), s)]).unwrap_err())
}

#[test]
fn initializer_rules_are_static_model_errors() {
    assert_eq!(
        with_initializer(|i, _| i.retain(|(n, _)| n != "requester")),
        vec!["INITIALIZER_MISSING_FIELD"]
    );
    assert_eq!(
        with_initializer(|i, _| i.retain(|(n, _)| n != "title")),
        vec!["INITIALIZER_MISSING_FIELD"]
    );
    assert_eq!(
        with_initializer(|i, _| i.retain(|(n, _)| n != "assignee")),
        vec!["INITIALIZER_MISSING_FIELD"]
    );
    assert_eq!(
        with_initializer(|i, b| i.push((
            "colour".into(),
            b.lit(WType::String, json!("red"), loc()).unwrap()
        ))),
        vec!["INITIALIZER_UNKNOWN_FIELD"]
    );
    assert_eq!(
        with_initializer(|i, b| i.push((
            "status".into(),
            b.lit(
                WType::Enum("PurchaseRequestStatus".into()),
                json!("Approved"),
                loc()
            )
            .unwrap()
        ))),
        vec!["INITIALIZER_SETS_STATUS"]
    );
    assert_eq!(
        with_initializer(|i, b| i.push((
            "title".into(),
            b.lit(WType::String, json!("x"), loc()).unwrap()
        ))),
        vec!["INITIALIZER_DUPLICATE_FIELD"]
    );
}

#[test]
fn an_ill_typed_initializer_value_is_a_fragment_error() {
    let errs = with_initializer(|i, b| {
        i.retain(|(n, _)| n != "title");
        i.push(("title".into(), b.lit(WType::Int, json!(3), loc()).unwrap()));
    });
    assert_eq!(errs, vec!["FRAGMENT_INVALID"]);
}

#[test]
fn creations_can_emit_commands_about_the_new_request() {
    let mut s = source();
    s.creations.insert(
        "create".into(),
        Box::new(|b: &mut Builder, sc: &CreationScope| {
            let mut f = standard(b, sc)?;
            f.commands.push(emit_notify(b, "request_id"));
            Ok(f)
        }),
    );
    let out = build_with(|_| {}, vec![spec(flow(), s)]).unwrap();
    assert_eq!(
        out.module.actions()["purchase_request__create"]
            .command_emissions()
            .len(),
        1
    );
}

#[test]
fn initializer_errors_name_the_creation_and_field() {
    let mut s = source();
    s.creations.insert(
        "create".into(),
        Box::new(|b: &mut Builder, sc: &CreationScope| {
            let mut f = standard(b, sc)?;
            f.initializer.retain(|(n, _)| n != "amount");
            Ok(f)
        }),
    );
    let errs = build_with(|_| {}, vec![spec(flow(), s)]).unwrap_err();
    assert_eq!(
        errs,
        vec![behavior_models::ModelError::InitializerMissingField {
            model_ref: "purchase_request/creation/create".into(),
            field: "amount".into(),
        }]
    );
}
