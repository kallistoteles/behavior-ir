//! Semantic ids and the generated action-name encoding (feature 501, research R4, FR-013).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

use behavior_models::ids::{SemanticId, decode, encode};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

#[test]
fn accepts_the_semantic_id_grammar() {
    for ok in [
        "a",
        "submit",
        "start_review",
        "a1",
        "purchase_request",
        "x9_y8_z7",
    ] {
        assert!(SemanticId::parse(ok).is_some(), "{ok}");
    }
}

#[test]
fn rejects_everything_outside_the_grammar() {
    for bad in [
        "", "_a", "a_", "a__b", "A", "Submit", "1a", "a-b", "a b", "a.b", "é", "a___b", "__",
    ] {
        assert!(SemanticId::parse(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn encodes_with_a_double_underscore() {
    let m = SemanticId::parse("purchase_request").unwrap();
    let t = SemanticId::parse("approve").unwrap();
    assert_eq!(encode(&m, &t), "purchase_request__approve");
}

/// Core's identifier grammar `[A-Za-z_][A-Za-z0-9_]*` (`wire::is_identifier`).
fn core_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn semantic_id() -> impl Strategy<Value = SemanticId> {
    "[a-z][a-z0-9]{0,3}(_[a-z0-9]{1,3}){0,3}".prop_map(|s| SemanticId::parse(&s).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2000,
        rng_seed: RngSeed::Fixed(501),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn encode_is_injective_decodable_and_core_valid(
        m1 in semantic_id(), t1 in semantic_id(), m2 in semantic_id(), t2 in semantic_id()
    ) {
        let a = encode(&m1, &t1);
        let b = encode(&m2, &t2);
        prop_assert!(core_identifier(&a));
        prop_assert_eq!(decode(&a), Some((m1.clone(), t1.clone())));
        if (m1, t1) != (m2, t2) {
            prop_assert_ne!(a, b);
        }
    }
}

// --- EncodeEntityName (feature 502, research R3, SC-010) -----------------------------------------

use behavior_models::ids::{decode_entity_name, encode_entity_name};

#[test]
fn entity_names_golden() {
    for (id, name) in [
        ("purchase_request", "PurchaseRequest"),
        ("a_1b", "A_1b"),
        ("a1b", "A1b"),
        ("x_2_y", "X_2Y"),
        ("order", "Order"),
    ] {
        assert_eq!(
            encode_entity_name(&SemanticId::parse(id).unwrap()),
            name,
            "{id}"
        );
    }
}

fn wide_semantic_id() -> impl Strategy<Value = SemanticId> {
    "[a-z][a-z0-9]{0,4}(_[a-z0-9]{1,4}){0,4}".prop_map(|s| SemanticId::parse(&s).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2000,
        rng_seed: RngSeed::Fixed(502),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// Plain PascalCase collides (`a_1b`, `a1b` → `A1b`); this encoding does not.
    #[test]
    fn entity_name_encoding_is_injective_decodable_and_core_valid(
        a in wide_semantic_id(), b in wide_semantic_id()
    ) {
        let ea = encode_entity_name(&a);
        let eb = encode_entity_name(&b);
        prop_assert!(core_identifier(&ea));
        prop_assert_eq!(decode_entity_name(&ea), Some(a.clone()));
        if a != b {
            prop_assert_ne!(ea, eb);
        }
    }
}
