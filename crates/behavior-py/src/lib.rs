//! PyO3 bindings: JSON in, JSON out. No logic lives here.

use pyo3::prelude::*;

/// Admits wire IR; returns the AdmissionResult JSON.
#[pyfunction]
fn admit(wire: &str) -> String {
    behavior_core::admission_report(wire).to_json_string()
}

/// Evaluates a request; returns the DecisionRecord JSON, or the AdmissionResult JSON if the
/// wire IR is not admitted.
#[pyfunction]
fn evaluate(wire: &str, request: &str) -> String {
    match behavior_core::admit(wire) {
        Ok(m) => behavior_core::evaluate(&m, request).to_json_string(),
        Err(r) => r.to_json_string(),
    }
}

/// Replays a decision record; returns the ReplayResult JSON, or the AdmissionResult JSON.
#[pyfunction]
fn replay(wire: &str, record: &str) -> String {
    match behavior_core::admit(wire) {
        Ok(m) => behavior_core::replay(&m, record).to_json_string(),
        Err(r) => r.to_json_string(),
    }
}

/// Evaluates a structured intent with the host context; returns a DecisionRecord JSON, an
/// IntentRejection JSON, or the AdmissionResult JSON.
#[pyfunction]
fn evaluate_intent(wire: &str, intent: &str, host: &str) -> String {
    match behavior_core::admit(wire) {
        Ok(m) => match behavior_core::evaluate_intent(&m, intent, host) {
            Ok(record) => record.to_json_string(),
            Err(rejection) => rejection.to_json_string(),
        },
        Err(r) => r.to_json_string(),
    }
}

#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(admit, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(replay, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_intent, m)?)?;
    Ok(())
}
