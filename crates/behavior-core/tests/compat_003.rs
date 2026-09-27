#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 003 changes nothing for modules that do not use fixed-scale types (SC-004).

mod common;

use behavior_core::{admission_report, admit, serialize::to_wire_json};
use serde_json::Value;

#[test]
fn existing_behavior_versions_are_frozen() {
    let frozen = common::json(&common::fixtures().join("frozen_versions_002.json"));
    let mut failures = Vec::new();
    for (file, expected) in frozen.as_object().unwrap() {
        let r = admission_report(&common::read(&common::fixtures().join(file)));
        if r.behavior_version.as_deref() != expected.as_str() {
            failures.push(format!("{file}: {:?} != {expected}", r.behavior_version));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn serialization_uses_the_lowest_sufficient_version() {
    for (file, version) in [
        ("wire/valid/invoice.json", "0.1"),
        ("wire/valid/constraints.json", "0.2"),
        ("wire/valid/fixed_scale.json", "0.3"),
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
