//! WorkQueue declarations and lowering (feature 507; data model §2–§4; safety premises P1, P3–P7
//! as IR checks).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use common::wq::*;

fn params(out: &behavior_models::BuildOutput, action: &str) -> Vec<(String, String)> {
    out.module
        .actions()
        .get(action)
        .unwrap_or_else(|| panic!("no action {action}"))
        .params()
        .iter()
        .map(|p| (p.name().to_string(), format!("{:?}", p.role())))
        .collect()
}

fn effects(out: &behavior_models::BuildOutput, action: &str) -> Vec<(String, String)> {
    out.module.actions()[action]
        .effects()
        .iter()
        .map(|e| (e.param().to_string(), e.field().to_string()))
        .collect()
}

fn p(name: &str, role: &str) -> (String, String) {
    (name.to_string(), role.to_string())
}

fn w(field: &str) -> (String, String) {
    ("document_work".to_string(), field.to_string())
}

#[test]
fn the_declarations_are_generated() {
    let out = build_reference();
    let m = &out.module;
    assert_eq!(
        m.enums()["DocumentWorkStatus"].values,
        vec!["READY", "LEASED", "SUCCEEDED", "FAILED", "CANCELLED"]
    );
    let e = &m.entities()["DocumentWork"];
    let fields: Vec<(String, WType)> = [
        "attempts",
        "available_at",
        "document",
        "lease_until",
        "owner",
        "status",
    ]
    .iter()
    .map(|f| (f.to_string(), e.field_type(f).unwrap().to_wire_type()))
    .collect();
    assert_eq!(
        fields,
        vec![
            ("attempts".into(), WType::Int),
            ("available_at".into(), time()),
            ("document".into(), WType::Id("Document".into())),
            ("lease_until".into(), WType::Option(Box::new(time()))),
            (
                "owner".into(),
                WType::Option(Box::new(WType::Id("Worker".into())))
            ),
            ("status".into(), WType::Enum("DocumentWorkStatus".into())),
        ]
    );
    assert!(m.invariants().contains_key("document_work__valid_work"));
    assert!(m.derived_items().contains_key("document_work__terminal"));
    assert!(m.reads().contains_key("document_work__ready"));
    assert!(m.reads().contains_key("document_work__expired"));
}

#[test]
fn every_action_has_the_generated_parameter_order() {
    let out = build_reference();
    assert_eq!(
        params(&out, "document_work__enqueue"),
        vec![
            p("id", "Input"),
            p("available_at", "Input"),
            p("document", "Input")
        ]
    );
    let subject = p("document_work", "State");
    assert_eq!(
        params(&out, "document_work__claim"),
        vec![
            subject.clone(),
            p("expected_generation", "Input"),
            p("lease_until", "Context"),
            p("now", "Context"),
            p("worker", "Context"),
        ]
    );
    assert_eq!(
        params(&out, "document_work__complete"),
        vec![
            subject.clone(),
            p("generation", "Input"),
            p("now", "Context"),
            p("worker", "Context")
        ]
    );
    assert_eq!(
        params(&out, "document_work__recover"),
        vec![
            subject.clone(),
            p("available_at", "Context"),
            p("now", "Context")
        ]
    );
    assert_eq!(params(&out, "document_work__cancel"), vec![subject]);
}

#[test]
fn every_action_has_its_generated_effects_and_postconditions() {
    let out = build_reference();
    type Case = (&'static str, Vec<(String, String)>, usize);
    let cases: [Case; 8] = [
        (
            "claim",
            vec![w("status"), w("attempts"), w("owner"), w("lease_until")],
            2,
        ),
        ("renew", vec![w("status"), w("lease_until")], 2),
        (
            "complete",
            vec![w("status"), w("owner"), w("lease_until")],
            2,
        ),
        (
            "retry",
            vec![w("status"), w("owner"), w("lease_until"), w("available_at")],
            2,
        ),
        ("fail", vec![w("status"), w("owner"), w("lease_until")], 2),
        (
            "recover",
            vec![w("status"), w("owner"), w("lease_until"), w("available_at")],
            1,
        ),
        (
            "exhaust",
            vec![w("status"), w("owner"), w("lease_until")],
            1,
        ),
        ("cancel", vec![w("status"), w("owner"), w("lease_until")], 1),
    ];
    for (c, expected, post) in cases {
        let name = format!("document_work__{c}");
        assert_eq!(effects(&out, &name), expected, "{c}");
        assert_eq!(
            out.module.actions()[&name].postconditions().len(),
            post,
            "{c}"
        );
    }
    // P1: only claim and enqueue touch `attempts`.
    for (name, action) in out.module.actions() {
        let writes = action.effects().iter().any(|e| e.field() == "attempts");
        assert_eq!(writes, name == "document_work__claim", "{name}");
    }
}

#[test]
fn the_generated_requirements_precede_the_authored_ones() {
    // Status requirement, the generated guards, then the authorization (claim: eligibility).
    let out = build_reference();
    let counts = [
        ("claim", 1 + 4 + 1),
        ("renew", 1 + 4),
        ("complete", 1 + 3),
        ("retry", 1 + 5),
        ("fail", 1 + 3),
        ("recover", 1 + 3 + 1),
        ("exhaust", 1 + 2 + 1),
        ("cancel", 1 + 1),
    ];
    for (c, n) in counts {
        let name = format!("document_work__{c}");
        assert_eq!(out.module.actions()[&name].preconditions().len(), n, "{c}");
    }
    assert_eq!(
        out.module.actions()["document_work__enqueue"]
            .preconditions()
            .len(),
        1
    );
}

#[test]
fn the_receipt_names_the_kind_versions_and_embedded_machine() {
    let out = build_reference();
    let r = &out.receipts[0].0;
    assert_eq!(r["model_kind"], "work_queue");
    assert_eq!(r["model_semantic_version"], 1);
    assert_eq!(r["lowerer_version"], 1);
    assert_eq!(r["model"]["format"], "behavior.model.work_queue.v1");
    assert_eq!(r["embedded"][0]["kind"], "state_machine");
    assert_eq!(r["embedded"][0]["model_semantic_version"], 2);
    assert!(r["generated"]["invariant"]["hash"].is_string());
    assert!(r["generated"]["reads"]["ready"].is_string());
}
