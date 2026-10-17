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
    // --- RequestFlow (feature 502) ------------------------------------------------------------
    #[error("{model_ref}: unknown enum `{name}` (the status enum must be declared by the author)")]
    UnknownEnum { model_ref: String, name: String },
    #[error("{model_ref}: `{name}` is a reserved request field")]
    ReservedField { model_ref: String, name: String },
    #[error("{model_ref}: a request flow needs at least one creation")]
    NoCreation { model_ref: String },
    #[error("{model_ref}: `{id}` is used by more than one creation or transition")]
    CapabilityIdCollision { model_ref: String, id: String },
    #[error("{model_ref}: the initializer does not set `{field}`")]
    InitializerMissingField { model_ref: String, field: String },
    #[error("{model_ref}: the initializer sets unknown field `{field}`")]
    InitializerUnknownField { model_ref: String, field: String },
    #[error(
        "{model_ref}: the initializer must not set `status`; it comes from the creation's target"
    )]
    InitializerSetsStatus { model_ref: String },
    #[error("{model_ref}: the initializer sets `{field}` more than once")]
    InitializerDuplicateField { model_ref: String, field: String },
    #[error("{model_ref}: `{name}` is already declared as {existing_kind}")]
    DeclarationCollision {
        name: String,
        model_ref: String,
        existing_kind: String,
    },
    #[error("{model_ref}: the declaration of `{name}` differs from the one the model contributes")]
    DeclarationMismatch { name: String, model_ref: String },
    #[error("`{action}` creates `{entity}`, which only flow `{owner}` may create")]
    UnauthorizedRequestCreation {
        action: String,
        entity: String,
        owner: String,
    },
    #[error("`{action}` writes `{entity}.requester`, which is fixed at creation by flow `{owner}`")]
    UnauthorizedRequesterWrite {
        action: String,
        entity: String,
        owner: String,
    },
    // --- Approval and the model-declaration DAG (feature 503) ---------------------------------
    #[error("{model_ref}: the {which} threshold must be at least 1, found {value}")]
    InvalidThreshold {
        model_ref: String,
        which: String,
        value: i64,
    },
    #[error("the generated declarations of {models:?} depend on each other in a cycle")]
    DeclarationCycle { models: Vec<String> },
    #[error("the generated definitions {names:?} depend on each other in a cycle")]
    DefinitionCycle { names: Vec<String> },
    #[error("{model_ref}: the definition of `{name}` differs from the one the model contributes")]
    DefinitionMismatch { name: String, model_ref: String },
    #[error("`{action}` creates `{entity}`, which only approval `{owner}` may create")]
    UnauthorizedDecisionCreation {
        action: String,
        entity: String,
        owner: String,
    },
    #[error("`{action}` writes `{entity}.{field}`: decisions of approval `{owner}` are immutable")]
    UnauthorizedDecisionWrite {
        action: String,
        entity: String,
        field: String,
        owner: String,
    },
    #[error("`{action}` removes `{entity}`: decisions of approval `{owner}` are never removed")]
    UnauthorizedDecisionRemoval {
        action: String,
        entity: String,
        owner: String,
    },
    // --- WorkQueue (feature 507, research R4, R5) -----------------------------------------------
    #[error("{model_ref}: max_attempts must be between 1 and i64::MAX - 1, found {value}")]
    InvalidMaxAttempts { model_ref: String, value: i64 },
    #[error("{model_ref}: the time type `{time}` must be a nominal declared with `order`")]
    TimeNotOrdered { model_ref: String, time: String },
    #[error("{model_ref}: the capability needs an authored authorization guard")]
    MissingAuthorization { model_ref: String },
    #[error("{model_ref}: `{name}` is a parameter name the work queue generates")]
    ReservedParameter { model_ref: String, name: String },
    #[error("{model_ref}: `{name}` is a field the work queue generates")]
    PayloadFieldReserved { model_ref: String, name: String },
    #[error("{model_ref}: the capability `{capability}` is not declared")]
    MissingCapability {
        model_ref: String,
        capability: String,
    },
    #[error("{model_ref}: `{name}` binds the queue's own work entity")]
    WorkItemBinding { model_ref: String, name: String },
    #[error("{model_ref}: an authored effect writes `{field}` of the work item")]
    AuthoredWorkEffect { model_ref: String, field: String },
    #[error("`{action}` creates `{entity}`, which only work queue `{owner}` may create")]
    UnauthorizedWorkCreation {
        action: String,
        entity: String,
        owner: String,
    },
    #[error("`{action}` writes `{entity}.{field}`, which work queue `{owner}` owns")]
    UnauthorizedWorkWrite {
        action: String,
        entity: String,
        field: String,
        owner: String,
    },
    #[error("`{action}` removes `{entity}`: work items of `{owner}` are never removed")]
    UnauthorizedWorkRemoval {
        action: String,
        entity: String,
        owner: String,
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
            ModelError::UnknownEnum { .. } => "UNKNOWN_ENUM",
            ModelError::ReservedField { .. } => "RESERVED_FIELD",
            ModelError::NoCreation { .. } => "NO_CREATION",
            ModelError::CapabilityIdCollision { .. } => "CAPABILITY_ID_COLLISION",
            ModelError::InitializerMissingField { .. } => "INITIALIZER_MISSING_FIELD",
            ModelError::InitializerUnknownField { .. } => "INITIALIZER_UNKNOWN_FIELD",
            ModelError::InitializerSetsStatus { .. } => "INITIALIZER_SETS_STATUS",
            ModelError::InitializerDuplicateField { .. } => "INITIALIZER_DUPLICATE_FIELD",
            ModelError::DeclarationCollision { .. } => "DECLARATION_COLLISION",
            ModelError::DeclarationMismatch { .. } => "DECLARATION_MISMATCH",
            ModelError::UnauthorizedRequestCreation { .. } => "UNAUTHORIZED_REQUEST_CREATION",
            ModelError::UnauthorizedRequesterWrite { .. } => "UNAUTHORIZED_REQUESTER_WRITE",
            ModelError::InvalidThreshold { .. } => "INVALID_THRESHOLD",
            ModelError::DeclarationCycle { .. } => "DECLARATION_CYCLE",
            ModelError::DefinitionCycle { .. } => "DEFINITION_CYCLE",
            ModelError::DefinitionMismatch { .. } => "DEFINITION_MISMATCH",
            ModelError::UnauthorizedDecisionCreation { .. } => "UNAUTHORIZED_DECISION_CREATION",
            ModelError::UnauthorizedDecisionWrite { .. } => "UNAUTHORIZED_DECISION_WRITE",
            ModelError::UnauthorizedDecisionRemoval { .. } => "UNAUTHORIZED_DECISION_REMOVAL",
            ModelError::InvalidMaxAttempts { .. } => "INVALID_MAX_ATTEMPTS",
            ModelError::TimeNotOrdered { .. } => "TIME_NOT_ORDERED",
            ModelError::MissingAuthorization { .. } => "MISSING_AUTHORIZATION",
            ModelError::ReservedParameter { .. } => "RESERVED_PARAMETER",
            ModelError::PayloadFieldReserved { .. } => "PAYLOAD_FIELD_RESERVED",
            ModelError::MissingCapability { .. } => "MISSING_CAPABILITY",
            ModelError::WorkItemBinding { .. } => "WORK_ITEM_BINDING",
            ModelError::AuthoredWorkEffect { .. } => "AUTHORED_WORK_EFFECT",
            ModelError::UnauthorizedWorkCreation { .. } => "UNAUTHORIZED_WORK_CREATION",
            ModelError::UnauthorizedWorkWrite { .. } => "UNAUTHORIZED_WORK_WRITE",
            ModelError::UnauthorizedWorkRemoval { .. } => "UNAUTHORIZED_WORK_REMOVAL",
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
            | ModelError::EntryStateNotAllowed { .. }
            | ModelError::UnauthorizedRequestCreation { .. }
            | ModelError::UnauthorizedRequesterWrite { .. } => Stage::Compose,
            ModelError::UnknownEnum { .. }
            | ModelError::ReservedField { .. }
            | ModelError::NoCreation { .. }
            | ModelError::CapabilityIdCollision { .. } => Stage::Resolve,
            ModelError::DeclarationCollision { .. } | ModelError::DeclarationMismatch { .. } => {
                Stage::Set
            }
            ModelError::InitializerMissingField { .. }
            | ModelError::InitializerUnknownField { .. }
            | ModelError::InitializerSetsStatus { .. }
            | ModelError::InitializerDuplicateField { .. } => Stage::Lower,
            ModelError::InvalidThreshold { .. } => Stage::Resolve,
            ModelError::DeclarationCycle { .. }
            | ModelError::DefinitionCycle { .. }
            | ModelError::DefinitionMismatch { .. } => Stage::Set,
            ModelError::UnauthorizedDecisionCreation { .. }
            | ModelError::UnauthorizedDecisionWrite { .. }
            | ModelError::UnauthorizedDecisionRemoval { .. } => Stage::Compose,
            ModelError::InvalidMaxAttempts { .. }
            | ModelError::TimeNotOrdered { .. }
            | ModelError::ReservedParameter { .. }
            | ModelError::PayloadFieldReserved { .. }
            | ModelError::MissingCapability { .. }
            | ModelError::WorkItemBinding { .. } => Stage::Resolve,
            ModelError::MissingAuthorization { .. } | ModelError::AuthoredWorkEffect { .. } => {
                Stage::Lower
            }
            ModelError::UnauthorizedWorkCreation { .. }
            | ModelError::UnauthorizedWorkWrite { .. }
            | ModelError::UnauthorizedWorkRemoval { .. } => Stage::Compose,
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
            ModelError::UnknownEnum { model_ref, name }
            | ModelError::ReservedField { model_ref, name } => {
                json!({"model_ref": model_ref, "name": name})
            }
            ModelError::NoCreation { model_ref }
            | ModelError::InitializerSetsStatus { model_ref } => {
                json!({"model_ref": model_ref})
            }
            ModelError::CapabilityIdCollision { model_ref, id } => {
                json!({"model_ref": model_ref, "id": id})
            }
            ModelError::InitializerMissingField { model_ref, field }
            | ModelError::InitializerUnknownField { model_ref, field }
            | ModelError::InitializerDuplicateField { model_ref, field } => {
                json!({"model_ref": model_ref, "field": field})
            }
            ModelError::DeclarationCollision {
                name,
                model_ref,
                existing_kind,
            } => json!({"name": name, "model_ref": model_ref, "existing_kind": existing_kind}),
            ModelError::DeclarationMismatch { name, model_ref } => {
                json!({"name": name, "model_ref": model_ref})
            }
            ModelError::UnauthorizedRequestCreation {
                action,
                entity,
                owner,
            }
            | ModelError::UnauthorizedRequesterWrite {
                action,
                entity,
                owner,
            } => json!({"action": action, "entity": entity, "owner": owner}),
            ModelError::InvalidThreshold {
                model_ref,
                which,
                value,
            } => json!({"model_ref": model_ref, "which": which, "value": value}),
            ModelError::DeclarationCycle { models } => json!({"models": models}),
            ModelError::DefinitionCycle { names } => json!({"names": names}),
            ModelError::DefinitionMismatch { name, model_ref } => {
                json!({"name": name, "model_ref": model_ref})
            }
            ModelError::UnauthorizedDecisionCreation {
                action,
                entity,
                owner,
            }
            | ModelError::UnauthorizedDecisionRemoval {
                action,
                entity,
                owner,
            } => json!({"action": action, "entity": entity, "owner": owner}),
            ModelError::UnauthorizedDecisionWrite {
                action,
                entity,
                field,
                owner,
            } => json!({"action": action, "entity": entity, "field": field, "owner": owner}),
            ModelError::InvalidMaxAttempts { model_ref, value } => {
                json!({"model_ref": model_ref, "value": value})
            }
            ModelError::TimeNotOrdered { model_ref, time } => {
                json!({"model_ref": model_ref, "time": time})
            }
            ModelError::MissingAuthorization { model_ref } => json!({"model_ref": model_ref}),
            ModelError::ReservedParameter { model_ref, name }
            | ModelError::PayloadFieldReserved { model_ref, name }
            | ModelError::WorkItemBinding { model_ref, name } => {
                json!({"model_ref": model_ref, "name": name})
            }
            ModelError::MissingCapability {
                model_ref,
                capability,
            } => json!({"model_ref": model_ref, "capability": capability}),
            ModelError::AuthoredWorkEffect { model_ref, field } => {
                json!({"model_ref": model_ref, "field": field})
            }
            ModelError::UnauthorizedWorkCreation {
                action,
                entity,
                owner,
            }
            | ModelError::UnauthorizedWorkRemoval {
                action,
                entity,
                owner,
            } => json!({"action": action, "entity": entity, "owner": owner}),
            ModelError::UnauthorizedWorkWrite {
                action,
                entity,
                field,
                owner,
            } => json!({"action": action, "entity": entity, "field": field, "owner": owner}),
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
