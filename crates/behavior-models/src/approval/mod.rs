//! The Approval model kind (feature 503, contract `approval-contract.md`).
//!
//! ```text
//! AM = (p, S, A, Π, k_A, k_R, Open, Eligible, {approve, reject})
//! L_Approval(AM) = Compose( Decl(AM), Defs(AM), Actions(AM) )
//! ```
//!
//! An approval records immutable decisions (APPROVE or REJECT) about an existing subject and
//! derives the outcome (PENDING, APPROVED or REJECTED) from them. It contributes a choice enum
//! and a decision entity, five derived definitions (the counts and the outcome predicates), and
//! two decision actions, each of which creates exactly one decision. It never changes the
//! subject: an outcome may guard another model's transition, but never triggers it.
//!
//! Terminology (locked): DENIED is an attempt that was not permitted; REJECT is a recorded
//! choice; REJECTED is the derived outcome.

pub mod decls;
pub mod identity;
pub mod input;
pub mod lower;
pub mod ownership;
pub mod resolve;
