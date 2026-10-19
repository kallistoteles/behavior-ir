//! Structural Safety Models over an exact admitted Core module (feature 510).
//!
//! Inventory extraction is trusted correspondence code. An inventory or successful lowering
//! is not a proof and never grants Store authorization.

pub mod certify;
pub mod evidence;
mod field_ownership;
pub mod guards;
pub mod identity;
pub mod input;
pub mod inventory;
pub mod lifecycle_dependency;
mod membership_orders;
mod proof_data;
pub mod resolve;
pub mod snapshot;

pub use input::{
    ActionSelector, FieldOwnershipInput, LifecycleDependencyInput, SafetyError,
    UncertifiedSafetyAnalysis,
};
pub use inventory::Inventory;

pub const FIELD_OWNERSHIP_SEMANTIC_VERSION: u32 = 1;
pub const SAFETY_CHECKER_VERSION: u32 = 2;

/// Structural analysis only. Existing Model ownership must already have succeeded.
pub fn analyze(
    module: &behavior_engine::semantic::Module,
    policies: &[FieldOwnershipInput],
) -> Result<UncertifiedSafetyAnalysis, SafetyError> {
    analyze_with_dependencies(module, policies, &[])
}

pub fn analyze_with_dependencies(
    module: &behavior_engine::semantic::Module,
    policies: &[FieldOwnershipInput],
    dependencies: &[LifecycleDependencyInput],
) -> Result<UncertifiedSafetyAnalysis, SafetyError> {
    let inventory = Inventory::extract(module)?;
    inventory.verify_module(module)?;
    let policies = resolve::resolve(&inventory, policies)?;
    field_ownership::check(&inventory, &policies)?;
    let dependencies = lifecycle_dependency::resolve(module, &inventory, &policies, dependencies)?;
    let (lifecycle_actions, issues, membership_matches) =
        lifecycle_dependency::inspect(module, &inventory, &dependencies)?;
    let policy_set_hash = identity::complete_policy_set_hash(&policies, &dependencies)?;
    Ok(UncertifiedSafetyAnalysis {
        inventory,
        policies,
        policy_set_hash,
        dependencies,
        issues,
        lifecycle_actions,
        membership_matches,
    })
}
