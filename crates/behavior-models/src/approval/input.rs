//! What a binding may construct for an Approval (data model §1): raw input with no authority,
//! and the callback that traces Core fragments in the decision scopes this crate opens.
//!
//! Nothing here is sorted, deduplicated, defaulted or validated; that is resolution's job.

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node};
use behavior_engine::wire::{Loc, WParam, WType};

use crate::state_machine::input::{BindingError, DisplayMeta, SourceLoc};

/// An approval as a binding describes it.
#[derive(Debug, Clone)]
pub struct ApprovalInput {
    pub approval_id: String,
    /// The subject entity `S` (authored or produced by another model).
    pub subject_entity: String,
    /// The actor entity `A`: decisions hold `Id<A>`, never a reference.
    pub actor_entity: String,
    /// `k_A`; resolution requires at least 1.
    pub approve_threshold: i64,
    /// `k_R`, or `None` for `⊥`; resolution requires at least 1.
    pub reject_threshold: Option<i64>,
    /// The payload mapping `FieldId → Type`, in any order.
    pub payload: Vec<(String, WType)>,
    /// Exactly the capabilities `approve` and `reject` (checked by resolution).
    pub capabilities: Vec<CapabilityInput>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// One decision capability (`approve` or `reject`) as a binding describes it.
#[derive(Debug, Clone)]
pub struct CapabilityInput {
    pub capability_id: String,
    /// Input mapping, in any order.
    pub inputs: Vec<(String, WType)>,
    /// Context mapping, in any order.
    pub context: Vec<(String, WType)>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

/// The scope opened for one decision capability (data model §4): `decision_id`, the inputs
/// sorted by name, `subject`, then the context sorted by name. The binding binds the author's
/// callables to `params` by position.
#[derive(Debug, Clone)]
pub struct DecisionScope {
    pub approval_id: String,
    pub capability_id: String,
    pub params: Vec<WParam>,
    /// The type the actor expression must have: `Id(A)`.
    pub actor_type: WType,
}

/// The author's parts of one decision capability, as Core nodes built in its scope.
#[derive(Debug, Clone)]
pub struct DecisionFragments {
    /// The actor expression, of type `Id(A)`. Never inferred from a parameter name.
    pub actor: (Node, Loc),
    /// `Open`; `None` is the constant true.
    pub open: Option<(Node, Loc)>,
    /// `Eligible`; `None` is the constant true.
    pub eligible: Option<(Node, Loc)>,
    /// `G_c`, in author order.
    pub guards: Vec<(Node, Loc)>,
    /// The payload initializer: payload fields only.
    pub payload: Vec<(String, Node)>,
    /// `K_c`, in author order.
    pub commands: Vec<CommandEmissionSpec>,
}

/// Implemented by a binding: traces one decision capability in the scope this crate has pushed.
pub trait ApprovalSource {
    fn trace_decision(
        &mut self,
        builder: &mut Builder,
        scope: &DecisionScope,
    ) -> Result<DecisionFragments, BindingError>;
}

/// One Approval supplied for composition.
pub struct ApprovalSpec<'a> {
    pub input: ApprovalInput,
    pub fragments: Box<dyn ApprovalSource + 'a>,
}
