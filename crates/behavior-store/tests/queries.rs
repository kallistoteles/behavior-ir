#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US1 (feature 007) in the store: queries are answered as of the evaluated position, decisions
//! record the instance and its members, and results do not depend on indexes or iteration order.

mod common;

use behavior_store::documents::{
    EntityKey, EntityVersion, Genesis, Head, RefChange, TransitionRecord,
};
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge};
use common::*;
use serde_json::json;

#[test]
fn close_customer_is_decided_over_the_open_orders() {
    let mut s = orders_store();
    // c1 has only a closed order: allowed, with the empty result recorded.
    let e = run_orders(&s, "close_customer", &[("customer", "c1")], json!({}));
    assert_eq!(e.record["result"], "ALLOW", "{}", e.record);
    let q = &e.record["facts"]["queries"][0];
    assert_eq!(q["members"], json!([]));
    let b = e.bundle.clone().unwrap();
    assert_eq!(b.read_facts["queries"][0]["members"], json!([]));
    assert!(
        b.read_facts["queries"][0]["result_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    // c2 has an open order: denied, with exactly that order recorded.
    let e2 = run_orders(&s, "close_customer", &[("customer", "c2")], json!({}));
    assert_eq!(e2.record["result"], "DENY");
    assert_eq!(
        e2.record["facts"]["queries"][0]["members"],
        json!([{"id": "o2"}])
    );
    // A forged read fact is refused at commit.
    let mut forged = b.clone();
    forged.read_facts["queries"][0]["members"] = json!([{"id": "o9"}]);
    assert_eq!(
        s.commit(&orders(), &forged.evaluated_state.clone(), &forged)
            .unwrap_err()
            .code(),
        "BUNDLE_INVALID"
    );
    commit_orders(&mut s, &e);
}

#[test]
fn the_result_hash_changes_when_a_member_joins() {
    // FR-015: a changed result is a changed dependency even though every old member is unchanged.
    let mut s = orders_store();
    let before = run_orders(&s, "check_orders", &[("customer", "c1")], json!({}));
    let e = run_orders(
        &s,
        "place_order",
        &[("customer", "c1")],
        json!({"order_id": "o7", "amount": "1.00"}),
    );
    commit_orders(&mut s, &e);
    let after = run_orders(&s, "check_orders", &[("customer", "c1")], json!({}));
    let facts =
        |e: &behavior_store::Evaluation| e.bundle.clone().unwrap().read_facts["queries"].clone();
    let (b, a) = (facts(&before), facts(&after));
    // The instance "orders of c1" (several instances list o1; this one gains o7): same identity,
    // one more member, a different result hash.
    let pairs: Vec<(&serde_json::Value, &serde_json::Value)> = b
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["members"] == json!([{"id": "o1"}]))
        .filter_map(|q| {
            a.as_array()
                .unwrap()
                .iter()
                .find(|l| l["instance"] == q["instance"])
                .map(|l| (q, l))
        })
        .collect();
    let (old, new) = pairs
        .iter()
        .find(|(_, l)| l["members"] == json!([{"id": "o1"}, {"id": "o7"}]))
        .unwrap();
    assert_ne!(new["result_hash"], old["result_hash"]);
}

/// A backend that reverses the order of `keys_at` and answers no field index.
struct Reversed(InMemoryBackend);

impl Backend for Reversed {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.0.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.0.head()
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        s: &[EntityVersion],
        r: &[RefChange],
    ) -> Result<(), BackendError> {
        self.0.create(g, h, s, r)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.0.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.0.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.0.record(p)
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.0.removed_at(k)
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.0.incoming_at(t, p)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        let mut keys = self.0.keys_at(t, p)?;
        keys.reverse();
        Ok(keys)
    }
    fn commit(
        &mut self,
        x: &str,
        v: &[EntityVersion],
        rm: &[EntityKey],
        refs: &[RefChange],
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.0.commit(x, v, rm, refs, r, h)
    }
}

#[test]
fn results_do_not_depend_on_indexes_or_iteration_order() {
    let seed = || {
        let mut s = orders_seed();
        s.push(order_seed("o3", "c1", "3.00", "open"));
        s.push(order_seed("o4", "c1", "4.00", "blocked"));
        s
    };
    let indexed = orders_store_with(InMemoryBackend::new(), seed());
    let reversed = orders_store_with(Reversed(InMemoryBackend::new()), seed());
    for action in ["close_customer", "check_orders"] {
        let a = run_orders(&indexed, action, &[("customer", "c1")], json!({}));
        let b = run_orders(&reversed, action, &[("customer", "c1")], json!({}));
        assert_eq!(a.record, b.record, "{action}");
    }
}

#[test]
fn type_and_field_indexes_answer_as_of_a_position() {
    let mut s = orders_store();
    let e = run_orders(
        &s,
        "place_order",
        &[("customer", "c1")],
        json!({"order_id": "o7", "amount": "1.00"}),
    );
    commit_orders(&mut s, &e);
    let b = s.backend();
    let ids = |keys: Vec<EntityKey>| keys.into_iter().map(|k| k.id).collect::<Vec<_>>();
    assert_eq!(ids(b.keys_at("Order", 0).unwrap()), ["o1", "o2"]);
    assert_eq!(ids(b.keys_at("Order", 1).unwrap()), ["o1", "o2", "o7"]);
    let by_c1 = |p| {
        b.keys_by_field_at("Order", "customer", &json!("c1"), p)
            .unwrap()
            .map(ids)
    };
    assert_eq!(by_c1(0), Some(vec!["o1".to_string()]));
    assert_eq!(by_c1(1), Some(vec!["o1".to_string(), "o7".to_string()]));
}
