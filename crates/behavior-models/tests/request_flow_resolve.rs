//! RequestFlow resolution rules (data model §3; T013).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use common::codes;
use common::rf::*;

fn errors(input: behavior_models::RequestFlowInput) -> Vec<&'static str> {
    codes(&build_with(|_| {}, vec![spec(input, source())]).unwrap_err())
}

#[test]
fn schema_is_canonical_and_uses_identities() {
    let out = build_reference();
    let e = &out.module.entities()["PurchaseRequest"];
    let fields: Vec<(String, WType)> = e
        .fields()
        .iter()
        .map(|(n, t)| (n.clone(), t.to_wire_type()))
        .filter(|(n, _)| n != "id") // Core's own identity field
        .collect();
    assert_eq!(
        fields,
        vec![
            ("amount".to_string(), WType::Int),
            (
                "assignee".to_string(),
                WType::Option(Box::new(WType::Id("User".into())))
            ),
            ("requester".to_string(), WType::Id("User".into())),
            (
                "status".to_string(),
                WType::Enum("PurchaseRequestStatus".into())
            ),
            ("title".to_string(), WType::String),
        ]
    );
    // Identities, never references (FR-003).
    assert!(e.fields().iter().all(|(n, _)| !e.is_reference(n)));
}

#[test]
fn without_assignee_there_is_no_assignee_field() {
    let mut f = flow();
    f.assignee = false;
    let mut s = source();
    for c in ["create", "import_submitted"] {
        s.creations.insert(
            c.into(),
            Box::new(
                |b: &mut behavior_engine::builder::Builder, sc: &behavior_models::CreationScope| {
                    let mut frag = standard(b, sc)?;
                    frag.initializer.retain(|(n, _)| n != "assignee");
                    Ok(frag)
                },
            ),
        );
    }
    s.transitions.clear();
    f.transitions.retain(|t| t.transition_id != "start_review");
    f.transitions.push(common::transition(
        "start_review",
        &["Submitted"],
        "InReview",
    ));
    let out = build_with(|_| {}, vec![spec(f, s)]).unwrap();
    assert!(
        out.module.entities()["PurchaseRequest"]
            .field_type("assignee")
            .is_none()
    );
}

#[test]
fn invalid_ids_unknown_actor_and_enum() {
    let mut f = flow();
    f.flow_id = "PurchaseRequest".into();
    assert_eq!(errors(f), vec!["INVALID_SEMANTIC_ID"]);
    let mut f = flow();
    f.actor_entity = "Ghost".into();
    assert_eq!(errors(f), vec!["UNKNOWN_ENTITY"]);
    let mut f = flow();
    f.status_enum = "Ghost".into();
    assert_eq!(errors(f), vec!["UNKNOWN_ENUM"]);
}

#[test]
fn reserved_and_duplicate_payload_fields() {
    for name in ["requester", "status", "assignee"] {
        let mut f = flow();
        f.payload.push((name.into(), WType::String));
        assert_eq!(errors(f), vec!["RESERVED_FIELD"], "{name}");
    }
    let mut f = flow();
    f.payload.push(("title".into(), WType::Int));
    assert_eq!(errors(f), vec!["INVALID_PARAMETER"]);
}

#[test]
fn creations_are_required_and_share_the_capability_namespace() {
    let mut f = flow();
    f.creations.clear();
    assert!(errors(f).contains(&"NO_CREATION"));
    let mut f = flow();
    f.creations[0].creation_id = "submit".into();
    assert_eq!(errors(f), vec!["CAPABILITY_ID_COLLISION"]);
}

#[test]
fn creation_targets_and_parameters() {
    let mut f = flow();
    f.creations[0].to_state = "Limbo".into();
    assert!(errors(f).contains(&"UNKNOWN_STATE"));
    let mut f = flow();
    f.creations[0]
        .inputs
        .push(("request_id".into(), WType::Int));
    assert_eq!(errors(f), vec!["INVALID_PARAMETER"]);
}

#[test]
fn the_entity_name_is_the_encoding_of_the_flow_id() {
    let mut f = flow();
    f.flow_id = "order_2b".into();
    // Without `commands`: the fixture's `Notify` names `PurchaseRequest`.
    let mut b = authored();
    let models = vec![spec(f, source())];
    behavior_models::declare_models(&mut b, &models).unwrap();
    let out = behavior_models::compose_models(&mut b, models, None).unwrap();
    assert!(out.module.entities().contains_key("Order_2b"));
    assert!(out.module.actions().contains_key("order_2b__create"));
}

#[test]
fn entry_states_are_derived_from_creation_targets() {
    let out = build_reference();
    let r = &out.receipts[0].0;
    assert_eq!(
        r["model"]["state_machine"]["document"]["entry_states"],
        serde_json::json!(["Draft", "Submitted"])
    );
}
