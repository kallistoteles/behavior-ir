//! What a binding may construct (data-model §2, §5): raw model input with no authority, and
//! the callback through which it traces Core fragments inside scopes this crate opens.
//!
//! Nothing here is sorted, deduplicated, defaulted or validated: that is resolution's job
//! (`resolve.rs`). Collections keep whatever order and duplicates the author supplied.

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node};
use behavior_engine::wire::{Loc, WParam, WType};

/// Non-semantic display metadata. Never part of ModelHash or BehaviorHash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayMeta {
    pub display_name: Option<String>,
    pub description: Option<String>,
}

/// Where a construct was authored. Provenance only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    pub file: String,
    pub line: u64,
}

/// A state machine as a binding describes it (data-model §2).
#[derive(Debug, Clone)]
pub struct ModelInput {
    pub model_id: String,
    pub subject_entity: String,
    pub state_field: String,
    /// The subject parameter name; `None` means the model's default, decided by resolution.
    pub subject_param: Option<String>,
    /// Case names, in any order, duplicates allowed.
    pub entry_states: Vec<String>,
    pub transitions: Vec<TransitionInput>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// One transition as a binding describes it.
#[derive(Debug, Clone)]
pub struct TransitionInput {
    pub transition_id: String,
    /// Case names, in any order, duplicates allowed.
    pub from_states: Vec<String>,
    pub to_state: String,
    /// A mapping `ParameterId → Type` given as a list in any order; never a sequence.
    pub inputs: Vec<(String, WType)>,
    /// A mapping `ParameterId → Type` given as a list in any order; never a sequence.
    pub context: Vec<(String, WType)>,
    /// State bindings `ParameterId → entity name` (feature 507, D1(a)), given as a list in any
    /// order. Further entities the transition may change atomically with the status; a binding
    /// never grants authority over an owned field. Empty for language level 2.
    pub state: Vec<(String, String)>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// The scope this crate opened for one transition: the binding traces the author's fragments
/// in it, binding the author's parameters to `params` by position.
#[derive(Debug, Clone)]
pub struct TransitionScope {
    pub model_id: String,
    pub transition_id: String,
    /// Subject first, then state bindings, inputs and context, each sorted by name (data-model
    /// §4; feature 507 for the bindings).
    pub params: Vec<WParam>,
    pub subject_param: String,
    pub owned_field: String,
}

/// The author's open transition parts, as Core nodes built in the transition's scope. Each list
/// is a sequence in the author's order (Core evaluation semantics), not a set.
#[derive(Debug, Clone, Default)]
pub struct TransitionFragments {
    pub guards: Vec<(Node, Loc)>,
    pub effects: Vec<(String, String, Node, Loc)>,
    pub postconditions: Vec<(Node, Loc)>,
    pub commands: Vec<CommandEmissionSpec>,
}

/// An authoring error raised while a binding traced fragments, with the binding's error code
/// (for example a DSL `TYPE_MISMATCH`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingError {
    pub code: String,
    pub message: String,
}

/// Implemented by a binding: traces the fragments of one transition in the scope this crate has
/// already pushed on `builder`. Called once per transition, in canonical order.
pub trait FragmentSource {
    fn trace(
        &mut self,
        builder: &mut Builder,
        scope: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError>;
}

/// A fragment source for a machine whose transitions have no open parts.
#[derive(Debug, Default)]
pub struct NoFragments;

impl FragmentSource for NoFragments {
    fn trace(
        &mut self,
        _: &mut Builder,
        _: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        Ok(TransitionFragments::default())
    }
}

/// One model supplied to [`crate::compose`]: its input and the binding's fragment source.
pub struct ModelSpec<'a> {
    pub input: ModelInput,
    pub fragments: Box<dyn FragmentSource + 'a>,
}

/// Fragment sources keyed by transition id, for callers that build fragments per transition.
pub type FragmentTable<'a> = BTreeMap<
    String,
    Box<
        dyn FnMut(&mut Builder, &TransitionScope) -> Result<TransitionFragments, BindingError> + 'a,
    >,
>;

impl FragmentSource for FragmentTable<'_> {
    fn trace(
        &mut self,
        builder: &mut Builder,
        scope: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        match self.get_mut(&scope.transition_id) {
            Some(f) => f(builder, scope),
            None => Ok(TransitionFragments::default()),
        }
    }
}
