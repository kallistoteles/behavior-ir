#![allow(clippy::unwrap_used, clippy::expect_used)]

//! SC-001: no admitted module can reach the runtime's exact-size limit. Randomized evaluations
//! with inputs from the full accepted ranges never produce the internal exact-bound error.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admit, evaluate};
use proptest::prelude::*;
use serde_json::{Value, json};

fn module(file: &str) -> Module {
    admit(&common::read(&common::fixtures().join(file))).unwrap()
}

/// A two-decimal amount anywhere in the type's range (|v| < 10^26).
fn money() -> impl Strategy<Value = String> {
    (
        any::<bool>(),
        0u128..100_000_000_000_000_000_000_000_000,
        0u32..100,
    )
        .prop_map(|(neg, int, cents)| format!("{}{int}.{cents:02}", if neg { "-" } else { "" }))
}

/// A general decimal with up to 28 significant digits at any scale.
fn decimal() -> impl Strategy<Value = String> {
    (
        any::<bool>(),
        1u128..10_000_000_000_000_000_000_000_000_000,
        0usize..=28,
    )
        .prop_map(|(neg, m, s)| {
            let digits = m.to_string();
            let text = if s == 0 {
                digits
            } else {
                let padded = format!("{digits:0>width$}", width = s + 1);
                let (i, f) = padded.split_at(padded.len() - s);
                format!("{i}.{f}")
            };
            format!("{}{text}", if neg { "-" } else { "" })
        })
}

fn no_internal_error(record: &Value) -> Result<(), TestCaseError> {
    for r in record["reasons"].as_array().unwrap() {
        let msg = r["message"].as_str().unwrap_or_default();
        prop_assert!(!msg.contains("internal"), "{msg}");
    }
    Ok(())
}

proptest! {
    // 5,000 cases per property, 10,000 evaluations in total (SC-001).
    #![proptest_config(ProptestConfig::with_cases(5_000))]

    #[test]
    fn fixed_scale_actions_never_hit_the_exact_bound(
        action in prop::sample::select(vec!["charge", "split3", "split3_nested", "scale_up", "per_unit",
                                            "third_exact", "third_via_derived", "compare_fee",
                                            "round_half_even", "round_floor", "check_margin", "add_spent"]),
        amount in money(), limit in money(), count in any::<i64>(),
    ) {
        let m = module("wire/valid/fixed_scale.json");
        let invoice = json!({"id": "i1", "amount": amount, "fee": "0"});
        let budget = json!({"id": "b1", "limit": limit, "spent": "0"});
        let (state, input) = match action {
            "check_margin" | "add_spent" => (json!({"invoice": invoice, "budget": budget}), json!({})),
            "per_unit" => (json!({"invoice": invoice}), json!({"count": count})),
            _ => (json!({"invoice": invoice}), json!({})),
        };
        let req = json!({"action": action, "data_version": "1", "state": state, "input": input, "context": {}});
        let rec: Value = serde_json::from_str(&evaluate(&m, &req.to_string()).to_json_string()).unwrap();
        no_internal_error(&rec)?;
    }

    #[test]
    fn the_largest_admitted_chain_never_hits_the_exact_bound(a in money(), d in decimal()) {
        // `exact_bound_ok` divides by `d` four times (470 bits): admitted, never overflows.
        let m = module("wire/valid/exact_bound_ok.json");
        let req = json!({"action": "check", "data_version": "1", "input": {}, "context": {},
                         "state": {"r": {"id": "r1", "a": a, "d": d}}});
        let rec: Value = serde_json::from_str(&evaluate(&m, &req.to_string()).to_json_string()).unwrap();
        no_internal_error(&rec)?;
    }
}
