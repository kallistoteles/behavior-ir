//! PyO3 bindings: Python holds engine objects (research R12, R17).
//!
//! `Builder` constructs behavior node by node, checking each node with the engine's type
//! checker; `Module` is an admitted module that evaluates, replays, and serializes itself.
//! No behavior logic lives here: this file only converts between Python and engine types.

use behavior_core::builder::{BuildError, Builder, Node, ScopeSite};
use behavior_core::semantic::Module;
use behavior_core::wire::{DerivedKind, Loc, WField, WParam, WType, decode_param, decode_type};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::{Value, json};

fn loc(file: String, line: u64) -> Loc {
    Loc {
        file,
        line: line.max(1),
    }
}

fn build_err(e: BuildError) -> PyErr {
    PyValueError::new_err(json!({"code": e.code, "message": e.message}).to_string())
}

fn bad_input(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(json!({"code": "DECODE_ERROR", "message": message.into()}).to_string())
}

fn parse(text: &str) -> PyResult<Value> {
    serde_json::from_str(text).map_err(|e| bad_input(format!("invalid JSON: {e}")))
}

fn wtype(text: &str) -> PyResult<WType> {
    decode_type(&parse(text)?, "$").map_err(|e| bad_input(format!("{e:?}")))
}

fn wparams(text: &str) -> PyResult<Vec<WParam>> {
    match parse(text)? {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| {
                decode_param(v, &format!("$[{i}]")).map_err(|e| bad_input(format!("{e:?}")))
            })
            .collect(),
        _ => Err(bad_input("parameters must be a list")),
    }
}

/// `[{"name", "type", "file", "line"}]`
fn wfields(text: &str) -> PyResult<Vec<WField>> {
    let Value::Array(items) = parse(text)? else {
        return Err(bad_input("fields must be a list"));
    };
    items
        .iter()
        .map(|f| {
            let name = f["name"]
                .as_str()
                .ok_or_else(|| bad_input("field name"))?
                .to_string();
            let ty = decode_type(&f["type"], "$.type").map_err(|e| bad_input(format!("{e:?}")))?;
            let file = f["file"].as_str().unwrap_or("<unknown>").to_string();
            let line = f["line"].as_u64().unwrap_or(1);
            Ok(WField {
                name,
                ty,
                loc: loc(file, line),
            })
        })
        .collect()
}

/// A typed expression node owned by the engine.
#[pyclass(name = "Node", frozen, module = "behavior._engine")]
#[derive(Clone)]
struct PyNode {
    inner: Node,
}

#[pymethods]
impl PyNode {
    /// The node's type as wire JSON, or None below an unresolved (cyclic) derived reference.
    #[getter]
    fn type_json(&self) -> Option<String> {
        self.inner.type_wire_json().map(|v| v.to_string())
    }

    /// The parameter role of field/param nodes: "state", "input", "context", or "read".
    #[getter]
    fn role(&self) -> Option<&'static str> {
        self.inner.role()
    }
}

/// An admitted behavior module.
#[pyclass(name = "Module", frozen, module = "behavior._engine")]
struct EngineModule {
    inner: Module,
}

#[pymethods]
impl EngineModule {
    /// Admits wire JSON; returns (module or None, AdmissionResult JSON).
    #[staticmethod]
    fn from_wire(wire: &str) -> (Option<EngineModule>, String) {
        match behavior_core::admit(wire) {
            Ok(m) => {
                let report = behavior_core::admission_result(&m).to_json_string();
                (Some(EngineModule { inner: m }), report)
            }
            Err(r) => (None, r.to_json_string()),
        }
    }

    #[getter]
    fn behavior_version(&self) -> String {
        self.inner.behavior_version()
    }

    fn admission_json(&self) -> String {
        behavior_core::admission_result(&self.inner).to_json_string()
    }

    /// Canonical wire JSON serialized from the admitted module.
    fn wire_json(&self) -> String {
        behavior_core::serialize::to_wire_json(&self.inner)
    }

    fn evaluate(&self, request: &str) -> String {
        behavior_core::evaluate(&self.inner, request).to_json_string()
    }

    /// DecisionRecord JSON, or IntentRejection JSON.
    fn evaluate_intent(&self, intent: &str, host: &str) -> String {
        match behavior_core::evaluate_intent(&self.inner, intent, host) {
            Ok(record) => record.to_json_string(),
            Err(rejection) => rejection.to_json_string(),
        }
    }

    fn replay(&self, record: &str) -> String {
        behavior_core::replay(&self.inner, record).to_json_string()
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

    fn declare_nominal(
        &mut self,
        name: &str,
        underlying_json: &str,
        ops: Vec<String>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let underlying = wtype(underlying_json)?;
        self.inner
            .declare_nominal(name, underlying, ops, loc(file, line))
            .map_err(build_err)
    }

    fn declare_entity(
        &mut self,
        name: &str,
        fields_json: &str,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let fields = wfields(fields_json)?;
        self.inner
            .declare_entity(name, fields, loc(file, line))
            .map_err(build_err)
    }

    /// `site` is "derived" (also used for invariants) or "action".
    fn push_scope(
        &mut self,
        site: &str,
        params_json: &str,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let site = match site {
            "action" => ScopeSite::Action,
            _ => ScopeSite::Derived,
        };
        let ps = wparams(params_json)?;
        self.inner
            .push_scope(site, ps, loc(file, line))
            .map_err(build_err)
    }

    fn pop_scope(&mut self) {
        self.inner.pop_scope();
    }

    fn lit(
        &mut self,
        type_json: &str,
        value_json: &str,
        file: String,
        line: u64,
    ) -> PyResult<PyNode> {
        let ty = wtype(type_json)?;
        let value = parse(value_json)?;
        let inner = self
            .inner
            .lit(ty, value, loc(file, line))
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

    fn in_(&mut self, arg: PyNode, values_json: &str, file: String, line: u64) -> PyResult<PyNode> {
        let Value::Array(values) = parse(values_json)? else {
            return Err(bad_input("values must be a list"));
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

    fn add_derived(
        &mut self,
        name: &str,
        kind: &str,
        params_json: &str,
        body: PyNode,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let kind = if kind == "rule" {
            DerivedKind::Rule
        } else {
            DerivedKind::Derived
        };
        let ps = wparams(params_json)?;
        self.inner
            .add_derived(name, kind, ps, body.inner, loc(file, line))
            .map_err(build_err)
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

    #[allow(clippy::too_many_arguments)]
    fn add_action(
        &mut self,
        name: &str,
        params_json: &str,
        preconditions: Vec<(PyNode, String, u64)>,
        effects: Vec<(String, String, PyNode, String, u64)>,
        postconditions: Vec<(PyNode, String, u64)>,
        file: String,
        line: u64,
    ) -> PyResult<()> {
        let ps = wparams(params_json)?;
        let cond = |(n, f, l): (PyNode, String, u64)| (n.inner, loc(f, l));
        self.inner
            .add_action(
                name,
                ps,
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

    /// Admits the built module: returns (module or None, AdmissionResult JSON).
    fn finish(&self, root: Option<String>) -> (Option<EngineModule>, String) {
        match self.inner.finish(root.as_deref()) {
            Ok(m) => {
                let report = behavior_core::admission_result(&m).to_json_string();
                (Some(EngineModule { inner: m }), report)
            }
            Err(r) => (None, r.to_json_string()),
        }
    }
}

#[pymodule]
fn _engine(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_class::<PyBuilder>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<EngineModule>()?;
    Ok(())
}
