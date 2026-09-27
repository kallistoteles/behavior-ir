#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-001: generated interleavings of evaluate/commit from several simulated hosts never commit a
//! decision on a state other than the one it was evaluated against, and never expose a partial
//! commit: the committed history replays to the store's content.

mod common;

use std::collections::BTreeMap;

use behavior_store::Backend as _;
use behavior_store::documents::{CommitBundle, EntityKey, StoreError};
use common::*;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Step {
    /// Host h evaluates a transfer between accounts (from, to) of `cents`.
    Transfer(usize, usize, usize, u32),
    /// Host h evaluates a freeze or touch.
    Unary(usize, bool, usize),
    /// Host h commits its pending bundle (if any), possibly with a new commit time.
    Commit(usize, bool),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0usize..4, 0usize..3, 0usize..3, 1u32..6000)
            .prop_map(|(h, f, t, c)| Step::Transfer(h, f, t, c)),
        (0usize..4, any::<bool>(), 0usize..3).prop_map(|(h, fz, a)| Step::Unary(h, fz, a)),
        (0usize..4, any::<bool>()).prop_map(|(h, r)| Step::Commit(h, r)),
    ]
}

const IDS: [&str; 3] = ["a1", "a2", "a3"];

fn run(steps: &[Step]) -> Result<(), TestCaseError> {
    let mut s = store();
    let mut pending: BTreeMap<usize, CommitBundle> = BTreeMap::new();
    let mut committed: BTreeMap<String, u64> = BTreeMap::new(); // transition hash -> position
    for st in steps {
        match st {
            Step::Transfer(h, f, t, c) if f != t => {
                let amount = format!("{}.{:02}", c / 100, c % 100);
                if let Some(b) = transfer(&s, IDS[*f], IDS[*t], &amount, T0).bundle {
                    pending.insert(*h, b);
                }
            }
            Step::Transfer(..) => {}
            Step::Unary(h, freeze, a) => {
                let b = unary(&s, if *freeze { "freeze" } else { "touch" }, IDS[*a]).bundle;
                pending.insert(*h, b.unwrap());
            }
            Step::Commit(h, retime) => {
                let Some(mut b) = pending.get(h).cloned() else {
                    continue;
                };
                if *retime {
                    b.commit_time = "2026-09-27T13:00:00Z".into();
                }
                let head_before = s.current().unwrap();
                match s.commit(&ledger(), &b.evaluated_state.clone(), &b) {
                    Ok(c) if c.already => {
                        prop_assert_eq!(
                            committed.get(&b.transition_hash),
                            Some(&c.result_state.position)
                        );
                    }
                    Ok(c) => {
                        prop_assert_eq!(&b.evaluated_state, &head_before);
                        prop_assert_eq!(c.result_state.position, head_before.position + 1);
                        committed.insert(b.transition_hash.clone(), c.result_state.position);
                    }
                    Err(StoreError::StateConflict { current, .. }) => {
                        prop_assert_ne!(&b.evaluated_state, &head_before);
                        prop_assert_eq!(current, head_before.clone());
                        prop_assert_eq!(s.current().unwrap(), head_before);
                        pending.remove(h);
                    }
                    Err(e) => return Err(TestCaseError::fail(format!("{e}"))),
                }
            }
        }
    }
    // Every record was committed on the state it was evaluated against, chained in order, and the
    // store's content equals the replay of the committed write sets.
    let head = s.current().unwrap();
    let mut prev = s.state_at(0).unwrap();
    let mut balances: BTreeMap<String, serde_json::Value> = IDS
        .iter()
        .map(|id| (id.to_string(), balance(&s, id, &prev)))
        .collect();
    for pos in 1..=head.position {
        let r = s.backend().record(pos).unwrap().unwrap();
        prop_assert_eq!(&r.evaluated_against, &r.committed_on);
        prop_assert_eq!(&r.committed_on, &prev);
        for w in &r.bundle.write_set {
            if w.field == "balance" {
                balances.insert(w.id.clone(), w.new.clone());
            }
        }
        prev = r.result_state.clone();
    }
    prop_assert_eq!(&prev, &head);
    for id in IDS {
        let key = EntityKey {
            entity: "Account".into(),
            id: id.into(),
        };
        prop_assert_eq!(
            &s.load(&key, &head).unwrap().value["balance"],
            &balances[id]
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    #[test]
    fn interleavings_commit_only_on_the_evaluated_state(steps in prop::collection::vec(step(), 1..25)) {
        run(&steps)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]
    /// SC-001 at full size (release: `cargo test --release -p behavior-store --test concurrency_props -- --ignored`).
    #[test]
    #[ignore]
    fn ten_thousand_interleavings(steps in prop::collection::vec(step(), 1..25)) {
        run(&steps)?;
    }
}
