//! Resolution rules (data-model §3; T015).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use behavior_models::state_machine::resolve::{resolve, snake_case};
use common::{codes, declarations, machine, transition};

fn base() -> behavior_engine::semantic::Module {
    declarations().finish(None).unwrap()
}

fn errors(input: behavior_models::ModelInput) -> Vec<&'static str> {
    let module = base();
    let errs = resolve(&input, &module).unwrap_err();
    codes(&errs)
}

#[test]
fn resolves_the_reference_machine() {
    let module = base();
    let r = resolve(&machine(), &module).unwrap();
    assert_eq!(r.model_id.as_str(), "purchase_request");
    assert_eq!(r.enum_type, "RequestStatus");
    assert_eq!(r.subject_param, "purchase_request");
    assert_eq!(r.entry_states.iter().collect::<Vec<_>>(), vec!["Draft"]);
    let ids: Vec<&str> = r.transitions.keys().map(|t| t.as_str()).collect();
    assert_eq!(ids, vec!["approve", "reject", "start_review", "submit"]);
    assert_eq!(
        r.transitions
            .values()
            .map(|t| t.action_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "purchase_request__approve",
            "purchase_request__reject",
            "purchase_request__start_review",
            "purchase_request__submit"
        ]
    );
}

#[test]
fn invalid_semantic_ids() {
    let mut m = machine();
    m.model_id = "PurchaseRequest".into();
    assert_eq!(errors(m), vec!["INVALID_SEMANTIC_ID"]);
    let mut m = machine();
    m.transitions[0].transition_id = "do__it".into();
    assert_eq!(errors(m), vec!["INVALID_SEMANTIC_ID"]);
}

#[test]
fn unknown_entity() {
    let mut m = machine();
    m.subject_entity = "Ghost".into();
    assert_eq!(errors(m), vec!["UNKNOWN_ENTITY"]);
}

#[test]
fn state_field_must_be_an_enum_type() {
    for field in ["approved_by", "note", "missing"] {
        let mut m = machine();
        m.state_field = field.into();
        assert_eq!(errors(m), vec!["STATE_FIELD_NOT_ENUM"], "{field}");
    }
}

#[test]
fn unknown_states_anywhere() {
    let mut m = machine();
    m.entry_states = vec!["Draft".into(), "Ghost".into()];
    m.transitions[0].from_states = vec!["Limbo".into()];
    m.transitions[1].to_state = "Nowhere".into();
    assert_eq!(
        errors(m),
        vec!["UNKNOWN_STATE", "UNKNOWN_STATE", "UNKNOWN_STATE"]
    );
}

#[test]
fn empty_sets() {
    let mut m = machine();
    m.entry_states = vec![];
    m.transitions[0].from_states = vec![];
    assert_eq!(errors(m), vec!["EMPTY_ENTRY_STATES", "EMPTY_SOURCE_STATES"]);
}

#[test]
fn duplicate_transition_ids() {
    let mut m = machine();
    m.transitions
        .push(transition("submit", &["Rejected"], "Draft"));
    assert_eq!(errors(m), vec!["DUPLICATE_TRANSITION_ID"]);
}

#[test]
fn invalid_parameters() {
    let mut m = machine();
    m.subject_param = Some("not valid".into());
    assert_eq!(errors(m), vec!["INVALID_PARAMETER"]);

    // An input named like the subject, and a context named like an input.
    let mut m = machine();
    m.transitions[0].inputs = vec![
        ("purchase_request".into(), WType::String),
        ("reason".into(), WType::String),
    ];
    m.transitions[0].context = vec![("reason".into(), WType::String)];
    assert_eq!(errors(m), vec!["INVALID_PARAMETER", "INVALID_PARAMETER"]);

    // The same input declared twice is not a mapping.
    let mut m = machine();
    m.transitions[0].inputs = vec![
        ("reason".into(), WType::String),
        ("reason".into(), WType::Int),
    ];
    assert_eq!(errors(m), vec!["INVALID_PARAMETER"]);
}

#[test]
fn duplicate_cases_in_input_sets_are_set_semantics() {
    let module = base();
    let mut m = machine();
    m.entry_states = vec!["Draft".into(), "Draft".into()];
    m.transitions[0].from_states = vec!["Draft".into(), "Draft".into()];
    let r = resolve(&m, &module).unwrap();
    assert_eq!(r.entry_states.len(), 1);
    assert_eq!(r.transitions.values().last().unwrap().from_states.len(), 1);
}

#[test]
fn subject_param_default_and_override() {
    assert_eq!(snake_case("PurchaseRequest"), "purchase_request");
    assert_eq!(snake_case("HTTPRequest"), "http_request");
    assert_eq!(snake_case("Order"), "order");
    assert_eq!(snake_case("Order2Line"), "order2_line");
    let module = base();
    let mut m = machine();
    m.subject_param = Some("request".into());
    assert_eq!(resolve(&m, &module).unwrap().subject_param, "request");
}

#[test]
fn inputs_and_context_are_mappings_sorted_by_name() {
    let module = base();
    let mut a = machine();
    a.transitions[0].inputs = vec![("zeta".into(), WType::Int), ("alpha".into(), WType::String)];
    let mut b = machine();
    b.transitions[0].inputs = vec![("alpha".into(), WType::String), ("zeta".into(), WType::Int)];
    let ra = resolve(&a, &module).unwrap();
    let rb = resolve(&b, &module).unwrap();
    let submit = |r: &behavior_models::state_machine::resolve::ResolvedStateMachine| {
        r.transitions
            .values()
            .find(|t| t.transition_id.as_str() == "submit")
            .unwrap()
            .inputs
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(submit(&ra), vec!["alpha", "zeta"]);
    assert_eq!(submit(&ra), submit(&rb));
}
