//! Composition: the model set, ownership, lowering order and the final pass over the admitted
//! module (contract `composition-and-ownership.md`).

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, WType};
use serde_json::json;

use crate::error::{ModelError, sorted_unique};
use crate::state_machine::resolve::ResolvedStateMachine;

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

/// A state machine of the internal composition set after lowering, with the actions it
/// generates. `supplied` is false for a machine derived from another model (feature 502): such
/// a machine is checked and lowered like any other, but gets no receipt of its own.
pub struct Composed {
    pub machine: ResolvedStateMachine,
    /// `action name → transition id` of every generated action.
    pub generated: BTreeMap<String, String>,
    pub supplied: bool,
}

/// An entity declaration a model contributes to the declaration environment (feature 502,
/// declaration-staging contract §3): its name and fields in canonical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSchema {
    pub model_id: String,
    pub entity: String,
    pub fields: Vec<(String, WType)>,
    pub loc: Loc,
}

/// How the declarations of `module` relate to a contributed schema.
pub(crate) enum Existing {
    /// No item has the schema's name.
    Absent,
    /// An entity with exactly the schema's fields (names, order, types; no references).
    Identical,
    /// Some other item has the name: its kind.
    Other(String),
}

/// Compares a contributed schema with what `module` declares (laws D4, D6).
pub(crate) fn existing(module: &Module, schema: &DeclaredSchema) -> Existing {
    if let Some(entity) = module.entities().get(&schema.entity) {
        // Core gives every entity its own identity field `id: Id(Self)`; it is not declared.
        let own_id = WType::Id(schema.entity.clone());
        let fields: Vec<(String, WType)> = entity
            .fields()
            .iter()
            .map(|(n, t)| (n.clone(), t.to_wire_type()))
            .filter(|(n, t)| !(n == "id" && *t == own_id))
            .collect();
        let references = entity.fields().iter().any(|(n, _)| entity.is_reference(n));
        return if fields == schema.fields && !references {
            Existing::Identical
        } else {
            Existing::Other("an entity with other fields".into())
        };
    }
    match module
        .name_table()
        .keys()
        .find(|(_, name)| name == &schema.entity)
    {
        Some((kind, _)) => Existing::Other(kind.as_str().to_string()),
        None => Existing::Absent,
    }
}

/// The canonical declaration merge (declaration-staging contract, laws D2, D3, D6): schemas in
/// model-id order, each identical-or-error against the declarations already in `builder`.
/// Without contributions the builder is not touched at all.
pub(crate) fn merge_declarations(
    builder: &mut Builder,
    mut schemas: Vec<DeclaredSchema>,
) -> Result<Vec<DeclaredSchema>, Vec<ModelError>> {
    if schemas.is_empty() {
        return Ok(schemas);
    }
    schemas.sort_by(|a, b| a.model_id.cmp(&b.model_id));
    let base = builder
        .finish(None)
        .map_err(|r| vec![admission_failed(r)])?;
    let mut errors = Vec::new();
    let mut declared = std::collections::BTreeSet::new();
    for schema in &schemas {
        if !declared.insert(schema.entity.clone()) {
            errors.push(ModelError::DeclarationCollision {
                name: schema.entity.clone(),
                model_ref: schema.model_id.clone(),
                existing_kind: "another model's declaration".into(),
            });
            continue;
        }
        match existing(&base, schema) {
            Existing::Identical => {}
            Existing::Other(kind) => errors.push(ModelError::DeclarationCollision {
                name: schema.entity.clone(),
                model_ref: schema.model_id.clone(),
                existing_kind: kind,
            }),
            Existing::Absent => {
                let fields = schema
                    .fields
                    .iter()
                    .map(|(name, ty)| behavior_engine::wire::WField {
                        name: name.clone(),
                        ty: ty.clone(),
                        loc: schema.loc.clone(),
                    })
                    .collect();
                if let Err(e) = builder.declare_entity(&schema.entity, fields, schema.loc.clone()) {
                    errors.push(ModelError::DeclarationCollision {
                        name: schema.entity.clone(),
                        model_ref: schema.model_id.clone(),
                        existing_kind: e.message,
                    });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(schemas)
    } else {
        Err(sorted_unique(errors))
    }
}

/// Declares `schemas` (in model-id order) without first admitting the authored declarations,
/// then admits the result. Used when the authored declarations alone do not admit because they
/// name a contributed entity (law D5: Core checks declarations together, so such a forward
/// reference is legal once the contribution is in place). A name already taken is left as it
/// is; the caller compares it with the contribution on the returned module.
pub(crate) fn declare_then_admit(
    builder: &mut Builder,
    mut schemas: Vec<DeclaredSchema>,
) -> Result<(Module, Vec<DeclaredSchema>), Vec<ModelError>> {
    schemas.sort_by(|a, b| a.model_id.cmp(&b.model_id));
    for schema in &schemas {
        let fields = schema
            .fields
            .iter()
            .map(|(name, ty)| behavior_engine::wire::WField {
                name: name.clone(),
                ty: ty.clone(),
                loc: schema.loc.clone(),
            })
            .collect();
        // `DUPLICATE_NAME`: an authored entity holds the name; compared below by the caller.
        let _ = builder.declare_entity(&schema.entity, fields, schema.loc.clone());
    }
    let module = builder
        .finish(None)
        .map_err(|r| vec![admission_failed(r)])?;
    Ok((module, schemas))
}

/// D6 after [`declare_then_admit`]: the declaration under the contributed name must be the
/// contribution itself (or an identical authored one).
pub(crate) fn check_contributed(module: &Module, schema: &DeclaredSchema) -> Option<ModelError> {
    match existing(module, schema) {
        Existing::Identical => None,
        Existing::Other(kind) => Some(ModelError::DeclarationCollision {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
            existing_kind: kind,
        }),
        Existing::Absent => Some(ModelError::DeclarationMismatch {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
        }),
    }
}

/// Law D4: at composition the builder must hold exactly the contributed declaration.
pub(crate) fn check_declared(module: &Module, schema: &DeclaredSchema) -> Option<ModelError> {
    match existing(module, schema) {
        Existing::Identical => None,
        _ => Some(ModelError::DeclarationMismatch {
            name: schema.entity.clone(),
            model_ref: schema.model_id.clone(),
        }),
    }
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
