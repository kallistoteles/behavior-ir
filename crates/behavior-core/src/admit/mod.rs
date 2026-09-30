//! Admission: the only way to obtain semantic Behavior IR (research R2, R3).
//!
//! `wire JSON → strict decode → declarations → reference graph and cycles → type check with
//! explicit conversions → content hashes → Module`. Errors from every stage that can run are
//! collected and sorted by (file, line, code); any error means no module and no hash.

pub mod bounds;
pub mod graph;
pub mod hash;
pub mod resolve;
pub mod typecheck;

use std::collections::BTreeMap;

use serde::Serialize;

use crate::canonical;
use crate::semantic::module::Module;
use crate::semantic::types::hash_display;
use crate::wire::{self, DecodeError, Loc};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmissionError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loc: Option<Loc>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_locs: Vec<Loc>,
}

impl AdmissionError {
    pub(crate) fn new(code: &str, message: impl Into<String>, loc: Option<&Loc>) -> Self {
        AdmissionError {
            code: code.to_string(),
            message: message.into(),
            loc: loc.cloned(),
            related_locs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AdmissionResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior_version: Option<String>,
    pub errors: Vec<AdmissionError>,
    pub evaluation_order: Vec<String>,
    /// `"<kind>:<name>"` → item hash, for display.
    pub items: BTreeMap<String, String>,
}

impl AdmissionResult {
    fn failed(mut errors: Vec<AdmissionError>) -> Self {
        errors.sort_by(|a, b| {
            let key = |e: &AdmissionError| {
                (
                    e.loc.as_ref().map(|l| (l.file.clone(), l.line)),
                    e.code.clone(),
                )
            };
            key(a).cmp(&key(b))
        });
        AdmissionResult {
            ok: false,
            behavior_version: None,
            errors,
            evaluation_order: Vec::new(),
            items: BTreeMap::new(),
        }
    }

    pub(crate) fn admitted(module: &Module) -> Self {
        AdmissionResult {
            ok: true,
            behavior_version: Some(module.behavior_version()),
            errors: Vec::new(),
            evaluation_order: module.evaluation_order.clone(),
            items: module
                .name_table
                .iter()
                .map(|((kind, name), h)| (format!("{}:{name}", kind.as_str()), hash_display(h)))
                .collect(),
        }
    }

    /// Canonical JSON (contracts/engine-api.md → AdmissionResult).
    pub fn to_json_string(&self) -> String {
        // Contains only strings, booleans, integers: canonicalization cannot fail.
        canonical::canonical(self).unwrap_or_default()
    }
}

/// Admits wire IR into a semantic module, or returns the failed AdmissionResult.
pub fn admit(wire_text: &str) -> Result<Module, AdmissionResult> {
    admit_wire(&decode(wire_text)?)
}

fn decode(wire_text: &str) -> Result<wire::WModule, AdmissionResult> {
    match wire::decode_module(wire_text) {
        Ok(w) => Ok(w),
        Err(DecodeError::UnsupportedVersion(v)) => {
            Err(AdmissionResult::failed(vec![AdmissionError::new(
                "UNSUPPORTED_IR_VERSION",
                format!(
                    "unsupported ir_version `{v}`; this engine accepts `{}`, `{}` and `{}` only. Wire \
                     IR 0.1–0.3 used rounded decimal arithmetic, 0.4 is exact: re-serialize from \
                     the DSL and declare a scale where computed values are stored",
                    wire::IR_VERSION_EXACT,
                    wire::IR_VERSION_LIFECYCLE,
                    wire::IR_VERSION_QUERIES
                ),
                None,
            )]))
        }
        Err(DecodeError::NeedsLifecycleVersion(form)) => {
            Err(AdmissionResult::failed(vec![AdmissionError::new(
                "UNSUPPORTED_IR_VERSION",
                format!(
                    "`{form}` is an entity lifecycle form and needs ir_version `{}`",
                    wire::IR_VERSION_LIFECYCLE
                ),
                None,
            )]))
        }
        Err(DecodeError::NeedsQueryVersion(form)) => {
            Err(AdmissionResult::failed(vec![AdmissionError::new(
                "UNSUPPORTED_IR_VERSION",
                format!(
                    "`{form}` is a relational form and needs ir_version `{}`",
                    wire::IR_VERSION_QUERIES
                ),
                None,
            )]))
        }
        Err(DecodeError::Structure { path, message }) => {
            Err(AdmissionResult::failed(vec![AdmissionError::new(
                "DECODE_ERROR",
                format!("at {path}: {message}"),
                None,
            )]))
        }
    }
}

/// The single entry point into the semantic IR, shared by JSON decoding and the builder
/// (research R17).
pub fn admit_wire(w: &wire::WModule) -> Result<Module, AdmissionResult> {
    let mut errors = Vec::new();
    let decls = resolve::declarations(w, &mut errors);
    if !errors.is_empty() {
        return Err(AdmissionResult::failed(errors));
    }
    let order = match graph::evaluation_order(&w.derived) {
        Ok(order) => order,
        Err(errs) => return Err(AdmissionResult::failed(errs)),
    };
    let module = typecheck::build_module(w, decls, order, &mut errors);
    match module {
        Some(m) if errors.is_empty() => Ok(m),
        _ => Err(AdmissionResult::failed(errors)),
    }
}

/// The AdmissionResult of an admitted module.
pub fn admission_result(module: &Module) -> AdmissionResult {
    AdmissionResult::admitted(module)
}

/// Admission as a report (contracts/engine-api.md → `admit`).
pub fn admission_report(wire_text: &str) -> AdmissionResult {
    match admit(wire_text) {
        Ok(m) => AdmissionResult::admitted(&m),
        Err(r) => r,
    }
}
