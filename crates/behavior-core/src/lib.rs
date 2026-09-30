//! Behavior IR core: wire decoding, admission into the semantic IR, content hashing,
//! deterministic evaluation, decision records, and the capability boundary.
#![forbid(unsafe_code)]

pub mod admit;
pub mod builder;
pub mod canonical;
pub mod decimal;
pub mod eval;
pub mod exact;
pub mod facts;
pub mod intent;
pub mod pretty;
pub mod record;
pub mod semantic;
pub mod serialize;
pub mod testing;
pub mod wire;

pub use admit::{
    AdmissionError, AdmissionResult, admission_report, admission_result, admit, admit_wire,
};
pub use eval::{
    IndexPlan, Observed, affects, canonical_entity, check_entity, check_global_invariants,
    decode_entity, evaluate, evaluate_observed, evaluate_with, held_uniques, index_hint,
    queried_types, query_matches,
};
pub use facts::{EvaluationFacts, FactError, Facts, QueryFact, QueryRequest, RefEdge};
pub use intent::{IntentError, IntentRejection, evaluate_intent};
pub use record::{DecisionRecord, ReplayResult, replay};
