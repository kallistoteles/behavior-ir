//! WorkQueue ownership over the final admitted module (feature 507, research R4; premises P1–P3).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::wire::{WLifecycle, WType};
use behavior_models::ModelError;
use common::wq::*;
use common::{codes, loc, state_param};
use serde_json::json;

const E: &str = "DocumentWork";

/// A hand-written action `name(item: State E)` that sets `item.field := value`.
fn writer(b: &mut Builder, name: &str, field: &str, value: serde_json::Value, ty: WType) {
    let p = state_param("item", E);
    b.push_scope(ScopeSite::Action, vec![p.clone()], loc())
        .unwrap();
    let v = b.lit(ty, value, loc()).unwrap();
    b.add_action(
        name,
        vec![p],
        vec![],
        vec![("item".into(), field.into(), v, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

fn single(r: Result<behavior_models::BuildOutput, Vec<ModelError>>) -> ModelError {
    let errs = r.unwrap_err();
    assert_eq!(errs.len(), 1, "{errs:#?}");
    errs.into_iter().next().unwrap()
}

#[test]
fn a_hand_written_write_of_each_owned_field_is_refused() {
    let cases: [(&str, serde_json::Value, WType); 5] = [
        ("attempts", json!(0), WType::Int),
        ("available_at", json!(0), time()),
        ("lease_until", json!(null), WType::Option(Box::new(time()))),
        (
            "owner",
            json!(null),
            WType::Option(Box::new(WType::Id("Worker".into()))),
        ),
        ("document", json!("d9"), WType::Id("Document".into())),
    ];
    for (field, v, ty) in cases {
        let e = single(build_with(
            move |b| writer(b, "tamper", field, v, ty),
            vec![spec(queue(3), source())],
        ));
        assert_eq!(
            e,
            ModelError::UnauthorizedWorkWrite {
                action: "tamper".into(),
                entity: E.into(),
                field: field.into(),
                owner: "document_work".into(),
            },
            "{field}"
        );
    }
}

#[test]
fn a_hand_written_status_write_is_the_machines_error() {
    let errs = build_with(
        |b| {
            writer(
                b,
                "tamper",
                "status",
                json!("READY"),
                WType::Enum("DocumentWorkStatus".into()),
            )
        },
        vec![spec(queue(3), source())],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_OWNED_FIELD_WRITE"]);
}

#[test]
fn a_hand_written_removal_is_refused() {
    let e = single(build_with(
        |b| {
            let ps = vec![state_param("item", E)];
            b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
            let remove = WLifecycle::Remove {
                param: "item".into(),
                loc: loc(),
            };
            b.add_action("purge", ps, vec![], vec![], vec![remove], vec![], loc())
                .unwrap();
            b.pop_scope();
        },
        vec![spec(queue(3), source())],
    ));
    assert_eq!(e.code(), "UNAUTHORIZED_WORK_REMOVAL");
}

#[test]
fn a_hand_written_creation_is_refused() {
    let errs = build_with(
        |b| {
            let id = behavior_engine::wire::WParam {
                name: "id".into(),
                role: Some(behavior_engine::wire::Role::Input),
                ty: WType::Id(E.into()),
            };
            b.push_scope(ScopeSite::Action, vec![id.clone()], loc())
                .unwrap();
            let mut fields = Vec::new();
            for (f, v, t) in [
                ("attempts", json!(0), WType::Int),
                ("available_at", json!(0), time()),
                ("document", json!("d1"), WType::Id("Document".into())),
                ("lease_until", json!(null), WType::Option(Box::new(time()))),
                (
                    "owner",
                    json!(null),
                    WType::Option(Box::new(WType::Id("Worker".into()))),
                ),
                (
                    "status",
                    json!("READY"),
                    WType::Enum("DocumentWorkStatus".into()),
                ),
            ] {
                fields.push((f.to_string(), b.lit(t, v, loc()).unwrap().into_wire()));
            }
            let id_node = b.param("id", loc()).unwrap();
            let create = WLifecycle::Create {
                entity: E.into(),
                id: id_node.into_wire(),
                fields,
                loc: loc(),
            };
            b.add_action(
                "forge",
                vec![id],
                vec![],
                vec![],
                vec![create],
                vec![],
                loc(),
            )
            .unwrap();
            b.pop_scope();
        },
        vec![spec(queue(3), source())],
    )
    .unwrap_err();
    assert_eq!(codes(&errs), vec!["UNAUTHORIZED_WORK_CREATION"]);
}

#[test]
fn another_queues_actions_cannot_write_this_queues_items() {
    // Two queues; queue `b`'s claim binds a work item of queue `a` (another entity type, so not
    // WORK_ITEM_BINDING) and writes its `attempts`.
    let mut qa = queue(3);
    qa.queue_id = "a".into();
    let mut qb = queue(3);
    qb.queue_id = "b".into();
    qb.capabilities[1].state = vec![("other".into(), "A".into())]; // claim
    let mut s = source();
    s.extra.insert(
        "claim".into(),
        Box::new(|b, _| {
            let v = b.lit(WType::Int, json!(0), loc()).unwrap();
            Ok(behavior_models::work_queue::input::CapabilityFragments {
                effects: vec![("other".into(), "attempts".into(), v, loc())],
                ..Default::default()
            })
        }),
    );
    let errs = build_with(|_| {}, vec![spec(qa, source()), spec(qb, s)]).unwrap_err();
    assert_eq!(
        errs,
        vec![ModelError::UnauthorizedWorkWrite {
            action: "b__claim".into(),
            entity: "A".into(),
            field: "attempts".into(),
            owner: "a".into(),
        }]
    );
}

#[test]
fn two_independent_queues_build_with_distinct_identities() {
    let mut qa = queue(3);
    qa.queue_id = "a".into();
    let mut qb = queue(2);
    qb.queue_id = "b".into();
    let out = build_with(|_| {}, vec![spec(qa, source()), spec(qb, source())])
        .unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(out.receipts.len(), 2);
    assert_ne!(
        out.receipts[0].0["model_hash"],
        out.receipts[1].0["model_hash"]
    );
    assert!(out.module.invariants().contains_key("a__valid_work"));
    assert!(out.module.invariants().contains_key("b__valid_work"));
}
