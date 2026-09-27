#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Identity across versions: a module keeps its pre-004 behavior version unless the feature 004
//! migration table (`tests/fixtures/migration_004.json`) lists it with a reason.

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::Value;

#[test]
fn behavior_versions_are_frozen_or_migrated() {
    let frozen = common::json(&common::fixtures().join("frozen_versions.json"));
    let migrated = common::json(&common::fixtures().join("migration_004.json"));
    let migrated = migrated["modules"].as_object().unwrap();
    let mut failures = Vec::new();
    for (file, expected) in frozen.as_object().unwrap() {
        let r = admission_report(&common::read(&common::fixtures().join(file)));
        let same = r.behavior_version.as_deref() == expected.as_str();
        match (same, migrated.contains_key(file)) {
            (true, true) => failures.push(format!("{file}: listed as migrated but unchanged")),
            (false, false) => failures.push(format!(
                "{file}: {:?} != {expected} and not in migration_004.json",
                r.behavior_version
            )),
            _ => {}
        }
    }
    for file in migrated.keys() {
        if frozen.get(file).is_none() {
            failures.push(format!("{file}: in migration_004.json but not frozen"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn serialization_writes_0_4_and_round_trips() {
    for (file, version) in [
        ("wire/valid/invoice.json", "0.4"),
        ("wire/valid/constraints.json", "0.4"),
        ("wire/valid/fixed_scale.json", "0.4"),
    ] {
        let text = common::read(&common::fixtures().join(file));
        let m = admit(&text).unwrap();
        let out = to_wire_json(&m);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ir_version"], version, "{file}");
        assert_eq!(
            admission_report(&out).behavior_version,
            admission_report(&text).behavior_version,
            "{file} round trip"
        );
    }
}
