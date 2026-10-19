//! Versioned semantic identity, delegated to Core's public canonical hashing API.

use serde_json::Value;

use super::input::SafetyError;

pub const INVENTORY_FORMAT: &str = "behavior.safety_inventory.v1";

pub(crate) fn policy_set_hash(
    policies: &[super::resolve::ResolvedOwnership],
) -> Result<String, SafetyError> {
    complete_policy_set_hash(policies, &[])
}

pub(crate) fn complete_policy_set_hash(
    policies: &[super::resolve::ResolvedOwnership],
    deps: &[super::lifecycle_dependency::ResolvedDependency],
) -> Result<String, SafetyError> {
    let mut members = policies
        .iter()
        .map(|p| p.model_hash())
        .chain(deps.iter().map(|p| p.model_hash()))
        .collect::<Vec<_>>();
    members.sort();
    tagged(
        "behavior.safety_policy_set.v1",
        &serde_json::json!({"policies":members}),
    )
}

pub(crate) fn tagged(tag: &str, payload: &Value) -> Result<String, SafetyError> {
    behavior_engine::canonical::tagged_hash(tag, payload)
        .map_err(|e| SafetyError::new("SAFETY_IDENTITY_FAILED", e.to_string()))
}

pub(crate) fn valid_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
