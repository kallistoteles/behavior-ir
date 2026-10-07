//! PyO3 bindings: Python holds engine objects and passes native values (research R12, R17).
//!
//! `Builder` constructs behavior node by node, checking each node with the engine's type
//! checker; `Module` is an admitted module that evaluates, replays, and serializes itself.
//! Values cross the boundary as Python objects; JSON appears only for artifacts (wire files,
//! the canonical serialization, decision records). No behavior logic lives here.

use behavior_engine::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_engine::semantic::Module;
use behavior_engine::wire::{DerivedKind, Loc, Role, WField, WLifecycle, WParam, WType};

/// A lifecycle effect from Python: `(kind, entity or param, id node, fields, file, line)`.
type LifecycleArg = (
    String,
    String,
    Option<PyNode>,
    Vec<(String, PyNode)>,
    String,
    u64,
);
use pyo3::IntoPyObjectExt;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use serde_json::{Map, Value, json};

create_exception!(
    _engine,
    EngineError,
    PyException,
    "An engine error: args are (code, message)."
);
create_exception!(
    _engine,
    EngineStoreConflict,
    PyException,
    "A state conflict: args are (current, changed), the store's state and the changed entities."
);
create_exception!(
    _engine,
    EngineStoreRefused,
    PyException,
    "A refused store operation: args are (code, message)."
);
create_exception!(
    _engine,
    EngineIntentRejected,
    PyException,
    "A rejected intent: args are (errors,), a list of {code, message, path} dicts."
);

fn loc(file: String, line: u64) -> Loc {
    Loc {
        file,
        line: line.max(1),
    }
}

fn build_err(e: BuildError) -> PyErr {
    EngineError::new_err((e.code, e.message))
}

// --- value conversion (JSON exists only inside Rust) ------------------------------------

/// Converts a Python value to the engine's value form. `Decimal` uses its plain `format(d,
/// "f")` notation; `Enum` members use their value; floats are rejected.
fn to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    if obj.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(obj.extract()?));
    }
    if obj.is_instance_of::<PyInt>() {
        let i: i64 = obj
            .extract()
            .map_err(|_| PyValueError::new_err("integer out of the signed 64-bit range"))?;
        return Ok(json!(i));
    }
    if obj.is_instance_of::<PyFloat>() {
        return Err(PyTypeError::new_err(
            "float values are not allowed: use decimal.Decimal for exact numbers",
        ));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(Value::String(s.to_str()?.to_string()));
    }
    let py = obj.py();
    let decimal = py.import("decimal")?.getattr("Decimal")?;
    if obj.is_instance(&decimal)? {
        let plain: String = obj.call_method1("__format__", ("f",))?.extract()?;
        return Ok(Value::String(plain));
    }
    let enum_base = py.import("enum")?.getattr("Enum")?;
    if obj.is_instance(&enum_base)? {
        return to_value(&obj.getattr("value")?);
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        let mut map = Map::new();
        for (k, v) in d.iter() {
            let key: String = k
                .extract()
                .map_err(|_| PyTypeError::new_err("dictionary keys must be strings"))?;
            map.insert(key, to_value(&v)?);
        }
        return Ok(Value::Object(map));
    }
    if obj.is_instance_of::<PyList>() || obj.is_instance_of::<PyTuple>() {
        let items: PyResult<Vec<Value>> = obj.try_iter()?.map(|x| to_value(&x?)).collect();
        return Ok(Value::Array(items?));
    }
    Err(PyTypeError::new_err(format!(
        "unsupported value {}",
        obj.repr()?
    )))
}

/// Converts an engine value to Python objects (dicts, lists, str, int, bool, None).
fn to_py(py: Python<'_>, v: &Value) -> PyResult<Py<PyAny>> {
    Ok(match v {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py_any(py)?,
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.into_py_any(py)?,
            (_, Some(u)) => u.into_py_any(py)?,
            _ => n.to_string().into_py_any(py)?,
        },
        Value::String(s) => s.into_py_any(py)?,
        Value::Array(items) => {
            let list = PyList::empty(py);
            for x in items {
                list.append(to_py(py, x)?)?;
            }
            list.into_py_any(py)?
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, x) in map {
                dict.set_item(k, to_py(py, x)?)?;
            }
            dict.into_py_any(py)?
        }
    })
}

fn object(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    match to_value(obj)? {
        v @ Value::Object(_) => Ok(v),
        _ => Err(PyTypeError::new_err("expected a dict")),
    }
}

/// Raw JSON text reaches Core unchanged; Python documents use the existing value boundary.
fn document_text(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = obj.downcast::<PyString>() {
        return Ok(text.to_str()?.to_string());
    }
    Ok(to_value(obj)?.to_string())
}

fn transport_err(e: behavior_engine::invocation::TransportError) -> PyErr {
    EngineError::new_err(("DECODE_ERROR", e.to_string()))
}

// --- types -------------------------------------------------------------------------------

/// A behavior type as the engine names it.
#[pyclass(name = "Type", frozen, eq, module = "behavior._engine")]
#[derive(Clone, PartialEq)]
struct PyType_ {
    inner: WType,
}

#[pymethods]
impl PyType_ {
    #[staticmethod]
    #[pyo3(name = "bool")]
    fn bool_() -> Self {
        PyType_ { inner: WType::Bool }
    }
    #[staticmethod]
    #[pyo3(name = "int")]
    fn int_() -> Self {
        PyType_ { inner: WType::Int }
    }
    #[staticmethod]
    fn decimal() -> Self {
        PyType_ {
            inner: WType::Decimal,
        }
    }
    #[staticmethod]
    fn string() -> Self {
        PyType_ {
            inner: WType::String,
        }
    }
    #[staticmethod]
    fn option(of: PyType_) -> Self {
        PyType_ {
            inner: WType::Option(Box::new(of.inner)),
        }
    }
    #[staticmethod]
    #[pyo3(name = "enum")]
    fn enum_(name: String) -> Self {
        PyType_ {
            inner: WType::Enum(name),
        }
    }
    #[staticmethod]
    fn nominal(name: String) -> Self {
        PyType_ {
            inner: WType::Nominal(name),
        }
    }
    #[staticmethod]
    fn id(entity: String) -> Self {
        PyType_ {
            inner: WType::Id(entity),
        }
    }
    /// A reference field type (feature 006): an `Id` whose target must exist.
    #[staticmethod]
    #[pyo3(name = "ref")]
    fn ref_(entity: String) -> Self {
        PyType_ {
            inner: WType::Ref(entity),
        }
    }
    #[staticmethod]
    #[pyo3(signature = (name=None))]
    fn exact(name: Option<String>) -> Self {
        PyType_ {
            inner: WType::Exact(name),
        }
    }
    #[staticmethod]
    fn entity(name: String) -> Self {
        PyType_ {
            inner: WType::Entity(name),
        }
    }

    /// The same type with its named types on the target side of a migration (feature 009).
    #[staticmethod]
    fn on_target(t: PyType_) -> Self {
        fn side(t: WType) -> WType {
            let target = |n: String| format!("{}{n}", behavior_engine::wire::TARGET_SIDE);
            match t {
                WType::Option(inner) => WType::Option(Box::new(side(*inner))),
                WType::Enum(n) => WType::Enum(target(n)),
                WType::Nominal(n) => WType::Nominal(target(n)),
                WType::Exact(Some(n)) => WType::Exact(Some(target(n))),
                other => other,
            }
        }
        PyType_ {
            inner: side(t.inner),
        }
    }

    fn is_option(&self) -> bool {
        matches!(self.inner, WType::Option(_))
    }

    /// The inner type of an option, or None.
    fn inner(&self) -> Option<PyType_> {
        match &self.inner {
            WType::Option(of) => Some(PyType_ {
                inner: (**of).clone(),
            }),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("Type({:?})", self.inner)
    }
}

/// `(name, role or None, Type)` tuples.
fn params(ps: Vec<(String, Option<String>, PyType_)>) -> PyResult<Vec<WParam>> {
    ps.into_iter()
        .map(|(name, role, ty)| {
            let role = match role.as_deref() {
                None => None,
                Some("state") => Some(Role::State),
                Some("input") => Some(Role::Input),
                Some("context") => Some(Role::Context),
                Some(other) => {
                    return Err(PyValueError::new_err(format!("unknown role `{other}`")));
                }
            };
            Ok(WParam {
                name,
                role,
                ty: ty.inner,
            })
        })
        .collect()
}

// --- nodes, records, modules -------------------------------------------------------------

/// A typed expression node owned by the engine.
#[pyclass(name = "Node", frozen, module = "behavior._engine")]
#[derive(Clone)]
struct PyNode {
    inner: Node,
}

#[pymethods]
impl PyNode {
    /// The node's type, or None below an unresolved (cyclic) derived reference.
    #[getter]
    fn r#type(&self) -> Option<PyType_> {
        self.inner.wire_type().map(|inner| PyType_ { inner })
    }

    /// The parameter role of field/param nodes: "state", "input", "context", or "read".
    #[getter]
    fn role(&self) -> Option<&'static str> {
        self.inner.role()
    }

    /// The entity type of a query node (feature 007), or None for a value.
    #[getter]
    fn query_entity(&self) -> Option<String> {
        self.inner.query_entity().map(str::to_string)
    }
}

/// A decision record: `data` for Python, `json` as the canonical artifact.
#[pyclass(name = "Record", frozen, module = "behavior._engine")]
struct PyRecord {
    data: Value,
    json: String,
    diagnostics: Value,
}

#[pymethods]
impl PyRecord {
    #[getter]
    fn result(&self) -> String {
        self.data["result"].as_str().unwrap_or("ERROR").to_string()
    }
    #[getter]
    fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.data)
    }
    #[getter]
    fn json(&self) -> String {
        self.json.clone()
    }
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.diagnostics)
    }
}

/// Core owns canonical invocation evidence and detached diagnostics.
#[pyclass(name = "InvocationRecord", frozen, module = "behavior._engine")]
struct PyInvocationRecord {
    inner: behavior_engine::invocation::InvocationRecord,
}

#[pymethods]
impl PyInvocationRecord {
    #[getter]
    fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, self.inner.as_json())
    }
    #[getter]
    fn json(&self) -> String {
        self.inner.to_json_string()
    }
    #[getter]
    fn record_id(&self) -> &str {
        self.inner.record_id()
    }
    #[getter]
    fn outcome_kind(&self) -> &str {
        self.inner.outcome_kind()
    }
    #[getter]
    fn refusal_stage(&self) -> Option<&str> {
        self.inner.refusal_stage()
    }
    #[getter]
    fn inner_record(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.inner.inner_record() {
            Some(record) => to_py(py, record),
            None => Ok(py.None()),
        }
    }
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, self.inner.diagnostics())
    }
}

/// An admitted ad-hoc read (feature 010): traced in Python, admitted against a module, never part
/// of it.
#[pyclass(name = "ReadItem", frozen, module = "behavior._engine")]
#[derive(Clone)]
struct PyReadItem {
    inner: behavior_engine::semantic::module::ReadItem,
}

#[pymethods]
impl PyReadItem {
    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }
    #[getter]
    fn hash(&self) -> String {
        behavior_engine::semantic::types::hash_display(self.inner.hash())
    }
}

/// A read's execution (feature 010): the full record and the capability response, each as data
/// and as canonical JSON. They are distinct objects; the response holds no evidence.
#[pyclass(name = "ReadExecution", frozen, module = "behavior._engine")]
struct PyReadExecution {
    record: Value,
    record_json: String,
    response: Value,
    response_json: String,
}

#[pymethods]
impl PyReadExecution {
    #[getter]
    fn record(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.record)
    }
    #[getter]
    fn record_json(&self) -> String {
        self.record_json.clone()
    }
    #[getter]
    fn response(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.response)
    }
    #[getter]
    fn response_json(&self) -> String {
        self.response_json.clone()
    }
}

fn read_execution(x: behavior_engine::read::ReadExecution) -> PyReadExecution {
    let response_json = x.response.to_json_string();
    PyReadExecution {
        record: x.record.as_json().clone(),
        record_json: x.record.to_json_string(),
        response: serde_json::from_str(&response_json).unwrap_or(Value::Null),
        response_json,
    }
}

/// An intent rejection as a Python exception (its errors as a list of dicts).
fn intent_rejected(py: Python<'_>, rejection: &behavior_engine::IntentRejection) -> PyErr {
    let errors = serde_json::to_value(&rejection.errors).unwrap_or(Value::Null);
    match to_py(py, &errors) {
        Ok(e) => EngineIntentRejected::new_err((e,)),
        Err(e) => e,
    }
}

/// A read source from Python: a declared read's name, or an admitted ad-hoc read.
fn read_source(source: &Bound<'_, PyAny>) -> PyResult<behavior_engine::read::ReadSource> {
    use behavior_engine::read::ReadSource;
    if let Ok(name) = source.extract::<String>() {
        return Ok(ReadSource::Declared(name));
    }
    if let Ok(item) = source.extract::<PyReadItem>() {
        return Ok(ReadSource::AdHoc(Box::new(item.inner)));
    }
    Err(PyTypeError::new_err(
        "a read is a declared read's name or an admitted ad-hoc read",
    ))
}

fn record(r: behavior_engine::DecisionRecord) -> PyRecord {
    PyRecord {
        json: r.to_json_string(),
        data: r.as_json().clone(),
        diagnostics: r.diagnostics().clone(),
    }
}

fn admission_dict(py: Python<'_>, r: &behavior_engine::AdmissionResult) -> PyResult<Py<PyAny>> {
    to_py(py, &serde_json::to_value(r).unwrap_or(Value::Null))
}

/// A verification attestation: `data` for Python, `json` as the canonical artifact.
#[pyclass(name = "Attestation", frozen, module = "behavior._engine")]
struct PyAttestation {
    data: Value,
    json: String,
}

#[pymethods]
impl PyAttestation {
    #[getter]
    fn result(&self) -> String {
        self.data["result"]
            .as_str()
            .unwrap_or("not_verified")
            .to_string()
    }
    #[getter]
    fn hash(&self) -> String {
        self.data["hash"].as_str().unwrap_or_default().to_string()
    }
    #[getter]
    fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.data)
    }
    #[getter]
    fn json(&self) -> String {
        self.json.clone()
    }
}

/// A commit authorization: `data` for Python, `json` as the canonical artifact.
#[pyclass(name = "Authorization", frozen, module = "behavior._engine")]
struct PyAuthorization {
    data: Value,
    json: String,
}

#[pymethods]
impl PyAuthorization {
    #[getter]
    fn decision(&self) -> String {
        self.data["decision"]
            .as_str()
            .unwrap_or("refuse")
            .to_string()
    }
    #[getter]
    fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.data)
    }
    #[getter]
    fn json(&self) -> String {
        self.json.clone()
    }
}

fn governance_err(e: behavior_engine::verify::governance::GovernanceError) -> PyErr {
    EngineError::new_err(("INVALID_GOVERNANCE_INPUT", e.to_string()))
}

/// The content hash of a waiver.
#[pyfunction]
fn waiver_hash(waiver: &Bound<'_, PyAny>) -> PyResult<String> {
    behavior_engine::verify::governance::waiver_hash(&object(waiver)?.to_string())
        .map_err(governance_err)
}

/// A detached Ed25519 signed attestation over a waiver's hash (seed: 32 bytes as hex).
#[pyfunction]
fn sign_waiver(py: Python<'_>, waiver: &Bound<'_, PyAny>, seed: &str) -> PyResult<Py<PyAny>> {
    let signed =
        behavior_engine::verify::governance::sign_waiver(seed, &object(waiver)?.to_string())
            .map_err(governance_err)?;
    to_py(py, &signed)
}

/// An admitted behavior module.
#[pyclass(name = "Module", frozen, module = "behavior._engine")]
struct EngineModule {
    inner: Module,
}

#[pymethods]
impl EngineModule {
    /// Admits wire JSON (a file's contents); returns (module or None, admission dict).
    #[staticmethod]
    fn from_wire(py: Python<'_>, wire: &str) -> PyResult<(Option<EngineModule>, Py<PyAny>)> {
        match behavior_engine::admit(wire) {
            Ok(m) => {
                let report = admission_dict(py, &behavior_engine::admission_result(&m))?;
                Ok((Some(EngineModule { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }

    #[getter]
    fn behavior_version(&self) -> String {
        self.inner.behavior_version()
    }

    /// The SchemaHash of the store schema the module declares (feature 009).
    #[getter]
    fn schema_hash(&self) -> String {
        behavior_engine::schema(&self.inner).hash
    }

    fn admission(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        admission_dict(py, &behavior_engine::admission_result(&self.inner))
    }

    /// Canonical wire JSON serialized from the admitted module.
    fn wire_json(&self) -> String {
        behavior_engine::serialize::to_wire_json(&self.inner)
    }

    /// Checked requested invocation, or capability intent with explicit host context.
    #[pyo3(signature = (document, snapshot, context=None))]
    fn invoke(
        &self,
        document: &Bound<'_, PyAny>,
        snapshot: &Bound<'_, PyAny>,
        context: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyInvocationRecord> {
        let context = context.map(object).transpose()?;
        let inner = behavior_engine::invocation::invoke_document(
            &self.inner,
            &document_text(document)?,
            &document_text(snapshot)?,
            context.as_ref(),
        )
        .map_err(transport_err)?;
        Ok(PyInvocationRecord { inner })
    }

    fn replay_invocation(&self, record_json: &str) -> (bool, Option<String>) {
        let result = behavior_engine::invocation::replay_invocation(&self.inner, record_json);
        (result.matches, result.diff)
    }

    /// Decides whether the transition in a decision record may be committed under a policy.
    #[pyo3(signature = (policy, record_json, attestation_json, waivers, signatures, now))]
    fn authorize(
        &self,
        policy: &Bound<'_, PyAny>,
        record_json: &str,
        attestation_json: Option<&str>,
        waivers: Vec<Bound<'_, PyAny>>,
        signatures: Vec<Bound<'_, PyAny>>,
        now: &str,
    ) -> PyResult<PyAuthorization> {
        let texts = |items: &[Bound<'_, PyAny>]| -> PyResult<Vec<String>> {
            items
                .iter()
                .map(|i| object(i).map(|v| v.to_string()))
                .collect()
        };
        let a = behavior_engine::verify::governance::authorize(
            &object(policy)?.to_string(),
            &self.inner,
            record_json,
            attestation_json,
            &texts(&waivers)?,
            &texts(&signatures)?,
            now,
        )
        .map_err(governance_err)?;
        Ok(PyAuthorization {
            json: a.to_json_string(),
            data: a.value,
        })
    }

    /// Verifies the module with the pinned solver (`BEHAVIOR_Z3`, else `z3` on `PATH`).
    #[pyo3(signature = (checks, rlimit, wall_clock_guard_ms, cache=None))]
    fn verify(
        &self,
        checks: Option<Vec<String>>,
        rlimit: u64,
        wall_clock_guard_ms: u64,
        cache: Option<String>,
    ) -> PyResult<PyAttestation> {
        let mut profile = behavior_engine::verify::Profile {
            rlimit,
            wall_clock_guard_ms,
            ..Default::default()
        };
        if let Some(names) = checks {
            profile.checks = names
                .iter()
                .map(|n| {
                    behavior_engine::verify::CheckKind::parse(n)
                        .ok_or_else(|| PyValueError::new_err(format!("unknown check `{n}`")))
                })
                .collect::<PyResult<_>>()?;
        }
        let solver = behavior_engine::verify::solver::Z3Process::from_env()
            .map_err(|e| EngineError::new_err(("SOLVER_UNAVAILABLE", e.to_string())))?
            .with_guard(std::time::Duration::from_millis(wall_clock_guard_ms));
        let a = behavior_engine::verify::verify(
            &self.inner,
            &profile,
            cache.as_deref().map(std::path::Path::new),
            &solver,
        );
        Ok(PyAttestation {
            json: a.to_json_string(),
            data: a.value,
        })
    }

    /// `facts`: the evaluation facts section (feature 006: existence, identities, references),
    /// or None.
    #[pyo3(signature = (action, state, input, context, data_version, git_revision=None, facts=None))]
    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &self,
        action: String,
        state: &Bound<'_, PyAny>,
        input: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        data_version: String,
        git_revision: Option<String>,
        facts: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRecord> {
        let mut request = json!({
            "action": action,
            "data_version": data_version,
            "state": object(state)?,
            "input": object(input)?,
            "context": object(context)?,
        });
        if let (Some(g), Value::Object(m)) = (git_revision, &mut request) {
            m.insert("git_revision".into(), json!(g));
        }
        if let (Some(f), Value::Object(m)) = (facts, &mut request) {
            m.insert("facts".into(), to_value(f)?);
        }
        Ok(record(behavior_engine::evaluate(
            &self.inner,
            &request.to_string(),
        )))
    }

    /// Evaluates a structured intent with host-supplied state and context; raises
    /// EngineIntentRejected listing every problem.
    #[pyo3(signature = (intent, state, context, data_version, git_revision=None))]
    fn evaluate_intent(
        &self,
        py: Python<'_>,
        intent: &Bound<'_, PyAny>,
        state: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        data_version: String,
        git_revision: Option<String>,
    ) -> PyResult<PyRecord> {
        let mut host = json!({
            "data_version": data_version,
            "state": object(state)?,
            "context": object(context)?,
        });
        if let (Some(g), Value::Object(m)) = (git_revision, &mut host) {
            m.insert("git_revision".into(), json!(g));
        }
        let intent = object(intent)?;
        match behavior_engine::evaluate_intent(&self.inner, &intent.to_string(), &host.to_string())
        {
            Ok(r) => Ok(record(r)),
            Err(rejection) => {
                let errors = serde_json::to_value(&rejection.errors).unwrap_or(Value::Null);
                Err(EngineIntentRejected::new_err((to_py(py, &errors)?,)))
            }
        }
    }

    /// Evaluates a read in plain mode (feature 010): `source` is a declared read's name or an
    /// admitted ad-hoc read; `facts` the state's facts, or None.
    #[pyo3(signature = (source, state, input, context, data_version, facts=None))]
    fn evaluate_read(
        &self,
        source: &Bound<'_, PyAny>,
        state: &Bound<'_, PyAny>,
        input: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        data_version: String,
        facts: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyReadExecution> {
        let source = read_source(source)?;
        let mut request = json!({
            "data_version": data_version,
            "state": object(state)?,
            "input": object(input)?,
            "context": object(context)?,
        });
        if let (Some(f), Value::Object(m)) = (facts, &mut request) {
            m.insert("facts".into(), to_value(f)?);
        }
        Ok(read_execution(behavior_engine::read::evaluate_read(
            &self.inner,
            &source,
            &request.to_string(),
        )))
    }

    /// Evaluates a read intent (feature 010) with the host's side `{data_version, state,
    /// context, facts}`; raises EngineIntentRejected listing every problem.
    fn evaluate_read_intent(
        &self,
        py: Python<'_>,
        intent: &Bound<'_, PyAny>,
        host: &Bound<'_, PyAny>,
    ) -> PyResult<PyReadExecution> {
        let intent = object(intent)?;
        match behavior_engine::read::evaluate_read_intent(
            &self.inner,
            &intent.to_string(),
            &to_value(host)?.to_string(),
        ) {
            Ok(x) => Ok(read_execution(x)),
            Err(rejection) => Err(intent_rejected(py, &rejection)),
        }
    }

    /// Replays a read record (feature 010) from its own facts; returns (matches, diff).
    fn replay_read(&self, record_json: &str) -> (bool, Option<String>) {
        let r = behavior_engine::read::replay_read(&self.inner, record_json);
        (r.matches, r.diff)
    }

    /// Replays a decision record (its canonical JSON text); returns (matches, diff).
    fn replay(&self, record_json: &str) -> (bool, Option<String>) {
        let r = behavior_engine::replay(&self.inner, record_json);
        (r.matches, r.diff)
    }
}

/// The admission report of an admitted migration.
fn migration_admission(m: &behavior_engine::migration::Migration) -> Value {
    json!({"ok": true, "errors": [], "hash": m.hash(), "summary": m.summary()})
}

/// An admitted migration (feature 009).
#[pyclass(name = "Migration", frozen, module = "behavior._engine")]
struct EngineMigration {
    inner: behavior_engine::migration::Migration,
}

#[pymethods]
impl EngineMigration {
    /// Admits a migration document between two modules: (migration or None, admission dict).
    #[staticmethod]
    fn from_json(
        py: Python<'_>,
        source: PyRef<'_, EngineModule>,
        target: PyRef<'_, EngineModule>,
        text: &str,
    ) -> PyResult<(Option<EngineMigration>, Py<PyAny>)> {
        match behavior_engine::migration::admit_migration(&source.inner, &target.inner, text) {
            Ok(m) => {
                let report = to_py(py, &migration_admission(&m))?;
                Ok((Some(EngineMigration { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }

    #[getter]
    fn hash(&self) -> String {
        self.inner.hash()
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }

    #[getter]
    fn source_schema(&self) -> String {
        self.inner.source_schema().to_string()
    }

    #[getter]
    fn target_schema(&self) -> String {
        self.inner.target_schema().to_string()
    }

    fn summary(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_py(py, &self.inner.summary())
    }

    /// Verifies the migration with the pinned solver (feature 009).
    #[pyo3(signature = (source, target, checks, rlimit, wall_clock_guard_ms))]
    fn verify(
        &self,
        source: PyRef<'_, EngineModule>,
        target: PyRef<'_, EngineModule>,
        checks: Option<Vec<String>>,
        rlimit: u64,
        wall_clock_guard_ms: u64,
    ) -> PyResult<PyAttestation> {
        let mut profile = behavior_engine::verify::Profile {
            rlimit,
            wall_clock_guard_ms,
            ..Default::default()
        };
        if let Some(names) = checks {
            profile.checks = names
                .iter()
                .map(|n| {
                    behavior_engine::verify::CheckKind::parse(n)
                        .ok_or_else(|| PyValueError::new_err(format!("unknown check `{n}`")))
                })
                .collect::<PyResult<_>>()?;
        }
        let solver = behavior_engine::verify::solver::Z3Process::from_env()
            .map_err(|e| EngineError::new_err(("SOLVER_UNAVAILABLE", e.to_string())))?
            .with_guard(std::time::Duration::from_millis(wall_clock_guard_ms));
        let a = behavior_engine::verify::verify_migration(
            &self.inner,
            &source.inner,
            &target.inner,
            &profile,
            None,
            &solver,
        );
        Ok(PyAttestation {
            json: a.to_json_string(),
            data: a.value,
        })
    }

    /// Decides whether this migration may be committed on the store state `data_version` under
    /// an execution policy (feature 009).
    fn authorize(
        &self,
        policy: &Bound<'_, PyAny>,
        data_version: &str,
        attestation_json: Option<&str>,
        waivers: Vec<Bound<'_, PyAny>>,
        signatures: Vec<Bound<'_, PyAny>>,
        now: &str,
    ) -> PyResult<PyAuthorization> {
        let texts = |items: &[Bound<'_, PyAny>]| -> PyResult<Vec<String>> {
            items
                .iter()
                .map(|i| object(i).map(|v| v.to_string()))
                .collect()
        };
        let a = behavior_engine::verify::governance::authorize_migration(
            &object(policy)?.to_string(),
            &self.inner,
            data_version,
            attestation_json,
            &texts(&waivers)?,
            &texts(&signatures)?,
            now,
        )
        .map_err(governance_err)?;
        Ok(PyAuthorization {
            json: a.to_json_string(),
            data: a.value,
        })
    }

    /// The resolved migration document (migration IR 0.1), canonical JSON.
    fn resolved_json(&self) -> String {
        behavior_engine::canonical::to_canonical_string(&self.inner.resolved()).unwrap_or_default()
    }
}

/// Applies a migration to a supplied source universe (plain mode, feature 009): a dict with
/// `result` ("MIGRATED" or the refusal code) and the target entities, requirement outcomes and
/// report, or the refusal's message and entities.
#[pyfunction]
fn apply_migration(
    py: Python<'_>,
    migration: PyRef<'_, EngineMigration>,
    source: PyRef<'_, EngineModule>,
    target: PyRef<'_, EngineModule>,
    entities: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let items: Vec<SeedEntity> = doc("source universe", entities)?;
    let universe: Vec<behavior_engine::migration::SourceEntity> = items
        .into_iter()
        .map(|e| behavior_engine::migration::SourceEntity {
            entity: e.entity,
            value: e.value,
        })
        .collect();
    to_py(
        py,
        &behavior_engine::migration::outcome_json(&behavior_engine::migration::apply_migration(
            &migration.inner,
            &source.inner,
            &target.inner,
            &universe,
        )),
    )
}

/// The engine's construction API; every node is type-checked as it is built.
#[pyclass(name = "Builder", module = "behavior._engine")]
struct PyBuilder {
    inner: Builder,
}

#[pymethods]
impl PyBuilder {
    #[new]
    fn new() -> Self {
        PyBuilder {
            inner: Builder::new(),
        }
    }

    fn declare_enum(
        &mut self,
        name: &str,
        values: Vec<String>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        self.inner
            .declare_enum(name, values, loc(file, line))
            .map_err(build_err)
    }

    #[pyo3(signature = (name, underlying, ops, file, line, scale=None))]
    fn declare_nominal(
        &mut self,
        name: &str,
        underlying: PyType_,
        ops: Vec<String>,
        file: String,
        line: u64,
        scale: Option<u64>,
    ) -> PyResult<()> {
        self.inner
            .declare_nominal(name, underlying.inner, ops, scale, loc(file, line))
            .map_err(build_err)
    }

    /// `fields`: `(name, Type, file, line)` tuples.
    fn declare_entity(
        &mut self,
        name: &str,
        fields: Vec<(String, PyType_, String, u64)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let fields = fields
            .into_iter()
            .map(|(n, t, f, l)| WField {
                name: n,
                ty: t.inner,
                loc: loc(f, l),
            })
            .collect();
        self.inner
            .declare_entity(name, fields, loc(file, line))
            .map_err(build_err)
    }

    /// `site` is "derived" (also used for invariants), "action", or "closed" (a module
    /// invariant, feature 007).
    fn push_scope(
        &mut self,
        site: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let site = match site {
            "action" => ScopeSite::Action,
            "closed" => ScopeSite::Closed,
            "read" => ScopeSite::Read,
            _ => ScopeSite::Derived,
        };
        self.inner
            .push_scope(site, params(ps)?, loc(file, line))
            .map_err(build_err)
    }

    fn pop_scope(&mut self) {
        self.inner.pop_scope();
    }

    /// `select(T)` (feature 007): a query node over every existing entity of type `entity`.
    fn select(&mut self, entity: &str, file: String, line: u64) -> PyResult<PyNode> {
        let inner = self
            .inner
            .select(entity, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    /// Opens a lambda body scope with the candidate `param` of type `entity`; close it with
    /// `pop_scope`.
    fn push_lambda(&mut self, param: &str, entity: &str) -> PyResult<()> {
        self.inner.push_lambda(param, entity).map_err(build_err)
    }

    /// A relational operator with a lambda (`where`, `any`, `all`, `sum`, `min`, `max`,
    /// `unique`).
    #[allow(clippy::too_many_arguments)]
    fn lambda_(
        &mut self,
        op: &str,
        query: PyNode,
        param: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let inner = self
            .inner
            .lambda(op, query.inner, param, body.inner, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    /// A module invariant (feature 007): a closed Bool expression over entity sets.
    fn add_global_invariant(
        &mut self,
        name: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        self.inner
            .add_global_invariant(name, body.inner, loc(file, line))
            .map_err(build_err)
    }

    fn lit(
        &mut self,
        ty: PyType_,
        value: &Bound<'_, PyAny>,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let value = to_value(value)?;
        let inner = self
            .inner
            .lit(ty.inner, value, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn field(&mut self, param: &str, field: &str, file: String, line: u64) -> PyResult<PyNode> {
        let inner = self
            .inner
            .field(param, field, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn param(&mut self, name: &str, file: String, line: u64) -> PyResult<PyNode> {
        let inner = self.inner.param(name, loc(file, line)).map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn derived_ref(
        &mut self,
        name: &str,
        args: Vec<String>,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let inner = self
            .inner
            .derived_ref(name, args, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn op(&mut self, op: &str, args: Vec<PyNode>, file: String, line: u64) -> PyResult<PyNode> {
        let args = args.into_iter().map(|n| n.inner).collect();
        let inner = self
            .inner
            .op(op, args, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn in_(
        &mut self,
        arg: PyNode,
        values: &Bound<'_, PyAny>,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let Value::Array(values) = to_value(values)? else {
            return Err(PyTypeError::new_err("values must be a list"));
        };
        let inner = self
            .inner
            .in_(arg.inner, values, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    /// A builder over an admitted module (feature 010): ad-hoc reads are traced against it.
    #[staticmethod]
    fn for_module(module: PyRef<'_, EngineModule>) -> Self {
        PyBuilder {
            inner: Builder::for_module(&module.inner),
        }
    }

    /// Adds a declared read with a value body (feature 010).
    fn add_read_value(
        &mut self,
        name: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let r = Builder::read_value(name, params(ps)?, body.inner, loc(file, line));
        self.inner.add_read(r).map_err(build_err)
    }

    /// Admits an ad-hoc read with a value body against this builder's module (feature 010).
    fn adhoc_read_value(
        &mut self,
        name: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<PyReadItem> {
        let r = Builder::read_value(name, params(ps)?, body.inner, loc(file, line));
        self.admit_adhoc(&r)
    }

    /// Adds a declared read with a projection body (feature 010): `over` is a query node, or
    /// None with `over_param` naming a `state` parameter; `items` are (kind, name) pairs with kind
    /// "field" or "derived".
    #[allow(clippy::too_many_arguments)]
    fn add_read_projection(
        &mut self,
        name: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        over: Option<PyNode>,
        over_param: Option<String>,
        member: &str,
        items: Vec<(String, String)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let r = projection_read(name, ps, over, over_param, member, items, file, line)?;
        self.inner.add_read(r).map_err(build_err)
    }

    /// Admits an ad-hoc read with a projection body against this builder's module (feature 010).
    #[allow(clippy::too_many_arguments)]
    fn adhoc_read_projection(
        &mut self,
        name: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        over: Option<PyNode>,
        over_param: Option<String>,
        member: &str,
        items: Vec<(String, String)>,
        file: String,
        line: u64,
    ) -> PyResult<PyReadItem> {
        let r = projection_read(name, ps, over, over_param, member, items, file, line)?;
        self.admit_adhoc(&r)
    }

    /// A builder for a migration from `source` to `target` (feature 009).
    #[staticmethod]
    fn for_migration(source: PyRef<'_, EngineModule>, target: PyRef<'_, EngineModule>) -> Self {
        PyBuilder {
            inner: Builder::for_migration(&source.inner, &target.inner),
        }
    }

    fn push_transform(&mut self, entity: &str, file: String, line: u64) -> PyResult<()> {
        self.inner
            .push_transform(entity, loc(file, line))
            .map_err(build_err)
    }

    fn push_requirement(&mut self) -> PyResult<()> {
        self.inner.push_requirement().map_err(build_err)
    }

    fn strict_unwrap(&mut self, arg: PyNode, file: String, line: u64) -> PyResult<PyNode> {
        let inner = self
            .inner
            .strict_unwrap(arg.inner, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    #[allow(clippy::too_many_arguments)]
    fn enum_map(
        &mut self,
        arg: PyNode,
        to: PyType_,
        mapping: Vec<(String, String)>,
        strict: bool,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let inner = self
            .inner
            .enum_map(arg.inner, to.inner, mapping, strict, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn set_field(&mut self, entity: &str, field: &str, value: PyNode) -> PyResult<()> {
        self.inner
            .set_field(entity, field, value.inner)
            .map_err(build_err)
    }

    fn drop_field(&mut self, entity: &str, field: &str) -> PyResult<()> {
        self.inner.drop_field(entity, field).map_err(build_err)
    }

    fn add_requirement(
        &mut self,
        name: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        self.inner
            .add_requirement(name, body.inner, loc(file, line))
            .map_err(build_err)
    }

    fn retire(&mut self, entity: &str) -> PyResult<()> {
        self.inner.retire(entity).map_err(build_err)
    }

    /// Admits the built migration: (migration or None, admission dict).
    fn finish_migration(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> PyResult<(Option<EngineMigration>, Py<PyAny>)> {
        match self.inner.finish_migration(name) {
            Ok(m) => {
                let report = to_py(py, &migration_admission(&m))?;
                Ok((Some(EngineMigration { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }

    fn wrap(&mut self, nominal: &str, arg: PyNode, file: String, line: u64) -> PyResult<PyNode> {
        let inner = self
            .inner
            .wrap(nominal, arg.inner, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn check_condition(&self, node: PyNode, what: &str) -> PyResult<()> {
        self.inner
            .check_condition(&node.inner, what)
            .map_err(build_err)
    }

    fn check_effect(&mut self, param: &str, field: &str, value: PyNode) -> PyResult<()> {
        self.inner
            .check_effect(param, field, &value.inner)
            .map_err(build_err)
    }

    #[pyo3(signature = (name, kind, ps, body, file, line, declared=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_derived(
        &mut self,
        name: &str,
        kind: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        body: PyNode,
        file: String,
        line: u64,
        declared: Option<PyType_>,
    ) -> PyResult<()> {
        let kind = if kind == "rule" {
            DerivedKind::Rule
        } else {
            DerivedKind::Derived
        };
        self.inner
            .add_derived(
                name,
                kind,
                params(ps)?,
                body.inner,
                declared.map(|t| t.inner),
                loc(file, line),
            )
            .map_err(build_err)
    }

    fn rescale(
        &mut self,
        arg: PyNode,
        nominal: &str,
        rounding: &str,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let inner = self
            .inner
            .rescale(arg.inner, nominal, rounding, loc(file, line))
            .map_err(build_err)?;
        Ok(PyNode { inner })
    }

    fn add_invariant(
        &mut self,
        name: &str,
        entity: &str,
        param: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        self.inner
            .add_invariant(name, entity, param, body.inner, loc(file, line))
            .map_err(build_err)
    }

    fn add_constraint(
        &mut self,
        name: &str,
        entity: &str,
        param: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        self.inner
            .add_constraint(name, entity, param, body.inner, loc(file, line))
            .map_err(build_err)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_action(
        &mut self,
        name: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        preconditions: Vec<(PyNode, String, u64)>,
        effects: Vec<(String, String, PyNode, String, u64)>,
        lifecycle: Vec<LifecycleArg>,
        postconditions: Vec<(PyNode, String, u64)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let cond = |(n, f, l): (PyNode, String, u64)| (n.inner, loc(f, l));
        let lifecycle = lifecycle
            .into_iter()
            .map(|(kind, name, id, fields, f, l)| match (kind.as_str(), id) {
                ("create", Some(id)) => Ok(WLifecycle::Create {
                    entity: name,
                    id: id.inner.into_wire(),
                    fields: fields
                        .into_iter()
                        .map(|(k, v)| (k, v.inner.into_wire()))
                        .collect(),
                    loc: loc(f, l),
                }),
                ("remove", None) => Ok(WLifecycle::Remove {
                    param: name,
                    loc: loc(f, l),
                }),
                _ => Err(PyValueError::new_err(format!(
                    "bad lifecycle effect `{kind}`"
                ))),
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.inner
            .add_action(
                name,
                params(ps)?,
                preconditions.into_iter().map(cond).collect(),
                effects
                    .into_iter()
                    .map(|(p, fld, n, f, l)| (p, fld, n.inner, loc(f, l)))
                    .collect(),
                lifecycle,
                postconditions.into_iter().map(cond).collect(),
                loc(file, line),
            )
            .map_err(build_err)
    }

    /// Admits the built module: returns (module or None, admission dict).
    fn finish(
        &self,
        py: Python<'_>,
        root: Option<String>,
    ) -> PyResult<(Option<EngineModule>, Py<PyAny>)> {
        match self.inner.finish(root.as_deref()) {
            Ok(m) => {
                let report = admission_dict(py, &behavior_engine::admission_result(&m))?;
                Ok((Some(EngineModule { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn projection_read(
    name: &str,
    ps: Vec<(String, Option<String>, PyType_)>,
    over: Option<PyNode>,
    over_param: Option<String>,
    member: &str,
    items: Vec<(String, String)>,
    file: String,
    line: u64,
) -> PyResult<behavior_engine::wire::WRead> {
    use behavior_engine::wire::WItem;
    let over = match (over, over_param) {
        (Some(node), None) => Ok(node.inner),
        (None, Some(param)) => Err(param),
        _ => {
            return Err(PyValueError::new_err(
                "a projection has a query or a parameter to range over",
            ));
        }
    };
    let items = items
        .into_iter()
        .map(|(kind, n)| match kind.as_str() {
            "field" => Ok(WItem::Field(n)),
            "derived" => Ok(WItem::Derived(n)),
            other => Err(PyValueError::new_err(format!(
                "bad projection item kind `{other}`"
            ))),
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(Builder::read_projection(
        name,
        params(ps)?,
        over,
        member,
        items,
        loc(file, line),
    ))
}

impl PyBuilder {
    fn admit_adhoc(&mut self, r: &behavior_engine::wire::WRead) -> PyResult<PyReadItem> {
        match self.inner.admit_read(r) {
            Ok(inner) => Ok(PyReadItem { inner }),
            Err(result) => Err(build_err(
                result
                    .errors
                    .into_iter()
                    .next()
                    .map(BuildError::from)
                    .unwrap_or_else(|| BuildError {
                        code: "TYPE_MISMATCH".into(),
                        message: "ill-typed read".into(),
                    }),
            )),
        }
    }
}

// --- persistence (feature 005) ------------------------------------------------------------

use behavior_engine::store::conformance::run as run_store_conformance;
use behavior_engine::store::documents::{
    CommitBundle, EntityKey, EntityVersion, Evidence, EvidencePolicy, Genesis, Head, HistoryRef,
    RefChange, SeedEntity, StateRef, StoreError, TransitionRecord, decode,
};
use behavior_engine::store::replay::replay_data;
use behavior_engine::store::{Backend, BackendError, CasOutcome, InMemoryBackend, RefEdge, Store};

fn to_json<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

fn doc<T: for<'de> serde::Deserialize<'de>>(what: &str, obj: &Bound<'_, PyAny>) -> PyResult<T> {
    decode(what, &to_value(obj)?).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn serialize_py<T: serde::Serialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value)
        .map_err(|e| EngineError::new_err(("SERIALIZATION_ERROR", e.to_string())))?;
    to_py(py, &value)
}

fn trusted_store_err(e: behavior_engine::verify::governance::TrustedError) -> PyErr {
    EngineStoreRefused::new_err((e.code, e.message))
}

fn checked_bundle(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<CommitBundle> {
    CommitBundle::from_json(&document_text(obj)?).map_err(|e| store_err(py, e))
}

fn store_invocation(
    py: Python<'_>,
    result: behavior_engine::invocation::Invocation,
) -> PyResult<(PyInvocationRecord, Py<PyAny>)> {
    let bundle = match result.bundle {
        Some(bundle) => serialize_py(py, &bundle)?,
        None => py.None(),
    };
    Ok((
        PyInvocationRecord {
            inner: result.record,
        },
        bundle,
    ))
}

/// Derive the exact signed-governance candidate through Core's checked bundle contract.
#[pyfunction]
fn governance_candidate(py: Python<'_>, bundle: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let candidate = checked_bundle(py, bundle)?
        .governance_candidate()
        .map_err(|e| store_err(py, e))?;
    to_py(py, &candidate.as_json())
}

#[pyfunction]
fn with_trusted_evidence(
    py: Python<'_>,
    bundle: &Bound<'_, PyAny>,
    evidence: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let bundle = checked_bundle(py, bundle)?;
    let evidence =
        behavior_engine::verify::governance::EvidenceV2::from_json(&document_text(evidence)?)
            .map_err(trusted_store_err)?;
    let attached = bundle
        .with_trusted_evidence(&evidence)
        .map_err(|e| store_err(py, e))?;
    serialize_py(py, &attached)
}

fn store_err(py: Python<'_>, e: StoreError) -> PyErr {
    match e {
        StoreError::StateConflict { current, changed } => {
            let current = to_py(py, &to_json(&current)).unwrap_or_else(|_| py.None());
            let changed = to_py(py, &to_json(&changed)).unwrap_or_else(|_| py.None());
            EngineStoreConflict::new_err((current, changed))
        }
        StoreError::SchemaMismatch {
            ref store,
            ref module,
            ref differing,
        } => {
            let details = json!({"store": store, "module": module, "differing": differing});
            let details = to_py(py, &details).unwrap_or_else(|_| py.None());
            EngineStoreRefused::new_err((e.code().to_string(), e.to_string(), details))
        }
        e => EngineStoreRefused::new_err((e.code().to_string(), e.to_string())),
    }
}

/// A backend implemented by a Python object with the seven backend methods (dicts in and out).
struct PyBackend {
    obj: Py<PyAny>,
}

impl PyBackend {
    fn call<T: for<'de> serde::Deserialize<'de>>(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Option<T>, BackendError> {
        Python::attach(|py| {
            let args: Vec<Py<PyAny>> = args
                .iter()
                .map(|a| to_py(py, a))
                .collect::<PyResult<_>>()
                .map_err(|e| BackendError(e.to_string()))?;
            let out = self
                .obj
                .bind(py)
                .call_method1(
                    method,
                    PyTuple::new(py, args).map_err(|e| BackendError(e.to_string()))?,
                )
                .map_err(|e| BackendError(format!("{method}: {e}")))?;
            if out.is_none() {
                return Ok(None);
            }
            let v = to_value(&out).map_err(|e| BackendError(format!("{method}: {e}")))?;
            serde_json::from_value(v)
                .map(Some)
                .map_err(|e| BackendError(format!("{method} returned an invalid document: {e}")))
        })
    }
}

impl Backend for PyBackend {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.call("genesis", vec![])
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.call("head", vec![])
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        seed: &[EntityVersion],
        seed_refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.call::<Value>(
            "create",
            vec![to_json(g), to_json(h), to_json(&seed), to_json(&seed_refs)],
        )
        .map(|_| ())
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.call("removed_at", vec![to_json(k)])
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        let edges: Option<Vec<Value>> = self.call("incoming_at", vec![to_json(t), json!(p)])?;
        edges
            .unwrap_or_default()
            .into_iter()
            .map(|e| {
                let field = |k: &str| {
                    e[k].as_str()
                        .map(str::to_string)
                        .ok_or_else(|| BackendError(format!("incoming_at: edge without `{k}`")))
                };
                Ok(RefEdge {
                    entity: field("entity")?,
                    id: field("id")?,
                    field: field("field")?,
                })
            })
            .collect()
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        let keys: Option<Vec<EntityKey>> = self.call("keys_at", vec![json!(t), json!(p)])?;
        Ok(keys.unwrap_or_default())
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        // Optional in Python backends: "not indexed" otherwise.
        let has = Python::attach(|py| {
            self.obj
                .bind(py)
                .hasattr("keys_by_field_at")
                .unwrap_or(false)
        });
        if !has {
            return Ok(None);
        }
        self.call(
            "keys_by_field_at",
            vec![json!(t), json!(f), v.clone(), json!(p)],
        )
    }
    fn used_at(&self, k: &EntityKey, p: u64) -> Result<bool, BackendError> {
        // Optional in Python backends: the contract's default otherwise.
        let has = Python::attach(|py| self.obj.bind(py).hasattr("used_at").unwrap_or(false));
        if has {
            let used: Option<bool> = self.call("used_at", vec![to_json(k), json!(p)])?;
            return Ok(used.unwrap_or(false));
        }
        Ok(self.version_at(k, p)?.is_some())
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.call("version_at", vec![to_json(k), json!(p)])
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.call("version", vec![to_json(k), json!(r)])
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.call("record", vec![json!(p)])
    }
    fn commit(
        &mut self,
        expected: &str,
        versions: &[EntityVersion],
        removals: &[EntityKey],
        ref_changes: &[RefChange],
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        let out: Option<String> = self.call(
            "commit",
            vec![
                json!(expected),
                to_json(&versions),
                to_json(&removals),
                to_json(&ref_changes),
                to_json(record),
                to_json(head),
            ],
        )?;
        match out.as_deref() {
            Some("applied") => Ok(CasOutcome::Applied),
            Some("head_moved") => Ok(CasOutcome::HeadMoved),
            other => Err(BackendError(format!(
                "commit must return \"applied\" or \"head_moved\", got {other:?}"
            ))),
        }
    }
}

/// The reference backend or a Python backend.
enum AnyBackend {
    Memory(Box<InMemoryBackend>),
    Py(PyBackend),
}

macro_rules! delegate {
    ($self:ident, $b:ident => $e:expr) => {
        match $self {
            AnyBackend::Memory($b) => $e,
            AnyBackend::Py($b) => $e,
        }
    };
}

impl Backend for AnyBackend {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        delegate!(self, b => b.genesis())
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        delegate!(self, b => b.head())
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        s: &[EntityVersion],
        r: &[RefChange],
    ) -> Result<(), BackendError> {
        delegate!(self, b => b.create(g, h, s, r))
    }
    fn removed_at(&self, k: &EntityKey) -> Result<Option<u64>, BackendError> {
        delegate!(self, b => b.removed_at(k))
    }
    fn incoming_at(&self, t: &EntityKey, p: u64) -> Result<Vec<RefEdge>, BackendError> {
        delegate!(self, b => b.incoming_at(t, p))
    }
    fn used_at(&self, k: &EntityKey, p: u64) -> Result<bool, BackendError> {
        delegate!(self, b => b.used_at(k, p))
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        delegate!(self, b => b.keys_at(t, p))
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        delegate!(self, b => b.keys_by_field_at(t, f, v, p))
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        delegate!(self, b => b.version_at(k, p))
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        delegate!(self, b => b.version(k, r))
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        delegate!(self, b => b.record(p))
    }
    fn commit(
        &mut self,
        x: &str,
        v: &[EntityVersion],
        rm: &[EntityKey],
        refs: &[RefChange],
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        delegate!(self, b => b.commit(x, v, rm, refs, r, h))
    }
}

/// Marker for the in-memory reference backend (`Store.create(InMemoryBackend(), ...)`).
#[pyclass(name = "InMemoryBackend", frozen, module = "behavior._engine")]
#[derive(Clone)]
struct PyInMemoryBackend {}

#[pymethods]
impl PyInMemoryBackend {
    #[new]
    fn new() -> Self {
        PyInMemoryBackend {}
    }
}

fn any_backend(obj: &Bound<'_, PyAny>) -> AnyBackend {
    if obj.is_instance_of::<PyInMemoryBackend>() {
        AnyBackend::Memory(Box::new(InMemoryBackend::new()))
    } else {
        AnyBackend::Py(PyBackend {
            obj: obj.clone().unbind(),
        })
    }
}

fn state_ref(obj: &Bound<'_, PyAny>) -> PyResult<StateRef> {
    doc("state reference", obj)
}

#[pyclass(name = "Store", module = "behavior._engine")]
struct PyStore {
    inner: Store<AnyBackend>,
}

#[pymethods]
impl PyStore {
    /// A genesis for the module's entity declarations: `policy` is an evidence policy dict (or
    /// None for "no evidence required"), `seed` a list of {entity, value} dicts.
    #[staticmethod]
    #[pyo3(signature = (module, seed, policy=None))]
    fn genesis_for(
        py: Python<'_>,
        module: &EngineModule,
        seed: &Bound<'_, PyAny>,
        policy: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let seed: Vec<SeedEntity> = doc("seed", seed)?;
        let policy = match policy {
            Some(p) => doc::<EvidencePolicy>("evidence policy", p)?,
            None => EvidencePolicy::none(),
        };
        to_py(
            py,
            &to_json(&behavior_engine::store::store::genesis_for(
                &module.inner,
                policy,
                seed,
            )),
        )
    }

    /// Explicit store-v2 history with a checked trusted evidence policy.
    #[staticmethod]
    fn genesis_v2_for(
        py: Python<'_>,
        module: &EngineModule,
        seed: &Bound<'_, PyAny>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let policy = behavior_engine::verify::governance::EvidencePolicyV2::from_json(
            &document_text(policy)?,
        )
        .map_err(trusted_store_err)?;
        let genesis = behavior_engine::store::store::genesis_v2_for(
            &module.inner,
            policy,
            doc("seed", seed)?,
        )
        .map_err(|e| store_err(py, e))?;
        serialize_py(py, &genesis)
    }

    #[staticmethod]
    fn create(
        py: Python<'_>,
        backend: &Bound<'_, PyAny>,
        module: &EngineModule,
        genesis: &Bound<'_, PyAny>,
    ) -> PyResult<PyStore> {
        let genesis: Genesis = doc("genesis", genesis)?;
        Store::create(any_backend(backend), &module.inner, genesis)
            .map(|inner| PyStore { inner })
            .map_err(|e| store_err(py, e))
    }

    #[staticmethod]
    fn open(py: Python<'_>, backend: &Bound<'_, PyAny>) -> PyResult<PyStore> {
        if backend.is_instance_of::<PyInMemoryBackend>() {
            return Err(PyValueError::new_err(
                "a fresh in-memory backend has no store to open",
            ));
        }
        Store::open(any_backend(backend))
            .map(|inner| PyStore { inner })
            .map_err(|e| store_err(py, e))
    }

    fn store_id(&self, py: Python<'_>) -> PyResult<String> {
        self.inner.store_id().map_err(|e| store_err(py, e))
    }

    fn current(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let r = self.inner.current().map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&r))
    }

    fn current_history(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let history = self.inner.current_history().map_err(|e| store_err(py, e))?;
        to_py(py, &history.as_json())
    }

    fn history_at(&self, py: Python<'_>, position: u64) -> PyResult<Py<PyAny>> {
        let history = self
            .inner
            .history_at(position)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &history.as_json())
    }

    fn commands_since(&self, py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let request = behavior_engine::store::commands::CommandStreamRequest::from_json(
            &document_text(request)?,
        )
        .map_err(|e| store_err(py, e))?;
        let page = self
            .inner
            .commands_since(&request)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &page.as_json())
    }

    fn export_seed_at(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        history: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let history =
            HistoryRef::from_json(&document_text(history)?).map_err(|e| store_err(py, e))?;
        let exported = self
            .inner
            .export_seed_at(&module.inner, &history)
            .map_err(|e| store_err(py, e))?;
        serialize_py(py, &exported)
    }

    fn state_at(&self, py: Python<'_>, position: u64) -> PyResult<Py<PyAny>> {
        let r = self
            .inner
            .state_at(position)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&r))
    }

    fn load(
        &self,
        py: Python<'_>,
        entity: String,
        id: String,
        at: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let v = self
            .inner
            .load(&EntityKey { entity, id }, &state_ref(at)?)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&v))
    }

    /// Evaluates against one consistent snapshot; returns (record, bundle or None).
    #[pyo3(signature = (module, action, bindings, input, context, commit_time, evidence=None))]
    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        action: String,
        bindings: &Bound<'_, PyAny>,
        input: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        commit_time: String,
        evidence: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(PyRecord, Py<PyAny>)> {
        let bindings: std::collections::BTreeMap<String, String> = doc("bindings", bindings)?;
        let evidence: Option<Evidence> = evidence.map(|e| doc("evidence", e)).transpose()?;
        let ev = self
            .inner
            .evaluate(
                &module.inner,
                &action,
                &bindings,
                &object(input)?,
                &object(context)?,
                &commit_time,
                evidence,
            )
            .map_err(|e| store_err(py, e))?;
        let rec = PyRecord {
            json: behavior_engine::canonical::to_canonical_string(&ev.record).unwrap_or_default(),
            data: ev.record,
            diagnostics: Value::Null,
        };
        let bundle = match &ev.bundle {
            Some(b) => to_py(py, &to_json(b))?,
            None => py.None(),
        };
        Ok((rec, bundle))
    }

    /// Core's typed store request boundary reports shape problems before resolution.
    #[pyo3(signature = (module, document, commit_time, evidence=None, at=None))]
    fn invoke(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        document: &Bound<'_, PyAny>,
        commit_time: &str,
        evidence: Option<&Bound<'_, PyAny>>,
        at: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(PyInvocationRecord, Py<PyAny>)> {
        let (requested, errors) =
            behavior_engine::invocation::RequestedInvocation::decode(&document_text(document)?)
                .map_err(transport_err)?;
        let Some(requested) = requested else {
            let diagnostics = serde_json::to_string(&errors)
                .map_err(|e| EngineError::new_err(("SERIALIZATION_ERROR", e.to_string())))?;
            return Err(EngineError::new_err(("INVALID_INVOCATION", diagnostics)));
        };
        let at = at.map(state_ref).transpose()?;
        let evidence = evidence
            .map(|e| {
                let value = behavior_engine::canonical::decode_strict(&document_text(e)?)
                    .map_err(|e| EngineError::new_err(("DECODE_ERROR", e.to_string())))?;
                decode::<Evidence>("evidence", &value).map_err(|e| store_err(py, e))
            })
            .transpose()?;
        let result = self
            .inner
            .invoke(
                &module.inner,
                &requested,
                commit_time,
                evidence,
                at.as_ref(),
            )
            .map_err(|e| store_err(py, e))?;
        store_invocation(py, result)
    }

    /// Core's store capability-intent boundary retains parseable decode refusals as records.
    #[pyo3(signature = (module, intent, context, commit_time, at=None))]
    fn invoke_intent(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        intent: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        commit_time: &str,
        at: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(PyInvocationRecord, Py<PyAny>)> {
        let at = at.map(state_ref).transpose()?;
        let result = self
            .inner
            .invoke_intent(
                &module.inner,
                &document_text(intent)?,
                &object(context)?,
                commit_time,
                at.as_ref(),
            )
            .map_err(|e| store_err(py, e))?;
        store_invocation(py, result)
    }

    fn replay_invocation(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        record_json: &str,
    ) -> PyResult<(bool, Option<String>)> {
        let result = self
            .inner
            .replay_invocation(&module.inner, record_json)
            .map_err(|e| store_err(py, e))?;
        Ok((result.matches, result.diff))
    }

    /// A read intent (feature 010) against this store at `at` or the head; raises
    /// EngineIntentRejected listing every problem.
    #[pyo3(signature = (module, intent, context, at=None))]
    fn read_intent(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        intent: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        at: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyReadExecution> {
        let at = at.map(state_ref).transpose()?;
        let intent = object(intent)?;
        match self
            .inner
            .read_intent(
                &module.inner,
                &intent.to_string(),
                &object(context)?,
                at.as_ref(),
            )
            .map_err(|e| store_err(py, e))?
        {
            Ok(x) => Ok(read_execution(x)),
            Err(rejection) => Err(intent_rejected(py, &rejection)),
        }
    }

    /// Replays a read record (feature 010) against this store; returns (matches, diff).
    fn replay_read(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        record_json: &str,
    ) -> PyResult<(bool, Option<String>)> {
        let r = self
            .inner
            .replay_read(&module.inner, record_json)
            .map_err(|e| store_err(py, e))?;
        Ok((r.matches, r.diff))
    }

    /// Reads (feature 010) at `at` (a StateRef dict) or the head; never writes.
    #[pyo3(signature = (module, source, bindings, input, context, at=None))]
    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        py: Python<'_>,
        module: &EngineModule,
        source: &Bound<'_, PyAny>,
        bindings: &Bound<'_, PyAny>,
        input: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        at: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyReadExecution> {
        let source = read_source(source)?;
        let bindings: std::collections::BTreeMap<String, String> = doc("bindings", bindings)?;
        let at = at.map(state_ref).transpose()?;
        let x = self
            .inner
            .read(
                &module.inner,
                &source,
                &bindings,
                &object(input)?,
                &object(context)?,
                at.as_ref(),
            )
            .map_err(|e| store_err(py, e))?;
        Ok(read_execution(x))
    }

    /// Commits a bundle on `expected_parent`; returns {record_id, result_state, already,
    /// evidence_trust}. Raises EngineStoreConflict or EngineStoreRefused.
    #[pyo3(signature = (module, expected_parent, bundle))]
    fn commit(
        &mut self,
        py: Python<'_>,
        module: &EngineModule,
        expected_parent: Option<&Bound<'_, PyAny>>,
        bundle: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let bundle = checked_bundle(py, bundle)?;
        let parent = expected_parent
            .map(state_ref)
            .transpose()?
            .unwrap_or_else(|| bundle.evaluated_state.clone());
        let c = self
            .inner
            .commit(&module.inner, &parent, &bundle)
            .map_err(|e| store_err(py, e))?;
        to_py(
            py,
            &json!({"record_id": c.record_id, "result_state": to_json(&c.result_state),
                    "already": c.already, "evidence_trust": c.evidence_trust}),
        )
    }

    /// Supply live host context independently from the signed evidence in the bundle.
    #[pyo3(signature = (module, expected_parent, bundle, context, execution_policy))]
    #[allow(clippy::too_many_arguments)]
    fn commit_with_context(
        &mut self,
        py: Python<'_>,
        module: &EngineModule,
        expected_parent: Option<&Bound<'_, PyAny>>,
        bundle: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        execution_policy: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let bundle = checked_bundle(py, bundle)?;
        let parent = expected_parent
            .map(state_ref)
            .transpose()?
            .unwrap_or_else(|| bundle.evaluated_state.clone());
        let policy = behavior_engine::verify::governance::ExecutionPolicyV2::from_json(
            &document_text(execution_policy)?,
        )
        .map_err(trusted_store_err)?;
        let context = behavior_engine::verify::governance::AuthorizationContextV2::from_json(
            &document_text(context)?,
            &policy,
        )
        .map_err(trusted_store_err)?;
        let committed = self
            .inner
            .commit_with_context(&module.inner, &parent, &bundle, &context)
            .map_err(|e| store_err(py, e))?;
        to_py(
            py,
            &json!({"record_id": committed.record_id,
            "result_state": committed.result_state, "already": committed.already,
            "evidence_trust": committed.evidence_trust}),
        )
    }

    fn transitions(
        &self,
        py: Python<'_>,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let rs = self
            .inner
            .transitions(&state_ref(from)?, &state_ref(to)?)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&rs))
    }

    /// Applies a migration as one atomic transition at the next position (feature 009).
    #[pyo3(signature = (migration, source, target, commit_time, evidence=None))]
    fn migrate(
        &mut self,
        py: Python<'_>,
        migration: PyRef<'_, EngineMigration>,
        source: PyRef<'_, EngineModule>,
        target: PyRef<'_, EngineModule>,
        commit_time: &str,
        evidence: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let evidence: Option<Evidence> = match evidence {
            Some(e) if !e.is_none() => Some(doc("evidence", e)?),
            _ => None,
        };
        let c = self
            .inner
            .migrate(
                &migration.inner,
                &source.inner,
                &target.inner,
                commit_time,
                evidence,
            )
            .map_err(|e| store_err(py, e))?;
        to_py(
            py,
            &json!({"record_id": c.record_id, "result_state": to_json(&c.result_state),
                    "already": c.already, "evidence_trust": c.evidence_trust}),
        )
    }

    /// `store:<id>;state:<state>;position:<n>` of state `at`: what an authorization binds.
    fn data_version(&self, py: Python<'_>, at: &Bound<'_, PyAny>) -> PyResult<String> {
        let store = self.inner.store_id().map_err(|e| store_err(py, e))?;
        Ok(behavior_engine::store::documents::data_version(
            &store,
            &state_ref(at)?,
        ))
    }

    /// The schema under which state `at` is valid (feature 009).
    fn schema_at(&self, py: Python<'_>, at: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let s = self
            .inner
            .schema_at(&state_ref(at)?)
            .map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&s))
    }

    /// Every schema the store has had, oldest first (feature 009).
    fn schema_history(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let h = self.inner.schema_history().map_err(|e| store_err(py, e))?;
        to_py(py, &to_json(&h))
    }

    fn replay_data(
        &self,
        py: Python<'_>,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        to_py(
            py,
            &to_json(&replay_data(
                &self.inner,
                &state_ref(from)?,
                &state_ref(to)?,
            )),
        )
    }

    /// `migrations`: (migration, source module, target module) triples (feature 009).
    #[pyo3(signature = (modules, from, to, migrations=Vec::new()))]
    fn replay_behavior(
        &self,
        py: Python<'_>,
        modules: Vec<PyRef<'_, EngineModule>>,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
        migrations: Vec<(
            PyRef<'_, EngineMigration>,
            PyRef<'_, EngineModule>,
            PyRef<'_, EngineModule>,
        )>,
    ) -> PyResult<Py<PyAny>> {
        let modules = modules
            .iter()
            .map(|m| (m.inner.behavior_version(), m.inner.clone()))
            .collect();
        let migrations = migrations
            .iter()
            .map(|(m, s, t)| {
                (
                    m.inner.hash(),
                    (m.inner.clone(), s.inner.clone(), t.inner.clone()),
                )
            })
            .collect();
        to_py(
            py,
            &to_json(&behavior_engine::store::replay::replay_behavior_with(
                &self.inner,
                &modules,
                &migrations,
                &state_ref(from)?,
                &state_ref(to)?,
            )),
        )
    }
}

/// Runs the conformance suite against backends from `factory` (a callable returning a backend
/// object); returns a list of (name, ok, message).
#[pyfunction]
fn run_conformance(factory: &Bound<'_, PyAny>) -> PyResult<Vec<(String, bool, String)>> {
    let factory = factory.clone().unbind();
    let report = run_store_conformance(|| {
        Python::attach(|py| match factory.bind(py).call0() {
            Ok(obj) => any_backend(&obj),
            Err(e) => AnyBackend::Py(PyBackend {
                obj: e.value(py).clone().into_any().unbind(),
            }),
        })
    });
    Ok(report
        .cases
        .into_iter()
        .map(|c| (c.name.to_string(), c.ok, c.message))
        .collect())
}

/// Every version of this release (feature 008): the engine, its formats, the store documents and
/// the verifier. The binding adds its own version in Python.
#[pyfunction]
fn engine_info(py: Python<'_>) -> PyResult<Py<PyAny>> {
    to_py(py, &behavior_engine::engine_info())
}

/// A notice if the solver verification would use is not the supported version (feature 008).
#[pyfunction]
fn solver_notice() -> Option<String> {
    behavior_engine::verify::solver::Z3Process::from_env()
        .ok()
        .and_then(|z| z.version_mismatch())
}

#[pymodule]
fn _engine(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add("ENGINE_VERSION", env!("CARGO_PKG_VERSION"))?;
    // The Core Release this extension is built against, as core-release.json declares it
    // (feature 011); the engine itself reports its own version through `engine_info`.
    let core = pyo3::types::PyDict::new(m.py());
    core.set_item("version", env!("BEHAVIOR_CORE_VERSION"))?;
    core.set_item("commit", env!("BEHAVIOR_CORE_COMMIT"))?;
    m.add("CORE", core)?;
    m.add_function(wrap_pyfunction!(engine_info, m)?)?;
    m.add_function(wrap_pyfunction!(solver_notice, m)?)?;
    m.add_class::<PyType_>()?;
    m.add_class::<PyBuilder>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyRecord>()?;
    m.add_class::<PyInvocationRecord>()?;
    m.add_class::<PyReadItem>()?;
    m.add_class::<PyReadExecution>()?;
    m.add_class::<EngineModule>()?;
    m.add_class::<PyAttestation>()?;
    m.add_class::<PyAuthorization>()?;
    m.add_function(wrap_pyfunction!(waiver_hash, m)?)?;
    m.add_function(wrap_pyfunction!(sign_waiver, m)?)?;
    m.add_class::<PyStore>()?;
    m.add_class::<EngineMigration>()?;
    m.add_function(wrap_pyfunction!(apply_migration, m)?)?;
    m.add("TARGET_SIDE", behavior_engine::wire::TARGET_SIDE)?;
    m.add_class::<PyInMemoryBackend>()?;
    m.add_function(wrap_pyfunction!(run_conformance, m)?)?;
    m.add_function(wrap_pyfunction!(governance_candidate, m)?)?;
    m.add_function(wrap_pyfunction!(with_trusted_evidence, m)?)?;
    m.add(
        "EngineStoreConflict",
        m.py().get_type::<EngineStoreConflict>(),
    )?;
    m.add(
        "EngineStoreRefused",
        m.py().get_type::<EngineStoreRefused>(),
    )?;
    m.add("EngineError", m.py().get_type::<EngineError>())?;
    m.add(
        "EngineIntentRejected",
        m.py().get_type::<EngineIntentRejected>(),
    )?;
    Ok(())
}
