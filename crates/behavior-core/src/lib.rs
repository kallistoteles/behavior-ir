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
pub mod migration;
pub mod pretty;
pub mod record;
pub mod schema;
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
pub use schema::{StoreSchema, schema};

/// The engine version and the document formats it reads and writes (feature 008): the wire IR
/// versions and the decision record versions, in order, taken from the constants that implement
/// them.
pub fn format_versions() -> serde_json::Value {
    use eval::{RECORD_VERSION, RECORD_VERSION_LIFECYCLE, RECORD_VERSION_QUERIES};
    use wire::{
        IR_VERSION, IR_VERSION_CONSTRAINTS, IR_VERSION_EXACT, IR_VERSION_FIXED_SCALE,
        IR_VERSION_LIFECYCLE, IR_VERSION_QUERIES,
    };
    serde_json::json!({
        "engine": env!("CARGO_PKG_VERSION"),
        "wire_ir": [
            IR_VERSION,
            IR_VERSION_CONSTRAINTS,
            IR_VERSION_FIXED_SCALE,
            IR_VERSION_EXACT,
            IR_VERSION_LIFECYCLE,
            IR_VERSION_QUERIES,
        ],
        "records": [RECORD_VERSION, RECORD_VERSION_LIFECYCLE, RECORD_VERSION_QUERIES],
    })
}
