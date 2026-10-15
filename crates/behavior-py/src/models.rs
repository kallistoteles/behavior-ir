//! Behavior Models for Python (features 501, 502, 503): glue between the Python authoring layer and the
//! model authority `behavior-models`. Nothing here decides model semantics: Python model input is
//! passed through unchanged, and fragments are traced by Python inside the scopes the model crate
//! opens.
//!
//! The model crate drives the module's one Core builder for the whole build. For each
//! transition it pushes the scope and calls back here; the builder is then moved back into the
//! Python `Builder` object while Python traces the author's code (the DSL calls that object), and
//! moved out again before the model crate continues.

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node};
use behavior_engine::wire::{Loc, WParam};
use behavior_models::{
    AnyModel, ApprovalInput, ApprovalSource, ApprovalSpec, BindingError, CapabilityInput,
    CreationFragments, CreationInput, CreationScope, DecisionFragments, DecisionScope, DisplayMeta,
    FragmentSource, ModelInput, ModelSpec, RequestFlowInput, RequestFlowSource, RequestFlowSpec,
    SourceLoc, TransitionFragments, TransitionInput, TransitionScope,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use super::{EngineModule, PyBuilder, PyNode, PyType_, loc, to_py};

fn item<'py>(d: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    d.get_item(key)
}

fn opt_string(d: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    item(d, key)?.extract()
}

fn display(d: &Bound<'_, PyAny>) -> PyResult<DisplayMeta> {
    Ok(DisplayMeta {
        display_name: opt_string(d, "display_name")?,
        description: opt_string(d, "description")?,
    })
}

fn source(d: &Bound<'_, PyAny>) -> PyResult<Option<SourceLoc>> {
    let file: Option<String> = item(d, "file")?.extract()?;
    let line: Option<u64> = item(d, "line")?.extract()?;
    Ok(file.map(|file| SourceLoc {
        file,
        line: line.unwrap_or(1),
    }))
}

fn params(
    d: &Bound<'_, PyAny>,
    key: &str,
) -> PyResult<Vec<(String, behavior_engine::wire::WType)>> {
    let raw: Vec<(String, PyType_)> = item(d, key)?.extract()?;
    Ok(raw.into_iter().map(|(n, t)| (n, t.inner)).collect())
}

fn transitions(d: &Bound<'_, PyAny>) -> PyResult<Vec<TransitionInput>> {
    item(d, "transitions")?
        .try_iter()?
        .map(|t| {
            let t = t?;
            Ok(TransitionInput {
                transition_id: item(&t, "transition_id")?.extract()?,
                from_states: item(&t, "from_states")?.extract()?,
                to_state: item(&t, "to_state")?.extract()?,
                inputs: params(&t, "inputs")?,
                context: params(&t, "context")?,
                display: display(&t)?,
                source: source(&t)?,
            })
        })
        .collect()
}

/// A StateMachine input dict, exactly as the authoring layer recorded it.
fn model_input(d: &Bound<'_, PyAny>) -> PyResult<ModelInput> {
    Ok(ModelInput {
        model_id: item(d, "model_id")?.extract()?,
        subject_entity: item(d, "subject_entity")?.extract()?,
        state_field: item(d, "state_field")?.extract()?,
        subject_param: opt_string(d, "subject_param")?,
        entry_states: item(d, "entry_states")?.extract()?,
        transitions: transitions(d)?,
        display: display(d)?,
        source: source(d)?,
    })
}

/// A RequestFlow input dict (feature 502), exactly as the authoring layer recorded it.
fn request_flow_input(d: &Bound<'_, PyAny>) -> PyResult<RequestFlowInput> {
    let creations = item(d, "creations")?
        .try_iter()?
        .map(|c| {
            let c = c?;
            Ok(CreationInput {
                creation_id: item(&c, "creation_id")?.extract()?,
                to_state: item(&c, "to_state")?.extract()?,
                inputs: params(&c, "inputs")?,
                state: item(&c, "state")?.extract()?,
                context: params(&c, "context")?,
                display: display(&c)?,
                source: source(&c)?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(RequestFlowInput {
        flow_id: item(d, "flow_id")?.extract()?,
        actor_entity: item(d, "actor_entity")?.extract()?,
        status_enum: item(d, "status_enum")?.extract()?,
        assignee: item(d, "assignee")?.extract()?,
        payload: params(d, "payload")?,
        creations,
        transitions: transitions(d)?,
        display: display(d)?,
        source: source(d)?,
    })
}

/// An Approval input dict (feature 503), exactly as the authoring layer recorded it.
fn approval_input(d: &Bound<'_, PyAny>) -> PyResult<ApprovalInput> {
    let capabilities = item(d, "capabilities")?
        .try_iter()?
        .map(|c| {
            let c = c?;
            Ok(CapabilityInput {
                capability_id: item(&c, "capability_id")?.extract()?,
                inputs: params(&c, "inputs")?,
                context: params(&c, "context")?,
                display: display(&c)?,
                source: source(&c)?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(ApprovalInput {
        approval_id: item(d, "approval_id")?.extract()?,
        subject_entity: item(d, "subject_entity")?.extract()?,
        actor_entity: item(d, "actor_entity")?.extract()?,
        approve_threshold: item(d, "approve_threshold")?.extract()?,
        reject_threshold: item(d, "reject_threshold")?.extract()?,
        payload: params(d, "payload")?,
        capabilities,
        display: display(d)?,
        source: source(d)?,
    })
}

/// `(command, when node or None, [(field, node)], file, line)` from the Python tracer.
type PyEmission<'py> = (
    String,
    Option<Bound<'py, PyAny>>,
    Vec<(String, Bound<'py, PyAny>)>,
    String,
    u64,
);

/// Python tracers: `tracer(transition_id, [(name, role), ...]) -> dict` with `guards`,
/// `effects`, `postconditions` and `commands` built on the session builder; for a RequestFlow
/// also `creation_tracer(creation_id, [(name, role), ...])`, whose dict adds `initializer`.
struct PyFragments<'py> {
    py: Python<'py>,
    builder: Py<PyBuilder>,
    tracer: Py<PyAny>,
    creation_tracer: Option<Py<PyAny>>,
    decision_tracer: Option<Py<PyAny>>,
}

fn roles(ps: &[WParam]) -> Vec<(String, &'static str)> {
    ps.iter()
        .map(|p| {
            let role = match p.role {
                Some(behavior_engine::wire::Role::State) => "state",
                Some(behavior_engine::wire::Role::Input) => "input",
                Some(behavior_engine::wire::Role::Context) => "context",
                None => "read",
            };
            (p.name.clone(), role)
        })
        .collect()
}

fn node(o: &Bound<'_, PyAny>) -> PyResult<Node> {
    Ok(o.extract::<PyRef<PyNode>>()?.inner.clone())
}

type Conditions = Vec<(Node, Loc)>;
type Effects = Vec<(String, String, Node, Loc)>;

/// The fragments every tracer returns: `(guards, effects, postconditions, commands)`.
fn fragments(
    out: &Bound<'_, PyAny>,
) -> PyResult<(Conditions, Effects, Conditions, Vec<CommandEmissionSpec>)> {
    let conds = |key: &str| -> PyResult<Vec<_>> {
        item(out, key)?
            .try_iter()?
            .map(|c| {
                let (n, f, l): (Bound<'_, PyAny>, String, u64) = c?.extract()?;
                Ok((node(&n)?, loc(f, l)))
            })
            .collect()
    };
    let guards = conds("guards")?;
    let postconditions = conds("postconditions")?;
    let effects = item(out, "effects")?
        .try_iter()?
        .map(|e| {
            let (p, fld, n, f, l): (String, String, Bound<'_, PyAny>, String, u64) =
                e?.extract()?;
            Ok((p, fld, node(&n)?, loc(f, l)))
        })
        .collect::<PyResult<Vec<_>>>()?;
    // `(command, when node or None, {field: node}, file, line)`
    let commands = item(out, "commands")?
        .try_iter()?
        .map(|c| {
            let (command, when, payload, f, l): PyEmission<'_> = c?.extract()?;
            Ok(CommandEmissionSpec {
                command,
                when: when.as_ref().map(node).transpose()?,
                payload: payload
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), node(v)?)))
                    .collect::<PyResult<_>>()?,
                loc: loc(f, l),
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok((guards, effects, postconditions, commands))
}

impl PyFragments<'_> {
    fn swap(&self, b: &mut Builder) {
        let mut pb = self.builder.borrow_mut(self.py);
        std::mem::swap(&mut pb.inner, b);
    }

    fn call(&self, scope: &TransitionScope) -> PyResult<TransitionFragments> {
        let out = self
            .tracer
            .bind(self.py)
            .call1((scope.transition_id.clone(), roles(&scope.params)))?;
        let (guards, effects, postconditions, commands) = fragments(&out)?;
        Ok(TransitionFragments {
            guards,
            effects,
            postconditions,
            commands,
        })
    }

    fn call_creation(&self, scope: &CreationScope) -> PyResult<CreationFragments> {
        let Some(tracer) = &self.creation_tracer else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "the model has no creation tracer",
            ));
        };
        let out = tracer
            .bind(self.py)
            .call1((scope.creation_id.clone(), roles(&scope.params)))?;
        let initializer = item(&out, "initializer")?
            .try_iter()?
            .map(|v| {
                let (field, n): (String, Bound<'_, PyAny>) = v?.extract()?;
                Ok((field, node(&n)?))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let (guards, effects, postconditions, commands) = fragments(&out)?;
        Ok(CreationFragments {
            initializer,
            guards,
            effects,
            postconditions,
            commands,
        })
    }

    fn call_decision(&self, scope: &DecisionScope) -> PyResult<DecisionFragments> {
        let Some(tracer) = &self.decision_tracer else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "the model has no decision tracer",
            ));
        };
        let out = tracer
            .bind(self.py)
            .call1((scope.capability_id.clone(), roles(&scope.params)))?;
        let cond = |key: &str| -> PyResult<Option<(Node, Loc)>> {
            let v = item(&out, key)?;
            if v.is_none() {
                return Ok(None);
            }
            let (n, f, l): (Bound<'_, PyAny>, String, u64) = v.extract()?;
            Ok(Some((node(&n)?, loc(f, l))))
        };
        let actor = cond("actor")?.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("a decision capability needs an actor")
        })?;
        let payload = item(&out, "payload")?
            .try_iter()?
            .map(|v| {
                let (field, n): (String, Bound<'_, PyAny>) = v?.extract()?;
                Ok((field, node(&n)?))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let (guards, _, _, commands) = fragments(&out)?;
        Ok(DecisionFragments {
            actor,
            open: cond("open")?,
            eligible: cond("eligible")?,
            guards,
            payload,
            commands,
        })
    }

    fn binding_error(&self, e: PyErr) -> BindingError {
        let value = e.value(self.py);
        let code = value
            .getattr("code")
            .and_then(|c| c.extract::<String>())
            .unwrap_or_else(|_| "BINDING_ERROR".to_string());
        BindingError {
            code,
            message: value.to_string(),
        }
    }
}

impl FragmentSource for PyFragments<'_> {
    fn trace(
        &mut self,
        builder: &mut Builder,
        scope: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        // The scope is pushed on `builder`; Python traces on the same builder.
        self.swap(builder);
        let result = self.call(scope);
        self.swap(builder);
        result.map_err(|e| self.binding_error(e))
    }
}

impl RequestFlowSource for PyFragments<'_> {
    fn trace_creation(
        &mut self,
        builder: &mut Builder,
        scope: &CreationScope,
    ) -> Result<CreationFragments, BindingError> {
        self.swap(builder);
        let result = self.call_creation(scope);
        self.swap(builder);
        result.map_err(|e| self.binding_error(e))
    }
}

impl ApprovalSource for PyFragments<'_> {
    fn trace_decision(
        &mut self,
        builder: &mut Builder,
        scope: &DecisionScope,
    ) -> Result<DecisionFragments, BindingError> {
        self.swap(builder);
        let result = self.call_decision(scope);
        self.swap(builder);
        result.map_err(|e| self.binding_error(e))
    }
}

/// A model dict of any kind (`kind` is `"request_flow"` or `"approval"`; StateMachine dicts
/// from feature 501 carry no kind).
fn any_model<'py>(
    py: Python<'py>,
    builder: &Py<PyBuilder>,
    d: &Bound<'py, PyDict>,
) -> PyResult<AnyModel<'py>> {
    let d = d.as_any();
    let kind: Option<String> = d.get_item("kind").ok().map(|k| k.extract()).transpose()?;
    let tracers = |creation: bool| -> PyResult<PyFragments<'py>> {
        Ok(PyFragments {
            py,
            builder: builder.clone_ref(py),
            tracer: item(d, "tracer")?.unbind(),
            creation_tracer: if creation {
                Some(item(d, "creation_tracer")?.unbind())
            } else {
                None
            },
            decision_tracer: None,
        })
    };
    Ok(match kind.as_deref() {
        Some("request_flow") => AnyModel::RequestFlow(RequestFlowSpec {
            input: request_flow_input(d)?,
            fragments: Box::new(tracers(true)?),
        }),
        Some("approval") => AnyModel::Approval(ApprovalSpec {
            input: approval_input(d)?,
            fragments: Box::new(PyFragments {
                py,
                builder: builder.clone_ref(py),
                tracer: item(d, "decision_tracer")?.unbind(),
                creation_tracer: None,
                decision_tracer: Some(item(d, "decision_tracer")?.unbind()),
            }),
        }),
        None | Some("state_machine") => AnyModel::StateMachine(ModelSpec {
            input: model_input(d)?,
            fragments: Box::new(tracers(false)?),
        }),
        Some(other) => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown model kind `{other}`"
            )));
        }
    })
}

fn error_list(py: Python<'_>, errors: &[behavior_models::ModelError]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for e in errors {
        list.append(to_py(py, &e.to_json())?)?;
    }
    Ok(list.into_any().unbind())
}

/// `EncodeEntityName(flow_id)` (feature 502, research R3): the Core name of a flow's request
/// entity. Raises `ValueError` for an invalid semantic id.
#[pyfunction]
pub(crate) fn encode_entity_name(flow_id: &str) -> PyResult<String> {
    behavior_models::ids::SemanticId::parse(flow_id)
        .map(|id| behavior_models::ids::encode_entity_name(&id))
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("`{flow_id}` is not a semantic id"))
        })
}

/// `DecisionNames(approval_id)` (feature 503, data model §3): the Core names of an approval's
/// choice enum and decision entity. Raises `ValueError` for an invalid semantic id.
#[pyfunction]
pub(crate) fn decision_names(approval_id: &str) -> PyResult<(String, String)> {
    behavior_models::ids::SemanticId::parse(approval_id)
        .map(|id| behavior_models::approval::resolve::decision_names(&id))
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("`{approval_id}` is not a semantic id"))
        })
}

/// `(schemas, errors, definitions)` of P2.
type Declared = (Py<PyAny>, Py<PyAny>, Py<PyAny>);

/// P2 of the build protocol (declaration-staging contract, feature 503 `declaration-dag.md`):
/// declares every model's contributed declarations and definitions in the session builder.
/// Returns `(schemas, errors, definitions)`. Each schema is `{kind: "entity", model_id, entity,
/// fields: [(name, Type)]}` (canonical field order) or `{kind: "enum", model_id, name, cases}`;
/// each definition is `{model_id, local, name, kind, params: [(name, Type)], result: Type}`.
#[pyfunction]
pub(crate) fn declare_models(
    py: Python<'_>,
    builder: Py<PyBuilder>,
    models: Vec<Bound<'_, PyDict>>,
) -> PyResult<Declared> {
    let specs = models
        .iter()
        .map(|m| any_model(py, &builder, m))
        .collect::<PyResult<Vec<_>>>()?;
    let mut b = std::mem::take(&mut builder.borrow_mut(py).inner);
    let result = behavior_models::declare_models(&mut b, &specs);
    builder.borrow_mut(py).inner = b;
    let schemas = PyList::empty(py);
    let definitions = PyList::empty(py);
    match result {
        Ok(declared) => {
            for s in declared.schemas {
                let d = PyDict::new(py);
                d.set_item("kind", "entity")?;
                d.set_item("model_id", s.model_id)?;
                d.set_item("entity", s.entity)?;
                let fields = PyList::empty(py);
                for (n, t) in s.fields {
                    fields.append((n, PyType_ { inner: t }))?;
                }
                d.set_item("fields", fields)?;
                schemas.append(d)?;
            }
            for e in declared.enums {
                let d = PyDict::new(py);
                d.set_item("kind", "enum")?;
                d.set_item("model_id", e.model_id)?;
                d.set_item("name", e.name)?;
                d.set_item("cases", e.cases)?;
                schemas.append(d)?;
            }
            for s in declared.definitions {
                let d = PyDict::new(py);
                d.set_item("model_id", s.model_id)?;
                d.set_item("local", s.local)?;
                d.set_item("name", s.name)?;
                d.set_item(
                    "kind",
                    match s.kind {
                        behavior_engine::wire::DerivedKind::Rule => "rule",
                        behavior_engine::wire::DerivedKind::Derived => "derived",
                    },
                )?;
                let ps = PyList::empty(py);
                for p in s.params {
                    ps.append((p.name, PyType_ { inner: p.ty }))?;
                }
                d.set_item("params", ps)?;
                d.set_item("result", PyType_ { inner: s.result })?;
                definitions.append(d)?;
            }
            Ok((
                schemas.into_any().unbind(),
                PyList::empty(py).into_any().unbind(),
                definitions.into_any().unbind(),
            ))
        }
        Err(errors) => Ok((
            schemas.into_any().unbind(),
            error_list(py, &errors)?,
            definitions.into_any().unbind(),
        )),
    }
}

/// `(module or None, errors, receipts, diagnostics)`.
type Composed = (Option<EngineModule>, Py<PyAny>, Py<PyAny>, Py<PyAny>);

/// Composes `models` (input dicts, each with a `tracer`) into the session builder and admits
/// the module. Returns `(module or None, errors, receipts, diagnostics)`; errors are the model
/// crate's typed errors as dicts, unchanged.
#[pyfunction]
#[pyo3(signature = (builder, models, root=None))]
pub(crate) fn compose_models(
    py: Python<'_>,
    builder: Py<PyBuilder>,
    models: Vec<Bound<'_, PyDict>>,
    root: Option<String>,
) -> PyResult<Composed> {
    let specs = models
        .iter()
        .map(|m| any_model(py, &builder, m))
        .collect::<PyResult<Vec<_>>>()?;
    let mut b = std::mem::take(&mut builder.borrow_mut(py).inner);
    let result = behavior_models::compose_models(&mut b, specs, root.as_deref());
    builder.borrow_mut(py).inner = b;
    let empty = || -> PyResult<Py<PyAny>> { Ok(PyList::empty(py).into_any().unbind()) };
    match result {
        Ok(out) => {
            let receipts = PyList::empty(py);
            for r in &out.receipts {
                receipts.append(to_py(py, &r.0)?)?;
            }
            let diagnostics = PyList::empty(py);
            for d in &out.diagnostics {
                let j = serde_json::json!({
                    "code": d.code, "severity": "warning",
                    "model_ref": d.model_ref, "message": d.message
                });
                diagnostics.append(to_py(py, &j)?)?;
            }
            Ok((
                Some(EngineModule { inner: out.module }),
                empty()?,
                receipts.into_any().unbind(),
                diagnostics.into_any().unbind(),
            ))
        }
        Err(errors) => Ok((None, error_list(py, &errors)?, empty()?, empty()?)),
    }
}
