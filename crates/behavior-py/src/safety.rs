//! Thin Safety Model binding. Every policy/proof decision stays in behavior-models.
use super::{EngineError, EngineModule, to_py, to_value};
use behavior_models::safety::certify::{ProofLimits, ProofToolchain};
use behavior_models::safety::evidence::SafetyAssessment;
use behavior_models::safety::{
    self, FieldOwnershipInput, LifecycleDependencyInput, UncertifiedSafetyAnalysis,
};
use pyo3::prelude::*;
use serde_json::{Value, json};

#[pyclass(name = "SafetyAnalysis", frozen, module = "behavior._engine")]
pub(crate) struct PySafetyAnalysis {
    inner: UncertifiedSafetyAnalysis,
}
#[pymethods]
impl PySafetyAnalysis {
    #[getter]
    fn status(&self) -> &'static str {
        self.inner.status()
    }
    #[getter]
    fn is_certified(&self) -> bool {
        self.inner.is_certified()
    }
    #[getter]
    fn policy_set_hash(&self) -> &str {
        self.inner.policy_set_hash()
    }
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.inner.to_json())
    }
}

#[pyclass(name = "SafetyCertificate", frozen, module = "behavior._engine")]
pub(crate) struct PySafetyCertificate {
    record: Value,
}
#[pymethods]
impl PySafetyCertificate {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.record)
    }
}
#[pyclass(name = "SafetyAssessment", frozen, module = "behavior._engine")]
pub(crate) struct PySafetyAssessment {
    inner: SafetyAssessment,
}
#[pymethods]
impl PySafetyAssessment {
    #[getter]
    fn outcome(&self) -> String {
        self.inner.to_json()["outcome"]
            .as_str()
            .unwrap_or("Failed")
            .into()
    }
    #[getter]
    fn certificate(&self) -> Option<PySafetyCertificate> {
        self.inner.certificate().map(|c| PySafetyCertificate {
            record: c.to_json(),
        })
    }
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.inner.to_json())
    }
}

#[pyfunction]
pub(crate) fn analyze_safety(
    py: Python<'_>,
    module: &EngineModule,
    inputs: &Bound<'_, PyAny>,
) -> PyResult<(Option<PySafetyAnalysis>, Py<PyAny>)> {
    let raw = to_value(inputs)?;
    let inputs = raw
        .as_array()
        .ok_or_else(|| EngineError::new_err(("SAFETY_INVALID_POLICY", "Expected policy list")))?;
    let result = (|| {
        let mut ownership = Vec::new();
        let mut dependencies = Vec::new();
        for input in inputs {
            if input.get("parent").is_some() {
                dependencies.push(LifecycleDependencyInput::from_json(input)?);
            } else {
                ownership.push(FieldOwnershipInput::from_json(input)?);
            }
        }
        safety::analyze_with_dependencies(&module.inner, &ownership, &dependencies)
    })();
    match result {
        Ok(inner) => Ok((Some(PySafetyAnalysis { inner }), to_py(py, &json!([]))?)),
        Err(e) => {
            let mut error = e.to_json();
            error["message"] = json!(e.to_string());
            Ok((None, to_py(py, &json!([error]))?))
        }
    }
}

#[pyfunction]
#[pyo3(signature=(module, analysis, output, toolchain=None, limits=None))]
pub(crate) fn certify_safety(
    module: &EngineModule,
    analysis: &PySafetyAnalysis,
    output: &str,
    toolchain: Option<&str>,
    limits: Option<&Bound<'_, PyAny>>,
) -> PyResult<PySafetyAssessment> {
    let (tools, config) = proof_configuration(toolchain, limits)?;
    let inner = safety::certify::certify(
        &module.inner,
        &analysis.inner,
        std::path::Path::new(output),
        &tools,
        &config,
    );
    if inner.certificate().is_some() {
        safety::certify::write_bundle(
            &behavior_engine::serialize::to_wire_json(&module.inner),
            &analysis.inner,
            &inner,
            std::path::Path::new(output),
        )
        .map_err(|e| EngineError::new_err((e.code(), e.to_string())))?;
    }
    Ok(PySafetyAssessment { inner })
}

fn proof_configuration(
    toolchain: Option<&str>,
    limits: Option<&Bound<'_, PyAny>>,
) -> PyResult<(ProofToolchain, ProofLimits)> {
    let tools = match toolchain {
        Some(path) => ProofToolchain::from_root(std::path::Path::new(path)),
        None => ProofToolchain::locked(),
    }
    .map_err(|e| EngineError::new_err((e.code(), e.to_string())))?;
    let mut config = ProofLimits::default();
    if let Some(raw) = limits {
        let v = to_value(raw)?;
        let object = v.as_object().ok_or_else(|| {
            EngineError::new_err(("SAFETY_INVALID_LIMITS", "Expected numeric proof limits"))
        })?;
        for (key, value) in object {
            let number = value.as_u64().ok_or_else(|| {
                EngineError::new_err(("SAFETY_INVALID_LIMITS", "Limits must be unsigned integers"))
            })?;
            match key.as_str() {
                "max_heartbeats" => config.max_heartbeats = number,
                "memory_mb" => config.memory_mb = number,
                "address_space_mb" => config.address_space_mb = number,
                "wall_seconds" => config.wall_seconds = number,
                _ => {
                    return Err(EngineError::new_err((
                        "SAFETY_INVALID_LIMITS",
                        format!("Unknown limit {key}"),
                    )));
                }
            }
        }
    }
    Ok((tools, config))
}

#[pyclass(name = "UnverifiedSafetyBundle", frozen, module = "behavior._engine")]
pub(crate) struct PyUnverifiedSafetyBundle {
    inner: safety::evidence::UnverifiedSafetyBundle,
}
#[pymethods]
impl PyUnverifiedSafetyBundle {
    #[getter]
    fn is_verified(&self) -> bool {
        false
    }
    #[getter]
    fn claimed_outcome(&self) -> String {
        self.inner.to_json()["evidence"]["outcome"]
            .as_str()
            .unwrap_or("Unknown")
            .into()
    }
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.inner.to_json())
    }
}
#[pyfunction]
pub(crate) fn import_safety_bundle(raw: &Bound<'_, PyAny>) -> PyResult<PyUnverifiedSafetyBundle> {
    let data = to_value(raw)?;
    let inner = safety::evidence::UnverifiedSafetyBundle::from_json(&data)
        .map_err(|e| EngineError::new_err((e.code(), e.to_string())))?;
    Ok(PyUnverifiedSafetyBundle { inner })
}
#[pyfunction]
pub(crate) fn load_safety_bundle(path: &str) -> PyResult<PyUnverifiedSafetyBundle> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| EngineError::new_err(("SAFETY_BUNDLE_IO", e.to_string())))?;
    let inner = safety::evidence::UnverifiedSafetyBundle::from_text(&text)
        .map_err(|e| EngineError::new_err((e.code(), e.to_string())))?;
    Ok(PyUnverifiedSafetyBundle { inner })
}
#[pyfunction]
#[pyo3(signature=(module,analysis,bundle,output,toolchain=None,limits=None))]
pub(crate) fn verify_safety_evidence(
    module: &EngineModule,
    analysis: &PySafetyAnalysis,
    bundle: &PyUnverifiedSafetyBundle,
    output: &str,
    toolchain: Option<&str>,
    limits: Option<&Bound<'_, PyAny>>,
) -> PyResult<PySafetyAssessment> {
    let (tools, config) = proof_configuration(toolchain, limits)?;
    let inner = safety::certify::verify_safety_evidence(
        &module.inner,
        &analysis.inner,
        &bundle.inner,
        std::path::Path::new(output),
        &tools,
        &config,
    );
    if inner.certificate().is_some() {
        safety::certify::write_bundle(
            &behavior_engine::serialize::to_wire_json(&module.inner),
            &analysis.inner,
            &inner,
            std::path::Path::new(output),
        )
        .map_err(|e| EngineError::new_err((e.code(), e.to_string())))?;
    }
    Ok(PySafetyAssessment { inner })
}

#[pyclass(name = "SafetyStateValidation", frozen, module = "behavior._engine")]
pub(crate) struct PyStateValidation {
    inner: safety::evidence::StateValidationRecord,
}
#[pymethods]
impl PyStateValidation {
    #[getter]
    fn status(&self) -> String {
        self.inner.status().into()
    }
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.inner.to_json())
    }
    fn matches_anchor(
        &self,
        module: &EngineModule,
        analysis: &PySafetyAnalysis,
        history: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let history: behavior_engine::store::documents::HistoryRef =
            super::doc("history reference", history)?;
        Ok(self
            .inner
            .matches_anchor(&module.inner, &analysis.inner, &history))
    }
}
#[pyfunction]
pub(crate) fn validate_safety_snapshot(
    py: Python<'_>,
    store: &super::PyStore,
    module: &EngineModule,
    analysis: &PySafetyAnalysis,
    history: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyStateValidation> {
    let history = match history {
        Some(value) => super::doc("history reference", value)?,
        None => store
            .inner
            .current_history()
            .map_err(|e| super::store_err(py, e))?,
    };
    Ok(PyStateValidation {
        inner: safety::snapshot::validate_store_snapshot(
            &module.inner,
            &analysis.inner,
            &store.inner,
            &history,
        ),
    })
}
