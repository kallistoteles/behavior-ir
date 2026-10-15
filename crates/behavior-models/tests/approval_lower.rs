//! Approval lowering (feature 503, data model §5–§6): the derived definitions (T015), the
//! decision action layout (T016), the initializer checks (T034), and the outcome through the
//! public engine and store.
//!
//! Terminology: DENIED = an attempt that was not permitted; REJECT = a recorded choice;
//! REJECTED = the derived outcome.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::{Role, WType};
use common::ap::*;
use common::{codes, loc};

const DEFINITIONS: [&str; 5] = [
    "doc_approval__approve_count",
    "doc_approval__reject_count",
    "doc_approval__approved",
    "doc_approval__rejected",
    "doc_approval__pending",
];

// --- T015: the five derived definitions -------------------------------------------------------

#[test]
fn the_five_definitions_take_the_subject_and_are_rules_or_counts() {
    for k_r in [None, Some(1)] {
        let out = build(2, k_r);
        let items = out.module.derived_items();
        for name in DEFINITIONS {
            let d = items.get(name).unwrap_or_else(|| panic!("{name} missing"));
            assert_eq!(d.params().len(), 1, "{name}");
            assert_eq!(d.params()[0].name(), "s", "{name}");
            assert_eq!(
                d.params()[0].ty().to_wire_type(),
                WType::Entity("Document".into()),
                "{name}"
            );
            assert_eq!(d.is_rule(), !name.ends_with("_count"), "{name}");
        }
    }
}

#[test]
fn rejected_is_false_without_a_reject_threshold() {
    let without = build(2, None);
    let with = build(2, Some(1));
    let hash = |o: &behavior_models::BuildOutput, n: &str| *o.module.derived_items()[n].hash();
    // Counts do not depend on the thresholds; the predicates do.
    for n in ["doc_approval__approve_count", "doc_approval__reject_count"] {
        assert_eq!(hash(&without, n), hash(&with, n), "{n}");
    }
    for n in [
        "doc_approval__approved",
        "doc_approval__rejected",
        "doc_approval__pending",
    ] {
        assert_ne!(hash(&without, n), hash(&with, n), "{n}");
    }
    let rejected = &without.module.derived_items()["doc_approval__rejected"];
    assert!(rejected.body().children().is_empty(), "a literal");
}

/// The three outcome predicates are mutually exclusive and exhaustive on every state, valid or
/// not: seeded states with arbitrary counts, including both thresholds reached.
#[test]
fn outcomes_are_exclusive_on_arbitrary_states() {
    let out = build(2, Some(2));
    let counts: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 1), (1, 2), (2, 2), (3, 3)];
    for (a, r) in counts {
        let mut seeds = Vec::new();
        let ids: Vec<String> = (0..a + r).map(|i| format!("x{i}")).collect();
        let users: Vec<String> = (0..a + r).map(|i| format!("u{}", i + 1)).collect();
        for i in 0..a + r {
            let choice = if i < a { "APPROVE" } else { "REJECT" };
            seeds.push((ids[i].as_str(), "d1", users[i].as_str(), choice));
        }
        let store = store(&out.module, &["d1"], &seeds);
        let (ac, rc, outcome) = outcome(&store, &out.module, "d1");
        assert_eq!((ac, rc), (a as i64, r as i64));
        let expected = if r >= 2 {
            "REJECTED"
        } else if a >= 2 {
            "APPROVED"
        } else {
            "PENDING"
        };
        assert_eq!(outcome, expected, "A={a}, R={r}");
    }
}

// --- T016: the decision action layout ---------------------------------------------------------

#[test]
fn decision_actions_have_the_fixed_layout() {
    for (k_r, posts) in [(None, 1), (Some(1), 2)] {
        let out = build(2, k_r);
        for (capability, choice) in [("approve", "APPROVE"), ("reject", "REJECT")] {
            let name = format!("doc_approval__{capability}");
            let a = &out.module.actions()[&name];
            let params: Vec<(&str, Option<Role>)> = a
                .params()
                .iter()
                .map(|p| (p.name(), p.ty().to_wire_type()))
                .map(|(n, _)| (n, None))
                .collect();
            assert_eq!(
                params.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
                vec!["decision_id", "subject", "actor"],
                "{name}"
            );
            // [0] Open [1] pending [2] Eligible [3] no decision yet; no author guards here.
            assert_eq!(a.preconditions().len(), 4, "{name}");
            assert_eq!(a.postconditions().len(), posts, "{name}");
            assert!(a.effects().is_empty() && a.removes().is_empty(), "{name}");
            assert_eq!(a.creates().len(), 1, "{name}");
            let created = &a.creates()[0];
            assert_eq!(created.entity(), "DocApprovalDecision");
            let fields: Vec<&str> = created.fields().iter().map(|(f, _)| f.as_str()).collect();
            assert_eq!(fields, vec!["actor", "choice", "subject"], "{name}");
            let _ = choice;
        }
        // The two actions differ only in their choice literal: same guards and checks.
        let ap = &out.module.actions()["doc_approval__approve"];
        let rj = &out.module.actions()["doc_approval__reject"];
        for i in 0..4 {
            assert_eq!(
                ap.preconditions()[i].expr().hash(),
                rj.preconditions()[i].expr().hash()
            );
        }
        let choice = |a: &behavior_engine::semantic::Module, n: &str| {
            *a.actions()[n].creates()[0]
                .fields()
                .iter()
                .find(|(f, _)| f == "choice")
                .unwrap()
                .1
                .hash()
        };
        assert_ne!(
            choice(&out.module, "doc_approval__approve"),
            choice(&out.module, "doc_approval__reject")
        );
    }
}

#[test]
fn the_choice_enum_and_decision_entity_are_generated() {
    let out = build(1, None);
    let e = &out.module.enums()["DocApprovalDecisionChoice"];
    assert_eq!(e.values, vec!["APPROVE", "REJECT"]);
    let x = &out.module.entities()["DocApprovalDecision"];
    let fields: Vec<(String, WType)> = x
        .fields()
        .iter()
        .map(|(n, t)| (n.clone(), t.to_wire_type()))
        .filter(|(n, _)| n != "id")
        .collect();
    assert_eq!(
        fields,
        vec![
            ("actor".into(), WType::Id("User".into())),
            (
                "choice".into(),
                WType::Enum("DocApprovalDecisionChoice".into())
            ),
            ("subject".into(), WType::Id("Document".into())),
        ]
    );
    assert!(
        x.fields().iter().all(|(n, _)| !x.is_reference(n)),
        "Id, never Ref"
    );
}

// --- The outcome through the store (quickstart 1–4, Rust side) --------------------------------

/// `(capability, actor, expected allowed, expected outcome after)` sequences per threshold.
#[test]
fn thresholds_through_the_store() {
    type Step<'a> = (&'a str, &'a str, bool, &'a str);
    let cases: Vec<((i64, Option<i64>), Vec<Step<'_>>)> = vec![
        (
            (1, None),
            vec![
                ("approve", "u1", true, "APPROVED"),
                ("approve", "u2", false, "APPROVED"),
            ],
        ),
        (
            (2, None),
            vec![
                ("approve", "u1", true, "PENDING"),
                ("reject", "u2", true, "PENDING"),
                ("approve", "u3", true, "APPROVED"),
                ("reject", "u4", false, "APPROVED"),
            ],
        ),
        (
            (2, Some(1)),
            vec![
                ("approve", "u1", true, "PENDING"),
                ("reject", "u2", true, "REJECTED"),
                ("approve", "u3", false, "REJECTED"),
            ],
        ),
        (
            (3, Some(2)),
            vec![
                ("approve", "u1", true, "PENDING"),
                ("reject", "u2", true, "PENDING"),
                ("approve", "u3", true, "PENDING"),
                ("reject", "u4", true, "REJECTED"),
                ("approve", "u5", false, "REJECTED"),
            ],
        ),
        (
            (1, Some(1)),
            vec![
                ("reject", "u1", true, "REJECTED"),
                ("approve", "u2", false, "REJECTED"),
            ],
        ),
    ];
    for ((k_a, k_r), steps) in cases {
        let out = build(k_a, k_r);
        let mut s = store(&out.module, &["d1"], &[]);
        for (i, (capability, actor, allowed, expected)) in steps.into_iter().enumerate() {
            let attempt = decide(
                &mut s,
                &out.module,
                capability,
                &format!("x{i}"),
                "d1",
                actor,
            );
            assert_eq!(
                attempt.allowed, allowed,
                "({k_a},{k_r:?}) step {i}: {}",
                attempt.record
            );
            assert_eq!(
                outcome(&s, &out.module, "d1").2,
                expected,
                "({k_a},{k_r:?}) step {i}"
            );
        }
    }
}

#[test]
fn one_decision_per_actor_and_no_changes() {
    let out = build(2, None);
    let mut s = store(&out.module, &["d1"], &[]);
    assert!(decide(&mut s, &out.module, "approve", "x1", "d1", "u1").allowed);
    // A second approval, or a rejection, by the same actor is DENIED; counts are unchanged.
    assert!(!decide(&mut s, &out.module, "approve", "x2", "d1", "u1").allowed);
    assert!(!decide(&mut s, &out.module, "reject", "x3", "d1", "u1").allowed);
    assert_eq!(outcome(&s, &out.module, "d1"), (1, 0, "PENDING"));
    // The same actor decides independently on another subject.
    let mut s = store(&out.module, &["d1", "d2"], &[]);
    assert!(decide(&mut s, &out.module, "approve", "x1", "d1", "u1").allowed);
    assert!(decide(&mut s, &out.module, "approve", "x2", "d2", "u1").allowed);
}

// --- T034: initializer checks -----------------------------------------------------------------

fn with_payload(
    payload: Vec<(&str, WType)>,
    init: impl Fn(
        &mut behavior_engine::builder::Builder,
    ) -> Vec<(String, behavior_engine::builder::Node)>
    + 'static,
) -> Vec<behavior_models::ModelError> {
    let mut input = approval(1, None);
    input.payload = payload
        .into_iter()
        .map(|(n, t)| (n.to_string(), t))
        .collect();
    let src = Source(Box::new(move |b, s| {
        let mut f = standard(b, s)?;
        f.payload = init(b);
        Ok(f)
    }));
    build_with(|_| {}, vec![spec(input, src)]).unwrap_err()
}

#[test]
fn the_initializer_sets_every_payload_field_exactly_once() {
    let text = |b: &mut behavior_engine::builder::Builder| {
        b.lit(WType::String, serde_json::json!("ok"), loc())
            .unwrap()
    };
    let errs = with_payload(vec![("reason", WType::String)], |_| vec![]);
    assert_eq!(codes(&errs), vec!["INITIALIZER_MISSING_FIELD"; 2]);
    assert!(
        errs[0].to_json()["model_ref"]
            .as_str()
            .unwrap()
            .contains("/capability/")
    );
    for field in ["subject", "actor", "choice", "id", "other"] {
        let errs = with_payload(vec![("reason", WType::String)], move |b| {
            vec![("reason".into(), text(b)), (field.into(), text(b))]
        });
        assert_eq!(
            codes(&errs),
            vec!["INITIALIZER_UNKNOWN_FIELD"; 2],
            "{field}"
        );
        assert_eq!(errs[0].to_json()["field"], field);
    }
    let errs = with_payload(vec![("reason", WType::String)], move |b| {
        vec![("reason".into(), text(b)), ("reason".into(), text(b))]
    });
    assert_eq!(codes(&errs), vec!["INITIALIZER_DUPLICATE_FIELD"; 2]);
    // A value of the wrong type is Core's typing error.
    let errs = with_payload(vec![("reason", WType::String)], |b| {
        vec![(
            "reason".into(),
            b.lit(WType::Int, serde_json::json!(1), loc()).unwrap(),
        )]
    });
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"]);
}

#[test]
fn the_actor_expression_must_be_an_actor_identity() {
    let src = Source(Box::new(|b, s| {
        let mut f = standard(b, s)?;
        f.actor = (
            b.lit(WType::String, serde_json::json!("u1"), loc())
                .unwrap(),
            loc(),
        );
        Ok(f)
    }));
    let errs = build_with(|_| {}, vec![spec(approval(1, None), src)]).unwrap_err();
    assert_eq!(codes(&errs), vec!["FRAGMENT_INVALID"; 2]);
    assert_eq!(errs[0].to_json()["core_code"], "TYPE_MISMATCH");
}
