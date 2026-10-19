#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_models::safety::analyze_with_dependencies;
use serde_json::json;
use support::*;

#[test]
fn safe_lifecycle_operations_and_core_ref_constraints_match_independent_oracle() {
    let m = module(&Options::default());
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    assert!(a.issues().is_empty());
    let mut st = store(
        &m,
        vec![parent("p1", "Open"), child("Child", "c1", "p1", "Active")],
    );
    assert!(property(&snapshot(&st, &m)));
    let binding = |entity, id| json!({"entity":entity,"id":id});
    assert!(
        invoke(
            &st,
            &m,
            "stop",
            json!({"p":binding("Parent","p1")}),
            json!({})
        )
        .bundle
        .is_none()
    );
    for (action, bindings, input) in [
        ("finish", json!({"c":binding("Child","c1")}), json!({})),
        ("stop", json!({"p":binding("Parent","p1")}), json!({})),
    ] {
        let bundle = invoke(&st, &m, action, bindings, input).bundle.unwrap();
        st.commit(&m, &st.current().unwrap(), &bundle).unwrap();
        assert!(property(&snapshot(&st, &m)));
    }
    // A terminal child may reference Gone, but its reference still blocks physical removal.
    assert!(
        invoke(
            &st,
            &m,
            "remove_parent",
            json!({"p":binding("Parent","p1")}),
            json!({})
        )
        .bundle
        .is_none()
    );
    assert!(
        invoke(
            &st,
            &m,
            "activate",
            json!({"p":binding("Parent","p1"),"c":binding("Child","c1")}),
            json!({})
        )
        .bundle
        .is_none()
    );
    assert!(
        invoke(
            &st,
            &m,
            "create_child",
            json!({"p":binding("Parent","p1")}),
            json!({"new_id":"c2"})
        )
        .bundle
        .is_none()
    );
    for (action, bindings, input) in [
        (
            "remove_child",
            json!({"c":binding("Child","c1")}),
            json!({}),
        ),
        (
            "remove_parent",
            json!({"p":binding("Parent","p1")}),
            json!({}),
        ),
        ("create_parent", json!({}), json!({"new_id":"p2"})),
        (
            "create_child",
            json!({"p":binding("Parent","p2")}),
            json!({"new_id":"c2"}),
        ),
    ] {
        let bundle = invoke(&st, &m, action, bindings, input).bundle.unwrap();
        st.commit(&m, &st.current().unwrap(), &bundle).unwrap();
        assert!(property(&snapshot(&st, &m)));
    }
}

#[test]
fn atomic_mixed_effects_can_break_property_despite_safe_source_guard_and_are_uncertifiable() {
    let m = module(&Options {
        mixed_creation: true,
        ..Options::default()
    });
    let analysis = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    assert!(!analysis.issues().is_empty());
    let mut st = store(&m, vec![parent("p1", "Open")]);
    assert!(property(&snapshot(&st, &m)));
    let attempt = invoke(
        &st,
        &m,
        "mixed",
        json!({"p":{"entity":"Parent","id":"p1"}}),
        json!({"new_id":"c1"}),
    );
    st.commit(&m, &st.current().unwrap(), &attempt.bundle.unwrap())
        .unwrap();
    assert!(
        !property(&snapshot(&st, &m)),
        "The source-only guard must not conceal the resulting-state violation"
    );
}

#[test]
fn conflicting_commits_require_reevaluation_and_core_replay_stays_ordinary() {
    let m = module(&Options::default());
    analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let mut st = store(&m, vec![parent("p1", "Open")]);
    let bindings = json!({"p":{"entity":"Parent","id":"p1"}});
    let old = st.current().unwrap();
    let stop = invoke(&st, &m, "stop", bindings.clone(), json!({}))
        .bundle
        .unwrap();
    let create = invoke(
        &st,
        &m,
        "create_child",
        bindings.clone(),
        json!({"new_id":"c1"}),
    )
    .bundle
    .unwrap();
    st.commit(&m, &old, &create).unwrap();
    assert!(format!("{:?}", st.commit(&m, &old, &stop)).contains("Conflict"));
    assert!(
        invoke(&st, &m, "stop", bindings, json!({}))
            .bundle
            .is_none()
    );
    assert!(property(&snapshot(&st, &m)));
}
