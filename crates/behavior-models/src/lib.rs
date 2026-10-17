//! Behavior Models (feature 501): the single, binding-neutral authority for every model kind.
//!
//! A Behavior Model is a deterministic authoring abstraction that lowers completely to ordinary
//! Behavior IR. This crate owns everything that gives a model meaning: resolution,
//! normalization, validation, canonical identity (ModelHash), composition and ownership,
//! generated names, lowering, receipts and source maps. Bindings only construct model input and
//! trace Core fragments inside scopes this crate opens; they decide no model semantics.
//!
//! The only Core dependency is the public `behavior-engine` facade of the pinned Core Release.
//! Core never learns that a model existed: `Meaning(M) := Meaning_Core(L(M))`.
#![forbid(unsafe_code)]

pub mod approval;
pub mod composition;
pub mod error;
pub mod identity;
pub mod ids;
pub mod kinds;
pub mod request_flow;
pub mod source_map;
pub mod state_machine;
pub mod work_queue;

use behavior_engine::builder::Builder;
use behavior_engine::semantic::Module;

pub use approval::input::{
    ApprovalInput, ApprovalSource, ApprovalSpec, CapabilityInput, DecisionFragments, DecisionScope,
};
pub use composition::{Contribution, DeclaredEnum, DeclaredSchema, DefinitionSignature};
pub use error::ModelError;
pub use kinds::{AnyModel, Staged, compose_models, declare_models};
pub use request_flow::input::{
    CreationFragments, CreationInput, CreationScope, RequestFlowInput, RequestFlowSource,
    RequestFlowSpec,
};
pub use source_map::{Diagnostic, LoweringReceipt};
pub use state_machine::input::{
    BindingError, DisplayMeta, FragmentSource, ModelInput, ModelSpec, NoFragments, SourceLoc,
    TransitionFragments, TransitionInput, TransitionScope,
};

/// The StateMachine model semantic version: changes when the meaning or admitted structure of
/// the model language changes (lowering contract §7). Version 2 (feature 505): the canonical
/// document references every declaration it names by its Core identity.
pub const STATE_MACHINE_SEMANTIC_VERSION: u32 = 2;
/// StateMachine language level 3 (feature 507, research R13): transitions may bind further
/// entities. A machine is level 3 exactly when one of its transitions has a binding; a machine
/// without bindings stays level 2 with its established canonical document and mappings.
pub const STATE_MACHINE_BINDINGS_SEMANTIC_VERSION: u32 = 3;
/// The StateMachine lowerer version: changes when the same model contract intentionally lowers
/// to different Core IR (lowering contract §7).
pub const STATE_MACHINE_LOWERER_VERSION: u32 = 1;

/// The RequestFlow model semantic version (feature 502, data model §9). Version 2 (feature
/// 505): the canonical document references every declaration it names by its Core identity.
pub const REQUEST_FLOW_SEMANTIC_VERSION: u32 = 2;
/// RequestFlow language level 3 (feature 507, research R13): a flow whose transitions bind
/// further entities, with an embedded StateMachine at level 3. Otherwise level 2, unchanged.
pub const REQUEST_FLOW_BINDINGS_SEMANTIC_VERSION: u32 = 3;
/// The RequestFlow lowerer version (feature 502, data model §9).
pub const REQUEST_FLOW_LOWERER_VERSION: u32 = 1;

/// The Approval model semantic version (feature 503, data model §10).
pub const APPROVAL_SEMANTIC_VERSION: u32 = 1;
/// The Approval lowerer version (feature 503, data model §10).
pub const APPROVAL_LOWERER_VERSION: u32 = 1;

/// The WorkQueue model semantic version (feature 507, research R6).
pub const WORK_QUEUE_SEMANTIC_VERSION: u32 = 1;
/// The WorkQueue lowerer version (feature 507, research R6).
pub const WORK_QUEUE_LOWERER_VERSION: u32 = 1;

/// The exact Core Release this crate builds against (`core-release.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreRelease {
    pub version: &'static str,
    pub commit: &'static str,
}

pub const CORE_RELEASE: CoreRelease = CoreRelease {
    version: env!("BEHAVIOR_CORE_RELEASE_VERSION"),
    commit: env!("BEHAVIOR_CORE_RELEASE_COMMIT"),
};

/// A successful build: the admitted module, one receipt per model, and diagnostics.
#[derive(Debug)]
pub struct BuildOutput {
    pub module: Module,
    pub receipts: Vec<LoweringReceipt>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lowers `models` into `builder` and admits the composed module (lowering contract §4).
///
/// Protocol: before calling, the binding has traced *all* ordinary declarations and
/// hand-written behavior into `builder`. Then, in this fixed order:
///
/// 1. the ordinary behavior is admitted as a snapshot, and every model input is resolved
///    against its declarations;
/// 2. set-level checks: unique model ids, one owner per field;
/// 3. every transition is lowered into `builder`, in `(model_id, transition_id)` order, with its
///    fragments traced by the model's [`FragmentSource`] in a scope opened here;
/// 4. Core admits the composed module (`Builder::finish`);
/// 5. the final composition pass checks every write and creation in the admitted module;
/// 6. ModelHash, receipts, source maps and diagnostics are produced.
///
/// Errors of steps 1–3 are collected over all models before returning. There is no option to
/// skip any step (spec FR-018). Equal inputs give an identical, sorted error list.
pub fn compose(
    builder: &mut Builder,
    models: Vec<ModelSpec<'_>>,
    root: Option<&str>,
) -> Result<BuildOutput, Vec<ModelError>> {
    kinds::compose_models(
        builder,
        models.into_iter().map(AnyModel::from).collect(),
        root,
    )
}
