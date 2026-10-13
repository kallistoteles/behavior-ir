//! Lowering laws as properties (lowering contract §2; T045, T060). Fixed seed: the gate is
//! deterministic (constitution IV).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::wire::WType;
use behavior_models::{BuildOutput, DisplayMeta, ModelInput, SourceLoc};
use common::*;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 1000,
        rng_seed: RngSeed::Fixed(501),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// The reference machine with an extra multi-source transition and a transition with inputs
/// and context, so every unordered collection has more than one element.
fn rich() -> ModelInput {
    let mut m = machine();
    m.entry_states = vec!["Draft".into(), "Submitted".into()];
    m.transitions.push(transition(
        "withdraw",
        &["Draft", "Submitted", "InReview"],
        "Rejected",
    ));
    let mut annotate = transition("annotate", &["Rejected"], "Draft");
    annotate.inputs = vec![
        ("reason".into(), WType::String),
        ("amount".into(), WType::Int),
        ("urgent".into(), WType::Bool),
    ];
    annotate.context = vec![
        ("operator".into(), WType::Id("Actor".into())),
        ("deputy".into(), WType::Id("Actor".into())),
    ];
    m.transitions.push(annotate);
    m
}

fn identity(out: &BuildOutput) -> (String, String) {
    (
        out.receipts[0].0["model_hash"]
            .as_str()
            .unwrap()
            .to_string(),
        out.module.behavior_version(),
    )
}

fn reference_identity() -> (String, String) {
    identity(&build(|_| {}, vec![spec(rich())]).unwrap())
}

fn permuted(order: &[usize], v: &[String]) -> Vec<String> {
    order.iter().map(|&i| v[i].clone()).collect()
}

fn shuffled(n: usize) -> impl Strategy<Value = Vec<usize>> {
    Just((0..n).collect::<Vec<_>>()).prop_shuffle()
}

proptest! {
    #![proptest_config(config())]

    /// L3: permuting entry states, source sets, transitions, inputs and context leaves ModelHash,
    /// the lowered module and BehaviorHash unchanged (spec FR-006, FR-027).
    #[test]
    fn order_independence(
        entry in shuffled(2),
        transitions in shuffled(6),
        sources in shuffled(3),
        inputs in shuffled(3),
        context in shuffled(2),
        duplicate in any::<bool>(),
    ) {
        let base = rich();
        let mut m = base.clone();
        m.entry_states = permuted(&entry, &base.entry_states);
        if duplicate {
            m.entry_states.push(base.entry_states[0].clone());
        }
        m.transitions = transitions.iter().map(|&i| base.transitions[i].clone()).collect();
        for t in &mut m.transitions {
            if t.transition_id == "withdraw" {
                t.from_states = permuted(&sources, &t.from_states);
            }
            if t.transition_id == "annotate" {
                t.inputs = inputs.iter().map(|&i| t.inputs[i].clone()).collect();
                t.context = context.iter().map(|&i| t.context[i].clone()).collect();
            }
        }
        let out = build(|_| {}, vec![spec(m)]).unwrap();
        prop_assert_eq!(identity(&out), reference_identity());
        // Generated parameter order is canonical: subject, inputs and context sorted by name.
        let a = &out.module.actions()["purchase_request__annotate"];
        let names: Vec<&str> = a.params().iter().map(|p| p.name()).collect();
        prop_assert_eq!(
            names,
            vec!["purchase_request", "amount", "reason", "urgent", "deputy", "operator"]
        );
    }

    /// L9: display names, descriptions and source locations never change identity.
    #[test]
    fn metadata_neutrality(name in "[A-Za-z ]{0,12}", line in 1u64..10_000, file in "[a-z]{1,8}") {
        let mut m = rich();
        m.display = DisplayMeta { display_name: Some(name.clone()), description: Some(name.clone()) };
        m.source = Some(SourceLoc { file: format!("{file}.py"), line });
        for t in &mut m.transitions {
            t.display.description = Some(name.clone());
            t.source = Some(SourceLoc { file: format!("{file}/t.py"), line });
        }
        prop_assert_eq!(identity(&build(|_| {}, vec![spec(m)]).unwrap()), reference_identity());
    }
}

/// L7 and SC-003: every semantic change changes ModelHash. Equal ModelHash ⇒ equal BehaviorHash
/// is exercised by the properties above (all equal pairs) and by this table (all distinct).
#[test]
fn every_semantic_change_changes_the_model_hash() {
    type Change = Box<dyn Fn(&mut ModelInput)>;
    let changes: Vec<(&str, Change)> = vec![
        (
            "entry state",
            Box::new(|m| m.entry_states = vec!["Draft".into()]),
        ),
        (
            "transition target",
            Box::new(|m| m.transitions[0].to_state = "InReview".into()),
        ),
        (
            "transition sources",
            Box::new(|m| m.transitions[0].from_states.push("Rejected".into())),
        ),
        (
            "transition id",
            Box::new(|m| m.transitions[1].transition_id = "begin_review".into()),
        ),
        (
            "extra transition",
            Box::new(|m| {
                m.transitions
                    .push(transition("reopen", &["Rejected"], "Draft"))
            }),
        ),
        ("model id", Box::new(|m| m.model_id = "procurement".into())),
        (
            "subject param",
            Box::new(|m| m.subject_param = Some("request".into())),
        ),
        (
            "input type",
            Box::new(|m| {
                let t = m
                    .transitions
                    .iter_mut()
                    .find(|t| t.transition_id == "annotate")
                    .unwrap();
                t.inputs[0].1 = WType::Int;
            }),
        ),
        ("owned field", Box::new(|m| m.state_field = "review".into())),
    ];
    let reference = reference_identity().0;
    let mut seen = std::collections::BTreeSet::from([reference.clone()]);
    for (what, change) in changes {
        let mut m = rich();
        change(&mut m);
        let out = build(|_| {}, vec![spec(m)]).unwrap();
        let h = identity(&out).0;
        assert!(seen.insert(h), "{what} did not change ModelHash");
    }
}

/// Changing a guard (a Core fragment) changes ModelHash; rewriting it in another but
/// semantically identical way does not (fragment identity is Core's semantic hash).
#[test]
fn fragment_identity_is_semantic() {
    use behavior_engine::builder::Builder;
    use behavior_models::state_machine::input::FragmentTable;
    use behavior_models::{ModelSpec, TransitionFragments};
    use serde_json::json;
    use std::collections::BTreeMap;

    let with_guard = |flip: bool, rename_locals: bool| {
        let mut t: FragmentTable<'static> = BTreeMap::new();
        t.insert(
            "submit".into(),
            Box::new(
                move |b: &mut Builder, s: &behavior_models::TransitionScope| {
                    // The scope's names are the only names a fragment can use; a binding's own
                    // variable names never reach the builder.
                    let subject_name = if rename_locals {
                        s.params[0].name.clone()
                    } else {
                        subject(s)
                    };
                    let note = b.field(&subject_name, "note", loc()).unwrap();
                    let empty = b.lit(WType::String, json!(""), loc()).unwrap();
                    let op = if flip { "ne" } else { "eq" };
                    let g = b.op(op, vec![note, empty], loc()).unwrap();
                    Ok(TransitionFragments {
                        guards: vec![(g, loc())],
                        ..TransitionFragments::default()
                    })
                },
            ),
        );
        let out = build(
            |_| {},
            vec![ModelSpec {
                input: machine(),
                fragments: Box::new(t),
            }],
        )
        .unwrap();
        identity(&out)
    };
    assert_eq!(with_guard(false, false), with_guard(false, true));
    assert_ne!(with_guard(false, false).0, with_guard(true, false).0);
}

/// L3 for composition (T060): the order in which models are supplied and in which hand-written
/// behavior was traced changes nothing: not the module, not BehaviorHash, not the error list.
mod composition_order {
    use super::*;
    use behavior_engine::builder::Builder;
    use behavior_models::ModelSpec;
    use serde_json::json;

    fn review() -> ModelInput {
        let mut m = machine();
        m.model_id = "review_flow".into();
        m.state_field = "review".into();
        m.transitions = vec![transition("open", &["Draft"], "InReview")];
        m
    }

    fn models(include_conflict: bool) -> Vec<ModelSpec<'static>> {
        let mut v = vec![
            spec(machine()),
            spec_without_fragments(invoice_machine()),
            spec_without_fragments(review()),
        ];
        if include_conflict {
            let mut other = machine();
            other.model_id = "another".into();
            v.push(spec(other));
        }
        v
    }

    fn hand_written(b: &mut Builder, order: &[usize]) {
        with_invoice(b);
        for &i in order {
            hand_written_set(
                b,
                &format!("note_{i}"),
                "note",
                json!(format!("{i}")),
                WType::String,
            );
        }
    }

    fn outcome(
        models_order: &[usize],
        trace_order: &[usize],
        conflict: bool,
    ) -> Result<(Vec<String>, String), Vec<String>> {
        let all = models(conflict);
        let mut slots: Vec<Option<ModelSpec<'static>>> = all.into_iter().map(Some).collect();
        let supplied: Vec<ModelSpec<'static>> = models_order
            .iter()
            .filter_map(|&i| slots.get_mut(i).and_then(Option::take))
            .collect();
        let trace = trace_order.to_vec();
        match build(move |b| hand_written(b, &trace), supplied) {
            Ok(out) => Ok((
                out.receipts
                    .iter()
                    .map(|r| r.0["model_hash"].as_str().unwrap().to_string())
                    .collect(),
                out.module.behavior_version(),
            )),
            Err(errs) => Err(errs.iter().map(|e| e.to_json().to_string()).collect()),
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn supply_and_trace_order_do_not_matter(
            models_order in shuffled(3),
            conflict_order in shuffled(4),
            trace_order in shuffled(3),
        ) {
            let ok = outcome(&models_order, &trace_order, false);
            prop_assert!(ok.is_ok());
            prop_assert_eq!(ok, outcome(&[0, 1, 2], &[0, 1, 2], false));
            let failed = outcome(&conflict_order, &trace_order, true);
            prop_assert!(failed.is_err());
            prop_assert_eq!(failed, outcome(&[0, 1, 2, 3], &[0, 1, 2], true));
        }
    }
}
