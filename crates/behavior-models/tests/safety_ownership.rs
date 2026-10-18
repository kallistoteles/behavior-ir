#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/safety.rs"]
mod support;

use behavior_models::safety::{FieldOwnershipInput, analyze};
use serde_json::{Value, json};

fn policy() -> Value {
    json!({"id":"notes", "entity":"Item", "writers":{"note":["edit","copy"]},
        "creators":["create"], "removers":["remove"]})
}
fn input(v: Value) -> FieldOwnershipInput {
    FieldOwnershipInput::from_json(&v).unwrap()
}

#[test]
fn authorized_bindings_initialization_and_removal() {
    let m = support::fixture();
    let a = analyze(&m, &[input(policy())]).unwrap();
    assert!(!a.is_certified());
    assert_eq!(a.inventory().actions().len(), 4);
    // Creation has no assignment authority: its initializers use creator permission.
    let mut p = policy();
    p["writers"]["phase"] = json!([]);
    analyze(&m, &[input(p)]).unwrap();
}

#[test]
fn each_permission_is_independent_and_errors_identify_actual_action() {
    for (path, name, code) in [
        ("note", "copy", "SAFETY_UNAUTHORIZED_WRITE"),
        ("creators", "create", "SAFETY_UNAUTHORIZED_CREATE"),
        ("removers", "remove", "SAFETY_UNAUTHORIZED_REMOVE"),
    ] {
        let mut p = policy();
        if path == "note" {
            p["writers"][path] = json!(["edit"]);
        } else {
            p[path] = json!([]);
        }
        let e = analyze(&support::fixture(), &[input(p)]).unwrap_err();
        assert_eq!(e.code(), code);
        assert_eq!(e.to_json()["action"], name);
        assert_eq!(e.to_json()["entity"], "Item");
    }
}

#[test]
fn noop_and_unreachable_writers_have_no_semantic_exception() {
    for no_op in [false, true] {
        let mut b = support::declarations();
        support::edit(&mut b, "forbidden", "arbitrary_binding", no_op);
        let m = b.finish(None).unwrap();
        let p = json!({"id":"notes", "entity":"Item", "writers":{"note":[]}});
        assert_eq!(
            analyze(&m, &[input(p)]).unwrap_err().code(),
            "SAFETY_UNAUTHORIZED_WRITE"
        );
    }
    let mut b = support::declarations();
    let params = vec![support::state("unused"), support::state("secondary")];
    b.push_scope(
        behavior_engine::builder::ScopeSite::Action,
        params.clone(),
        support::loc(),
    )
    .unwrap();
    let never = b
        .lit(
            behavior_engine::wire::WType::Bool,
            json!(false),
            support::loc(),
        )
        .unwrap();
    let v = b.field("secondary", "note", support::loc()).unwrap();
    b.add_action(
        "unreachable",
        params,
        vec![(never, support::loc())],
        vec![("secondary".into(), "note".into(), v, support::loc())],
        vec![],
        vec![],
        support::loc(),
    )
    .unwrap();
    b.pop_scope();
    assert!(
        analyze(
            &b.finish(None).unwrap(),
            &[input(
                json!({"id":"n", "entity":"Item", "writers":{"note":[]}})
            )]
        )
        .is_err()
    );
}

#[test]
fn actual_core_effects_match_assignment_creation_removal_and_frame_cases() {
    use behavior_engine::invocation::RequestedInvocation;
    use behavior_engine::store::{InMemoryBackend, Store, documents::SeedEntity};
    let m = support::fixture();
    analyze(&m, &[input(policy())]).unwrap();
    let governance=behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#).unwrap();
    let seed = ["i1", "i2"]
        .into_iter()
        .map(|id| SeedEntity {
            entity: "Item".into(),
            value: json!({"id":id,"note":id,"phase":"Idle"}),
        })
        .collect();
    let genesis = behavior_engine::store::store::genesis_v2_for(&m, governance, seed).unwrap();
    let mut store = Store::create(InMemoryBackend::default(), &m, genesis).unwrap();
    for (action, bindings, input, expected) in [
        (
            "copy",
            json!({"left":{"entity":"Item","id":"i1"},"right":{"entity":"Item","id":"i2"}}),
            json!({}),
            vec![
                json!({"id":"i1","note":"i2","phase":"Idle"}),
                json!({"id":"i2","note":"i2","phase":"Idle"}),
            ],
        ),
        (
            "create",
            json!({}),
            json!({"new_id":"i3"}),
            vec![
                json!({"id":"i1","note":"i2","phase":"Idle"}),
                json!({"id":"i2","note":"i2","phase":"Idle"}),
                json!({"id":"i3","note":"","phase":"Idle"}),
            ],
        ),
        (
            "remove",
            json!({"doomed":{"entity":"Item","id":"i2"}}),
            json!({}),
            vec![
                json!({"id":"i1","note":"i2","phase":"Idle"}),
                json!({"id":"i3","note":"","phase":"Idle"}),
            ],
        ),
    ] {
        let (request, errors) = RequestedInvocation::decode(
            &json!({"format":"behavior.invocation.v1","capability":action,
            "bindings":bindings,"input":input,"context":{}})
            .to_string(),
        )
        .unwrap();
        let invocation = store
            .invoke(
                &m,
                &request.unwrap_or_else(|| panic!("{errors:?}")),
                "2026-10-18T00:00:00Z",
                None,
                None,
            )
            .unwrap();
        store
            .commit(&m, &store.current().unwrap(), &invocation.bundle.unwrap())
            .unwrap();
        let head = store.current().unwrap();
        let snapshot = store
            .export_seed_at(&m, &store.history_at(head.position).unwrap())
            .unwrap();
        assert_eq!(
            snapshot
                .seed
                .into_iter()
                .map(|e| e.value)
                .collect::<Vec<_>>(),
            expected,
            "{action}"
        );
    }
}

#[test]
fn overlapping_policies_intersect_and_labels_have_no_authority() {
    let mut denied = policy();
    denied["id"] = json!("another_label");
    denied["writers"]["note"] = json!([]);
    assert!(analyze(&support::fixture(), &[input(policy()), input(denied)]).is_err());
    let a = analyze(&support::fixture(), &[input(policy())]).unwrap();
    let mut renamed = policy();
    renamed["id"] = json!("different_label");
    let z = analyze(&support::fixture(), &[input(renamed)]).unwrap();
    assert_eq!(a.policy_set_hash(), z.policy_set_hash());
}

#[test]
fn malformed_stale_duplicate_and_implicit_permissions_fail_closed() {
    for p in [
        json!({"id":"x","entity":"Item","writers":false}),
        json!({"id":"x","entity":"Item","writers":{},"certified":true}),
        json!({"id":"x","entity":"Item","writers":{"note":["edit","edit"]}}),
    ] {
        assert!(FieldOwnershipInput::from_json(&p).is_err());
    }
    for (key, value) in [
        ("entity", json!("Missing")),
        ("writers", json!({"missing":[]})),
        ("writers", json!({"note":["stale"]})),
        ("creators", json!([])),
    ] {
        let mut p = policy();
        p[key] = value;
        assert!(analyze(&support::fixture(), &[input(p)]).is_err());
    }
    assert!(analyze(&support::fixture(), &[input(policy()), input(policy())]).is_err());
}

#[test]
fn binding_spelling_and_policy_order_never_grant_permissions() {
    for binding in ["subject", "not_the_owner", "z_alias"] {
        let mut b = support::declarations();
        support::edit(&mut b, "edit", binding, false);
        let m = b.finish(None).unwrap();
        let allowed = input(json!({"id":"allow", "entity":"Item","writers":{"note":["edit"]}}));
        let denied = input(json!({"id":"deny", "entity":"Item","writers":{"note":[]}}));
        analyze(&m, std::slice::from_ref(&allowed)).unwrap();
        assert_eq!(
            analyze(&m, &[allowed.clone(), denied.clone()])
                .unwrap_err()
                .code(),
            analyze(&m, &[denied, allowed]).unwrap_err().code()
        );
    }
    let m = support::fixture();
    let mut reverse = policy();
    reverse["writers"]["note"] = json!(["copy", "edit"]);
    assert_eq!(
        analyze(&m, &[input(policy())]).unwrap().policy_set_hash(),
        analyze(&m, &[input(reverse)]).unwrap().policy_set_hash()
    );
}
