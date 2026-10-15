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
        ModelError::UnknownEnum {
            model_ref: m(),
            name: "T".into(),
        },
        ModelError::ReservedField {
            model_ref: m(),
            name: "status".into(),
        },
        ModelError::NoCreation { model_ref: m() },
        ModelError::CapabilityIdCollision {
            model_ref: m(),
            id: "submit".into(),
        },
        ModelError::InitializerMissingField {
            model_ref: m(),
            field: "title".into(),
        },
        ModelError::InitializerUnknownField {
            model_ref: m(),
            field: "x".into(),
        },
        ModelError::InitializerSetsStatus { model_ref: m() },
        ModelError::InitializerDuplicateField {
            model_ref: m(),
            field: "title".into(),
        },
        ModelError::DeclarationCollision {
            name: "E".into(),
            model_ref: m(),
            existing_kind: "enum".into(),
        },
        ModelError::DeclarationMismatch {
            name: "E".into(),
            model_ref: m(),
        },
        ModelError::UnauthorizedRequestCreation {
            action: "a".into(),
            entity: "E".into(),
            owner: m(),
        },
        ModelError::UnauthorizedRequesterWrite {
            action: "a".into(),
            entity: "E".into(),
            owner: m(),
        },
        ModelError::InvalidThreshold {
            model_ref: m(),
            which: "approve".into(),
            value: 0,
        },
        ModelError::DeclarationCycle {
            models: vec!["a".into(), "b".into()],
        },
        ModelError::DefinitionCycle {
            names: vec!["f".into(), "g".into()],
        },
        ModelError::DefinitionMismatch {
            name: "m__approved".into(),
            model_ref: m(),
        },
        ModelError::UnauthorizedDecisionCreation {
            action: "a".into(),
            entity: "MDecision".into(),
            owner: m(),
        },
        ModelError::UnauthorizedDecisionWrite {
            action: "a".into(),
            entity: "MDecision".into(),
            field: "choice".into(),
            owner: m(),
        },
        ModelError::UnauthorizedDecisionRemoval {
            action: "a".into(),
            entity: "MDecision".into(),
            owner: m(),
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
    // Feature 503 (T004).
    for code in [
        "INVALID_THRESHOLD",
        "DECLARATION_CYCLE",
        "DEFINITION_CYCLE",
        "DEFINITION_MISMATCH",
        "UNAUTHORIZED_DECISION_CREATION",
        "UNAUTHORIZED_DECISION_WRITE",
        "UNAUTHORIZED_DECISION_REMOVAL",
    ] {
        assert!(codes.contains(&code), "{code}");
    }
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
        Some("CAPABILITY_ID_COLLISION")
    );
    assert_eq!(
        expected.last().map(ModelError::code),
        Some("UNAUTHORIZED_REQUEST_CREATION")
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

/// Feature 503 (T004): the new codes carry their stage and payload.
#[test]
fn approval_and_dag_errors_have_their_stage_and_payload() {
    use behavior_models::error::Stage;
    let by_code = |code: &str| all().into_iter().find(|e| e.code() == code).unwrap();
    assert_eq!(by_code("INVALID_THRESHOLD").stage(), Stage::Resolve);
    for code in [
        "DECLARATION_CYCLE",
        "DEFINITION_CYCLE",
        "DEFINITION_MISMATCH",
    ] {
        assert_eq!(by_code(code).stage(), Stage::Set, "{code}");
    }
    for code in [
        "UNAUTHORIZED_DECISION_CREATION",
        "UNAUTHORIZED_DECISION_WRITE",
        "UNAUTHORIZED_DECISION_REMOVAL",
    ] {
        assert_eq!(by_code(code).stage(), Stage::Compose, "{code}");
    }
    let j = by_code("INVALID_THRESHOLD").to_json();
    assert_eq!(
        (j["which"].as_str(), j["value"].as_i64()),
        (Some("approve"), Some(0))
    );
    assert_eq!(
        by_code("DECLARATION_CYCLE").to_json()["models"],
        serde_json::json!(["a", "b"])
    );
    assert_eq!(
        by_code("DEFINITION_CYCLE").to_json()["names"],
        serde_json::json!(["f", "g"])
    );
    assert_eq!(
        by_code("UNAUTHORIZED_DECISION_WRITE").to_json()["field"],
        "choice"
    );
}
