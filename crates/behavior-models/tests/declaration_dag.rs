//! The declaration DAG across kinds (feature 503, US4: T038, T040 and T061): approvals over
//! flow entities and over each other's decision entities, cycles, and declaration and
//! capability collisions.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::Builder;
use behavior_engine::wire::{WField, WType};
use behavior_models::{
    AnyModel, ApprovalInput, BuildOutput, ModelError, Staged, compose_models, declare_models,
};
use serde_json::json;

use common::{ap, codes, loc, rf};

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

/// P1 for every case here: the RequestFlow fixture's declarations plus `Document { title }`.
fn authored() -> Builder {
    let mut b = rf::authored();
    b.declare_entity("Document", vec![field("title", WType::String)], loc())
        .unwrap();
    b
}

/// An approval `id` over `subject`, actor `User`, thresholds (1, 1).
fn approval(id: &str, subject: &str) -> ApprovalInput {
    let mut input = ap::approval(1, Some(1));
    input.approval_id = id.into();
    input.subject_entity = subject.into();
    input
}

fn spec(input: ApprovalInput) -> AnyModel<'static> {
    ap::spec(input, ap::source())
}

fn flow() -> AnyModel<'static> {
    rf::spec(rf::flow(), rf::source())
}

/// P2 only: what was declared, or the errors.
fn stage(models: &[AnyModel<'static>]) -> Result<(Staged, Builder), Vec<ModelError>> {
    let mut b = authored();
    let staged = declare_models(&mut b, models)?;
    Ok((staged, b))
}

/// The full protocol (P1 → P2 → P2b commands → P4); the flow's `Notify` command only with a
/// flow, since it names the flow's entity.
fn build(models: Vec<AnyModel<'static>>) -> Result<BuildOutput, Vec<ModelError>> {
    let mut b = authored();
    declare_models(&mut b, &models)?;
    if models.iter().any(|m| matches!(m, AnyModel::RequestFlow(_))) {
        rf::commands(&mut b);
    }
    compose_models(&mut b, models, None)
}

fn names(staged: &Staged) -> (Vec<String>, Vec<String>) {
    (
        staged.enums.iter().map(|e| e.name.clone()).collect(),
        staged.schemas.iter().map(|s| s.entity.clone()).collect(),
    )
}

fn position(list: &[String], name: &str) -> usize {
    list.iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("{name} not in {list:?}"))
}

/// The module's identity: equal behavior versions are the same admitted module.
fn identity(out: &BuildOutput) -> String {
    out.module.behavior_version()
}

// --- T038: edges, order, cycles ----------------------------------------------------------------

/// An approval over the flow's request entity depends on the flow: the flow's entity is declared
/// first, and both supply orders give the same module.
#[test]
fn approval_over_a_flow_entity_is_declared_after_the_flow() {
    let models = || {
        (
            flow(),
            spec(approval("purchase_request_approval", "PurchaseRequest")),
        )
    };
    let (f, a) = models();
    let (staged, _) = stage(&[f, a]).unwrap_or_else(|e| panic!("{e:#?}"));
    let (_, schemas) = names(&staged);
    assert!(
        position(&schemas, "PurchaseRequest")
            < position(&schemas, "PurchaseRequestApprovalDecision"),
        "{schemas:?}"
    );

    let (f, a) = models();
    let forward = build(vec![f, a]).unwrap_or_else(|e| panic!("{e:#?}"));
    let (f, a) = models();
    let backward = build(vec![a, f]).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(identity(&forward), identity(&backward));
}

/// Two approvals whose subjects are each other's decision entities form a cycle, in every
/// supply order, and nothing is declared.
#[test]
fn mutually_dependent_approvals_are_a_declaration_cycle() {
    let a = || spec(approval("a", "BDecision"));
    let b = || spec(approval("b", "ADecision"));
    for models in [vec![a(), b()], vec![b(), a()]] {
        let mut builder = authored();
        let errors = declare_models(&mut builder, &models).unwrap_err();
        assert_eq!(codes(&errors), vec!["DECLARATION_CYCLE"]);
        assert!(
            matches!(&errors[0], ModelError::DeclarationCycle { models } if models == &["a", "b"]),
            "{errors:#?}"
        );
        let module = builder.finish(None).unwrap();
        for generated in ["ADecision", "BDecision"] {
            assert!(
                !module.entities().contains_key(generated),
                "{generated} declared"
            );
        }
        for generated in ["ADecisionChoice", "BDecisionChoice"] {
            assert!(
                !module.enums().contains_key(generated),
                "{generated} declared"
            );
        }
    }
}

/// A review of a decision (spec Edge Cases): approval `decision_review` over `doc_approval`'s
/// decision entity is acyclic. It builds in both orders to the same module, `doc_approval`'s
/// declarations come first, and a review decision commits on an existing decision.
#[test]
fn review_of_a_decision_is_acyclic_and_commits() {
    let models = || {
        (
            spec(approval("doc_approval", "Document")),
            spec(approval("decision_review", "DocApprovalDecision")),
        )
    };
    let (a, r) = models();
    let (staged, _) = stage(&[r, a]).unwrap_or_else(|e| panic!("{e:#?}"));
    let (enums, schemas) = names(&staged);
    assert!(
        position(&enums, "DocApprovalDecisionChoice")
            < position(&enums, "DecisionReviewDecisionChoice")
    );
    assert!(
        position(&schemas, "DocApprovalDecision") < position(&schemas, "DecisionReviewDecision")
    );

    let (a, r) = models();
    let forward = build(vec![a, r]).unwrap_or_else(|e| panic!("{e:#?}"));
    let (a, r) = models();
    let backward = build(vec![r, a]).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(identity(&forward), identity(&backward));

    // A review decision on an existing `doc_approval` decision commits.
    let module = &forward.module;
    let mut store = ap::store(module, &["d1"], &[("x1", "d1", "u1", "APPROVE")]);
    let document = json!({
        "format": "behavior.invocation.v1",
        "capability": "decision_review__approve",
        "bindings": {"subject": {"entity": "DocApprovalDecision", "id": "x1"}},
        "input": {"decision_id": "r1"},
        "context": {"actor": "u2"},
    });
    let (requested, errors) =
        behavior_engine::invocation::RequestedInvocation::decode(&document.to_string()).unwrap();
    let requested = requested.unwrap_or_else(|| panic!("{errors:?}"));
    let inv = store.invoke(module, &requested, ap::T, None, None).unwrap();
    let bundle = inv
        .bundle
        .unwrap_or_else(|| panic!("review denied: {}", inv.record.as_json()));
    let head = store.current().unwrap();
    store.commit(module, &head, &bundle).unwrap();
}

/// Approvals over authored entities have no edges: they are declared in model-id order,
/// whatever the supply order.
#[test]
fn approvals_over_authored_entities_follow_model_id_order() {
    let models = || {
        (
            spec(approval("b_approval", "Document")),
            spec(approval("a_approval", "Document")),
        )
    };
    let expected = vec![
        "AApprovalDecision".to_string(),
        "BApprovalDecision".to_string(),
    ];
    let (b, a) = models();
    let (staged, _) = stage(&[b, a]).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(names(&staged).1, expected);
    let (b, a) = models();
    let (staged, _) = stage(&[a, b]).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(names(&staged).1, expected);
}

// --- T040: collisions ---------------------------------------------------------------------------

/// A flow `x_decision` and an approval `x` both produce `XDecision`: the common merge reports
/// the clash, and nothing is renamed.
#[test]
fn a_flow_entity_and_a_decision_entity_with_one_name_collide() {
    let mut input = rf::flow();
    input.flow_id = "x_decision".into();
    let models = vec![
        rf::spec(input, rf::source()),
        spec(approval("x", "Document")),
    ];
    let errors = stage(&models).map(|_| ()).unwrap_err();
    assert_eq!(codes(&errors), vec!["DECLARATION_COLLISION"]);
    assert!(
        matches!(&errors[0], ModelError::DeclarationCollision { name, .. } if name == "XDecision"),
        "{errors:#?}"
    );
}

/// An authored entity with the decision entity's name collides.
#[test]
fn an_authored_entity_with_the_decision_entity_name_collides() {
    let mut b = authored();
    b.declare_entity(
        "PurchaseRequestApprovalDecision",
        vec![field("note", WType::String)],
        loc(),
    )
    .unwrap();
    let errors = declare_models(
        &mut b,
        &[spec(approval("purchase_request_approval", "Document"))],
    )
    .unwrap_err();
    assert_eq!(codes(&errors), vec!["DECLARATION_COLLISION"]);
}

/// A hand-written derived value with a generated definition's name collides.
#[test]
fn a_hand_written_derived_value_with_a_definition_name_collides() {
    let mut b = authored();
    b.push_scope(behavior_engine::builder::ScopeSite::Derived, vec![], loc())
        .unwrap();
    let t = b.lit(WType::Bool, json!(true), loc()).unwrap();
    b.pop_scope();
    b.add_derived(
        "doc_approval__approved",
        behavior_engine::wire::DerivedKind::Rule,
        vec![],
        t,
        None,
        loc(),
    )
    .unwrap();
    let errors = declare_models(&mut b, &[spec(approval("doc_approval", "Document"))]).unwrap_err();
    assert_eq!(codes(&errors), vec!["GENERATED_NAME_COLLISION"]);
}

/// A hand-written action with a generated capability's name (`doc_approval__approve`) collides:
/// the common composition reports it, and nothing is renamed (US4/AC7).
#[test]
fn a_hand_written_action_with_a_capability_name_collides() {
    for capability in ["doc_approval__approve", "doc_approval__reject"] {
        let models = vec![spec(approval("doc_approval", "Document"))];
        let mut b = authored();
        declare_models(&mut b, &models).unwrap_or_else(|e| panic!("{e:#?}"));
        let ps = vec![behavior_engine::wire::WParam {
            name: "doc".into(),
            role: Some(behavior_engine::wire::Role::State),
            ty: WType::Entity("Document".into()),
        }];
        b.push_scope(
            behavior_engine::builder::ScopeSite::Action,
            ps.clone(),
            loc(),
        )
        .unwrap();
        let title = b.lit(WType::String, json!("forged"), loc()).unwrap();
        b.add_action(
            capability,
            ps,
            vec![],
            vec![("doc".into(), "title".into(), title, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
        let errors = compose_models(&mut b, models, None)
            .map(|_| ())
            .unwrap_err();
        assert_eq!(
            codes(&errors),
            vec!["GENERATED_NAME_COLLISION"],
            "{errors:#?}"
        );
        assert!(
            matches!(&errors[0], ModelError::GeneratedNameCollision { name, .. } if name == capability),
            "{errors:#?}"
        );
    }
}

/// Model-generated capabilities cannot collide across kinds: model ids share one namespace and
/// `encode` is injective, so a StateMachine with the approval's id is a duplicate model id
/// before any capability is generated.
#[test]
fn a_state_machine_with_the_approval_id_is_a_duplicate_model_id() {
    let machine = || {
        AnyModel::StateMachine(behavior_models::ModelSpec {
            input: behavior_models::ModelInput {
                model_id: "doc_approval".into(),
                subject_entity: "PurchaseRequest".into(),
                state_field: "status".into(),
                subject_param: None,
                entry_states: vec!["Draft".into()],
                transitions: vec![common::transition("approve", &["Draft"], "Submitted")],
                display: behavior_models::DisplayMeta::default(),
                source: None,
            },
            fragments: Box::new(behavior_models::NoFragments),
        })
    };
    let approval = || spec(approval("doc_approval", "Document"));
    for models in [
        vec![flow(), machine(), approval()],
        vec![approval(), machine(), flow()],
    ] {
        let errors = stage(&models).map(|_| ()).unwrap_err();
        assert_eq!(codes(&errors), vec!["DUPLICATE_MODEL_ID"], "{errors:#?}");
    }
}

/// An approval and a flow with the same id: one id namespace.
#[test]
fn an_approval_and_a_flow_with_one_id_are_a_duplicate_model_id() {
    let errors = stage(&[flow(), spec(approval("purchase_request", "Document"))])
        .map(|_| ())
        .unwrap_err();
    assert_eq!(codes(&errors), vec!["DUPLICATE_MODEL_ID"]);
}

// --- T042: authored forward references (law D5) -------------------------------------------------

/// An authored entity may name the decision entity and the choice enum before P2 declares them:
/// the contributions are declared in topological order, then everything is admitted together.
#[test]
fn authored_declarations_may_name_the_decision_entity_and_choice_enum() {
    let mut b = authored();
    b.declare_entity(
        "Audit",
        vec![
            field("decision", WType::Id("DocApprovalDecision".into())),
            field("choice", WType::Enum("DocApprovalDecisionChoice".into())),
        ],
        loc(),
    )
    .unwrap();
    let models = vec![spec(approval("doc_approval", "Document"))];
    let staged = declare_models(&mut b, &models).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(names(&staged).1, vec!["DocApprovalDecision".to_string()]);
    let out = compose_models(&mut b, models, None).unwrap_or_else(|e| panic!("{e:#?}"));
    let audit = &out.module.entities()["Audit"];
    assert!(audit.field_type("decision").is_some() && audit.field_type("choice").is_some());
    assert!(out.module.entities().contains_key("DocApprovalDecision"));
}
