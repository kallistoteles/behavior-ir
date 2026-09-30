#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-004 (feature 007): a query result the engine derives for the resulting state equals a full
//! re-evaluation of the query over the committed state, across creations, removals, bound field
//! changes and changed captures.

mod common;

use std::collections::BTreeMap;

use common::*;
use proptest::prelude::*;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
enum Step {
    /// `place_order_unchecked` for customer c(i) with an amount in cents (a creation).
    Place(usize, u32),
    /// `remove_cheapest` for customer c(i) (a removal).
    RemoveCheapest(usize),
    /// `raise_limit` of customer c(i) to a limit in cents (a bound change and a changed capture).
    Raise(usize, u32),
    /// `renumber` of employee e(i) to number N(j) (a bound candidate re-classified).
    Renumber(usize, usize),
    /// `hire_unchecked` of a new employee with number N(j).
    Hire(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0usize..2, 0u32..6000).prop_map(|(c, a)| Step::Place(c, a)),
        2 => (0usize..2).prop_map(Step::RemoveCheapest),
        2 => (0usize..2, 0u32..20_000).prop_map(|(c, l)| Step::Raise(c, l)),
        1 => (0usize..3, 0usize..4).prop_map(|(e, n)| Step::Renumber(e, n)),
        1 => (0usize..4).prop_map(Step::Hire),
    ]
}

fn money(cents: u32) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

/// The oracle: the committed state, re-evaluated in full.
#[derive(Default)]
struct Model {
    orders: BTreeMap<String, (String, u32)>,
    limits: BTreeMap<String, u32>,
    employees: BTreeMap<String, String>,
}

impl Model {
    fn orders_of(&self, c: &str) -> Vec<(String, u32)> {
        self.orders
            .iter()
            .filter(|(_, (oc, _))| oc == c)
            .map(|(id, (_, a))| (id.clone(), *a))
            .collect()
    }
}

/// The value a trace step read for the expression starting with `prefix`.
fn read(rec: &Value, phase: &str, prefix: &str) -> Value {
    let step = rec["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["phase"] == phase
                && s["reads"]
                    .as_object()
                    .is_some_and(|r| r.keys().any(|k| k.starts_with(prefix)))
        })
        .unwrap_or_else(|| panic!("no {phase} read of {prefix}: {rec}"));
    let reads = step["reads"].as_object().unwrap();
    reads
        .iter()
        .find(|(k, _)| k.starts_with(prefix))
        .unwrap()
        .1
        .clone()
}

fn run(steps: &[Step]) -> Result<(), TestCaseError> {
    let mut s = orders_store_with(
        behavior_store::InMemoryBackend::new(),
        vec![
            customer_seed("c0", "100.00"),
            customer_seed("c1", "100.00"),
            order_seed("o_a", "c0", "30.00", "open"),
            employee_seed("e0", "N0"),
            employee_seed("e1", "N1"),
        ],
    );
    let mut m = Model::default();
    m.limits.insert("c0".into(), 10_000);
    m.limits.insert("c1".into(), 10_000);
    m.orders.insert("o_a".into(), ("c0".into(), 3000));
    m.employees.insert("e0".into(), "N0".into());
    m.employees.insert("e1".into(), "N1".into());
    let mut next = 0usize;
    for st in steps {
        match st {
            Step::Place(ci, cents) => {
                let c = format!("c{ci}");
                let id = format!("o{next}");
                next += 1;
                let e = run_orders(
                    &s,
                    "place_order_unchecked",
                    &[("customer", &c)],
                    json!({"order_id": id, "amount": money(*cents)}),
                );
                let total: u32 = m.orders_of(&c).iter().map(|(_, a)| a).sum::<u32>() + cents;
                prop_assert_eq!(
                    read(&e.record, "postcondition", "sum("),
                    json!(money(total))
                );
                let allow = total <= m.limits[&c];
                prop_assert_eq!(e.record["result"] == "ALLOW", allow, "{}", e.record);
                if allow {
                    commit_orders(&mut s, &e);
                    m.orders.insert(id, (c, *cents));
                }
            }
            Step::RemoveCheapest(ci) => {
                let c = format!("c{ci}");
                let Some((id, _)) = m
                    .orders_of(&c)
                    .into_iter()
                    .min_by_key(|(id, a)| (*a, id.clone()))
                else {
                    continue;
                };
                let e = run_orders(
                    &s,
                    "remove_cheapest",
                    &[("order", &id), ("customer", &c)],
                    json!({}),
                );
                prop_assert_eq!(&e.record["result"], "ALLOW", "{}", e.record);
                let rest: Option<u32> = m
                    .orders_of(&c)
                    .iter()
                    .filter(|(o, _)| *o != id)
                    .map(|(_, a)| *a)
                    .min();
                let got = read(&e.record, "postcondition", "min(");
                let want = rest.map_or(Value::Null, |a| json!(money(a)));
                prop_assert_eq!(got, want, "{}", e.record);
                commit_orders(&mut s, &e);
                m.orders.remove(&id);
            }
            Step::Raise(ci, cents) => {
                let c = format!("c{ci}");
                let e = run_orders(
                    &s,
                    "raise_limit",
                    &[("customer", &c)],
                    json!({"limit": money(*cents)}),
                );
                let above = |l: u32| m.orders.values().filter(|(_, a)| *a > l).count();
                let old = m.limits[&c];
                let allow = *cents >= old && above(old) == 0;
                prop_assert_eq!(e.record["result"] == "ALLOW", allow, "{}", e.record);
                if allow {
                    prop_assert_eq!(
                        read(&e.record, "postcondition", "count("),
                        json!(above(*cents))
                    );
                    commit_orders(&mut s, &e);
                    m.limits.insert(c, *cents);
                }
            }
            Step::Renumber(ei, ni) => {
                let emp = format!("e{ei}");
                if !m.employees.contains_key(&emp) {
                    continue;
                }
                let number = format!("N{ni}");
                let e = run_orders(
                    &s,
                    "renumber",
                    &[("employee", &emp)],
                    json!({"number": number}),
                );
                let clash = m.employees.iter().any(|(k, v)| *k != emp && *v == number);
                prop_assert_eq!(e.record["result"] == "ALLOW", !clash, "{}", e.record);
                if !clash {
                    commit_orders(&mut s, &e);
                    m.employees.insert(emp, number);
                }
            }
            Step::Hire(ni) => {
                let emp = format!("e{}", m.employees.len());
                let number = format!("N{ni}");
                let e = run_orders(
                    &s,
                    "hire_unchecked",
                    &[],
                    json!({"employee_id": emp, "number": number}),
                );
                let clash = m.employees.values().any(|v| *v == number);
                prop_assert_eq!(e.record["result"] == "ALLOW", !clash, "{}", e.record);
                if !clash {
                    commit_orders(&mut s, &e);
                    m.employees.insert(emp, number);
                }
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn derived_resulting_state_queries_match_full_reevaluation(steps in prop::collection::vec(step(), 1..24)) {
        run(&steps)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]
    #[test]
    #[ignore = "release-mode SC-004 variant"]
    fn derived_resulting_state_queries_match_full_reevaluation_10k(steps in prop::collection::vec(step(), 1..24)) {
        run(&steps)?;
    }
}
