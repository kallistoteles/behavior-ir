#![allow(clippy::unwrap_used, clippy::expect_used)]

use behavior_verify::hashing::{check_key, document_hash, finding_hash};
use serde_json::json;

fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

#[test]
fn finding_hash_depends_only_on_kind_and_cites() {
    let a = finding_hash("preservation", &[("action", h('a')), ("invariant", h('b'))]);
    let same = finding_hash("preservation", &[("action", h('a')), ("invariant", h('b'))]);
    let other_kind = finding_hash(
        "postcondition",
        &[("action", h('a')), ("invariant", h('b'))],
    );
    let other_cite = finding_hash("preservation", &[("action", h('a')), ("invariant", h('c'))]);
    assert_eq!(a, same);
    assert_ne!(a, other_kind);
    assert_ne!(a, other_cite);
    assert!(a.starts_with("sha256:") && a.len() == 71);
}

#[test]
fn check_key_changes_with_every_input() {
    let base = check_key(
        "preservation",
        &[h('a'), h('b')],
        &h('p'),
        "0.2.0",
        "z3 4.16.0",
    );
    assert_ne!(
        base,
        check_key(
            "preservation",
            &[h('a'), h('c')],
            &h('p'),
            "0.2.0",
            "z3 4.16.0"
        )
    );
    assert_ne!(
        base,
        check_key(
            "preservation",
            &[h('a'), h('b')],
            &h('q'),
            "0.2.0",
            "z3 4.16.0"
        )
    );
    assert_ne!(
        base,
        check_key(
            "preservation",
            &[h('a'), h('b')],
            &h('p'),
            "0.2.1",
            "z3 4.16.0"
        )
    );
    assert_ne!(
        base,
        check_key(
            "preservation",
            &[h('a'), h('b')],
            &h('p'),
            "0.2.0",
            "z3 4.17.0"
        )
    );
    assert_ne!(
        base,
        check_key(
            "postcondition",
            &[h('a'), h('b')],
            &h('p'),
            "0.2.0",
            "z3 4.16.0"
        )
    );
}

#[test]
fn document_hash_ignores_hash_field_and_uses_tag() {
    let doc = json!({"a": 1, "b": "x"});
    let with_hash = json!({"a": 1, "b": "x", "hash": "sha256:whatever"});
    assert_eq!(
        document_hash("behavior.waiver.v1", &doc).unwrap(),
        document_hash("behavior.waiver.v1", &with_hash).unwrap()
    );
    assert_ne!(
        document_hash("behavior.waiver.v1", &doc).unwrap(),
        document_hash("behavior.policy.v1", &doc).unwrap()
    );
}
