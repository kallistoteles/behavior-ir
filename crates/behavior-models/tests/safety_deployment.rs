#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_models::safety::{analyze_with_dependencies, snapshot::validate_store_snapshot};
use serde_json::json;
use support::*;

#[test]
fn alternative_same_schema_module_is_not_a_core_allowlist_and_incoming_anchor_is_fresh() {
    let good = module(&Options::default());
    let bad = module(&Options {
        missing_guard: true,
        ..Options::default()
    });
    assert_eq!(
        behavior_engine::schema(&good).hash,
        behavior_engine::schema(&bad).hash
    );
    let a = analyze_with_dependencies(&good, &ownership(&good), &[dependency("Child")]).unwrap();
    let mut st = store(
        &good,
        vec![parent("p", "Open"), child("Child", "c", "p", "Active")],
    );
    let anchor = st.current_history().unwrap();
    let record = validate_store_snapshot(&good, &a, &st, &anchor);
    assert_eq!(record.status(), "Valid");
    // Core accepts this alternate admitted module: restricting it is the host's job.
    let bundle = invoke(
        &st,
        &bad,
        "stop",
        json!({"p":{"entity":"Parent","id":"p"}}),
        json!({}),
    )
    .bundle
    .unwrap();
    st.commit(&bad, &st.current().unwrap(), &bundle).unwrap();
    assert!(!property(&snapshot(&st, &good)));
    assert!(!record.matches_anchor(&good, &a, &st.current_history().unwrap()));
    assert_eq!(
        validate_store_snapshot(&good, &a, &st, &st.current_history().unwrap()).status(),
        "Invalid"
    );
    assert_eq!(
        validate_store_snapshot(&good, &a, &st, &anchor).status(),
        "Valid"
    ); // no claim about later states
}
#[test]
fn incoming_policy_change_requires_new_anchor_even_for_identical_core() {
    let m = module(&Options::default());
    let mut less = dependency("Child");
    less.prohibited.remove("Abandoned");
    let a = analyze_with_dependencies(&m, &ownership(&m), &[less]).unwrap();
    let b = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let st = store(
        &m,
        vec![parent("p", "Abandoned"), child("Child", "c", "p", "Active")],
    );
    let at = st.current_history().unwrap();
    let r = validate_store_snapshot(&m, &a, &st, &at);
    assert_eq!(r.status(), "Valid");
    assert!(!r.matches_anchor(&m, &b, &at));
    assert_eq!(
        validate_store_snapshot(&m, &b, &st, &at).status(),
        "Invalid"
    );
}

#[test]
fn command_only_commit_keeps_state_but_invalidates_full_history_anchor() {
    use behavior_engine::{
        builder::{CommandEmissionSpec, ScopeSite},
        wire::{WField, WType},
    };
    use std::collections::BTreeMap;
    let mut b = declarations(WType::Ref("Parent".into()), false);
    b.add_command(
        "Notify",
        vec![WField {
            name: "message".into(),
            ty: WType::String,
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
    b.push_scope(ScopeSite::Action, vec![], loc()).unwrap();
    let message = b.lit(WType::String, json!("tick"), loc()).unwrap();
    b.add_action_with_commands(
        "tick",
        vec![],
        vec![],
        vec![],
        vec![],
        vec![CommandEmissionSpec {
            command: "Notify".into(),
            when: None,
            payload: BTreeMap::from([("message".into(), message)]),
            loc: loc(),
        }],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let m = b.finish(None).unwrap();
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let mut st = store(&m, vec![parent("p", "Open")]);
    let before = st.current_history().unwrap();
    let r = validate_store_snapshot(&m, &a, &st, &before);
    let bundle = invoke(&st, &m, "tick", json!({}), json!({}))
        .bundle
        .unwrap();
    st.commit(&m, &st.current().unwrap(), &bundle).unwrap();
    let after = st.current_history().unwrap();
    assert_eq!(before.state, after.state);
    assert_ne!(before, after);
    assert!(!r.matches_anchor(&m, &a, &after));
    assert_eq!(
        validate_store_snapshot(&m, &a, &st, &after).status(),
        "Valid"
    );
}
#[test]
fn actual_core_valid_migration_can_violate_model_relation() {
    use behavior_engine::{
        builder::{Builder, SemanticProfile},
        migration::admit_migration,
        wire::{WField, WType},
    };
    let source = module(&Options::default());
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum(
        "ParentPhase",
        vec!["Open".into(), "Gone".into(), "Abandoned".into()],
        loc(),
    )
    .unwrap();
    b.declare_enum(
        "ChildPhase",
        vec!["Active".into(), "Terminal".into()],
        loc(),
    )
    .unwrap();
    let f = |name: &str, ty| WField {
        name: name.into(),
        ty,
        loc: loc(),
    };
    b.declare_entity(
        "Parent",
        vec![
            f("phase", WType::Enum("ParentPhase".into())),
            f("note", WType::String),
            f("migration_marker", WType::String),
        ],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Child",
        vec![
            f("phase", WType::Enum("ChildPhase".into())),
            f("owner", WType::Ref("Parent".into())),
        ],
        loc(),
    )
    .unwrap();
    let target = b.finish(None).unwrap();
    let l = json!({"file":"migration-test","line":1});
    let doc = json!({"migration_ir":"0.1","name":"unsafe_for_safety_model","source":behavior_engine::schema(&source).hash,"target":behavior_engine::schema(&target).hash,
        "constants":[],"requirements":[],"transforms":[{"entity":"Parent","fields":{
            "phase":{"op":"lit","type":{"t":"enum","name":"ParentPhase"},"value":"Gone","loc":l},
            "migration_marker":{"op":"lit","type":{"t":"string"},"value":"applied","loc":l}},"drops":[]}],"retire":[]});
    let migration = admit_migration(&source, &target, &doc.to_string()).unwrap();
    let mut st = store(
        &source,
        vec![parent("p", "Open"), child("Child", "c", "p", "Active")],
    );
    st.migrate(&migration, &source, &target, "2026-10-18T01:00:00Z", None)
        .unwrap();
    let a =
        analyze_with_dependencies(&target, &ownership(&target), &[dependency("Child")]).unwrap();
    assert!(!property(&snapshot(&st, &target)));
    assert_eq!(
        validate_store_snapshot(&target, &a, &st, &st.current_history().unwrap()).status(),
        "Invalid"
    );
}
#[test]
fn conflict_requires_new_evaluation_under_activated_module() {
    let m = module(&Options::default());
    let mut st = store(&m, vec![parent("p", "Open")]);
    let bindings = json!({"p":{"entity":"Parent","id":"p"}});
    let at = st.current().unwrap();
    let stop = invoke(&st, &m, "stop", bindings.clone(), json!({}))
        .bundle
        .unwrap();
    let create = invoke(
        &st,
        &m,
        "create_child",
        bindings.clone(),
        json!({"new_id":"c"}),
    )
    .bundle
    .unwrap();
    st.commit(&m, &at, &create).unwrap();
    assert_eq!(
        st.commit(&m, &at, &stop).unwrap_err().code(),
        "STATE_CONFLICT"
    );
    assert!(
        invoke(&st, &m, "stop", bindings, json!({}))
            .bundle
            .is_none()
    );
}
