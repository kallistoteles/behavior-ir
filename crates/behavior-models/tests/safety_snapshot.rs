#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/lifecycle.rs"]
mod support;
use behavior_models::safety::{
    analyze_with_dependencies,
    snapshot::{inspect_snapshot, validate_store_snapshot},
};
use serde_json::json;
use support::*;

#[test]
fn original_predicate_all_dependencies_and_exact_anchor() {
    for two in [false, true] {
        let m = module(&Options {
            two_children: two,
            ..Options::default()
        });
        let deps = if two {
            vec![dependency("Child"), dependency("OtherChild")]
        } else {
            vec![dependency("Child")]
        };
        let a = analyze_with_dependencies(&m, &ownership(&m), &deps).unwrap();
        for (phase, child_phase, valid) in [
            ("Open", "Active", true),
            ("Gone", "Terminal", true),
            ("Gone", "Active", false),
            ("Abandoned", "Active", false),
        ] {
            let mut rows = vec![
                parent("same", "Open"),
                parent("p", phase),
                child(
                    if two { "OtherChild" } else { "Child" },
                    "same",
                    "p",
                    child_phase,
                ),
            ];
            // Same textual IDs in different entity types must not alias.
            if two {
                rows.push(child("Child", "same", "same", "Active"));
            }
            assert_eq!(property(&rows), valid);
            let st = store(&m, rows);
            let at = st.current_history().unwrap();
            let r = validate_store_snapshot(&m, &a, &st, &at);
            assert_eq!(r.status(), if valid { "Valid" } else { "Invalid" });
            let doc = r.to_json();
            assert_eq!(doc["history"], json!(at));
            assert_eq!(doc["policy_set_hash"], a.policy_set_hash());
            assert_eq!(doc["initial_validity_scope"], "OnlyThisHistoryAnchor");
            assert_eq!(doc["certificate"], serde_json::Value::Null);
            assert_eq!(r.matches_anchor(&m, &a, &at), valid);
            let mut stale = at.clone();
            stale.position += 1;
            assert!(!r.matches_anchor(&m, &a, &stale));
        }
    }
}
#[test]
fn detached_data_never_establishes_provenance_even_with_true_property_or_forged_metadata() {
    let m = module(&Options::default());
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let st = store(
        &m,
        vec![parent("p", "Gone"), child("Child", "c", "p", "Terminal")],
    );
    let at = st.current_history().unwrap();
    let export = st.export_seed_at(&m, &at).unwrap();
    let mut variants = vec![export.clone()];
    let mut v = export.clone();
    v.seed.remove(0);
    variants.push(v); // missing parent or child: never trust either
    let mut v = export.clone();
    v.seed.push(v.seed[0].clone());
    variants.push(v);
    let mut v = export.clone();
    v.seed
        .iter_mut()
        .find(|e| e.entity == "Parent")
        .unwrap()
        .value["phase"] = json!("Unknown");
    variants.push(v);
    let mut v = export.clone();
    v.seed
        .iter_mut()
        .find(|e| e.entity == "Child")
        .unwrap()
        .value["owner"] = json!(false);
    variants.push(v);
    let mut v = export.clone();
    v.seed.retain(|e| e.entity != "Parent");
    variants.push(v); // terminal child still needs parent
    let mut v = export.clone();
    v.schema = format!("sha256:{}", "a".repeat(64));
    variants.push(v);
    let mut v = export.clone();
    v.state = format!("sha256:{}", "b".repeat(64));
    variants.push(v);
    let mut v = export.clone();
    v.source_history.record = format!("sha256:{}", "c".repeat(64));
    variants.push(v);
    let mut v = export.clone();
    v.source_history.store = format!("sha256:{}", "d".repeat(64));
    variants.push(v);
    let mut v = export.clone();
    v.source_history.position += 1;
    variants.push(v);
    for v in variants {
        let r = inspect_snapshot(&m, &a, &v);
        assert_eq!(r.status(), "Unestablished");
        assert!(!r.matches_anchor(&m, &a, &at));
    }
    let mut missing_parent = export;
    missing_parent.seed.retain(|e| e.entity != "Parent");
    assert!(
        !inspect_snapshot(&m, &a, &missing_parent).to_json()["findings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn changed_module_policy_or_history_is_not_the_original_validation() {
    let m = module(&Options::default());
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let st = store(&m, vec![parent("p", "Open")]);
    let at = st.current_history().unwrap();
    let r = validate_store_snapshot(&m, &a, &st, &at);
    let other = module(&Options {
        missing_guard: true,
        ..Options::default()
    });
    let other_a =
        analyze_with_dependencies(&other, &ownership(&other), &[dependency("Child")]).unwrap();
    assert!(!r.matches_anchor(&other, &other_a, &at));
    let no_deps = analyze_with_dependencies(&m, &ownership(&m), &[]).unwrap();
    assert!(!r.matches_anchor(&m, &no_deps, &at));
    for key in ["state", "store", "record"] {
        let mut doc = json!(at);
        doc[key] = json!(format!("sha256:{}", "a".repeat(64)));
        let changed = serde_json::from_value(doc).unwrap();
        assert_eq!(
            validate_store_snapshot(&m, &a, &st, &changed).status(),
            "Unestablished"
        );
    }
}

#[test]
fn state_record_identity_excludes_itself_and_transported_flags_have_no_authority() {
    let m = module(&Options::default());
    let a = analyze_with_dependencies(&m, &ownership(&m), &[dependency("Child")]).unwrap();
    let st = store(&m, vec![parent("p", "Open")]);
    let at = st.current_history().unwrap();
    let record = validate_store_snapshot(&m, &a, &st, &at);
    let mut doc = record.to_json();
    let hash = doc
        .as_object_mut()
        .unwrap()
        .remove("validation_hash")
        .unwrap();
    assert_eq!(
        hash,
        behavior_engine::canonical::tagged_hash("behavior.safety_state_validation.v1", &doc)
            .unwrap()
    );
    doc["status"] = json!("Invalid");
    assert_eq!(record.status(), "Valid");
    assert!(record.matches_anchor(&m, &a, &at));
    assert_eq!(
        validate_store_snapshot(&m, &a, &st, &at).to_json(),
        record.to_json()
    );
}

#[test]
fn no_safety_property_is_not_a_validation_claim() {
    let m = module(&Options::default());
    let a = analyze_with_dependencies(&m, &[], &[]).unwrap();
    let st = store(&m, vec![parent("p", "Open")]);
    assert_eq!(
        validate_store_snapshot(&m, &a, &st, &st.current_history().unwrap()).status(),
        "Unestablished"
    );
}
