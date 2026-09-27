//! The semantic Behavior IR. Every node is typed, resolved, and content-hashed.
//!
//! Semantic nodes have crate-private fields and constructors; outside this crate they can only
//! be obtained from `admit`, so invalid semantic IR cannot be built (research R2).

pub mod expr;
pub mod module;
pub mod types;
pub mod value;

pub use expr::{Expr, ExprKind};
pub use module::{
    ActionItem, Condition, ConstraintItem, DerivedItem, Effect, EntityItem, InvariantItem, Kind,
    Module, Param,
};
pub use types::{Hash, Type, hash_display};
pub use value::Value;
