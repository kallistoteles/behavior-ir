#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Feature 008: the format versions the engine reports come from the constants that implement
//! them (research R2).

use behavior_core::eval::{RECORD_VERSION, RECORD_VERSION_LIFECYCLE, RECORD_VERSION_QUERIES};
use behavior_core::wire::{
    IR_VERSION, IR_VERSION_CONSTRAINTS, IR_VERSION_EXACT, IR_VERSION_FIXED_SCALE,
    IR_VERSION_LIFECYCLE, IR_VERSION_QUERIES,
};
use serde_json::json;

#[test]
fn format_versions_report_the_implemented_constants() {
    let v = behavior_core::format_versions();
    assert_eq!(v["engine"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        v["wire_ir"],
        json!([
            IR_VERSION,
            IR_VERSION_CONSTRAINTS,
            IR_VERSION_FIXED_SCALE,
            IR_VERSION_EXACT,
            IR_VERSION_LIFECYCLE,
            IR_VERSION_QUERIES
        ])
    );
    assert_eq!(
        v["wire_ir"],
        json!(["0.1", "0.2", "0.3", "0.4", "0.5", "0.6"])
    );
    assert_eq!(
        v["records"],
        json!([
            RECORD_VERSION,
            RECORD_VERSION_LIFECYCLE,
            RECORD_VERSION_QUERIES
        ])
    );
    assert_eq!(v["records"], json!(["0.4", "0.5", "0.6"]));
}
