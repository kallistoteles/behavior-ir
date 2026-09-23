#![allow(clippy::unwrap_used, clippy::expect_used)]

use behavior_core::canonical::to_canonical_string;
use serde_json::json;

#[test]
fn sorts_keys_at_every_depth_without_whitespace() {
    let v = json!({"b": 1, "a": {"z": [3, {"y": true, "x": null}], "m": "é"}});
    assert_eq!(
        to_canonical_string(&v).unwrap(),
        r#"{"a":{"m":"é","z":[3,{"x":null,"y":true}]},"b":1}"#
    );
}

#[test]
fn keys_sort_by_bytes() {
    let v = json!({"b": 0, "B": 0, "a_": 0, "a": 0});
    assert_eq!(
        to_canonical_string(&v).unwrap(),
        r#"{"B":0,"a":0,"a_":0,"b":0}"#
    );
}

#[test]
fn rejects_floats() {
    assert!(to_canonical_string(&json!({"x": 1.5})).is_err());
    assert!(to_canonical_string(&json!([1.0])).is_err());
}

#[test]
fn round_trip_is_stable() {
    let v = json!({"k": ["a", 1, -2, {"q": "ü"}]});
    let s = to_canonical_string(&v).unwrap();
    let back: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(to_canonical_string(&back).unwrap(), s);
    assert!(!s.ends_with('\n'));
}
