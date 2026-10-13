//! RequestFlow in composition with other models and hand-written behavior (spec US5; T047, T048).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, SemanticProfile};
use behavior_engine::wire::{WField, WType};
use behavior_models::{
    AnyModel, BuildOutput, DisplayMeta, ModelError, ModelInput, compose_models, declare_models,
};
use common::rf::*;
use common::{codes, loc, spec_without_fragments, transition};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

fn machine_on(model_id: &str, entity: &str, field: &str) -> ModelInput {
    ModelInput {
        model_id: model_id.into(),
        subject_entity: entity.into(),
        state_field: field.into(),
        subject_param: None,
        entry_states: vec!["Open".into()],
        transitions: vec![transition("close", &["Open"], "Closed")],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// A standalone machine's subject: `Ticket { state: TicketState }`.
fn tickets(b: &mut Builder) {
    b.declare_enum("TicketState", vec!["Open".into(), "Closed".into()], loc())
        .unwrap();
    b.declare_entity(
        "Ticket",
        vec![WField {
            name: "state".into(),
            ty: WType::Enum("TicketState".into()),
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
}

#[test]
fn two_flows_share_local_ids_but_not_names() {
    let out = build_with(
        |_| {},
        vec![
            spec(flow(), source()),
            spec(input_for("leave_request"), source()),
        ],
    )
    .unwrap();
    for name in [
        "purchase_request__submit",
        "leave_request__submit",
        "leave_request__create",
    ] {
        assert!(out.module.actions().contains_key(name), "{name}");
    }
    assert!(out.module.entities().contains_key("LeaveRequest"));
    assert_eq!(out.receipts.len(), 2);
}

fn input_for(flow_id: &str) -> behavior_models::RequestFlowInput {
    let mut f = flow();
    f.flow_id = flow_id.into();
    f
}

#[test]
fn model_ids_are_one_namespace_across_kinds() {
    let duplicate = vec![ModelError::DuplicateModelId {
        model_id: "purchase_request".into(),
    }];
    // A standalone machine with the flow's id: reported in P2 already.
    let mut b = authored();
    tickets(&mut b);
    let models: Vec<AnyModel<'static>> = vec![
        spec(flow(), source()),
        spec_without_fragments(machine_on("purchase_request", "Ticket", "state")).into(),
    ];
    assert_eq!(declare_models(&mut b, &models).unwrap_err(), duplicate);
    // Composition reports it too when P2 is not given the whole set.
    let mut b = authored();
    tickets(&mut b);
    declare_models(&mut b, &[spec(flow(), source())]).unwrap();
    commands(&mut b);
    assert_eq!(compose_models(&mut b, models, None).unwrap_err(), duplicate);
    // Two flows with one id: the duplicate id, not the declaration clash it would cause.
    let errs =
        build_with(|_| {}, vec![spec(flow(), source()), spec(flow(), source())]).unwrap_err();
    assert_eq!(errs, duplicate);
}

#[test]
fn a_standalone_machine_cannot_claim_the_request_status() {
    let errs = build_with(
        |_| {},
        vec![
            spec(flow(), source()),
            spec_without_fragments(ModelInput {
                entry_states: vec!["Draft".into()],
                transitions: vec![transition("force", &["Draft"], "Approved")],
                ..machine_on("rogue", "PurchaseRequest", "status")
            })
            .into(),
        ],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["CONFLICTING_OWNERSHIP"]);
}

#[test]
fn a_hand_written_action_with_a_generated_name_collides() {
    let errs = build_with(
        |b| {
            let r = behavior_engine::wire::WParam {
                name: "r".into(),
                role: Some(behavior_engine::wire::Role::State),
                ty: WType::Entity("PurchaseRequest".into()),
            };
            b.push_scope(
                behavior_engine::builder::ScopeSite::Action,
                vec![r.clone()],
                loc(),
            )
            .unwrap();
            let v = b.lit(WType::String, serde_json::json!("x"), loc()).unwrap();
            b.add_action(
                "purchase_request__create",
                vec![r],
                vec![],
                vec![("r".into(), "title".into(), v, loc())],
                vec![],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(flow(), source())],
    )
    .unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::GeneratedNameCollision {
            model_ref: "purchase_request/creation/create".into(),
            name: "purchase_request__create".into(),
        }]
    );
}

// --- T048: supply order and declaration order never matter --------------------------------

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 1000,
        rng_seed: RngSeed::Fixed(502),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Builds the set `{purchase_request, leave_request, tickets}` (or, `broken`, with the second
/// flow re-using the first id) in the given supply and declaration orders.
fn compose_in(
    order: &[usize],
    decls: &[usize],
    broken: bool,
) -> Result<BuildOutput, Vec<ModelError>> {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    let mut steps: Vec<Step> = vec![
        Box::new(|b| {
            b.declare_enum(
                "PurchaseRequestStatus",
                STATUSES.iter().map(|s| s.to_string()).collect(),
                loc(),
            )
            .unwrap();
        }),
        Box::new(|b| {
            b.declare_entity(
                "User",
                vec![WField {
                    name: "name".into(),
                    ty: WType::String,
                    loc: loc(),
                }],
                loc(),
            )
            .unwrap();
        }),
        Box::new(|b| {
            b.declare_entity(
                "Budget",
                vec![WField {
                    name: "reserved".into(),
                    ty: WType::Int,
                    loc: loc(),
                }],
                loc(),
            )
            .unwrap();
        }),
        Box::new(tickets),
    ];
    let ordered: Vec<_> = decls
        .iter()
        .map(|&i| std::mem::replace(&mut steps[i], Box::new(|_| {})))
        .collect();
    for step in ordered {
        step(&mut b);
    }
    let second = if broken {
        "purchase_request"
    } else {
        "leave_request"
    };
    let mut all: Vec<Option<AnyModel<'static>>> = vec![
        Some(spec(flow(), source())),
        Some(spec(input_for(second), source())),
        Some(spec_without_fragments(machine_on("tickets", "Ticket", "state")).into()),
    ];
    let models: Vec<AnyModel<'static>> = order.iter().map(|&i| all[i].take().unwrap()).collect();
    declare_models(&mut b, &models)?;
    commands(&mut b);
    compose_models(&mut b, models, None)
}

type Step = Box<dyn Fn(&mut Builder)>;

fn shuffled(n: usize) -> impl Strategy<Value = Vec<usize>> {
    Just((0..n).collect::<Vec<_>>()).prop_shuffle()
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn composition_is_independent_of_order(
        order in shuffled(3),
        decls in shuffled(4),
        broken in any::<bool>(),
    ) {
        let reference = compose_in(&[0, 1, 2], &[0, 1, 2, 3], broken);
        let got = compose_in(&order, &decls, broken);
        match (reference, got) {
            (Ok(a), Ok(b)) => {
                prop_assert_eq!(a.module.behavior_version(), b.module.behavior_version());
                prop_assert_eq!(
                    a.receipts.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
                    b.receipts.iter().map(|r| r.0.clone()).collect::<Vec<_>>()
                );
            }
            (Err(a), Err(b)) => prop_assert_eq!(a, b),
            (a, b) => prop_assert!(false, "{:?} vs {:?}", a.is_ok(), b.is_ok()),
        }
    }
}
