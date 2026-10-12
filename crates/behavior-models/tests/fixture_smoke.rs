//! The shared fixture's declarations are admitted by Core on their own (T014 checkpoint).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

#[test]
fn declarations_admit_without_models() {
    let out = common::build(|_| {}, vec![]).unwrap();
    let names = common::item_names(&out.module);
    assert!(names.contains(&"PurchaseRequest".to_string()));
    assert!(out.receipts.is_empty());
}
