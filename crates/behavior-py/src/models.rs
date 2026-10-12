//! Behavior Models for Python (feature 501): glue between the Python authoring layer and the
//! model authority `behavior-models`. Nothing here decides model semantics: Python model input is
//! passed through unchanged, and fragments are traced by Python inside the scopes the model crate
//! opens.
//!
//! The model crate drives the module's one Core builder for the whole build. For each
//! transition it pushes the scope and calls back here; the builder is then moved back into the
//! Python `Builder` object while Python traces the author's code (the DSL calls that object), and
//! moved out again before the model crate continues.

use behavior_engine::builder::{Builder, CommandEmissionSpec};
use behavior_models::{
    BindingError, DisplayMeta, FragmentSource, ModelInput, ModelSpec, SourceLoc,
    TransitionFragments, TransitionInput, TransitionScope,
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

/// A model input dict, exactly as the authoring layer recorded it.
fn model_input(d: &Bound<'_, PyAny>) -> PyResult<ModelInput> {
    let transitions = item(d, "transitions")?
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
        .collect::<PyResult<Vec<_>>>()?;
    Ok(ModelInput {
        model_id: item(d, "model_id")?.extract()?,
        subject_entity: item(d, "subject_entity")?.extract()?,
        state_field: item(d, "state_field")?.extract()?,
        subject_param: opt_string(d, "subject_param")?,
        entry_states: item(d, "entry_states")?.extract()?,
        transitions,
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

/// A Python tracer: `tracer(transition_id, [(name, role), ...]) -> dict` with `guards`,
/// `effects` and `postconditions` built on the session builder.
struct PyFragments<'py> {
    py: Python<'py>,
    builder: Py<PyBuilder>,
    tracer: Py<PyAny>,
}

impl PyFragments<'_> {
    fn swap(&self, b: &mut Builder) {
        let mut pb = self.builder.borrow_mut(self.py);
        std::mem::swap(&mut pb.inner, b);
    }

    fn call(&self, scope: &TransitionScope) -> PyResult<TransitionFragments> {
        let py = self.py;
        let ps: Vec<(String, &'static str)> = scope
            .params
            .iter()
            .map(|p| {
                let role = match p.role {
                    Some(behavior_engine::wire::Role::State) => "state",
                    Some(behavior_engine::wire::Role::Input) => "input",
                    Some(behavior_engine::wire::Role::Context) => "context",
                    None => "read",
                };
                (p.name.clone(), role)
            })
            .collect();
        let out = self
            .tracer
            .bind(py)
            .call1((scope.transition_id.clone(), ps))?;
        let node = |o: &Bound<'_, PyAny>| -> PyResult<behavior_engine::builder::Node> {
            Ok(o.extract::<PyRef<PyNode>>()?.inner.clone())
        };
        let conds = |key: &str| -> PyResult<Vec<_>> {
            item(&out, key)?
                .try_iter()?
                .map(|c| {
                    let (n, f, l): (Bound<'_, PyAny>, String, u64) = c?.extract()?;
                    Ok((node(&n)?, loc(f, l)))
                })
                .collect()
        };
        let guards = conds("guards")?;
        let postconditions = conds("postconditions")?;
        let effects = item(&out, "effects")?
            .try_iter()?
            .map(|e| {
                let (p, fld, n, f, l): (String, String, Bound<'_, PyAny>, String, u64) =
                    e?.extract()?;
                Ok((p, fld, node(&n)?, loc(f, l)))
            })
            .collect::<PyResult<Vec<_>>>()?;
        // `(command, when node or None, {field: node}, file, line)`
        let commands = item(&out, "commands")?
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
        Ok(TransitionFragments {
            guards,
            effects,
            postconditions,
            commands,
        })
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
        result.map_err(|e| {
            let py = self.py;
            let value = e.value(py);
            let code = value
                .getattr("code")
                .and_then(|c| c.extract::<String>())
                .unwrap_or_else(|_| "BINDING_ERROR".to_string());
            BindingError {
                code,
                message: value.to_string(),
            }
        })
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
    let mut specs = Vec::new();
    for m in &models {
        let input = model_input(m.as_any())?;
        let tracer: Py<PyAny> = item(m.as_any(), "tracer")?.unbind();
        specs.push(ModelSpec {
            input,
            fragments: Box::new(PyFragments {
                py,
                builder: builder.clone_ref(py),
                tracer,
            }),
        });
    }
    let mut b = std::mem::take(&mut builder.borrow_mut(py).inner);
    let result = behavior_models::compose(&mut b, specs, root.as_deref());
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
        Err(errors) => {
            let list = PyList::empty(py);
            for e in &errors {
                list.append(to_py(py, &e.to_json())?)?;
            }
            Ok((None, list.into_any().unbind(), empty()?, empty()?))
        }
    }
}
