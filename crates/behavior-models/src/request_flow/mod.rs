//! The RequestFlow model kind (feature 502, contract `request-flow-contract.md`).
//!
//! ```text
//! L_RF(R) = Compose( Decls(R), Creates(R), L_SM(SM(R)) )
//! ```
//!
//! A flow contributes one declaration (the request entity), lowers its creations, and hands its
//! status evolution to the existing StateMachine implementation, which it never copies.

pub mod create;
pub mod decls;
pub mod identity;
pub mod input;
pub mod ownership;
pub mod resolve;
