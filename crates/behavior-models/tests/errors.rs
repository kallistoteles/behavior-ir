//! The error catalogue sorts deterministically and without duplicates (lowering contract §6).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

use behavior_models::error::{ModelError, sorted_unique};

fn all() -> Vec<ModelError> {
    let m = || "m".to_string();
    vec![
        ModelError::InvalidSemanticId {
            model_ref: m(),
            id: "A".into(),
        },
        ModelError::UnknownEntity {
            model_ref: m(),
            entity: "E".into(),
        },
        ModelError::StateFieldNotEnum {
            model_ref: m(),
            entity: "E".into(),
            field: "f".into(),
        },
        ModelError::UnknownState {
            model_ref: m(),
            enum_name: "T".into(),
            state: "X".into(),
        },
        ModelError::EmptyEntryStates { model_ref: m() },
        ModelError::EmptySourceStates { model_ref: m() },
        ModelError::DuplicateTransitionId { model_ref: m() },
        ModelError::InvalidParameter {
            model_ref: m(),
            name: "1".into(),
        },
        ModelError::DuplicateModelId { model_id: m() },
        ModelError::ConflictingOwnership {
            entity: "E".into(),
            field: "f".into(),
            models: vec!["a".into(), "b".into()],
        },
        ModelError::IncompatibleContribution {
            name: "x".into(),
            models: vec![m()],
        },
        ModelError::FragmentInvalid {
            model_ref: m(),
            core_code: "NOT_BOOLEAN".into(),
            core_message: "x".into(),
        },
        ModelError::TransitionWritesOwnedField { model_ref: m() },
        ModelError::PureSelfTransition { model_ref: m() },
        ModelError::GeneratedNameCollision {
            model_ref: m(),
            name: "m__t".into(),
        },
        ModelError::CoreAdmissionFailed {
            errors: vec![("C".into(), "msg".into())],
        },
        ModelError::UnauthorizedOwnedFieldWrite {
            action: "a".into(),
            entity: "E".into(),
            field: "f".into(),
            owner: m(),
        },
        ModelError::EntryStateNotStatic {
            action: "a".into(),
            entity: "E".into(),
            field: "f".into(),
        },
        ModelError::EntryStateNotAllowed {
            action: "a".into(),
            entity: "E".into(),
            field: "f".into(),
            state: "X".into(),
        },
    ]
}

#[test]
fn every_code_is_distinct_and_stable() {
    let codes: Vec<&str> = all().iter().map(ModelError::code).collect();
    let mut unique = codes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), codes.len());
    assert!(codes.contains(&"UNAUTHORIZED_OWNED_FIELD_WRITE"));
}

#[test]
fn sorting_is_deterministic_and_removes_duplicates() {
    let expected = sorted_unique(all());
    let mut shuffled = all();
    shuffled.reverse();
    shuffled.extend(all());
    shuffled.rotate_left(7);
    assert_eq!(sorted_unique(shuffled), expected);
    assert_eq!(expected.len(), all().len());
    // Stages are ordered resolve < set < lower < finish < compose.
    assert_eq!(
        expected.first().map(ModelError::code),
        Some("DUPLICATE_TRANSITION_ID")
    );
    assert_eq!(
        expected.last().map(ModelError::code),
        Some("UNAUTHORIZED_OWNED_FIELD_WRITE")
    );
}

#[test]
fn errors_render_as_json_with_their_code() {
    let e = ModelError::UnknownState {
        model_ref: "m".into(),
        enum_name: "T".into(),
        state: "X".into(),
    };
    let j = e.to_json();
    assert_eq!(j["code"], "UNKNOWN_STATE");
    assert_eq!(j["state"], "X");
}
