//! Restricted lifecycle policies. No state, effects or guards are generated for execution.
use super::{
    Inventory, LifecycleDependencyInput, SafetyError,
    guards::{CanonicalExpression, CanonicalProbes},
    identity::tagged,
    inventory::{ActionData, Operation},
    resolve::ResolvedOwnership,
};
use behavior_engine::{semantic::Module, wire::WType};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct ResolvedDependency {
    pub(crate) author_id: String,
    pub(crate) parent: String,
    pub(crate) child: String,
    pub(crate) owner_field: String,
    pub(crate) parent_state: String,
    pub(crate) child_state: String,
    pub(crate) prohibited: BTreeSet<String>,
    pub(crate) terminal: BTreeSet<String>,
    pub(crate) controls: BTreeSet<String>,
    pub(crate) parent_literals: BTreeMap<String, String>,
    pub(crate) child_literals: BTreeMap<String, String>,
    document: Value,
    model_hash: String,
}
impl ResolvedDependency {
    pub fn model_hash(&self) -> &str {
        &self.model_hash
    }
    pub fn document(&self) -> Value {
        self.document.clone()
    }
}
#[derive(Debug, Clone, Default)]
pub struct ActionProfile {
    pub relevant_operations: usize,
    pub dependencies: BTreeSet<String>,
}

fn invalid(detail: impl Into<String>) -> SafetyError {
    SafetyError::new("SAFETY_INVALID_DEPENDENCY", detail)
}

pub(crate) fn resolve(
    module: &Module,
    inventory: &Inventory,
    policies: &[ResolvedOwnership],
    inputs: &[LifecycleDependencyInput],
) -> Result<Vec<ResolvedDependency>, SafetyError> {
    if inputs.is_empty() {
        return Ok(vec![]);
    }
    let probes = CanonicalProbes::new(module)?;
    let mut ids = policies
        .iter()
        .map(|p| p.author_id.clone())
        .collect::<BTreeSet<_>>();
    let mut hashes = BTreeSet::new();
    let mut out = vec![];
    for input in inputs {
        if crate::ids::SemanticId::parse(&input.author_id).is_none()
            || !ids.insert(input.author_id.clone())
        {
            return Err(invalid("Invalid/repeated Safety policy reference"));
        }
        let parent = inventory
            .entities()
            .get(&input.parent)
            .ok_or_else(|| invalid("Unknown parent entity"))?;
        let child = inventory
            .entities()
            .get(&input.child)
            .ok_or_else(|| invalid("Unknown child entity"))?;
        let owner = child
            .fields
            .get(&input.owner_field)
            .ok_or_else(|| invalid("Unknown ownership field"))?;
        if !owner.reference || owner.ty != WType::Id(input.parent.clone()) {
            return Err(invalid(
                "LifecycleDependency v1 requires a nonoptional Ref[Parent]",
            ));
        }
        let enum_members = |entity: &str,
                            field: &str,
                            selected: &BTreeSet<String>|
         -> Result<BTreeMap<String, String>, SafetyError> {
            let e = inventory
                .entities()
                .get(entity)
                .ok_or_else(|| invalid("Unknown entity"))?;
            let Some(WType::Enum(name)) = e.fields.get(field).map(|f| &f.ty) else {
                return Err(invalid("Lifecycle status must be an enum field"));
            };
            let en = module
                .enums()
                .get(name)
                .ok_or_else(|| invalid("Unknown lifecycle enum"))?;
            if selected.iter().any(|v| !en.values.contains(v)) {
                return Err(invalid("Unknown lifecycle enum member"));
            }
            en.values
                .iter()
                .map(|v| {
                    Ok((
                        v.clone(),
                        probes.expression(
                            &[],
                            &CanonicalExpression::EnumLiteral {
                                entity: entity.into(),
                                field: field.into(),
                                member: v.clone(),
                            },
                        )?,
                    ))
                })
                .collect()
        };
        let parent_literals = enum_members(&input.parent, &input.parent_state, &input.prohibited)?;
        let child_literals = enum_members(&input.child, &input.child_state, &input.terminal)?;
        let controls = input
            .ownership
            .iter()
            .map(|id| {
                policies
                    .iter()
                    .find(|p| &p.author_id == id)
                    .ok_or_else(|| invalid("Missing referenced FieldOwnership policy"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (entity, field) in [
            (&input.parent, &input.parent_state),
            (&input.child, &input.child_state),
            (&input.child, &input.owner_field),
        ] {
            if !controls
                .iter()
                .any(|p| &p.entity == entity && p.writers.contains_key(field))
            {
                return Err(invalid(format!(
                    "Missing explicit FieldOwnership control for {entity}.{field}"
                )));
            }
        }
        // Permissions compose by intersection. No final action may be permitted to assign owner.
        let owner_controls = policies
            .iter()
            .filter(|p| p.entity == input.child && p.writers.contains_key(&input.owner_field))
            .collect::<Vec<_>>();
        for action in inventory.actions().values() {
            if owner_controls
                .iter()
                .all(|p| p.writers[&input.owner_field].contains(&action.hash))
            {
                return Err(invalid(
                    "Owner must be immutable, including value-preserving assignments",
                ));
            }
        }
        let control_hashes = controls
            .iter()
            .map(|p| p.model_hash().to_owned())
            .collect::<BTreeSet<_>>();
        let document = json!({"kind":"LifecycleDependency","model_semantic_version":1,
            "parent":parent.hash,"child":child.hash,"owner_field":input.owner_field,"parent_state":input.parent_state,"child_state":input.child_state,
            "prohibited":input.prohibited,"terminal":input.terminal,"ownership":control_hashes});
        let model_hash = tagged(crate::identity::MODEL_HASH_TAG, &document)?;
        if !hashes.insert(model_hash.clone()) {
            return Err(invalid("Duplicate normalized lifecycle policy"));
        }
        out.push(ResolvedDependency {
            author_id: input.author_id.clone(),
            parent: input.parent.clone(),
            child: input.child.clone(),
            owner_field: input.owner_field.clone(),
            parent_state: input.parent_state.clone(),
            child_state: input.child_state.clone(),
            prohibited: input.prohibited.clone(),
            terminal: input.terminal.clone(),
            controls: control_hashes,
            parent_literals,
            child_literals,
            document,
            model_hash,
        });
    }
    out.sort_by(|a, b| a.model_hash.cmp(&b.model_hash));
    Ok(out)
}

/// Footprints are derived from every actual operation and every configured property.
/// Return (parent role, child role); creation/removal affects all projected fields.
pub(crate) fn roles(op: &Operation, d: &ResolvedDependency) -> (bool, bool) {
    match op {
        Operation::Assign { entity, field, .. } => (
            entity == &d.parent && field == &d.parent_state,
            entity == &d.child && (field == &d.child_state || field == &d.owner_field),
        ),
        Operation::Create { entity, .. } | Operation::Remove { entity, .. } => {
            (entity == &d.parent, entity == &d.child)
        }
    }
}
fn captures(a: &ActionData) -> Vec<(String, String)> {
    a.params
        .iter()
        .filter_map(|p| match &p.ty {
            WType::Entity(e) => Some((p.name.clone(), e.clone())),
            _ => None,
        })
        .collect()
}
fn issue(action: &str, code: &'static str, detail: impl Into<String>) -> SafetyError {
    let mut e = SafetyError::new(code, detail);
    e.action = Some(action.into());
    e
}
pub(crate) fn inspect(
    module: &Module,
    inventory: &Inventory,
    deps: &[ResolvedDependency],
) -> Result<(BTreeMap<String, ActionProfile>, Vec<SafetyError>), SafetyError> {
    if deps.is_empty() {
        return Ok((BTreeMap::new(), vec![]));
    }
    let probes = CanonicalProbes::new(module)?;
    let mut profiles = BTreeMap::new();
    let mut issues = vec![];
    for (name, a) in inventory.actions() {
        let mut profile = ActionProfile::default();
        for op in &a.operations {
            let relevant = deps
                .iter()
                .filter(|d| {
                    let (p, c) = roles(op, d);
                    p || c
                })
                .collect::<Vec<_>>();
            if !relevant.is_empty() {
                profile.relevant_operations += 1;
            }
            if relevant.iter().any(|d| roles(op, d).0) && relevant.iter().any(|d| roles(op, d).1) {
                issues.push(issue(name,"SAFETY_MIXED_LIFECYCLE_ROLES","One operation affects both parent and child roles across the complete dependency set"));
            }
            for d in relevant {
                profile.dependencies.insert(d.model_hash.clone());
                let (p, c) = roles(op, d);
                if p && c {
                    continue;
                }
                if let Some(e) = check_operation(&probes, a, op, d)? {
                    issues.push(e);
                }
            }
        }
        if profile.relevant_operations > 1 {
            issues.push(issue(
                name,
                "SAFETY_MULTIPLE_LIFECYCLE_OPERATIONS",
                "v1 allows one relevant operation across the union of dependency footprints",
            ));
        }
        profiles.insert(name.clone(), profile);
    }
    Ok((profiles, issues))
}

fn check_operation(
    probes: &CanonicalProbes,
    a: &ActionData,
    op: &Operation,
    d: &ResolvedDependency,
) -> Result<Option<SafetyError>, SafetyError> {
    let (parent_role, _) = roles(op, d);
    let fail = |detail: &str| {
        Some(issue(
            &a.name,
            "SAFETY_UNSUPPORTED_LIFECYCLE",
            format!("{}: {detail}", d.author_id),
        ))
    };
    let (param, value) = match op {
        Operation::Assign { param, value, .. } => (Some(param.as_str()), Some(value.as_str())),
        Operation::Create { fields, .. } => (
            None,
            fields
                .get(if parent_role {
                    &d.parent_state
                } else {
                    &d.child_state
                })
                .map(String::as_str),
        ),
        Operation::Remove { .. } => return Ok(None),
    };
    let literals = if parent_role {
        &d.parent_literals
    } else {
        &d.child_literals
    };
    let Some(member) = literals
        .iter()
        .find_map(|(m, h)| (Some(h.as_str()) == value).then_some(m))
    else {
        return Ok(fail("Lifecycle destination is not a supported literal"));
    };
    let caps = captures(a);
    let root = |expr| -> Result<bool, SafetyError> {
        let hash = probes.expression(&caps, &expr)?;
        probes.mandatory_root(&a.name, &hash)
    };
    if parent_role {
        // Parent creation is fresh by Core's successful ordinary-transition contract.
        if let Some(p) = param
            && d.prohibited.contains(member)
            && !root(CanonicalExpression::NoNonterminalChildren {
                parent: p.into(),
                child: d.child.clone(),
                owner_field: d.owner_field.clone(),
                child_state: d.child_state.clone(),
                terminal: d.terminal.iter().cloned().collect(),
            })?
        {
            return Ok(fail("Missing mandatory no-nonterminal-dependent guard"));
        }
    } else {
        let mut candidate = false;
        for (p, entity) in &caps {
            if entity != &d.parent || !probes.is_state_binding(&a.name, p, entity)? {
                continue;
            }
            let relationship = match op {
                Operation::Create { fields, .. } => {
                    fields.get(&d.owner_field)
                        == Some(&probes.expression(
                            &caps,
                            &CanonicalExpression::BoundId { binding: p.clone() },
                        )?)
                }
                Operation::Assign { param: c, .. } => root(CanonicalExpression::OwnerEquality {
                    child: c.clone(),
                    owner_field: d.owner_field.clone(),
                    parent: p.clone(),
                })?,
                _ => false,
            };
            let permitted = d.terminal.contains(member)
                || root(CanonicalExpression::Membership {
                    binding: p.clone(),
                    field: d.parent_state.clone(),
                    values: d.prohibited.iter().cloned().collect(),
                    negated: true,
                })?;
            candidate |= relationship && permitted;
        }
        // Terminal assignments don't need a parent guard: immutable Ref and source validity suffice.
        if !(matches!(op, Operation::Assign { .. }) && d.terminal.contains(member) || candidate) {
            return Ok(fail(
                "Missing bound State parent, exact owner association or mandatory permitted-parent guard",
            ));
        }
    }
    Ok(None)
}
