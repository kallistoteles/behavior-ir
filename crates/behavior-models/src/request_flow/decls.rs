//! `Decls(RF) = { E }`: the one declaration a flow contributes (declaration-staging contract).
//!
//! Computed from the flow input and the authored declarations only (law D1): it never traces a
//! fragment or lowers anything, so a binding can declare the entity before any hand-written
//! behavior is traced, and obtain its schema for a typed handle.

use behavior_engine::semantic::Module;

use crate::composition::DeclaredSchema;
use crate::error::ModelError;
use crate::request_flow::input::RequestFlowInput;
use crate::request_flow::resolve::schema;

/// The request entity this flow contributes, in canonical field order.
pub fn decls(
    input: &RequestFlowInput,
    authored: &Module,
) -> Result<DeclaredSchema, Vec<ModelError>> {
    schema(input, authored)
}
