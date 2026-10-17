//! What a binding may construct for a WorkQueue (data model §1): raw input with no authority,
//! and the callback that traces the author's fragments in the capability scopes this crate
//! opens.
//!
//! Nothing here is sorted, deduplicated, defaulted or validated; that is resolution's job.

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node};
use behavior_engine::wire::{Loc, WParam, WType};

use crate::state_machine::input::{BindingError, DisplayMeta, SourceLoc};

/// A work queue as a binding describes it.
#[derive(Debug, Clone)]
pub struct WorkQueueInput {
    pub queue_id: String,
    /// The worker entity `A`: work items hold `Id<A>`, never a reference.
    pub worker_entity: String,
    /// The time type `T`: must be a nominal declared with `order`.
    pub time: WType,
    /// The payload mapping `FieldId → Type`, in any order; immutable after enqueue.
    pub payload: Vec<(String, WType)>,
    /// `M`, the maximum number of claims per item.
    pub max_attempts: i64,
    /// The nine capabilities, each exactly once (checked by resolution).
    pub capabilities: Vec<CapabilityInput>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// One capability as a binding describes it: only the author's own parameters. The generated
/// ones (`worker`, `now`, `generation`, …) are the model's.
#[derive(Debug, Clone)]
pub struct CapabilityInput {
    pub capability_id: String,
    pub inputs: Vec<(String, WType)>,
    pub context: Vec<(String, WType)>,
    /// State bindings `ParameterId → entity`; never the queue's own work entity.
    pub state: Vec<(String, String)>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// The scope opened for one capability (data model §3). The binding binds the author's
/// callables to `params` by position.
#[derive(Debug, Clone)]
pub struct CapabilityScope {
    pub queue_id: String,
    pub capability_id: String,
    pub params: Vec<WParam>,
}

/// The author's parts of one capability, as Core nodes built in its scope.
#[derive(Debug, Clone, Default)]
pub struct CapabilityFragments {
    /// The authorization guard: mandatory for enqueue, claim (eligibility), cancel, recover and
    /// exhaust; absent otherwise.
    pub authorization: Option<(Node, Loc)>,
    pub guards: Vec<(Node, Loc)>,
    /// Effects over the authored state bindings only.
    pub effects: Vec<(String, String, Node, Loc)>,
    pub postconditions: Vec<(Node, Loc)>,
    pub commands: Vec<CommandEmissionSpec>,
}

/// Implemented by a binding: traces one capability in the scope this crate has pushed.
pub trait WorkQueueSource {
    fn trace_capability(
        &mut self,
        builder: &mut Builder,
        scope: &CapabilityScope,
    ) -> Result<CapabilityFragments, BindingError>;
}

/// A source without authored parts. Every capability that needs authorization then fails with
/// `MISSING_AUTHORIZATION`.
#[derive(Debug, Default)]
pub struct NoCapabilityFragments;

impl WorkQueueSource for NoCapabilityFragments {
    fn trace_capability(
        &mut self,
        _: &mut Builder,
        _: &CapabilityScope,
    ) -> Result<CapabilityFragments, BindingError> {
        Ok(CapabilityFragments::default())
    }
}

/// One WorkQueue supplied for composition.
pub struct WorkQueueSpec<'a> {
    pub input: WorkQueueInput,
    pub fragments: Box<dyn WorkQueueSource + 'a>,
}
