//! Prints a fixed 200-transition ledger history as canonical JSON: every state reference,
//! transition record, and both replay reports (used by scripts/determinism-check.sh, SC-005).

use std::collections::BTreeMap;

use behavior_core::canonical::to_canonical_string;
use behavior_store::conformance::LEDGER_WIRE;
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::replay::{replay_behavior, replay_data};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let m = behavior_core::admit(LEDGER_WIRE).map_err(|r| format!("{:?}", r.errors))?;
    let seed = ["a1", "a2", "a3"]
        .iter()
        .map(|id| SeedEntity {
            entity: "Account".into(),
            value: json!({"id": id, "active": true, "balance": "100.00"}),
        })
        .collect();
    let mut s = Store::create(
        InMemoryBackend::new(),
        &m,
        genesis_for(&m, EvidencePolicy::none(), seed),
    )?;
    let ids = ["a1", "a2", "a3"];
    let mut i: usize = 0;
    while s.current()?.position < 200 {
        let (f, t) = (ids[i % 3], ids[(i + 1 + i / 3) % 3]);
        i += 1;
        let bindings = BTreeMap::from([
            ("from_".to_string(), f.to_string()),
            ("to".to_string(), t.to_string()),
        ]);
        if f == t {
            continue;
        }
        let amount = format!("{}.{:02}", i % 5, i % 100);
        let ev = s.evaluate(
            &m,
            "transfer",
            &bindings,
            &json!({"amount": amount}),
            &json!({}),
            "2026-09-27T12:00:00Z",
            None,
        )?;
        if let Some(b) = ev.bundle {
            s.commit(&m, &b.evaluated_state.clone(), &b)?;
        }
    }
    let (from, to) = (s.state_at(0)?, s.current()?);
    for pos in 1..=to.position {
        let r = s.backend().record(pos)?.ok_or("missing record")?;
        println!("{}", to_canonical_string(&serde_json::to_value(&r)?)?);
    }
    let modules = BTreeMap::from([(m.behavior_version(), m.clone())]);
    println!(
        "{}",
        to_canonical_string(&serde_json::to_value(replay_data(&s, &from, &to))?)?
    );
    println!(
        "{}",
        to_canonical_string(&serde_json::to_value(replay_behavior(
            &s, &modules, &from, &to
        ))?)?
    );
    Ok(())
}
