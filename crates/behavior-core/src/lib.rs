//! Behavior IR core: wire decoding, admission into the semantic IR, content hashing,
//! deterministic evaluation, decision records, and the capability boundary.
#![forbid(unsafe_code)]

pub mod admit;
pub mod canonical;
pub mod decimal;
pub mod eval;
pub mod intent;
pub mod pretty;
pub mod record;
pub mod semantic;
pub mod testing;
pub mod wire;

pub use admit::{AdmissionError, AdmissionResult, admission_report, admit};
pub use eval::evaluate;
pub use record::{DecisionRecord, ReplayResult, replay};
