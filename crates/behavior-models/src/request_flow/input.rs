//! What a binding may construct for a RequestFlow (data model §1): raw input with no
//! authority, and the callback that traces Core fragments in scopes this crate opens.
//!
//! Nothing here is sorted, deduplicated, defaulted or validated; that is resolution's job. There
//! is deliberately no entry-state field: entry states are derived from the creations
//! (`resolve::sm_input` is the only place they are computed).

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node};
use behavior_engine::wire::{Loc, WParam, WType};

use crate::state_machine::input::{
    BindingError, DisplayMeta, FragmentSource, SourceLoc, TransitionInput,
};

/// A request flow as a binding describes it.
#[derive(Debug, Clone)]
pub struct RequestFlowInput {
    pub flow_id: String,
    /// The authored actor entity `A`.
    pub actor_entity: String,
    /// The authored status enum `T_s` (never generated).
    pub status_enum: String,
    /// Whether the request has `assignee: Option<Id<A>>`.
    pub assignee: bool,
    /// The payload mapping `FieldId → Type`, in any order.
    pub payload: Vec<(String, WType)>,
    pub creations: Vec<CreationInput>,
    /// Status transitions, exactly as for a StateMachine.
    pub transitions: Vec<TransitionInput>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// One creation as a binding describes it.
#[derive(Debug, Clone)]
pub struct CreationInput {
    pub creation_id: String,
    /// `q_c`: the fixed initial status.
    pub to_state: String,
    /// Input mapping, in any order.
    pub inputs: Vec<(String, WType)>,
    /// State bindings `S_c`: existing entities (`name → entity`), in any order.
    pub state: Vec<(String, String)>,
    /// Context mapping, in any order.
    pub context: Vec<(String, WType)>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// The scope opened for one creation: `request_id`, then inputs, state bindings and context,
/// each sorted by name (data model §4). The binding binds the author's callables by position.
#[derive(Debug, Clone)]
pub struct CreationScope {
    pub flow_id: String,
    pub creation_id: String,
    pub params: Vec<WParam>,
}

/// The author's parts of one creation, as Core nodes built in its scope.
#[derive(Debug, Clone, Default)]
pub struct CreationFragments {
    /// `V_c`: the new request's field values (requester, payload, optional assignee).
    pub initializer: Vec<(String, Node)>,
    pub guards: Vec<(Node, Loc)>,
    /// `E_c`, over the state bindings.
    pub effects: Vec<(String, String, Node, Loc)>,
    pub postconditions: Vec<(Node, Loc)>,
    pub commands: Vec<CommandEmissionSpec>,
}

/// Implemented by a binding: traces creation scopes, and (as a [`FragmentSource`]) the flow's
/// transition scopes, which the StateMachine lowerer opens unchanged.
pub trait RequestFlowSource: FragmentSource {
    fn trace_creation(
        &mut self,
        builder: &mut Builder,
        scope: &CreationScope,
    ) -> Result<CreationFragments, BindingError>;
}

/// One RequestFlow supplied for composition.
pub struct RequestFlowSpec<'a> {
    pub input: RequestFlowInput,
    pub fragments: Box<dyn RequestFlowSource + 'a>,
}
