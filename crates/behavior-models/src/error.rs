//! The typed error catalogue of model builds (lowering contract §6).
//!
//! Every error has a stable `code`. A build reports all errors it found, sorted by
//! `(stage, code, subject, detail)` and without duplicates, so equal inputs always give an
//! identical error list.

use serde_json::{Value, json};

/// The build stage an error belongs to, in pipeline order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Resolve,
    Set,
    Lower,
    Finish,
    Compose,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("{model_ref}: `{id}` is not a semantic id ([a-z][a-z0-9]*(_[a-z0-9]+)*)")]
    InvalidSemanticId { model_ref: String, id: String },
    #[error("{model_ref}: unknown entity `{entity}`")]
    UnknownEntity { model_ref: String, entity: String },
    #[error("{model_ref}: `{entity}.{field}` is not a field of an enum type")]
    StateFieldNotEnum {
        model_ref: String,
        entity: String,
        field: String,
    },
    #[error("{model_ref}: `{state}` is not a case of `{enum_name}`")]
    UnknownState {
        model_ref: String,
        enum_name: String,
        state: String,
    },
    #[error("{model_ref}: a state machine needs at least one entry state")]
    EmptyEntryStates { model_ref: String },
    #[error("{model_ref}: a transition needs at least one source state")]
    EmptySourceStates { model_ref: String },
    #[error("{model_ref}: transition id declared more than once")]
    DuplicateTransitionId { model_ref: String },
    #[error("{model_ref}: invalid or duplicate parameter `{name}`")]
    InvalidParameter { model_ref: String, name: String },
    #[error("model id `{model_id}` is used by more than one model")]
    DuplicateModelId { model_id: String },
    #[error("`{entity}.{field}` is claimed by more than one model: {models:?}")]
    ConflictingOwnership {
        entity: String,
        field: String,
        models: Vec<String>,
    },
    #[error("`{name}` is contributed incompatibly by {models:?}")]
    IncompatibleContribution { name: String, models: Vec<String> },
    #[error("{model_ref}: {core_code}: {core_message}")]
    FragmentInvalid {
        model_ref: String,
        core_code: String,
        core_message: String,
    },
    #[error("{model_ref}: an extra effect writes the machine-owned state field")]
    TransitionWritesOwnedField { model_ref: String },
    #[error("{model_ref}: a self-transition without any other effect or command does nothing")]
    PureSelfTransition { model_ref: String },
    #[error("{model_ref}: generated action name `{name}` is already taken")]
    GeneratedNameCollision { model_ref: String, name: String },
    #[error("Core did not admit the module: {errors:?}")]
    CoreAdmissionFailed { errors: Vec<(String, String)> },
    #[error("`{action}` writes `{entity}.{field}`, which only state machine `{owner}` may change")]
    UnauthorizedOwnedFieldWrite {
        action: String,
        entity: String,
        field: String,
        owner: String,
    },
    #[error("`{action}` creates `{entity}` with a `{field}` that is not a literal entry state")]
    EntryStateNotStatic {
        action: String,
        entity: String,
        field: String,
    },
    #[error("`{action}` creates `{entity}` with `{field} = {state}`, which is not an entry state")]
    EntryStateNotAllowed {
        action: String,
        entity: String,
        field: String,
        state: String,
    },
}

impl ModelError {
    pub fn code(&self) -> &'static str {
        match self {
            ModelError::InvalidSemanticId { .. } => "INVALID_SEMANTIC_ID",
            ModelError::UnknownEntity { .. } => "UNKNOWN_ENTITY",
            ModelError::StateFieldNotEnum { .. } => "STATE_FIELD_NOT_ENUM",
            ModelError::UnknownState { .. } => "UNKNOWN_STATE",
            ModelError::EmptyEntryStates { .. } => "EMPTY_ENTRY_STATES",
            ModelError::EmptySourceStates { .. } => "EMPTY_SOURCE_STATES",
            ModelError::DuplicateTransitionId { .. } => "DUPLICATE_TRANSITION_ID",
            ModelError::InvalidParameter { .. } => "INVALID_PARAMETER",
            ModelError::DuplicateModelId { .. } => "DUPLICATE_MODEL_ID",
            ModelError::ConflictingOwnership { .. } => "CONFLICTING_OWNERSHIP",
            ModelError::IncompatibleContribution { .. } => "INCOMPATIBLE_CONTRIBUTION",
            ModelError::FragmentInvalid { .. } => "FRAGMENT_INVALID",
            ModelError::TransitionWritesOwnedField { .. } => "TRANSITION_WRITES_OWNED_FIELD",
            ModelError::PureSelfTransition { .. } => "PURE_SELF_TRANSITION",
            ModelError::GeneratedNameCollision { .. } => "GENERATED_NAME_COLLISION",
            ModelError::CoreAdmissionFailed { .. } => "CORE_ADMISSION_FAILED",
            ModelError::UnauthorizedOwnedFieldWrite { .. } => "UNAUTHORIZED_OWNED_FIELD_WRITE",
            ModelError::EntryStateNotStatic { .. } => "ENTRY_STATE_NOT_STATIC",
            ModelError::EntryStateNotAllowed { .. } => "ENTRY_STATE_NOT_ALLOWED",
        }
    }

    pub fn stage(&self) -> Stage {
        match self {
            ModelError::InvalidSemanticId { .. }
            | ModelError::UnknownEntity { .. }
            | ModelError::StateFieldNotEnum { .. }
            | ModelError::UnknownState { .. }
            | ModelError::EmptyEntryStates { .. }
            | ModelError::EmptySourceStates { .. }
            | ModelError::DuplicateTransitionId { .. }
            | ModelError::InvalidParameter { .. } => Stage::Resolve,
            ModelError::DuplicateModelId { .. }
            | ModelError::ConflictingOwnership { .. }
            | ModelError::IncompatibleContribution { .. } => Stage::Set,
            ModelError::FragmentInvalid { .. }
            | ModelError::TransitionWritesOwnedField { .. }
            | ModelError::PureSelfTransition { .. }
            | ModelError::GeneratedNameCollision { .. } => Stage::Lower,
            ModelError::CoreAdmissionFailed { .. } => Stage::Finish,
            ModelError::UnauthorizedOwnedFieldWrite { .. }
            | ModelError::EntryStateNotStatic { .. }
            | ModelError::EntryStateNotAllowed { .. } => Stage::Compose,
        }
    }

    /// The error as a JSON object: `code` plus its payload fields.
    pub fn to_json(&self) -> Value {
        let mut v = match self {
            ModelError::InvalidSemanticId { model_ref, id } => {
                json!({"model_ref": model_ref, "id": id})
            }
            ModelError::UnknownEntity { model_ref, entity } => {
                json!({"model_ref": model_ref, "entity": entity})
            }
            ModelError::StateFieldNotEnum {
                model_ref,
                entity,
                field,
            } => json!({"model_ref": model_ref, "entity": entity, "field": field}),
            ModelError::UnknownState {
                model_ref,
                enum_name,
                state,
            } => json!({"model_ref": model_ref, "enum": enum_name, "state": state}),
            ModelError::EmptyEntryStates { model_ref }
            | ModelError::EmptySourceStates { model_ref }
            | ModelError::DuplicateTransitionId { model_ref }
            | ModelError::TransitionWritesOwnedField { model_ref }
            | ModelError::PureSelfTransition { model_ref } => json!({"model_ref": model_ref}),
            ModelError::InvalidParameter { model_ref, name } => {
                json!({"model_ref": model_ref, "name": name})
            }
            ModelError::DuplicateModelId { model_id } => json!({"model_id": model_id}),
            ModelError::ConflictingOwnership {
                entity,
                field,
                models,
            } => json!({"entity": entity, "field": field, "models": models}),
            ModelError::IncompatibleContribution { name, models } => {
                json!({"name": name, "models": models})
            }
            ModelError::FragmentInvalid {
                model_ref,
                core_code,
                core_message,
            } => {
                json!({"model_ref": model_ref, "core_code": core_code, "core_message": core_message})
            }
            ModelError::GeneratedNameCollision { model_ref, name } => {
                json!({"model_ref": model_ref, "name": name})
            }
            ModelError::CoreAdmissionFailed { errors } => json!({
                "errors": errors.iter().map(|(c, m)| json!({"code": c, "message": m})).collect::<Vec<_>>()
            }),
            ModelError::UnauthorizedOwnedFieldWrite {
                action,
                entity,
                field,
                owner,
            } => json!({"action": action, "entity": entity, "field": field, "owner": owner}),
            ModelError::EntryStateNotStatic {
                action,
                entity,
                field,
            } => json!({"action": action, "entity": entity, "field": field}),
            ModelError::EntryStateNotAllowed {
                action,
                entity,
                field,
                state,
            } => json!({"action": action, "entity": entity, "field": field, "state": state}),
        };
        if let Value::Object(m) = &mut v {
            m.insert("code".into(), json!(self.code()));
            m.insert("message".into(), json!(self.to_string()));
        }
        v
    }

    /// The deterministic sort key `(stage, code, subject, detail)`.
    fn key(&self) -> (Stage, &'static str, String) {
        (self.stage(), self.code(), self.to_json().to_string())
    }
}

impl PartialOrd for ModelError {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModelError {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key().cmp(&other.key())
    }
}

/// Sorted and deduplicated: the canonical form of an error list.
pub fn sorted_unique(mut errors: Vec<ModelError>) -> Vec<ModelError> {
    errors.sort();
    errors.dedup();
    errors
}
