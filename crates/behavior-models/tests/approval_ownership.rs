//! Approval ownership (feature 503, US2, data model §7): every semantic path to the decision
//! entity `X`, over every action of the final module, with no exemption by origin (T029), and
//! the completeness premise of the induction (T030).
//!
//! Terminology: DENIED = an attempt that was not permitted; REJECT = a recorded choice;
//! REJECTED = the derived outcome.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::wire::{Role, WLifecycle, WParam, WType};
use behavior_models::{AnyModel, ModelError, ModelInput, ModelSpec};
use common::ap::*;
use common::{codes, loc, transition};
use serde_json::json;

const X: &str = "DocApprovalDecision";
const C: &str = "DocApprovalDecisionChoice";
const APPROVE: &str = "doc_approval__approve";
const REJECT: &str = "doc_approval__reject";

fn input(name: &str, ty: WType) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::Input),
        ty,
    }
}

fn state(name: &str, entity: &str) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::State),
        ty: WType::Entity(entity.into()),
    }
}

/// `doc_approval` with the payload field `note: String`, initialized to `"ok"`.
fn with_note() -> AnyModel<'static> {
    let mut input = approval(1, Some(1));
    input.payload = vec![("note".into(), WType::String)];
    let src = Source(Box::new(|b, s| {
        let mut f = standard(b, s)?;
        f.payload = vec![(
            "note".into(),
            b.lit(WType::String, json!("ok"), loc()).unwrap(),
        )];
        Ok(f)
    }));
    spec(input, src)
}

/// The single error of a failed build, with its JSON form.
fn single(result: Result<behavior_models::BuildOutput, Vec<ModelError>>) -> serde_json::Value {
    let errs = result.expect_err("the build must fail");
    assert_eq!(errs.len(), 1, "{errs:#?}");
    errs[0].to_json()
}

fn errors(result: Result<behavior_models::BuildOutput, Vec<ModelError>>) -> Vec<ModelError> {
    result.expect_err("the build must fail")
}

// --- hand-written behavior over X -------------------------------------------------------------

/// `forge(id: Input Id<X>, a: Input Id<User>, s: Input Id<Document>)` creating an APPROVE
/// decision outside the approval.
fn forge(b: &mut Builder) {
    let ps = vec![
        input("id", WType::Id(X.into())),
        input("a", WType::Id("User".into())),
        input("s", WType::Id("Document".into())),
    ];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let id = b.param("id", loc()).unwrap();
    let a = b.param("a", loc()).unwrap();
    let s = b.param("s", loc()).unwrap();
    let choice = b
        .lit(WType::Enum(C.into()), json!("APPROVE"), loc())
        .unwrap();
    let note = b.lit(WType::String, json!("forged"), loc()).unwrap();
    let create = WLifecycle::Create {
        entity: X.into(),
        id: id.into_wire(),
        fields: vec![
            ("actor".into(), a.into_wire()),
            ("choice".into(), choice.into_wire()),
            ("note".into(), note.into_wire()),
            ("subject".into(), s.into_wire()),
        ],
        loc: loc(),
    };
    b.add_action("forge", ps, vec![], vec![], vec![create], vec![], loc())
        .unwrap();
    b.pop_scope();
}

/// `amend(d: State X, ...)` writing `d.<field>`.
fn amend(b: &mut Builder, field: &str) {
    let mut ps = vec![state("d", X)];
    let value = match field {
        "subject" => {
            ps.push(input("s", WType::Id("Document".into())));
            None
        }
        "actor" => {
            ps.push(input("a", WType::Id("User".into())));
            None
        }
        _ => Some(field),
    };
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let v = match value {
        None => b.param(&ps[1].name, loc()).unwrap(),
        Some("choice") => b
            .lit(WType::Enum(C.into()), json!("REJECT"), loc())
            .unwrap(),
        Some(_) => b.lit(WType::String, json!("edited"), loc()).unwrap(),
    };
    b.add_action(
        "amend",
        ps,
        vec![],
        vec![("d".into(), field.into(), v, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// `retract(d: State X)` removing the decision.
fn retract(b: &mut Builder) {
    let ps = vec![state("d", X)];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let remove = WLifecycle::Remove {
        param: "d".into(),
        loc: loc(),
    };
    b.add_action("retract", ps, vec![], vec![], vec![remove], vec![], loc())
        .unwrap();
    b.pop_scope();
}

#[test]
fn a_hand_written_creation_of_a_decision_is_refused() {
    let e = single(build_with(forge, vec![with_note()]));
    assert_eq!(e["code"], "UNAUTHORIZED_DECISION_CREATION");
    assert_eq!(e["action"], "forge");
    assert_eq!(e["entity"], X);
    assert_eq!(e["owner"], "doc_approval");
}

#[test]
fn a_hand_written_write_of_any_decision_field_is_refused() {
    for field in ["subject", "actor", "choice", "note"] {
        let e = single(build_with(move |b| amend(b, field), vec![with_note()]));
        assert_eq!(e["code"], "UNAUTHORIZED_DECISION_WRITE", "{field}");
        assert_eq!(e["action"], "amend", "{field}");
        assert_eq!(e["entity"], X, "{field}");
        assert_eq!(e["field"], field);
        assert_eq!(e["owner"], "doc_approval", "{field}");
    }
}

#[test]
fn a_hand_written_removal_of_a_decision_is_refused() {
    let e = single(build_with(retract, vec![with_note()]));
    assert_eq!(e["code"], "UNAUTHORIZED_DECISION_REMOVAL");
    assert_eq!(e["action"], "retract");
    assert_eq!(e["entity"], X);
    assert_eq!(e["owner"], "doc_approval");
}

// --- models over X ----------------------------------------------------------------------------

/// A StateMachine whose subject is `X`, with state field `choice`: `flip` APPROVE → REJECT.
fn machine_over_decisions() -> AnyModel<'static> {
    AnyModel::StateMachine(ModelSpec {
        input: ModelInput {
            model_id: "decision_lifecycle".into(),
            subject_entity: X.into(),
            state_field: "choice".into(),
            subject_param: None,
            entry_states: vec!["APPROVE".into(), "REJECT".into()],
            transitions: vec![transition("flip", &["APPROVE"], "REJECT")],
            display: behavior_models::DisplayMeta::default(),
            source: None,
        },
        fragments: Box::new(behavior_models::NoFragments),
    })
}

#[test]
fn a_state_machine_over_decisions_is_refused_on_its_transitions() {
    let errs = errors(build_with(
        |_| {},
        vec![spec(approval(1, None), source()), machine_over_decisions()],
    ));
    let writes: Vec<serde_json::Value> = errs
        .iter()
        .filter(|e| e.code() == "UNAUTHORIZED_DECISION_WRITE")
        .map(ModelError::to_json)
        .collect();
    assert_eq!(writes.len(), 1, "{errs:#?}");
    assert_eq!(writes[0]["action"], "decision_lifecycle__flip");
    assert_eq!(writes[0]["field"], "choice");
    assert_eq!(writes[0]["owner"], "doc_approval");
}

#[test]
fn a_request_flow_creation_writing_a_bound_decision_is_refused() {
    use common::rf;
    // Approval `budget_approval` over the flow fixture's `Budget`, actor `User`.
    let mut ap_input = approval(1, None);
    ap_input.approval_id = "budget_approval".into();
    ap_input.subject_entity = "Budget".into();
    let x = "BudgetApprovalDecision";
    let c = "BudgetApprovalDecisionChoice";

    // The flow's `create` binds an existing decision `d` and writes `d.choice`.
    let mut flow = rf::flow();
    flow.creations[0].state = vec![("d".into(), x.into())];
    let mut src = rf::source();
    src.creations.insert(
        "create".into(),
        Box::new(move |b, s| {
            let mut f = rf::standard(b, s)?;
            let v = b
                .lit(WType::Enum(c.into()), json!("REJECT"), loc())
                .unwrap();
            f.effects = vec![("d".into(), "choice".into(), v, loc())];
            Ok(f)
        }),
    );
    let errs = errors(rf::build_with(
        |_| {},
        vec![rf::spec(flow, src), spec(ap_input, source())],
    ));
    let writes: Vec<serde_json::Value> = errs
        .iter()
        .filter(|e| e.code() == "UNAUTHORIZED_DECISION_WRITE")
        .map(ModelError::to_json)
        .collect();
    assert_eq!(writes.len(), 1, "{errs:#?}");
    assert_eq!(writes[0]["action"], "purchase_request__create");
    assert_eq!(writes[0]["entity"], x);
    assert_eq!(writes[0]["field"], "choice");
    assert_eq!(writes[0]["owner"], "budget_approval");
}

/// Approval `q` over the same subject: its actions create `X_q`, never `X_p`, by construction;
/// and the check exempts nothing by origin: in the presence of `q`, a creation of `X_p` outside
/// `p`'s two actions is still refused.
#[test]
fn another_approvals_actions_never_create_this_approvals_decisions() {
    let mut q = approval(1, None);
    q.approval_id = "legal_review".into();
    let out = build_with(
        |_| {},
        vec![spec(approval(1, None), source()), spec(q.clone(), source())],
    )
    .unwrap_or_else(|e| panic!("{e:#?}"));
    for (name, action) in out.module.actions() {
        let created: Vec<&str> = action.creates().iter().map(|c| c.entity()).collect();
        let expected = if name.starts_with("doc_approval__") {
            vec![X]
        } else {
            assert!(name.starts_with("legal_review__"), "{name}");
            vec!["LegalReviewDecision"]
        };
        assert_eq!(created, expected, "{name}");
    }
    let errs = errors(build_with(forge, vec![with_note(), spec(q, source())]));
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_DECISION_CREATION"]);
}

/// Reads, queries, derived values and guards over `X` are anyone's.
#[test]
fn reading_decisions_is_allowed() {
    let out = build_with(
        |b| {
            // A derived count of all decisions, a declared read over X and an action guarded by
            // a query over X.
            b.push_scope(ScopeSite::Derived, vec![], loc()).unwrap();
            let all = b.select(X, loc()).unwrap();
            let n = b.op("count", vec![all], loc()).unwrap();
            b.pop_scope();
            b.add_derived(
                "decision_total",
                behavior_engine::wire::DerivedKind::Derived,
                vec![],
                n,
                Some(WType::Int),
                loc(),
            )
            .unwrap();
            let ps = vec![state("doc", "Document")];
            b.push_scope(ScopeSite::Read, ps.clone(), loc()).unwrap();
            let all = b.select(X, loc()).unwrap();
            let n = b.op("count", vec![all], loc()).unwrap();
            b.pop_scope();
            let read = Builder::read_value("decisions", ps.clone(), n, loc());
            b.add_read(read).unwrap();
            b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
            let all = b.select(X, loc()).unwrap();
            b.push_lambda("d", X).unwrap();
            let subject = b.field("d", "subject", loc()).unwrap();
            let id = b.field("doc", "id", loc()).unwrap();
            let same = b.op("eq", vec![subject, id], loc()).unwrap();
            b.pop_scope();
            let any = b.lambda("any", all, "d", same, loc()).unwrap();
            let title = b.lit(WType::String, json!("decided"), loc()).unwrap();
            b.add_action(
                "retitle",
                ps.clone(),
                vec![(any, loc())],
                vec![("doc".into(), "title".into(), title, loc())],
                vec![],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(approval(1, None), source())],
    );
    let out = out.unwrap_or_else(|e| panic!("{e:#?}"));
    assert!(out.module.actions().contains_key("retitle"));
}

// --- T030: completeness -----------------------------------------------------------------------

/// Every action's state changes touching `X`: (creates, effect fields, removals).
fn touches(module: &behavior_engine::semantic::Module, x: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, action) in module.actions() {
        let is_x = |param: &str| {
            action
                .params()
                .iter()
                .any(|p| p.name() == param && p.ty().to_wire_type() == WType::Entity(x.into()))
        };
        for c in action.creates() {
            if c.entity() == x {
                out.push((name.clone(), "create".into()));
            }
        }
        for e in action.effects() {
            if is_x(e.param()) {
                out.push((name.clone(), format!("write {}", e.field())));
            }
        }
        for r in action.removes() {
            if is_x(r.param()) {
                out.push((name.clone(), "remove".into()));
            }
        }
    }
    out
}

/// The premise of the induction (T021): in an accepted module, the only state changes touching
/// `X` are the single creation of each of `p`'s two actions. Checked here on `doc_approval`
/// with hand-written behavior around it; the reference modules `ap_*` are added by T048.
#[test]
fn only_the_two_decision_actions_touch_decisions() {
    for k_r in [None, Some(1)] {
        let out = build(1, k_r);
        assert_eq!(
            touches(&out.module, X),
            vec![
                (APPROVE.to_string(), "create".to_string()),
                (REJECT.to_string(), "create".to_string()),
            ]
        );
        for name in [APPROVE, REJECT] {
            let a = &out.module.actions()[name];
            assert_eq!(a.creates().len(), 1, "{name}");
            assert!(a.effects().is_empty() && a.removes().is_empty(), "{name}");
        }
    }
}
