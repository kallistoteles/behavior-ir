//! Composition: the model set, ownership, lowering order and the final pass over the admitted
//! module (contract `composition-and-ownership.md`).

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, WType};
use serde_json::json;

use crate::BuildOutput;
use crate::error::{ModelError, sorted_unique};
use crate::state_machine::input::ModelSpec;
use crate::state_machine::lower::lower_transition;
use crate::state_machine::resolve::{ResolvedStateMachine, resolve};

/// A model-crate failure reported in the Core admission error shape (`code`, `message`).
pub(crate) fn admission_failed_message(code: &str, message: String) -> ModelError {
    ModelError::CoreAdmissionFailed {
        errors: vec![(code.to_string(), message)],
    }
}

/// Core admission errors as model errors, verbatim.
pub(crate) fn admission_failed(report: behavior_engine::AdmissionResult) -> ModelError {
    ModelError::CoreAdmissionFailed {
        errors: report
            .errors
            .into_iter()
            .map(|e| (e.code, e.message))
            .collect(),
    }
}

/// `(entity, field) → owning model id` (data-model §8).
pub type OwnershipTable = BTreeMap<(String, String), String>;

/// A model of the set after resolution, with the actions it generates.
pub struct Composed {
    pub machine: ResolvedStateMachine,
    /// `action name → transition id` of every generated action.
    pub generated: BTreeMap<String, String>,
}

pub(crate) fn compose(
    builder: &mut Builder,
    models: Vec<ModelSpec<'_>>,
    root: Option<&str>,
) -> Result<BuildOutput, Vec<ModelError>> {
    // Step 1: the ordinary behavior, admitted on its own, provides the declarations.
    let base = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // Steps 1–2: resolve every model and check the set. Models are a set keyed by id.
    let mut errors = Vec::new();
    let mut by_id: BTreeMap<String, Vec<ModelSpec<'_>>> = BTreeMap::new();
    for spec in models {
        by_id
            .entry(spec.input.model_id.clone())
            .or_default()
            .push(spec);
    }
    let mut resolved = Vec::new();
    for (id, specs) in by_id {
        if specs.len() > 1 {
            errors.push(ModelError::DuplicateModelId { model_id: id });
            continue;
        }
        for spec in specs {
            match resolve(&spec.input, &base) {
                Ok(machine) => resolved.push((machine, spec)),
                Err(es) => errors.extend(es),
            }
        }
    }
    let ownership = ownership_table(resolved.iter().map(|(m, _)| m), &mut errors);
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Step 3: lower every transition, in (model_id, transition_id) order.
    let mut composed = Vec::new();
    for (machine, mut spec) in resolved {
        let mut generated = BTreeMap::new();
        for t in machine.transitions.values() {
            match lower_transition(builder, &machine, t, spec.fragments.as_mut()) {
                Ok(()) => {
                    generated.insert(t.action_name.clone(), t.transition_id.to_string());
                }
                Err(es) => errors.extend(es),
            }
        }
        composed.push(Composed { machine, generated });
    }
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Step 4: Core admits the composed module.
    let module = builder
        .finish(root)
        .map_err(|r| vec![admission_failed(r)])?;

    // Step 5: the final pass over every write and creation.
    let errors = final_pass(&module, &composed, &ownership);
    if !errors.is_empty() {
        return Err(sorted_unique(errors));
    }

    // Step 6: identity, receipts, diagnostics.
    let (receipts, diagnostics) = crate::identity::outputs(&module, &composed, root)?;
    Ok(BuildOutput {
        module,
        receipts,
        diagnostics,
    })
}

/// One owner per field (composition contract §2.2). Conflicts list their models sorted.
pub(crate) fn ownership_table<'a>(
    machines: impl Iterator<Item = &'a ResolvedStateMachine>,
    errors: &mut Vec<ModelError>,
) -> OwnershipTable {
    let mut claims: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for m in machines {
        claims
            .entry((m.subject.clone(), m.field.clone()))
            .or_default()
            .push(m.model_id.to_string());
    }
    let mut table = OwnershipTable::new();
    for ((entity, field), mut models) in claims {
        models.sort();
        if models.len() > 1 {
            errors.push(ModelError::ConflictingOwnership {
                entity,
                field,
                models,
            });
        } else if let Some(owner) = models.pop() {
            table.insert((entity, field), owner);
        }
    }
    table
}

/// The final composition pass over the admitted module (composition contract §4–§5).
///
/// In an admitted module, field values change only through action effects and creations, and
/// `actions()` lists every action. So if this pass accepts, every ordinary write of an owned
/// field lies in one of its owner's generated actions, and every new subject entity starts in
/// a literal entry state (spec FR-017, FR-021; law L6). Reads, removals, derived values,
/// invariants and command emissions never write a field.
pub(crate) fn final_pass(
    module: &Module,
    composed: &[Composed],
    ownership: &OwnershipTable,
) -> Vec<ModelError> {
    let mut errors = Vec::new();
    if ownership.is_empty() {
        return errors;
    }
    let generated_by: BTreeMap<&str, &str> = composed
        .iter()
        .flat_map(|c| {
            c.generated
                .keys()
                .map(move |name| (name.as_str(), c.machine.model_id.as_str()))
        })
        .collect();
    let machines: BTreeMap<&str, &ResolvedStateMachine> = composed
        .iter()
        .map(|c| (c.machine.model_id.as_str(), &c.machine))
        .collect();
    let mut literals: BTreeMap<String, Result<BTreeMap<String, [u8; 32]>, ModelError>> =
        BTreeMap::new();

    for (name, action) in module.actions() {
        for effect in action.effects() {
            let Some(param) = action.params().iter().find(|p| p.name() == effect.param()) else {
                continue;
            };
            let WType::Entity(entity) = param.ty().to_wire_type() else {
                continue;
            };
            let key = (entity, effect.field().to_string());
            if let Some(owner) = ownership.get(&key)
                && generated_by.get(name.as_str()) != Some(&owner.as_str())
            {
                errors.push(ModelError::UnauthorizedOwnedFieldWrite {
                    action: name.clone(),
                    entity: key.0.clone(),
                    field: key.1.clone(),
                    owner: owner.clone(),
                });
            }
        }
        for created in action.creates() {
            for ((entity, field), owner) in ownership {
                if created.entity() != entity {
                    continue;
                }
                let Some(machine) = machines.get(owner.as_str()) else {
                    continue;
                };
                let Some((_, initial)) = created.fields().iter().find(|(f, _)| f == field) else {
                    continue;
                };
                let lits = literals
                    .entry(owner.clone())
                    .or_insert_with(|| literal_hashes(module, &machine.enum_type));
                let lits = match lits {
                    Ok(l) => l,
                    Err(e) => {
                        errors.push(e.clone());
                        continue;
                    }
                };
                let case = lits.iter().find(|(_, h)| *h == initial.hash());
                match case {
                    Some((case, _)) if machine.entry_states.contains(case) => {}
                    Some((case, _)) => errors.push(ModelError::EntryStateNotAllowed {
                        action: name.clone(),
                        entity: entity.clone(),
                        field: field.clone(),
                        state: case.clone(),
                    }),
                    None => errors.push(ModelError::EntryStateNotStatic {
                        action: name.clone(),
                        entity: entity.clone(),
                        field: field.clone(),
                    }),
                }
            }
        }
    }
    errors
}

/// Core's semantic hash of every literal `lit(T, case)` (research R8). Core builds them in a
/// throwaway module that re-declares `T` with the module's cases and profile; the module is
/// never emitted. Literals hash through the enum declaration's hash, so the probe checks that its
/// enum is identical to the module's before trusting the literal hashes.
fn literal_hashes(
    module: &Module,
    enum_type: &str,
) -> Result<BTreeMap<String, [u8; 32]>, ModelError> {
    let probe_error = |message: String| ModelError::CoreAdmissionFailed {
        errors: vec![("MODEL_PROBE_FAILED".into(), message)],
    };
    let info = module
        .enums()
        .get(enum_type)
        .ok_or_else(|| probe_error(format!("unknown enum `{enum_type}`")))?;
    let loc = || Loc {
        file: "behavior-models.probe".into(),
        line: 1,
    };
    let core = |e: behavior_engine::builder::BuildError| probe_error(e.to_string());
    let mut b = Builder::with_semantic_profile(module.semantic_profile());
    b.declare_enum(&info.name, info.values.clone(), loc())
        .map_err(core)?;
    b.push_scope(ScopeSite::Closed, vec![], loc())
        .map_err(core)?;
    let ty = WType::Enum(info.name.clone());
    for (i, case) in info.values.iter().enumerate() {
        let a = b.lit(ty.clone(), json!(case), loc()).map_err(core)?;
        let c = b.lit(ty.clone(), json!(case), loc()).map_err(core)?;
        let eq = b.op("eq", vec![a, c], loc()).map_err(core)?;
        b.add_global_invariant(&format!("__probe_{i}"), eq, loc())
            .map_err(core)?;
    }
    b.pop_scope();
    let probe = b.finish(None).map_err(admission_failed)?;
    if probe.enums().get(enum_type).map(|e| e.hash) != Some(info.hash) {
        return Err(probe_error(format!(
            "the probe's `{enum_type}` differs from the module's"
        )));
    }
    let mut out = BTreeMap::new();
    for (i, case) in info.values.iter().enumerate() {
        let inv = probe
            .global_invariants()
            .get(&format!("__probe_{i}"))
            .ok_or_else(|| probe_error(format!("probe `{case}` missing")))?;
        let lit = inv
            .body()
            .children()
            .first()
            .map(|e| *e.hash())
            .ok_or_else(|| probe_error(format!("probe `{case}` has no literal")))?;
        out.insert(case.clone(), lit);
    }
    Ok(out)
}
