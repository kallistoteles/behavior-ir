//! Prints a fixed lifecycle history over the accounts module as canonical JSON: every transition
//! record (creations, removals, reference changes) and the data, behavior and reference replay
//! reports (used by scripts/determinism-check.sh, feature 006).

use std::collections::BTreeMap;

use behavior_core::canonical::to_canonical_string;
use behavior_store::conformance::ACCOUNTS_WIRE;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::replay::{replay_behavior, replay_data, replay_index};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let m = behavior_core::admit(ACCOUNTS_WIRE).map_err(|r| format!("{:?}", r.errors))?;
    let seed = vec![SeedEntity {
        entity: "Customer".into(),
        value: json!({"id": "hub", "name": "Hub"}),
    }];
    let mut s = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )?;
    let step = |s: &mut Store<InMemoryBackend>,
                action: &str,
                bindings: &[(&str, &str)],
                input: Value|
     -> Result<(), Box<dyn std::error::Error>> {
        let bindings: BTreeMap<String, String> = bindings
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let ev = s.evaluate(
            &m,
            action,
            &bindings,
            &input,
            &json!({}),
            "2026-09-27T12:00:00Z",
            None,
        )?;
        let b = ev
            .bundle
            .ok_or_else(|| format!("{action}: {}", ev.record["result"]))?;
        s.commit(&m, &b.evaluated_state.clone(), &b)?;
        Ok(())
    };
    for k in 0..40 {
        let (c, a) = (format!("c{k}"), format!("a{k}"));
        step(
            &mut s,
            "register_customer",
            &[],
            json!({"customer_id": c, "name": "N"}),
        )?;
        step(
            &mut s,
            "open_account",
            &[("owner", &c)],
            json!({"account_id": a, "initial": "0.00"}),
        )?;
        if k % 2 == 0 {
            // Retarget to the hub and remove the old owner in one transition.
            step(
                &mut s,
                "switch_and_remove",
                &[("account", &a), ("old", &c), ("new", "hub")],
                json!({}),
            )?;
            step(&mut s, "close_account", &[("account", &a)], json!({}))?;
        } else {
            step(&mut s, "close_account", &[("account", &a)], json!({}))?;
            step(&mut s, "remove_customer", &[("customer", &c)], json!({}))?;
        }
    }
    let (from, to) = (s.state_at(0)?, s.current()?);
    for pos in 1..=to.position {
        let r = s.backend().record(pos)?.ok_or("missing record")?;
        println!("{}", to_canonical_string(&serde_json::to_value(&r)?)?);
    }
    let modules = BTreeMap::from([(m.behavior_version(), m.clone())]);
    for report in [
        replay_data(&s, &from, &to),
        replay_behavior(&s, &modules, &from, &to),
        replay_index(&s, &m, &from, &to),
    ] {
        println!("{}", to_canonical_string(&serde_json::to_value(report)?)?);
    }
    Ok(())
}
