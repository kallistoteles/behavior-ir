#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3 (feature 006): referential integrity on S', `referenced` on S and S', plain identities.

mod common;

use behavior_core::semantic::module::Module;
use behavior_core::{admit, evaluate, replay};
use serde_json::{Value, json};

fn accounts_with_helpers() -> Module {
    let mut w = common::json(&common::fixtures().join("wire/valid/accounts.json"));
    let l = json!({"file": "t.py", "line": 1});
    let p = |name: &str| json!({"op": "param", "param": name, "loc": l});
    // Open an account for a customer given by identity only (its existence is a fact).
    // Register a customer and open an account for them in one transition.
    // Remove a customer and check `referenced` on S'.
    w["actions"].as_array_mut().unwrap().extend([
        json!({"name": "open_for", "loc": l, "preconditions": [], "postconditions": [],
               "params": [{"name": "c", "type": {"t": "id", "entity": "Customer"}, "role": "input"},
                          {"name": "a", "type": {"t": "id", "entity": "Account"}, "role": "input"}],
               "effects": [{"create": "Account", "id": p("a"), "loc": l,
                            "fields": {"owner": p("c"), "balance": {"op": "lit", "type": {"t": "nominal", "name": "Money"}, "value": "0", "loc": l}}}]}),
        json!({"name": "register_and_open", "loc": l, "preconditions": [], "postconditions": [],
               "params": [{"name": "c", "type": {"t": "id", "entity": "Customer"}, "role": "input"},
                          {"name": "a", "type": {"t": "id", "entity": "Account"}, "role": "input"}],
               "effects": [
                   {"create": "Customer", "id": p("c"), "loc": l, "fields": {"name": {"op": "lit", "type": {"t": "string"}, "value": "New", "loc": l}}},
                   {"create": "Account", "id": p("a"), "loc": l,
                    "fields": {"owner": p("c"), "balance": {"op": "lit", "type": {"t": "nominal", "name": "Money"}, "value": "0", "loc": l}}}]}),
        json!({"name": "remove_if_unreferenced_after", "loc": l, "preconditions": [],
               "params": [{"name": "customer", "type": {"t": "entity", "name": "Customer"}, "role": "state"}],
               "effects": [{"remove": "customer", "loc": l}],
               "postconditions": [{"loc": l, "expr": {"op": "not", "loc": l, "args": [
                   {"op": "referenced", "loc": l, "args": [{"op": "field", "param": "customer", "field": "id", "loc": l}]}]}}]}),
    ]);
    admit(&w.to_string()).unwrap()
}

fn run(m: &Module, request: &Value) -> Value {
    let text = evaluate(m, &request.to_string()).to_json_string();
    let replayed = replay(m, &text);
    assert!(replayed.matches, "{:?}", replayed.diff);
    serde_json::from_str(&text).unwrap()
}

fn req(action: &str, state: Value, input: Value, facts: Value) -> Value {
    json!({"action": action, "data_version": "1", "context": {}, "state": state,
           "input": input, "facts": facts})
}

fn edge(id: &str) -> Value {
    json!({"entity": "Account", "id": id, "field": "owner"})
}

#[test]
fn a_surviving_reference_blocks_the_removal() {
    let m = accounts_with_helpers();
    let customer = json!({"customer": {"id": "c1", "name": "Ada"}});
    let dangling = run(
        &m,
        &req(
            "remove_customer_unchecked",
            customer.clone(),
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": [edge("a1"), edge("a2")]}]}),
        ),
    );
    assert_eq!(dangling["result"], "DENY");
    assert_eq!(dangling["reasons"][0]["code"], "DANGLING_REFERENCE");
    assert_eq!(
        dangling["reasons"][0]["references"],
        json!([edge("a1"), edge("a2")])
    );
    let free = run(
        &m,
        &req(
            "remove_customer_unchecked",
            customer,
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": []}]}),
        ),
    );
    assert_eq!(free["result"], "ALLOW");
}

#[test]
fn retarget_and_remove_is_valid_on_s_prime() {
    let m = accounts_with_helpers();
    let rec = run(
        &m,
        &req(
            "switch_and_remove",
            json!({"account": {"id": "a1", "owner": "c1", "balance": "0.00"},
                    "old": {"id": "c1", "name": "Ada"}, "new": {"id": "c2", "name": "Bo"}}),
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": [edge("a1")]}]}),
        ),
    );
    assert_eq!(rec["result"], "ALLOW", "{rec}");
}

#[test]
fn created_references_need_an_existing_target() {
    let m = accounts_with_helpers();
    let facts = |exists: bool| {
        json!({"existence": [{"entity": "Customer", "id": "c9", "exists": exists}],
               "identities": [{"entity": "Account", "id": "a9", "used": false},
                              {"entity": "Customer", "id": "c9", "used": exists}]})
    };
    let input = json!({"c": "c9", "a": "a9"});
    let missing = run(&m, &req("open_for", json!({}), input.clone(), facts(false)));
    assert_eq!(missing["result"], "DENY", "{missing}");
    assert_eq!(missing["reasons"][0]["code"], "CONSTRAINT_VIOLATED");
    let present = run(&m, &req("open_for", json!({}), input.clone(), facts(true)));
    assert_eq!(present["result"], "ALLOW", "{present}");
    // Created in the same transition: `exists'` holds without an existence fact.
    let both = run(
        &m,
        &req(
            "register_and_open",
            json!({}),
            input,
            json!({"identities": [{"entity": "Account", "id": "a9", "used": false},
                                   {"entity": "Customer", "id": "c9", "used": false}]}),
        ),
    );
    assert_eq!(both["result"], "ALLOW", "{both}");
    assert!(both["facts"].get("existence").is_none(), "{both}");
}

#[test]
fn plain_identities_never_block_a_removal() {
    // `AuditNote.about` is a plain `Id<Customer>`: not an incoming reference at all.
    let m = accounts_with_helpers();
    assert_eq!(
        m.references_to("Customer"),
        [("Account".to_string(), "owner".to_string())]
    );
    let rec = run(
        &m,
        &req(
            "remove_customer_unchecked",
            json!({"customer": {"id": "c1", "name": "Ada"}}),
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": []}]}),
        ),
    );
    assert_eq!(rec["result"], "ALLOW");
}

#[test]
fn referenced_reads_s_before_and_s_prime_after() {
    let m = accounts_with_helpers();
    let customer = json!({"customer": {"id": "c1", "name": "Ada"}});
    // On S: the guarded removal is denied by the precondition.
    let pre = run(
        &m,
        &req(
            "remove_customer",
            customer.clone(),
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": [edge("a1")]}]}),
        ),
    );
    assert_eq!(pre["result"], "DENY");
    assert_eq!(pre["reasons"][0]["code"], "PRECONDITION_FAILED");
    // On S': the unbound referrer survives, so the postcondition fails before integrity.
    let post = run(
        &m,
        &req(
            "remove_if_unreferenced_after",
            customer,
            json!({}),
            json!({"references": [{"entity": "Customer", "id": "c1", "incoming": [edge("a1")]}]}),
        ),
    );
    assert_eq!(post["result"], "DENY", "{post}");
    assert_eq!(post["reasons"][0]["code"], "POSTCONDITION_FAILED");
}
