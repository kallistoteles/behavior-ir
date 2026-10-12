//! The StateMachine model kind (contract `state-machine-contract.md`).
//!
//! `SM = (E, f, Q_entry, 𝒯)` with `Q := Cases(T_f)`: the enum of the owned field defines the
//! state domain, the machine defines the legal transition relation over it.

pub mod input;
pub mod lower;
pub mod resolve;
