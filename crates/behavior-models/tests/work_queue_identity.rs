//! WorkQueue identity (feature 507, research R6; scenario W19).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::{WField, WType};
use behavior_models::DisplayMeta;
use behavior_models::work_queue::input::{CapabilityFragments, WorkQueueInput};
use common::loc;
use common::wq::*;
use serde_json::{Value, json};

fn hashes(input: WorkQueueInput, s: Source) -> (Value, String) {
    let out = build_with(|_| {}, vec![spec(input, s)]).unwrap_or_else(|e| panic!("{e:#?}"));
    (
        out.receipts[0].0["model_hash"].clone(),
        out.module.behavior_version(),
    )
}

fn reference() -> (Value, String) {
    hashes(queue(3), source())
}

#[test]
fn repeated_builds_give_identical_receipts() {
    let a = build_reference();
    let b = build_reference();
    assert_eq!(a.receipts[0].0, b.receipts[0].0);
}

#[test]
fn display_metadata_and_authoring_order_change_nothing() {
    let base = reference();
    let mut q = queue(3);
    q.display = DisplayMeta {
        display_name: Some("Documents".into()),
        description: Some("x".into()),
    };
    q.capabilities.reverse();
    assert_eq!(hashes(q, source()), base);
    let mut q = queue(3);
    q.payload.push(("priority".into(), WType::Int));
    let mut r = queue(3);
    r.payload.insert(0, ("priority".into(), WType::Int));
    assert_eq!(hashes(q, source()), hashes(r, source()));
}

#[test]
fn the_model_hash_changes_with_m_payload_and_guards() {
    let (base, behavior) = reference();
    let (m, mb) = hashes(queue(4), source());
    assert_ne!(m, base);
    assert_ne!(mb, behavior);
    let mut q = queue(3);
    q.payload.push(("priority".into(), WType::Int));
    assert_ne!(hashes(q, source()).0, base);
    let mut s = source();
    s.extra.insert(
        "complete".into(),
        Box::new(|b, _| {
            let f = b.lit(WType::Bool, json!(true), loc()).unwrap();
            Ok(CapabilityFragments {
                guards: vec![(f, loc())],
                ..Default::default()
            })
        }),
    );
    let (g, gb) = hashes(queue(3), s);
    assert_ne!(g, base);
    assert_ne!(gb, behavior);
}

#[test]
fn the_model_hash_changes_with_a_field_added_to_the_worker() {
    // Feature 505: exact declaration dependency.
    let (base, _) = reference();
    let mut b = authored_with_worker_field();
    behavior_models::declare_models(&mut b, &[spec(queue(3), source())]).unwrap();
    commands(&mut b);
    let out = behavior_models::compose_models(&mut b, vec![spec(queue(3), source())], None)
        .unwrap_or_else(|e| panic!("{e:#?}"));
    assert_ne!(out.receipts[0].0["model_hash"], base);
}

fn authored_with_worker_field() -> behavior_engine::builder::Builder {
    use behavior_engine::builder::{Builder, SemanticProfile};
    let field = |n: &str, t: WType| WField {
        name: n.into(),
        ty: t,
        loc: loc(),
    };
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_nominal("Time", WType::Int, vec!["order".into()], None, loc())
        .unwrap();
    b.declare_nominal("Plain", WType::Int, vec![], None, loc())
        .unwrap();
    b.declare_entity(
        "Worker",
        vec![field("name", WType::String), field("active", WType::Bool)],
        loc(),
    )
    .unwrap();
    b.declare_entity("User", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("Document", vec![field("processed", WType::Bool)], loc())
        .unwrap();
    b
}
