//! PyO3 bindings: Python holds engine objects and passes native values (research R12, R17).
//!
//! `Builder` constructs behavior node by node, checking each node with the engine's type
//! checker; `Module` is an admitted module that evaluates, replays, and serializes itself.
//! Values cross the boundary as Python objects; JSON appears only for artifacts (wire files,
//! the canonical serialization, decision records). No behavior logic lives here.

use behavior_core::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_core::semantic::Module;
use behavior_core::wire::{DerivedKind, Loc, Role, WField, WParam, WType};
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
}

/// A decision record: `data` for Python, `json` as the canonical artifact.
#[pyclass(name = "Record", frozen, module = "behavior._engine")]
struct PyRecord {
    data: Value,
    json: String,
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
}

fn record(r: behavior_core::DecisionRecord) -> PyRecord {
    PyRecord {
        json: r.to_json_string(),
        data: r.as_json().clone(),
    }
}

fn admission_dict(py: Python<'_>, r: &behavior_core::AdmissionResult) -> PyResult<Py<PyAny>> {
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

fn governance_err(e: behavior_verify::governance::GovernanceError) -> PyErr {
    EngineError::new_err(("INVALID_GOVERNANCE_INPUT", e.to_string()))
}

/// The content hash of a waiver.
#[pyfunction]
fn waiver_hash(waiver: &Bound<'_, PyAny>) -> PyResult<String> {
    behavior_verify::governance::waiver_hash(&object(waiver)?.to_string()).map_err(governance_err)
}

/// A detached Ed25519 signed attestation over a waiver's hash (seed: 32 bytes as hex).
#[pyfunction]
fn sign_waiver(py: Python<'_>, waiver: &Bound<'_, PyAny>, seed: &str) -> PyResult<Py<PyAny>> {
    let signed = behavior_verify::governance::sign_waiver(seed, &object(waiver)?.to_string())
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
        match behavior_core::admit(wire) {
            Ok(m) => {
                let report = admission_dict(py, &behavior_core::admission_result(&m))?;
                Ok((Some(EngineModule { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }

    #[getter]
    fn behavior_version(&self) -> String {
        self.inner.behavior_version()
    }

    fn admission(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        admission_dict(py, &behavior_core::admission_result(&self.inner))
    }

    /// Canonical wire JSON serialized from the admitted module.
    fn wire_json(&self) -> String {
        behavior_core::serialize::to_wire_json(&self.inner)
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
        let a = behavior_verify::governance::authorize(
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
        let mut profile = behavior_verify::Profile {
            rlimit,
            wall_clock_guard_ms,
            ..Default::default()
        };
        if let Some(names) = checks {
            profile.checks = names
                .iter()
                .map(|n| {
                    behavior_verify::CheckKind::parse(n)
                        .ok_or_else(|| PyValueError::new_err(format!("unknown check `{n}`")))
                })
                .collect::<PyResult<_>>()?;
        }
        let solver = behavior_verify::solver::Z3Process::from_env()
            .map_err(|e| EngineError::new_err(("SOLVER_UNAVAILABLE", e.to_string())))?
            .with_guard(std::time::Duration::from_millis(wall_clock_guard_ms));
        let a = behavior_verify::verify(
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

    #[pyo3(signature = (action, state, input, context, data_version, git_revision=None))]
    fn evaluate(
        &self,
        action: String,
        state: &Bound<'_, PyAny>,
        input: &Bound<'_, PyAny>,
        context: &Bound<'_, PyAny>,
        data_version: String,
        git_revision: Option<String>,
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
        Ok(record(behavior_core::evaluate(
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
        match behavior_core::evaluate_intent(&self.inner, &intent.to_string(), &host.to_string()) {
            Ok(r) => Ok(record(r)),
            Err(rejection) => {
                let errors = serde_json::to_value(&rejection.errors).unwrap_or(Value::Null);
                Err(EngineIntentRejected::new_err((to_py(py, &errors)?,)))
            }
        }
    }

    /// Replays a decision record (its canonical JSON text); returns (matches, diff).
    fn replay(&self, record_json: &str) -> (bool, Option<String>) {
        let r = behavior_core::replay(&self.inner, record_json);
        (r.matches, r.diff)
    }
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

    /// `site` is "derived" (also used for invariants) or "action".
    fn push_scope(
        &mut self,
        site: &str,
        ps: Vec<(String, Option<String>, PyType_)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let site = if site == "action" {
            ScopeSite::Action
        } else {
            ScopeSite::Derived
        };
        self.inner
            .push_scope(site, params(ps)?, loc(file, line))
            .map_err(build_err)
    }

    fn pop_scope(&mut self) {
        self.inner.pop_scope();
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
        postconditions: Vec<(PyNode, String, u64)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let cond = |(n, f, l): (PyNode, String, u64)| (n.inner, loc(f, l));
        self.inner
            .add_action(
                name,
                params(ps)?,
                preconditions.into_iter().map(cond).collect(),
                effects
                    .into_iter()
                    .map(|(p, fld, n, f, l)| (p, fld, n.inner, loc(f, l)))
                    .collect(),
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
                let report = admission_dict(py, &behavior_core::admission_result(&m))?;
                Ok((Some(EngineModule { inner: m }), report))
            }
            Err(r) => Ok((None, admission_dict(py, &r)?)),
        }
    }
}

#[pymodule]
fn _engine(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_class::<PyType_>()?;
    m.add_class::<PyBuilder>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyRecord>()?;
    m.add_class::<EngineModule>()?;
    m.add_class::<PyAttestation>()?;
    m.add_class::<PyAuthorization>()?;
    m.add_function(wrap_pyfunction!(waiver_hash, m)?)?;
    m.add_function(wrap_pyfunction!(sign_waiver, m)?)?;
    m.add("EngineError", m.py().get_type::<EngineError>())?;
    m.add(
        "EngineIntentRejected",
        m.py().get_type::<EngineIntentRejected>(),
    )?;
    Ok(())
}
