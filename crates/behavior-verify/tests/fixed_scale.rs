#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3 (feature 003): fixed-scale money is verified exactly — values on their grid, exact
//! quantities as rationals, rescale as its rounding function.

mod common;

use serde_json::Value;

const FIXTURES: [&str; 6] = [
    "purchase_money2",
    "purchase_money2_remaining",
    "purchase_money2_fixed",
    "invoice_money2",
    "discount_rescale",
    "rescale_modes",
];

#[test]
fn outcomes_match_expectations() {
    let mut failures = Vec::new();
    for name in FIXTURES {
        let (a, expected) = common::run(name);
        let got = common::outcomes(&a);
        let want = common::expected_outcomes(&expected);
        if got != want {
            failures.push(format!(
                "{name}\n  expected {}\n  got      {}",
                Value::Array(want),
                Value::Array(got)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Every fixed-scale value in a counterexample has exactly two fractional digits.
fn on_grid(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.values().all(on_grid),
        Value::String(s) if s.parse::<f64>().is_ok() && s.contains('.') => {
            s.split('.').nth(1).is_some_and(|f| f.len() == 2)
        }
        _ => true,
    }
}

#[test]
fn counterexamples_are_on_the_grid_and_confirmed() {
    for name in FIXTURES {
        let (a, _) = common::run(name);
        for f in a.findings() {
            let Some(cx) = f.get("counterexample") else {
                continue;
            };
            let record = &cx["record"];
            assert!(
                ["DENY", "ERROR"].contains(&record["result"].as_str().unwrap()),
                "{name}: {f}"
            );
            assert_eq!(record["record_version"], "0.4", "{name}");
            for section in ["state", "input", "context"] {
                // Money values are on the grid; general decimals (e.g. `rate`) are not checked.
                if section == "input" && name == "discount_rescale" {
                    continue;
                }
                assert!(on_grid(&cx[section]), "{name}: {section} {}", cx[section]);
            }
        }
    }
}

#[test]
fn money_arithmetic_leaves_nothing_inconclusive() {
    for name in ["purchase_money2_remaining", "invoice_money2"] {
        let (a, _) = common::run(name);
        let inconclusive: Vec<Value> = a
            .findings()
            .into_iter()
            .filter(|f| f["kind"] == "inconclusive")
            .collect();
        assert!(inconclusive.is_empty(), "{name}: {inconclusive:?}");
    }
}
