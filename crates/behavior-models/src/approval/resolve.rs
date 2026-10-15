//! Resolution: `ApprovalInput` → `ResolvedApproval` (data model §2–§4).
//!
//! The generated names, the decision schema and the capability parameters are computed here and
//! nowhere else. The declarations follow from the input alone (law D1); resolution then checks
//! the subject and actor against the declaration environment and the capabilities.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, Role, WParam, WType};

use crate::approval::input::ApprovalInput;
use crate::composition::{Contribution, DeclaredEnum, DeclaredSchema};
use crate::error::ModelError;
use crate::ids::{SemanticId, encode, encode_entity_name};
use crate::state_machine::input::{DisplayMeta, SourceLoc};

/// The decision's subject identity field, and the fixed name of the subject parameter.
pub const SUBJECT: &str = "subject";
/// The decision's actor identity field.
pub const ACTOR: &str = "actor";
/// The decision's choice field.
pub const CHOICE: &str = "choice";
/// The reserved name of a capability's decision-identity input (data model §4).
pub const DECISION_ID: &str = "decision_id";
/// Core's implicit identity field of every entity.
pub const ID: &str = "id";
/// The two capabilities, and the choice each records.
pub const CAPABILITIES: [(&str, &str); 2] = [("approve", "APPROVE"), ("reject", "REJECT")];
/// The choice enum's cases, in declaration order (they fix its Core hash).
pub const CHOICES: [&str; 2] = ["APPROVE", "REJECT"];

/// The generated names of approval `p` (data model §3): `(C, X)` with
/// `C = EncodeEntityName(p) ++ "DecisionChoice"` and `X = EncodeEntityName(p) ++ "Decision"`.
///
/// Both maps are injective, because `EncodeEntityName` is and a fixed suffix is appended. No
/// `C(p)` equals any `X(q)`: the last letters differ (`e`, `n`). So the names of all approvals
/// are pairwise distinct. Across model kinds a name may still collide; that is a
/// `DECLARATION_COLLISION` of the common merge, never a rename.
pub fn decision_names(id: &SemanticId) -> (String, String) {
    let e = encode_entity_name(id);
    (format!("{e}DecisionChoice"), format!("{e}Decision"))
}

/// A resolved, validated, canonical approval.
#[derive(Debug, Clone)]
pub struct ResolvedApproval {
    pub approval_id: SemanticId,
    /// `C`
    pub enum_name: String,
    /// `X`
    pub entity: String,
    /// `S`
    pub subject: String,
    /// `A`
    pub actor: String,
    /// `k_A ≥ 1`
    pub approve_threshold: u32,
    /// `k_R ≥ 1`, or `None` for `⊥`
    pub reject_threshold: Option<u32>,
    /// `Π`
    pub payload: BTreeMap<String, WType>,
    /// The decision entity's fields, sorted by name.
    pub schema: Vec<(String, WType)>,
    /// `approve` and `reject`, by id.
    pub capabilities: BTreeMap<String, ResolvedCapability>,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCapability {
    /// `approve` or `reject`
    pub capability_id: SemanticId,
    /// `APPROVE` or `REJECT`: fixed by the capability, never caller input.
    pub choice: String,
    pub inputs: BTreeMap<String, WType>,
    pub context: BTreeMap<String, WType>,
    /// `encode(p, capability_id)`
    pub action_name: String,
    pub display: DisplayMeta,
    pub source: Option<SourceLoc>,
}

impl ResolvedApproval {
    pub fn capability_ref(&self, c: &SemanticId) -> String {
        format!("{}/capability/{c}", self.approval_id)
    }

    /// The name of derived definition `local` (`approve_count`, …): `encode(p, local)`.
    pub fn definition_name(&self, local: &str) -> String {
        definition_name(&self.approval_id, local)
    }

    /// A decision action's parameters, in canonical order (data model §4).
    pub fn capability_params(&self, c: &ResolvedCapability) -> Vec<WParam> {
        let mut ps = vec![WParam {
            name: DECISION_ID.into(),
            role: Some(Role::Input),
            ty: WType::Id(self.entity.clone()),
        }];
        ps.extend(c.inputs.iter().map(|(n, t)| WParam {
            name: n.clone(),
            role: Some(Role::Input),
            ty: t.clone(),
        }));
        ps.push(WParam {
            name: SUBJECT.into(),
            role: Some(Role::State),
            ty: WType::Entity(self.subject.clone()),
        });
        ps.extend(c.context.iter().map(|(n, t)| WParam {
            name: n.clone(),
            role: Some(Role::Context),
            ty: t.clone(),
        }));
        ps
    }

    /// The payload fields an initializer must set.
    pub fn initialized_fields(&self) -> BTreeSet<String> {
        self.payload.keys().cloned().collect()
    }
}

/// `encode(p, local)` for a fixed definition id.
pub fn definition_name(p: &SemanticId, local: &str) -> String {
    // The local ids are fixed semantic ids, so this never falls back.
    match SemanticId::parse(local) {
        Some(l) => encode(p, &l),
        None => format!("{p}__{local}"),
    }
}

pub(crate) fn is_core_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn location(source: &Option<SourceLoc>, id: &str) -> Loc {
    match source {
        Some(s) => Loc {
            file: s.file.clone(),
            line: s.line,
        },
        None => Loc {
            file: format!("{id}.model"),
            line: 1,
        },
    }
}

fn threshold(
    model_ref: &str,
    which: &str,
    value: i64,
    errors: &mut Vec<ModelError>,
) -> Option<u32> {
    match u32::try_from(value) {
        Ok(v) if v >= 1 => Some(v),
        _ => {
            errors.push(ModelError::InvalidThreshold {
                model_ref: model_ref.to_string(),
                which: which.to_string(),
                value,
            });
            None
        }
    }
}

/// What the input alone determines (law D1): the id, the thresholds and the two contributions
/// `[Enum C, Entity X]`. No lookup of the subject or the actor.
pub fn input_contributions(input: &ApprovalInput) -> Result<Vec<Contribution>, Vec<ModelError>> {
    let model_ref = input.approval_id.clone();
    let Some(id) = SemanticId::parse(&input.approval_id) else {
        return Err(vec![ModelError::InvalidSemanticId {
            model_ref: model_ref.clone(),
            id: input.approval_id.clone(),
        }]);
    };
    let mut errors = Vec::new();
    threshold(&model_ref, "approve", input.approve_threshold, &mut errors);
    if let Some(k) = input.reject_threshold {
        threshold(&model_ref, "reject", k, &mut errors);
    }
    let (enum_name, entity) = decision_names(&id);
    let mut fields: BTreeMap<String, WType> = BTreeMap::new();
    fields.insert(ACTOR.into(), WType::Id(input.actor_entity.clone()));
    fields.insert(CHOICE.into(), WType::Enum(enum_name.clone()));
    fields.insert(SUBJECT.into(), WType::Id(input.subject_entity.clone()));
    let mut seen = BTreeSet::new();
    for (name, ty) in &input.payload {
        if [SUBJECT, ACTOR, CHOICE, ID].contains(&name.as_str()) {
            errors.push(ModelError::ReservedField {
                model_ref: model_ref.clone(),
                name: name.clone(),
            });
        } else if !is_core_identifier(name) || !seen.insert(name.clone()) {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: name.clone(),
            });
        } else {
            fields.insert(name.clone(), ty.clone());
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let loc = location(&input.source, &input.approval_id);
    Ok(vec![
        Contribution::Enum(DeclaredEnum {
            model_id: model_ref.clone(),
            name: enum_name,
            cases: CHOICES.iter().map(|c| c.to_string()).collect(),
            loc: loc.clone(),
        }),
        Contribution::Entity(DeclaredSchema {
            model_id: model_ref,
            entity,
            // Sorted by field name: payload order never reaches the entity hash.
            fields: fields.into_iter().collect(),
            loc,
        }),
    ])
}

/// The contributions, checked against the declarations in `module`: the subject and the actor
/// must be entities there (authored, or contributed earlier in the topological order).
pub fn decls(input: &ApprovalInput, module: &Module) -> Result<Vec<Contribution>, Vec<ModelError>> {
    let mut errors = Vec::new();
    let contributions = input_contributions(input);
    if SemanticId::parse(&input.approval_id).is_some() {
        for entity in [&input.subject_entity, &input.actor_entity] {
            if !module.entities().contains_key(entity) {
                errors.push(ModelError::UnknownEntity {
                    model_ref: input.approval_id.clone(),
                    entity: entity.clone(),
                });
            }
        }
    }
    match contributions {
        Ok(c) if errors.is_empty() => Ok(c),
        Ok(_) => Err(errors),
        Err(es) => {
            errors.extend(es);
            Err(errors)
        }
    }
}

/// Resolves an approval against `module`, which already holds its declarations (P2).
pub fn resolve(
    input: &ApprovalInput,
    module: &Module,
) -> Result<ResolvedApproval, Vec<ModelError>> {
    let contributions = decls(input, module)?;
    let model_ref = input.approval_id.clone();
    let approval_id = SemanticId::parse(&input.approval_id).ok_or_else(Vec::new)?;
    let mut errors = Vec::new();
    let schema = contributions
        .iter()
        .find_map(|c| match c {
            Contribution::Entity(s) => Some(s.clone()),
            Contribution::Enum(_) => None,
        })
        .ok_or_else(Vec::new)?;
    let (enum_name, entity) = decision_names(&approval_id);

    let mut capabilities = BTreeMap::new();
    let mut supplied = BTreeSet::new();
    for c in &input.capabilities {
        let choice = CAPABILITIES
            .iter()
            .find(|(id, _)| *id == c.capability_id)
            .map(|(_, choice)| choice.to_string());
        let (Some(choice), Some(capability_id)) = (choice, SemanticId::parse(&c.capability_id))
        else {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: c.capability_id.clone(),
            });
            continue;
        };
        if !supplied.insert(c.capability_id.clone()) {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: c.capability_id.clone(),
            });
            continue;
        }
        let c_ref = format!("{model_ref}/capability/{}", c.capability_id);
        let mut names = BTreeSet::from([DECISION_ID.to_string(), SUBJECT.to_string()]);
        let mut check = |name: &String, errors: &mut Vec<ModelError>| {
            let ok = is_core_identifier(name) && names.insert(name.clone());
            if !ok {
                errors.push(ModelError::InvalidParameter {
                    model_ref: c_ref.clone(),
                    name: name.clone(),
                });
            }
            ok
        };
        let mut inputs = BTreeMap::new();
        for (n, t) in &c.inputs {
            if check(n, &mut errors) {
                inputs.insert(n.clone(), t.clone());
            }
        }
        let mut context = BTreeMap::new();
        for (n, t) in &c.context {
            if check(n, &mut errors) {
                context.insert(n.clone(), t.clone());
            }
        }
        capabilities.insert(
            c.capability_id.clone(),
            ResolvedCapability {
                action_name: encode(&approval_id, &capability_id),
                capability_id,
                choice,
                inputs,
                context,
                display: c.display.clone(),
                source: c.source.clone(),
            },
        );
    }
    for (id, _) in CAPABILITIES {
        if !supplied.contains(id) {
            errors.push(ModelError::InvalidParameter {
                model_ref: model_ref.clone(),
                name: id.to_string(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ResolvedApproval {
        approval_id,
        enum_name,
        entity,
        subject: input.subject_entity.clone(),
        actor: input.actor_entity.clone(),
        // Checked by `decls`: both are at least 1.
        approve_threshold: u32::try_from(input.approve_threshold).unwrap_or(1),
        reject_threshold: input
            .reject_threshold
            .map(|k| u32::try_from(k).unwrap_or(1)),
        payload: input.payload.iter().cloned().collect(),
        schema: schema.fields,
        capabilities,
        display: input.display.clone(),
        source: input.source.clone(),
    })
}
