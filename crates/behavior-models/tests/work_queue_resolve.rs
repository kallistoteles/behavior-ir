//! WorkQueue resolution and build errors (feature 507, research R5).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use behavior_models::ModelError;
use common::codes;
use common::wq::*;

fn errors(input: behavior_models::work_queue::input::WorkQueueInput) -> Vec<ModelError> {
    build_with(|_| {}, vec![spec(input, source())]).unwrap_err()
}

#[test]
fn max_attempts_must_be_at_least_one_and_incrementable() {
    for m in [0, -1, i64::MAX] {
        assert_eq!(
            codes(&errors(queue(m))),
            vec!["INVALID_MAX_ATTEMPTS"],
            "{m}"
        );
    }
    assert!(build_with(|_| {}, vec![spec(queue(i64::MAX - 1), source())]).is_ok());
}

#[test]
fn the_time_type_must_be_an_ordered_nominal() {
    let mut q = queue(3);
    q.time = WType::Int;
    assert_eq!(codes(&errors(q)), vec!["TIME_NOT_ORDERED"]);
    let mut q = queue(3);
    q.time = WType::Nominal("Plain".into()); // declared without `order`
    assert_eq!(codes(&errors(q)), vec!["TIME_NOT_ORDERED"]);
}

#[test]
fn every_capability_that_needs_authorization_must_have_it() {
    let errs = build_with(
        |_| {},
        vec![spec(
            queue(3),
            Source {
                authorize: false,
                extra: Default::default(),
            },
        )],
    )
    .unwrap_err();
    let refs: Vec<String> = errs
        .iter()
        .map(|e| {
            assert_eq!(e.code(), "MISSING_AUTHORIZATION");
            e.to_json()["model_ref"].as_str().unwrap().to_string()
        })
        .collect();
    let mut expected: Vec<String> = NEEDS_AUTH
        .iter()
        .map(|c| format!("document_work/capability/{c}"))
        .collect();
    expected.sort();
    let mut refs = refs;
    refs.sort();
    // Enqueue stops the build first (its action cannot be taken back); the machine's four
    // follow when enqueue is authorized.
    assert!(refs.iter().all(|r| expected.contains(r)), "{refs:?}");
    let mut only_enqueue = source();
    only_enqueue.authorize = false;
    only_enqueue.extra.insert(
        "enqueue".into(),
        Box::new(|b, _| {
            let t = b
                .lit(WType::Bool, serde_json::json!(true), common::loc())
                .unwrap();
            Ok(behavior_models::work_queue::input::CapabilityFragments {
                authorization: Some((t, common::loc())),
                ..Default::default()
            })
        }),
    );
    let errs = build_with(|_| {}, vec![spec(queue(3), only_enqueue)]).unwrap_err();
    let mut refs: Vec<String> = errs
        .iter()
        .map(|e| e.to_json()["model_ref"].as_str().unwrap().to_string())
        .collect();
    refs.sort();
    let mut machine: Vec<String> = ["claim", "recover", "exhaust", "cancel"]
        .iter()
        .map(|c| format!("document_work/capability/{c}"))
        .collect();
    machine.sort();
    assert_eq!(refs, machine);
}

#[test]
fn generated_parameter_names_are_reserved() {
    for name in [
        "worker",
        "now",
        "lease_until",
        "available_at",
        "generation",
        "expected_generation",
        "id",
        "document_work",
    ] {
        let mut q = queue(3);
        q.capabilities[2].context = vec![(name.into(), WType::Int)]; // renew
        assert_eq!(codes(&errors(q)), vec!["RESERVED_PARAMETER"], "{name}");
    }
    // A payload field name is reserved for enqueue's inputs.
    let mut q = queue(3);
    q.capabilities[0].inputs = vec![("document".into(), WType::Int)];
    assert_eq!(codes(&errors(q)), vec!["RESERVED_PARAMETER"]);
}

#[test]
fn generated_field_names_cannot_be_payload_fields() {
    for name in ["status", "attempts", "owner", "lease_until", "available_at"] {
        let mut q = queue(3);
        q.payload.push((name.into(), WType::Int));
        assert_eq!(codes(&errors(q)), vec!["PAYLOAD_FIELD_RESERVED"], "{name}");
    }
}

#[test]
fn every_capability_must_be_declared() {
    for c in CAPABILITIES {
        let mut q = queue(3);
        q.capabilities.retain(|x| x.capability_id != c);
        assert_eq!(codes(&errors(q)), vec!["MISSING_CAPABILITY"], "{c}");
    }
}

#[test]
fn a_capability_cannot_bind_the_queues_own_work_entity() {
    let mut q = queue(3);
    q.capabilities[3].state = vec![("other".into(), "DocumentWork".into())]; // complete
    assert_eq!(codes(&errors(q)), vec!["WORK_ITEM_BINDING"]);
}

#[test]
fn an_authored_effect_on_the_subject_is_rejected() {
    let mut s = source();
    s.extra.insert(
        "complete".into(),
        Box::new(|b, scope| {
            let subject = scope.params[0].name.clone();
            let v = b
                .lit(WType::Int, serde_json::json!(0), common::loc())
                .unwrap();
            Ok(behavior_models::work_queue::input::CapabilityFragments {
                effects: vec![(subject, "attempts".into(), v, common::loc())],
                ..Default::default()
            })
        }),
    );
    let errs = build_with(|_| {}, vec![spec(queue(3), s)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["AUTHORED_WORK_EFFECT"]);
}
