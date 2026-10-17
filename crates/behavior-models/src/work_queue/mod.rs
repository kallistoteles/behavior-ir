//! The WorkQueue model kind (feature 507, contract `work-queue-contract.md`).
//!
//! ```text
//! WQ = (id, A, T, Π, M, G, K)
//! L_WQ(Q) = Compose( Decls(Q), Inv(Q), Defs(Q), Enqueue(Q), L_SM(SM(Q)), Reads(Q) )
//! ```
//!
//! A work queue governs persistent, claimable units of work. It contributes a status enum, a
//! work entity with the per-entity invariant `ValidWork`, the derived value `terminal`, the
//! reads `ready(now)` and `expired(now)`, an enqueue creation and an embedded StateMachine with
//! the transitions claim, renew, complete, retry, fail, recover, exhaust and cancel. It never
//! executes, schedules or orders work: every decision is Core's evaluation of the lowered IR.

pub mod decls;
pub mod identity;
pub mod input;
pub mod lower;
pub mod ownership;
pub mod resolve;
