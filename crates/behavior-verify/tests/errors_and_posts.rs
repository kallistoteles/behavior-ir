#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3: no allowed transition can fail its postconditions or hit an evaluation error — or a
//! confirmed counterexample shows how.

mod common;

use serde_json::Value;

/// `sum_rounding` guards against modelling decimal addition as exact: the engine rounds sums near
/// 10^28, so `w := v + amount; ensures w > v` has a confirmed counterexample.
const FIXTURES: [&str; 5] = [
    "postcondition",
    "overflow",
    "rounding",
    "sum_rounding",
    "project_margin",
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

#[test]
fn counterexamples_reproduce_the_failure() {
    for name in FIXTURES {
        let (a, _) = common::run(name);
        for f in a.findings() {
            let record = &f["counterexample"]["record"];
            match f["kind"].as_str().unwrap() {
                "postcondition" => {
                    assert_eq!(record["result"], "DENY", "{name}: {f}");
                    let failed = record["trace"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|s| s["outcome"] == Value::Bool(false))
                        .unwrap();
                    assert_eq!(failed["phase"], "postcondition");
                    assert_eq!(failed["hash"], f["cites"]["postcondition"]);
                }
                "evaluation_error" => {
                    assert_eq!(record["result"], "ERROR", "{name}: {f}");
                    let message = record["reasons"][0]["message"].as_str().unwrap();
                    let subject = a.value["checks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["subject"]["hash"] == f["cites"]["expression"])
                        .unwrap();
                    let text = subject["subject"]["name"].as_str().unwrap();
                    assert!(
                        text.starts_with(message.split(" in ").nth(1).unwrap()),
                        "{message} vs {text}"
                    );
                }
                "inconclusive" => assert_eq!(f["severity"], "blocking"),
                other => panic!("{name}: unexpected finding kind {other}"),
            }
        }
    }
}

#[test]
fn division_by_zero_counterexample_has_zero_revenue() {
    let (a, _) = common::run("project_margin");
    let f = a
        .findings()
        .into_iter()
        .find(|f| {
            f["kind"] == "evaluation_error"
                && f["explanation"]
                    .as_str()
                    .unwrap()
                    .contains("division by zero")
        })
        .unwrap();
    assert_eq!(f["counterexample"]["state"]["project"]["revenue"], "0");
}

#[test]
fn tight_bound_is_proven_under_exact_division() {
    // Feature 004: `1 / 3 < 0.333…34` (28 digits) was inconclusive under the rounding model.
    let (a, _) = common::run("rounding");
    let tight = a.value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"]["name"] == "third_tight" && c["kind"] == "postcondition")
        .unwrap();
    assert_eq!(tight["outcome"], "proven", "{tight}");
    assert!(tight.get("reason").is_none(), "{tight}");
}
