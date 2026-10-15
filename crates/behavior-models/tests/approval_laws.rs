//! The inductive step of the Approval guarantee (feature 503, T021; FR-008b), as properties
//! through the public engine and store:
//!
//! - **Local preservation.** From any parent `S` with `P(S)`, an allowed decision gives a child
//!   `S′` with `P(S′)`. `P` is the per-subject invariant: no `(s, a)` pair has two decisions, and
//!   never `A ≥ k_A ∧ R ≥ k_R`.
//! - **Frame.** A hand-written action that passes final composition leaves the decisions as
//!   they are: `D_S′ = D_S`.
//!
//! With the base-state rule and complete effect ownership (T032), these give `P` on every
//! reachable state. Terminology: an attempt that is not permitted is DENIED.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::semantic::Module;
use behavior_engine::store::{Backend, InMemoryBackend, Store};
use behavior_engine::wire::{Role, WLifecycle, WParam, WType};
use common::ap::*;
use common::loc;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;
use serde_json::{Value, json};

/// The thresholds every property draws from (spec SC-003).
const THRESHOLDS: [(i64, Option<i64>); 5] = [
    (1, None),
    (2, None),
    (2, Some(1)),
    (3, Some(2)),
    (1, Some(1)),
];
const ACTORS: [&str; 5] = ["u1", "u2", "u3", "u4", "u5"];
const X: &str = "DocApprovalDecision";

/// One decision of a parent: `(subject, actor, choice)`.
type Decision = (&'static str, &'static str, &'static str);

/// `P` on one subject's decisions.
fn holds(decisions: &[Decision], subject: &str, k_a: i64, k_r: Option<i64>) -> bool {
    let mine: Vec<&Decision> = decisions.iter().filter(|d| d.0 == subject).collect();
    let actors: BTreeSet<&str> = mine.iter().map(|d| d.1).collect();
    let a = mine.iter().filter(|d| d.2 == "APPROVE").count() as i64;
    let r = mine.iter().filter(|d| d.2 == "REJECT").count() as i64;
    actors.len() == mine.len() && !k_r.is_some_and(|k_r| a >= k_a && r >= k_r)
}

/// A parent with `P(S)` on both subjects: at most one decision per actor and subject.
fn parent(k_a: i64, k_r: Option<i64>) -> impl Strategy<Value = Vec<Decision>> {
    let choice = prop_oneof![Just(None), Just(Some("APPROVE")), Just(Some("REJECT"))];
    proptest::collection::vec(choice, 8)
        .prop_map(|cs| {
            let mut out = Vec::new();
            for (i, c) in cs.into_iter().enumerate() {
                if let Some(c) = c {
                    out.push((["d1", "d2"][i / 4], ACTORS[i % 4], c));
                }
            }
            out
        })
        .prop_filter("P(S)", move |ds| {
            holds(ds, "d1", k_a, k_r) && holds(ds, "d2", k_a, k_r)
        })
}

fn seeds(decisions: &[Decision]) -> Vec<(String, Decision)> {
    decisions
        .iter()
        .enumerate()
        .map(|(i, d)| (format!("x{i}"), *d))
        .collect()
}

fn seeded_store(module: &Module, decisions: &[Decision]) -> Store<InMemoryBackend> {
    let named = seeds(decisions);
    let rows: Vec<Seed<'_>> = named
        .iter()
        .map(|(id, (s, a, c))| (id.as_str(), *s, *a, *c))
        .collect();
    store(module, &["d1", "d2"], &rows)
}

/// The decision module for threshold pair `i`, built once.
fn decision_module(i: usize) -> &'static Module {
    static MODULES: OnceLock<Vec<Module>> = OnceLock::new();
    &MODULES.get_or_init(|| {
        THRESHOLDS
            .iter()
            .map(|(a, r)| build(*a, *r).module)
            .collect()
    })[i]
}

/// `(A, R)` of `subject` at the head, from the stored decisions (no evaluation involved).
fn counts(store: &Store<InMemoryBackend>, subject: &str) -> (i64, i64) {
    let head = store.current().unwrap();
    let mut out = (0, 0);
    for key in store.backend().keys_at(X, head.position).unwrap() {
        let v = store.load(&key, &head).unwrap().value;
        if v["subject"] == subject {
            match v["choice"].as_str() {
                Some("APPROVE") => out.0 += 1,
                _ => out.1 += 1,
            }
        }
    }
    out
}

/// The decision a committed record created: `(subject, actor, choice)`.
fn created(record: &Value) -> (String, String, String) {
    let lifecycle = record["outcome"]["record"]["lifecycle"].as_array().unwrap();
    assert_eq!(lifecycle.len(), 1, "exactly one creation: {record}");
    let l = &lifecycle[0];
    assert_eq!(l["entity"], X, "{record}");
    let v = &l["value"];
    let s = |k: &str| v[k].as_str().unwrap().to_string();
    (s("subject"), s("actor"), s("choice"))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: common::property_cases(),
        rng_seed: RngSeed::Fixed(503),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// `ALLOW ⇒ P(S′)`: from a valid parent, an allowed decision keeps `P` on its subject, and the
    /// other subject is untouched.
    #[test]
    fn an_allowed_decision_preserves_the_invariant(
        (i, decisions) in (0..THRESHOLDS.len()).prop_flat_map(|i| {
            let (k_a, k_r) = THRESHOLDS[i];
            (Just(i), parent(k_a, k_r))
        }),
        capability in prop_oneof![Just("approve"), Just("reject")],
        actor in proptest::sample::select(ACTORS.to_vec()),
        subject in prop_oneof![Just("d1"), Just("d2")],
    ) {
        let (k_a, k_r) = THRESHOLDS[i];
        let module = decision_module(i);
        let mut s = seeded_store(module, &decisions);
        let before: BTreeMap<&str, (i64, i64)> =
            ["d1", "d2"].iter().map(|d| (*d, counts(&s, d))).collect();
        let attempt = decide(&mut s, module, capability, "new", subject, actor);
        if attempt.allowed {
            let (cs, ca, cc) = created(&attempt.record);
            prop_assert_eq!((cs.as_str(), ca.as_str()), (subject, actor));
            let choice = if capability == "approve" { "APPROVE" } else { "REJECT" };
            prop_assert_eq!(cc.as_str(), choice);
            let mut child = decisions.clone();
            child.push((subject, actor, choice));
            prop_assert!(holds(&child, subject, k_a, k_r), "P(S′) fails: {:?}", child);
            // The store agrees with the child computed from the record.
            let (a0, r0) = before[subject];
            let step = if choice == "APPROVE" { (a0 + 1, r0) } else { (a0, r0 + 1) };
            prop_assert_eq!(counts(&s, subject), step);
            let other = if subject == "d1" { "d2" } else { "d1" };
            prop_assert_eq!(counts(&s, other), before[other]);
        } else {
            for d in ["d1", "d2"] {
                prop_assert_eq!(counts(&s, d), before[d]);
            }
        }
    }
}

// --- Frame: hand-written actions ------------------------------------------------------------

fn param(name: &str, role: Role, ty: WType) -> WParam {
    WParam {
        name: name.into(),
        role: Some(role),
        ty,
    }
}

fn doc() -> WParam {
    param("doc", Role::State, WType::Entity("Document".into()))
}

fn title() -> WParam {
    param("title", Role::Input, WType::String)
}

/// Hand-written actions that do not touch `X` (reads and guards over the approval are allowed):
/// - `retitle(doc, title)`;
/// - `retitle_if_approved(doc, title)`: guarded by `doc_approval__approved(doc)`;
/// - `count_guarded_retitle(doc, title)`: guarded by a query over `X`;
/// - `add_document(id)` and `remove_document(doc)`: the subject's lifecycle;
/// - `rename_user(user, name)`: the actor's entity.
fn hand_written(b: &mut Builder) {
    let set_title = |b: &mut Builder, name: &str, guard: Option<&dyn Fn(&mut Builder) -> _>| {
        let ps = vec![doc(), title()];
        b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
        let pre = guard.map(|g| vec![(g(b), loc())]).unwrap_or_default();
        let t = b.param("title", loc()).unwrap();
        b.add_action(
            name,
            ps,
            pre,
            vec![("doc".into(), "title".into(), t, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    };
    set_title(b, "retitle", None);
    set_title(
        b,
        "retitle_if_approved",
        Some(&|b: &mut Builder| {
            b.derived_ref("doc_approval__approved", vec!["doc".into()], loc())
                .unwrap()
        }),
    );
    set_title(
        b,
        "count_guarded_retitle",
        Some(&|b: &mut Builder| {
            let all = b.select(X, loc()).unwrap();
            let n = b.op("count", vec![all], loc()).unwrap();
            let ten = b.lit(WType::Int, json!(10), loc()).unwrap();
            b.op("lt", vec![n, ten], loc()).unwrap()
        }),
    );

    let ps = vec![param("id", Role::Input, WType::Id("Document".into()))];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let id = b.param("id", loc()).unwrap();
    let empty = b.lit(WType::String, json!("new"), loc()).unwrap();
    let create = WLifecycle::Create {
        entity: "Document".into(),
        id: id.into_wire(),
        fields: vec![("title".into(), empty.into_wire())],
        loc: loc(),
    };
    b.add_action(
        "add_document",
        ps,
        vec![],
        vec![],
        vec![create],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();

    let ps = vec![doc()];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let remove = WLifecycle::Remove {
        param: "doc".into(),
        loc: loc(),
    };
    b.add_action(
        "remove_document",
        ps,
        vec![],
        vec![],
        vec![remove],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();

    let ps = vec![
        param("user", Role::State, WType::Entity("User".into())),
        param("name", Role::Input, WType::String),
    ];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let n = b.param("name", loc()).unwrap();
    b.add_action(
        "rename_user",
        ps,
        vec![],
        vec![("user".into(), "name".into(), n, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// The module with the approval and the hand-written actions, for threshold pair `i`.
fn composed_module(i: usize) -> &'static Module {
    static MODULES: OnceLock<Vec<Module>> = OnceLock::new();
    &MODULES.get_or_init(|| {
        THRESHOLDS
            .iter()
            .map(|(a, r)| {
                build_with(hand_written, vec![spec(approval(*a, *r), source())])
                    .unwrap_or_else(|e| panic!("{e:#?}"))
                    .module
            })
            .collect()
    })[i]
}

/// Every decision in the store at the head, as its canonical value, read from the backend's
/// type index and loaded at the head (no evaluation involved).
fn decisions_at_head(store: &Store<InMemoryBackend>, _module: &Module) -> BTreeSet<String> {
    let head = store.current().unwrap();
    store
        .backend()
        .keys_at(X, head.position)
        .unwrap()
        .iter()
        .map(|key| store.load(key, &head).unwrap().value.to_string())
        .collect()
}

/// Invokes hand-written action `capability` and commits it if allowed. Returns the decision
/// record.
fn run(
    store: &mut Store<InMemoryBackend>,
    module: &Module,
    capability: &str,
    bindings: Value,
    input: Value,
) -> (bool, Value) {
    let document = json!({
        "format": "behavior.invocation.v1",
        "capability": capability,
        "bindings": bindings,
        "input": input,
        "context": {},
    });
    let (requested, errors) =
        behavior_engine::invocation::RequestedInvocation::decode(&document.to_string()).unwrap();
    let requested = requested.unwrap_or_else(|| panic!("{errors:?}"));
    let inv = store.invoke(module, &requested, T, None, None).unwrap();
    let record = inv.record.as_json()["outcome"]["record"].clone();
    match inv.bundle {
        Some(bundle) => {
            let head = store.current().unwrap();
            store.commit(module, &head, &bundle).unwrap();
            (true, record)
        }
        None => (false, record),
    }
}

/// `(capability, bindings, input)` of a random hand-written invocation.
fn hand_written_call() -> impl Strategy<Value = (&'static str, Value, Value)> {
    let doc = prop_oneof![Just("d1"), Just("d2")];
    let user = proptest::sample::select(ACTORS.to_vec());
    prop_oneof![
        (doc.clone(), "[a-z]{0,6}").prop_map(|(d, t)| (
            "retitle",
            json!({"doc": {"entity": "Document", "id": d}}),
            json!({"title": t})
        )),
        (doc.clone(), "[a-z]{0,6}").prop_map(|(d, t)| (
            "retitle_if_approved",
            json!({"doc": {"entity": "Document", "id": d}}),
            json!({"title": t})
        )),
        (doc.clone(), "[a-z]{0,6}").prop_map(|(d, t)| (
            "count_guarded_retitle",
            json!({"doc": {"entity": "Document", "id": d}}),
            json!({"title": t})
        )),
        Just(("add_document", json!({}), json!({"id": "d9"}))),
        doc.prop_map(|d| (
            "remove_document",
            json!({"doc": {"entity": "Document", "id": d}}),
            json!({})
        )),
        (user, "[a-z]{1,6}").prop_map(|(u, n)| (
            "rename_user",
            json!({"user": {"entity": "User", "id": u}}),
            json!({"name": n})
        )),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: common::property_cases(),
        rng_seed: RngSeed::Fixed(5030),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// `D_S′ = D_S`: a hand-written action that passes final composition never changes a
    /// decision, whatever it does to the subject, the actor or anything else.
    #[test]
    fn hand_written_actions_leave_the_decisions_unchanged(
        (i, decisions) in (0..THRESHOLDS.len()).prop_flat_map(|i| {
            let (k_a, k_r) = THRESHOLDS[i];
            (Just(i), parent(k_a, k_r))
        }),
        (capability, bindings, input) in hand_written_call(),
    ) {
        let module = composed_module(i);
        let mut s = seeded_store(module, &decisions);
        let before = decisions_at_head(&s, module);
        prop_assert_eq!(before.len(), decisions.len());
        let (allowed, record) = run(&mut s, module, capability, bindings, input);
        if allowed {
            let touches_x = record["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(record["lifecycle"].as_array().into_iter().flatten())
                .any(|c| c["entity"] == X);
            prop_assert!(!touches_x, "{}", record);
        }
        prop_assert_eq!(decisions_at_head(&s, module), before);
    }
}
