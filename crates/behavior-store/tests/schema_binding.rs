#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 009: exact store-schema binding (FR-005, research R2, R3). Every evaluation and commit
//! requires the module's SchemaHash to equal the store's schema at the evaluated position; the
//! comparison covers every entity type, not only the ones an action touches.

mod common;

use behavior_core::schema;
use behavior_core::semantic::module::Module;
use behavior_store::Backend;
use behavior_store::documents::StoreError;
use common::*;
use serde_json::{Value, json};

fn ledger_wire() -> Value {
    serde_json::from_str(&read(&fixtures().join("wire/valid/ledger.json"))).unwrap()
}

fn accounts_wire() -> Value {
    serde_json::from_str(&read(&fixtures().join("wire/valid/accounts.json"))).unwrap()
}

fn admit(w: &Value) -> Module {
    behavior_core::admit(&w.to_string()).unwrap_or_else(|e| panic!("{}", e.to_json_string()))
}

/// The ledger with one more action (a copy of `touch`): a behavior-only change.
fn ledger_with_extra_action() -> Module {
    let mut w = ledger_wire();
    let mut extra = w["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "touch")
        .unwrap()
        .clone();
    extra["name"] = json!("touch_again");
    w["actions"].as_array_mut().unwrap().push(extra);
    admit(&w)
}

/// The ledger with an extra entity type that no action touches.
fn ledger_with_note_type() -> Module {
    let mut w = ledger_wire();
    w["entities"].as_array_mut().unwrap().push(json!({
        "name": "Note", "loc": {"file": "ledger.py", "line": 40},
        "fields": [{"name": "text", "type": {"t": "string"}, "loc": {"file": "ledger.py", "line": 41}}],
    }));
    admit(&w)
}

fn mismatch(e: StoreError) -> (String, String, Vec<String>) {
    assert_eq!(e.code(), "SCHEMA_MISMATCH", "{e}");
    match e {
        StoreError::SchemaMismatch {
            store,
            module,
            differing,
        } => (
            store,
            module,
            differing.into_iter().map(|d| d.entity).collect(),
        ),
        other => panic!("{other}"),
    }
}

#[test]
fn a_behavior_only_change_evaluates_and_commits_against_an_existing_store() {
    let mut s = store();
    let m = ledger_with_extra_action();
    assert_ne!(m.behavior_version(), ledger().behavior_version());
    let e = s
        .evaluate(
            &m,
            "touch_again",
            &bind(&[("account", "a1")]),
            &json!({}),
            &json!({}),
            T0,
            None,
        )
        .unwrap();
    let b = e.bundle.expect("allowed");
    s.commit(&m, &b.evaluated_state.clone(), &b).unwrap();
}

#[test]
fn a_difference_in_an_untouched_entity_type_is_refused_before_any_decision() {
    let s = store();
    let m = ledger_with_note_type();
    let err = s
        .evaluate(
            &m,
            "touch",
            &bind(&[("account", "a1")]),
            &json!({}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    let (store_hash, module_hash, differing) = mismatch(err.clone());
    assert_eq!(store_hash, schema(&ledger()).hash);
    assert_eq!(module_hash, schema(&m).hash);
    assert_eq!(differing, vec!["Note".to_string()]);
    let text = err.to_string();
    assert!(
        text.contains(&store_hash) && text.contains(&module_hash),
        "{text}"
    );
}

#[test]
fn a_changed_declaration_of_another_type_is_refused_too() {
    // `deposit` touches only Account; AuditNote gains a field.
    let s = accounts_store();
    let mut w = accounts_wire();
    let note = w["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["name"] == "AuditNote")
        .unwrap();
    note["fields"].as_array_mut().unwrap().push(json!({
        "name": "pinned", "type": {"t": "bool"}, "loc": {"file": "accounts.py", "line": 18},
    }));
    let m = admit(&w);
    let err = s
        .evaluate(
            &m,
            "deposit",
            &bind(&[("account", "a1")]),
            &json!({"amount": "1.00"}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    let (_, _, differing) = mismatch(err);
    assert_eq!(differing, vec!["AuditNote".to_string()]);
}

#[test]
fn committing_with_a_module_of_another_schema_is_refused() {
    let mut s = store();
    let e = unary(&s, "touch", "a1");
    let b = e.bundle.expect("allowed");
    let err = s
        .commit(&ledger_with_note_type(), &b.evaluated_state.clone(), &b)
        .unwrap_err();
    mismatch(err);
    // The matching module still commits.
    commit(&mut s, &b);
}

#[test]
fn a_store_without_migrations_is_under_its_genesis_schema() {
    let mut s = store();
    let b = unary(&s, "touch", "a1").bundle.unwrap();
    commit(&mut s, &b);
    let genesis = schema(&ledger());
    for p in 0..=1 {
        let at = s.state_at(p).unwrap();
        let sr = s.schema_at(&at).unwrap();
        assert_eq!(sr.hash, genesis.hash);
        assert_eq!(sr.declarations, genesis.declarations);
        assert_eq!(sr.since, 0);
        assert_eq!(sr.migration_record, s.store_id().unwrap());
    }
    let history = s.schema_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].since, 0);
    assert_eq!(history[0].hash, genesis.hash);
}

#[test]
fn schema_at_refuses_a_state_of_another_store() {
    let s = store();
    let mut other = s.current().unwrap();
    other.state = "sha256:00".into();
    assert!(s.schema_at(&other).is_err());
}

#[test]
fn the_head_of_a_store_without_migrations_has_no_schema_field() {
    let mut s = store();
    let b = unary(&s, "touch", "a1").bundle.unwrap();
    commit(&mut s, &b);
    let head = s.backend().head().unwrap().unwrap();
    let v = serde_json::to_value(&head).unwrap();
    assert!(v.get("schema").is_none(), "{v}");
    assert_eq!(
        v.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        vec!["acc_den", "acc_num", "last_record", "state_ref"]
    );
}

#[test]
fn the_refusal_lists_every_differing_type_with_both_declarations() {
    let s = accounts_store();
    let mut w = accounts_wire();
    for e in w["entities"].as_array_mut().unwrap() {
        if e["name"] == "AuditNote" || e["name"] == "Customer" {
            e["fields"].as_array_mut().unwrap().push(json!({
                "name": "flag", "type": {"t": "bool"}, "loc": {"file": "a.py", "line": 1},
            }));
        }
    }
    // Creating a customer without `flag` would be incomplete: drop the creating actions.
    w["actions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| !a.to_string().contains("\"create\""));
    let m = admit(&w);
    let err = s
        .evaluate(
            &m,
            "deposit",
            &bind(&[("account", "a1")]),
            &json!({"amount": "1.00"}),
            &json!({}),
            T0,
            None,
        )
        .unwrap_err();
    let StoreError::SchemaMismatch { differing, .. } = &err else {
        panic!("{err}")
    };
    let names: Vec<&str> = differing.iter().map(|d| d.entity.as_str()).collect();
    assert_eq!(names, vec!["AuditNote", "Customer"], "sorted by name");
    let store_decls = s.genesis().unwrap().entity_declarations;
    let module_decls = behavior_core::schema(&m).declarations;
    let text = err.to_string();
    for d in differing {
        assert_eq!(d.store.as_ref(), store_decls.get(&d.entity));
        assert_eq!(d.module.as_ref(), module_decls.get(&d.entity));
        assert!(
            text.contains(d.store.as_ref().unwrap()) && text.contains(d.module.as_ref().unwrap()),
            "{text}"
        );
    }
}
