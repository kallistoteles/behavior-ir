//! The model-kind registry (feature 502): the one place that knows every model kind and
//! orchestrates them over the kind-neutral building blocks of `composition`, `identity` and
//! `source_map`.
//!
//! ```text
//! P2  declare_models   Decls(m) for every model, merged identical-or-error into the builder
//! P4  compose_models   resolve → set checks → lower (creations; transitions through the
//!                      StateMachine lowerer) → Core admission → final pass → identity
//! ```
//!
//! The supplied model set (receipts, the id rule) and the internal composition set (every state
//! machine, including the one each flow derives) are kept apart: a derived machine is checked and
//! lowered like any other, but is not a member of the supplied set.

use std::collections::BTreeMap;

use behavior_engine::builder::Builder;

use crate::BuildOutput;
use crate::composition::{
    Composed, DeclaredSchema, admission_failed, check_contributed, check_declared,
    declare_then_admit, final_pass, merge_declarations, ownership_table,
};
use crate::error::{ModelError, sorted_unique};
use crate::request_flow::create::lower_creation;
use crate::request_flow::identity::receipt_rf;
use crate::request_flow::input::RequestFlowSpec;
use crate::request_flow::resolve::{ResolvedRequestFlow, resolve as resolve_flow};
use crate::state_machine::input::{FragmentSource, ModelSpec};
use crate::state_machine::lower::lower_transition;
use crate::state_machine::resolve::resolve as resolve_machine;

/// A supplied model of any kind.
pub enum AnyModel<'a> {
    StateMachine(ModelSpec<'a>),
    RequestFlow(RequestFlowSpec<'a>),
}

impl<'a> From<ModelSpec<'a>> for AnyModel<'a> {
    fn from(m: ModelSpec<'a>) -> Self {
        AnyModel::StateMachine(m)
    }
}

impl<'a> From<RequestFlowSpec<'a>> for AnyModel<'a> {
    fn from(m: RequestFlowSpec<'a>) -> Self {
        AnyModel::RequestFlow(m)
    }
}

impl AnyModel<'_> {
    pub fn model_id(&self) -> &str {
        match self {
            AnyModel::StateMachine(m) => &m.input.model_id,
            AnyModel::RequestFlow(f) => &f.input.flow_id,
        }
    }
}

/// P2: declares every model's contributed declarations (declaration-staging contract). For a set
/// without contributions (every StateMachine-only set) the builder is not touched (law D2).
pub fn declare_models(
    builder: &mut Builder,
    models: &[AnyModel<'_>],
) -> Result<Vec<DeclaredSchema>, Vec<ModelError>> {
    let contributing: Vec<&RequestFlowSpec<'_>> = models
        .iter()
        .filter_map(|m| match m {
            AnyModel::RequestFlow(f) => Some(f),
            AnyModel::StateMachine(_) => None,
        })
        .collect();
    if contributing.is_empty() {
        return Ok(Vec::new());
    }
    // One id namespace for every kind: a duplicate id is reported as such, not as the
    // declaration clash it causes.
    let mut ids = std::collections::BTreeMap::<&str, usize>::new();
    for m in models {
        *ids.entry(m.model_id()).or_default() += 1;
    }
    let duplicates: Vec<ModelError> = ids
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(id, _)| ModelError::DuplicateModelId {
            model_id: id.to_string(),
        })
        .collect();
    if !duplicates.is_empty() {
        return Err(duplicates);
    }
    let mut schemas = Vec::new();
    let mut errors = Vec::new();
    let Ok(authored) = builder.finish(None) else {
        // The authored declarations name a contributed entity (law D5): derive the schemas
        // from the inputs, declare them, and check everything on the completed declarations.
        for f in &contributing {
            match crate::request_flow::resolve::input_schema(&f.input) {
                Ok(s) => schemas.push(s),
                Err(es) => errors.extend(es),
            }
        }
        if !errors.is_empty() {
            return Err(sorted_unique(errors));
        }
        let (declared, schemas) = declare_then_admit(builder, schemas)?;
        for f in &contributing {
            if let Err(es) = crate::request_flow::decls::decls(&f.input, &declared) {
                errors.extend(es);
            }
        }
        errors.extend(
            schemas
                .iter()
                .filter_map(|s| check_contributed(&declared, s)),
        );
        return if errors.is_empty() {
            Ok(schemas)
        } else {
            Err(sorted_unique(errors))
        };
    };
    for f in contributing {
        match crate::request_flow::decls::decls(&f.input, &authored) {
            Ok(s) => schemas.push(s),
            Err(es) => errors.extend(es),
        }
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }
    merge_declarations(builder, schemas)
}

enum Resolved<'a> {
    Machine(
        Box<crate::state_machine::resolve::ResolvedStateMachine>,
        ModelSpec<'a>,
    ),
    Flow(Box<ResolvedRequestFlow>, RequestFlowSpec<'a>),
}

/// P4: lowers and composes every supplied model (lowering contract §4, extended by feature 502).
pub fn compose_models(
    builder: &mut Builder,
    models: Vec<AnyModel<'_>>,
    root: Option<&str>,
) -> Result<BuildOutput, Vec<ModelError>> {
    let base = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // Resolution over the supplied set, keyed by id (one id namespace for every kind).
    let mut errors = Vec::new();
    let mut by_id: BTreeMap<String, Vec<AnyModel<'_>>> = BTreeMap::new();
    for m in models {
        by_id.entry(m.model_id().to_string()).or_default().push(m);
    }
    let mut resolved = Vec::new();
    for (id, ms) in by_id {
        if ms.len() > 1 {
            errors.push(ModelError::DuplicateModelId { model_id: id });
            continue;
        }
        for m in ms {
            match m {
                AnyModel::StateMachine(spec) => match resolve_machine(&spec.input, &base) {
                    Ok(machine) => resolved.push(Resolved::Machine(Box::new(machine), spec)),
                    Err(es) => errors.extend(es),
                },
                AnyModel::RequestFlow(spec) => {
                    // Law D4: the builder holds exactly the declaration the flow contributes.
                    match crate::request_flow::decls::decls(&spec.input, &base) {
                        Ok(schema) => {
                            if let Some(e) = check_declared(&base, &schema) {
                                errors.push(e);
                                continue;
                            }
                        }
                        Err(es) => {
                            errors.extend(es);
                            continue;
                        }
                    }
                    match resolve_flow(&spec.input, &base) {
                        Ok(flow) => resolved.push(Resolved::Flow(Box::new(flow), spec)),
                        Err(es) => errors.extend(es),
                    }
                }
            }
        }
    }
    // Ownership over the internal composition set: supplied machines and derived ones.
    let ownership = ownership_table(
        resolved.iter().map(|r| match r {
            Resolved::Machine(m, _) => m,
            Resolved::Flow(f, _) => &f.machine,
        }),
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Lowering, in canonical model-id order (`resolved` is in id order).
    let mut composed = Vec::new();
    let mut flows = Vec::new();
    for r in resolved {
        match r {
            Resolved::Machine(machine, mut spec) => {
                let generated =
                    lower_machine(builder, &machine, spec.fragments.as_mut(), &mut errors);
                composed.push(Composed {
                    machine: *machine,
                    generated,
                    supplied: true,
                });
            }
            Resolved::Flow(flow, mut spec) => {
                for c in flow.creations.values() {
                    if let Err(es) = lower_creation(builder, &flow, c, spec.fragments.as_mut()) {
                        errors.extend(es);
                        // Core's builder cannot take an action back: once an ill-typed creation
                        // is in it, every later admission repeats that one failure.
                        if builder.finish(None).is_err() {
                            return Err(sorted_unique(errors));
                        }
                    }
                }
                let source: &mut dyn FragmentSource = spec.fragments.as_mut();
                let generated = lower_machine(builder, &flow.machine, source, &mut errors);
                composed.push(Composed {
                    machine: flow.machine.clone(),
                    generated,
                    supplied: false,
                });
                flows.push(*flow);
            }
        }
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    let module = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // The final pass: StateMachine rules over every machine, then each kind's own rules.
    let mut errors = final_pass(&module, &composed, &ownership);
    errors.extend(crate::request_flow::ownership::check(&module, &flows));
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    let (mut receipts, diagnostics) = crate::identity::outputs(&module, &composed, root)?;
    for f in &flows {
        receipts.push(receipt_rf(f, &module, root).map_err(|e| vec![e])?);
    }
    receipts.sort_by_key(|r| receipt_id(&r.0));
    Ok(BuildOutput {
        module,
        receipts,
        diagnostics,
    })
}

fn receipt_id(r: &serde_json::Value) -> String {
    let m = &r["model"];
    m["model_id"]
        .as_str()
        .or_else(|| m["flow_id"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// Lowers every transition of `machine` through the StateMachine lowerer (unchanged).
fn lower_machine(
    builder: &mut Builder,
    machine: &crate::state_machine::resolve::ResolvedStateMachine,
    source: &mut dyn FragmentSource,
    errors: &mut Vec<ModelError>,
) -> BTreeMap<String, String> {
    let mut generated = BTreeMap::new();
    for t in machine.transitions.values() {
        match lower_transition(builder, machine, t, source) {
            Ok(()) => {
                generated.insert(t.action_name.clone(), t.transition_id.to_string());
            }
            Err(es) => errors.extend(es),
        }
    }
    generated
}
