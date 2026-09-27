#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The state identity (research R3): MuHash3072 over entity content hashes.

use behavior_store::documents::EntityContent;
use behavior_store::muhash::Accumulator;
use proptest::prelude::*;
use serde_json::json;

fn content(id: &str, balance: &str) -> String {
    EntityContent {
        entity: "Account".into(),
        declaration: "sha256:aa".into(),
        id: id.into(),
        value: json!({"id": id, "balance": balance}),
    }
    .hash()
    .unwrap()
}

fn of(items: &[String]) -> String {
    let mut a = Accumulator::empty();
    for c in items {
        a.insert(c).unwrap();
    }
    a.state_id().unwrap()
}

#[test]
fn empty_add_remove_and_normalization() {
    let empty = Accumulator::empty().state_id().unwrap();
    let mut a = Accumulator::empty();
    a.insert(&content("a1", "1.00")).unwrap();
    assert_ne!(a.state_id().unwrap(), empty);
    a.remove(&content("a1", "1.00")).unwrap();
    assert_eq!(a.state_id().unwrap(), empty);
    // Incremental update equals a fresh computation, and survives a hex round trip.
    let mut inc = Accumulator::empty();
    inc.insert(&content("a1", "1.00")).unwrap();
    inc.insert(&content("a2", "2.00")).unwrap();
    inc.remove(&content("a1", "1.00")).unwrap();
    inc.insert(&content("a1", "5.00")).unwrap();
    let fresh = of(&[content("a1", "5.00"), content("a2", "2.00")]);
    assert_eq!(inc.state_id().unwrap(), fresh);
    assert_eq!(inc.normalize().unwrap().state_id().unwrap(), fresh);
    let (n, d) = inc.to_hex();
    assert_eq!(
        Accumulator::from_hex(&n, &d).unwrap().state_id().unwrap(),
        fresh
    );
}

#[test]
fn swapped_values_change_the_identity() {
    let a = of(&[content("a1", "1.00"), content("a2", "2.00")]);
    let b = of(&[content("a1", "2.00"), content("a2", "1.00")]);
    assert_ne!(a, b);
}

proptest! {
    #[test]
    fn order_does_not_matter(values in prop::collection::vec(0u32..1000, 1..12), seed in any::<u64>()) {
        let items: Vec<String> = values.iter().enumerate()
            .map(|(i, v)| content(&format!("a{i}"), &format!("{v}.00"))).collect();
        let mut shuffled = items.clone();
        let n = shuffled.len();
        for i in 0..n {
            let j = usize::try_from(seed.wrapping_mul(i as u64 + 1) % n as u64).unwrap();
            shuffled.swap(i, j);
        }
        prop_assert_eq!(of(&items), of(&shuffled));
    }
}
