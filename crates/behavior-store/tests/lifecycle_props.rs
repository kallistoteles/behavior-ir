#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-003 and SC-007 (feature 006): colliding creations from several hosts never create one
//! identity twice, every commit lands on the state it was evaluated against, and removed
//! identities are never created again.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use behavior_store::Backend as _;
use behavior_store::documents::CommitBundle;
use common::*;
use proptest::prelude::*;
use serde_json::json;

#[derive(Debug, Clone)]
enum Step {
    /// Host h evaluates the creation of account `id` (from a small pool, so hosts collide).
    Open(usize, usize),
    /// Host h evaluates closing account `id`.
    Close(usize, usize),
    /// Host h commits its pending bundle, if any.
    Commit(usize),
}

const POOL: [&str; 3] = ["n1", "n2", "n3"];

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0usize..4, 0usize..3).prop_map(|(h, i)| Step::Open(h, i)),
        (0usize..4, 0usize..3).prop_map(|(h, i)| Step::Close(h, i)),
        (0usize..4).prop_map(Step::Commit),
    ]
}

fn run(steps: &[Step]) -> Result<(), TestCaseError> {
    let mut s = accounts_store();
    let mut pending: BTreeMap<usize, CommitBundle> = BTreeMap::new();
    let mut created: BTreeSet<String> = BTreeSet::new();
    let mut removed: BTreeSet<String> = BTreeSet::new();
    for st in steps {
        match st {
            Step::Open(h, i) => {
                let e = act(
                    &s,
                    "open_account",
                    &[("owner", "c1")],
                    json!({"account_id": POOL[*i], "initial": "0.00"}),
                );
                // SC-007: an identity once created is never created again, even after removal.
                if created.contains(POOL[*i]) {
                    prop_assert_eq!(e.record["result"].as_str(), Some("ENTITY_ID_ALREADY_USED"));
                }
                if let Some(b) = e.bundle {
                    pending.insert(*h, b);
                }
            }
            Step::Close(h, i) => {
                if created.contains(POOL[*i]) && !removed.contains(POOL[*i]) {
                    let e = act(&s, "close_account", &[("account", POOL[*i])], json!({}));
                    if let Some(b) = e.bundle {
                        pending.insert(*h, b);
                    }
                }
            }
            Step::Commit(h) => {
                let Some(b) = pending.remove(h) else { continue };
                match s.commit(&accounts(), &b.evaluated_state.clone(), &b) {
                    Ok(c) if !c.already => {
                        let rec = s
                            .backend()
                            .record(c.result_state.position)
                            .unwrap()
                            .unwrap();
                        prop_assert_eq!(&rec.evaluated_against, &rec.committed_on);
                        for v in &rec.created {
                            prop_assert!(created.insert(v.id.clone()), "{} created twice", v.id);
                        }
                        for r in &rec.removed {
                            prop_assert!(removed.insert(r.id.clone()));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => prop_assert_eq!(e.code(), "STATE_CONFLICT", "{}", e),
                }
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn colliding_creations_never_create_an_identity_twice(steps in prop::collection::vec(step(), 1..40)) {
        run(&steps)?;
    }
}

#[test]
#[ignore = "10,000 cases; run with --release --ignored"]
fn colliding_creations_at_scale() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(10_000));
    runner
        .run(&prop::collection::vec(step(), 1..40), |steps| run(&steps))
        .unwrap();
}
