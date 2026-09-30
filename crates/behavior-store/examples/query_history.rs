//! Prints a fixed history over the orders module as canonical JSON: every transition record
//! (with its query and field facts) and the data and behavior replay reports (used by
//! scripts/determinism-check.sh, feature 007).

use std::collections::BTreeMap;

use behavior_core::canonical::to_canonical_string;
use behavior_store::conformance::ORDERS_WIRE;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::replay::{replay_behavior, replay_data};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let m = behavior_core::admit(ORDERS_WIRE).map_err(|r| format!("{:?}", r.errors))?;
    let entity = |entity: &str, value: Value| SeedEntity {
        entity: entity.into(),
        value,
    };
    let seed = vec![
        entity(
            "Customer",
            json!({"id": "c1", "name": "Ada", "credit_limit": "500.00", "region": "north"}),
        ),
        entity(
            "Customer",
            json!({"id": "c2", "name": "Bo", "credit_limit": "500.00", "region": "south"}),
        ),
        entity("Employee", json!({"id": "e0", "personnel_number": "N0"})),
    ];
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
    for k in 0..30 {
        let c = if k % 2 == 0 { "c1" } else { "c2" };
        let o = format!("o{k:02}");
        let amount = format!("{}.{:02}", 1 + k % 7, (k * 13) % 100);
        step(
            &mut s,
            "place_order",
            &[("customer", c)],
            json!({"order_id": o, "amount": amount}),
        )?;
        if k % 5 == 4 {
            step(
                &mut s,
                "hire",
                &[],
                json!({"employee_id": format!("e{k}"), "number": format!("N{k}")}),
            )?;
            step(&mut s, "check_orders", &[("customer", c)], json!({})).ok();
        }
    }
    step(
        &mut s,
        "raise_limit",
        &[("customer", "c1")],
        json!({"limit": "900.00"}),
    )?;
    step(
        &mut s,
        "renumber",
        &[("employee", "e0")],
        json!({"number": "N100"}),
    )?;
    let (from, to) = (s.state_at(0)?, s.current()?);
    for pos in 1..=to.position {
        let r = s.backend().record(pos)?.ok_or("missing record")?;
        println!("{}", to_canonical_string(&serde_json::to_value(&r)?)?);
    }
    let modules = BTreeMap::from([(m.behavior_version(), m.clone())]);
    for report in [
        replay_data(&s, &from, &to),
        replay_behavior(&s, &modules, &from, &to),
    ] {
        println!("{}", to_canonical_string(&serde_json::to_value(report)?)?);
    }
    Ok(())
}
