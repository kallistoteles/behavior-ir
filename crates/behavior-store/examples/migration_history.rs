//! Prints a fixed history crossing two migrations (feature 009) as canonical JSON: every
//! transition record, then the data and behavior replay reports (used by
//! scripts/determinism-check.sh).

use std::collections::BTreeMap;

use behavior_core::canonical::to_canonical_string;
use behavior_core::migration::admit_migration;
use behavior_core::semantic::module::Module;
use behavior_store::conformance::{CULTURES_V1_TO_V2, CULTURES_V1_WIRE, CULTURES_V2_WIRE};
use behavior_store::documents::{EvidencePolicy, SeedEntity};
use behavior_store::replay::{replay_behavior_with, replay_data};
use behavior_store::store::genesis_for;
use behavior_store::{Backend, InMemoryBackend, Store};
use serde_json::{Value, json};

const V3_WIRE: &str = include_str!("../../../tests/fixtures/migration/modules/cultures_v3.json");
const V2_TO_V3: &str = include_str!("../../../tests/fixtures/migration/valid/region_required.json");
const T: &str = "2026-10-02T12:00:00Z";

fn admit(w: &str) -> Result<Module, Box<dyn std::error::Error>> {
    Ok(behavior_core::admit(w).map_err(|r| format!("{:?}", r.errors))?)
}

fn step(
    s: &mut Store<InMemoryBackend>,
    m: &Module,
    action: &str,
    bindings: &[(&str, &str)],
    input: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let bindings: BTreeMap<String, String> = bindings
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let ev = s.evaluate(m, action, &bindings, &input, &json!({}), T, None)?;
    let b = ev
        .bundle
        .ok_or_else(|| format!("{action}: {}", ev.record["result"]))?;
    s.commit(m, &b.evaluated_state.clone(), &b)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (v1, v2, v3) = (
        admit(CULTURES_V1_WIRE)?,
        admit(CULTURES_V2_WIRE)?,
        admit(V3_WIRE)?,
    );
    let broaden = admit_migration(&v1, &v2, CULTURES_V1_TO_V2).map_err(|r| format!("{r:?}"))?;
    let narrow = admit_migration(&v2, &v3, V2_TO_V3).map_err(|r| format!("{r:?}"))?;
    let seed = vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "c1", "name": "Ada", "email": "a@x"}),
        },
        SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                          "legacy_code": "L1", "price": "1.50", "fee": "0.1235"}),
        },
        SeedEntity {
            entity: "Order".into(),
            value: json!({"id": "o1", "customer": "c1", "region": null, "qty": 3}),
        },
    ];
    let mut s = Store::create(
        InMemoryBackend::new(),
        &v1,
        genesis_for(&v1, EvidencePolicy::none(), seed),
    )?;
    step(&mut s, &v1, "kill", &[("culture", "k1")], json!({}))?;
    s.migrate(&broaden, &v1, &v2, T, None)?;
    step(
        &mut s,
        &v2,
        "register_customer",
        &[],
        json!({"customer_id": "c2", "name": "Bo", "email": "b@x"}),
    )?;
    step(
        &mut s,
        &v2,
        "set_region",
        &[("order", "o1")],
        json!({"region": "south"}),
    )?;
    s.migrate(&narrow, &v2, &v3, T, None)?;
    step(
        &mut s,
        &v3,
        "set_region",
        &[("order", "o1")],
        json!({"region": "north"}),
    )?;
    let (from, to) = (s.state_at(0)?, s.current()?);
    for pos in 1..=to.position {
        let r = s.backend().record(pos)?.ok_or("missing record")?;
        println!("{}", to_canonical_string(&serde_json::to_value(&r)?)?);
    }
    let modules = [&v1, &v2, &v3]
        .into_iter()
        .map(|m| (m.behavior_version(), m.clone()))
        .collect();
    let migrations = BTreeMap::from([
        (broaden.hash(), (broaden.clone(), v1.clone(), v2.clone())),
        (narrow.hash(), (narrow.clone(), v2.clone(), v3.clone())),
    ]);
    let data = replay_data(&s, &from, &to);
    let behavior = replay_behavior_with(&s, &modules, &migrations, &from, &to);
    println!("{}", to_canonical_string(&serde_json::to_value(&data)?)?);
    println!(
        "{}",
        to_canonical_string(&serde_json::to_value(&behavior)?)?
    );
    Ok(())
}
