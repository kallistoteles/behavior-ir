//! Fixed numeric proof encoding. This correspondence code is explicitly trusted;
//! kernel checking derives relevance and premises from these raw finite inputs.
use super::{
    SafetyError, UncertifiedSafetyAnalysis,
    certify::{bytes, list},
    guards::{CanonicalExpression, CanonicalProbes},
    inventory::Operation,
};
use behavior_engine::{semantic::Module, wire::WType};
use std::collections::BTreeMap;

pub(super) fn lifecycle(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
) -> Result<String, SafetyError> {
    if a.dependencies.is_empty() {
        return Ok("{}".into());
    }
    let probes = CanonicalProbes::new(module)?;
    validate_membership_matches(module, a, &probes)?;
    let entities = a
        .inventory
        .entities()
        .keys()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let fields = a
        .inventory
        .entities()
        .iter()
        .map(|(n, e)| {
            (
                n.as_str(),
                e.fields
                    .keys()
                    .enumerate()
                    .map(|(i, f)| (f.as_str(), i))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let get = |map: &BTreeMap<&str, usize>, key: &str| {
        map.get(key).copied().ok_or_else(|| {
            SafetyError::new(
                "SAFETY_ENCODING_FAILED",
                format!("Missing resolved coordinate {key}"),
            )
        })
    };
    let mut deps = vec![];
    for d in &a.dependencies {
        let parent = get(&entities, &d.parent)?;
        let child = get(&entities, &d.child)?;
        let literals = |m: &BTreeMap<String, String>| {
            list(
                m.iter()
                    .map(|(member, hash)| format!("({}, {})", bytes(hash), bytes(member))),
            )
        };
        let controls = d
            .controls
            .iter()
            .map(|hash| {
                a.policies
                    .iter()
                    .position(|p| p.model_hash() == hash)
                    .map(|i| i.to_string())
                    .ok_or_else(|| {
                        SafetyError::new("SAFETY_ENCODING_FAILED", "Missing ownership control")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        deps.push(format!(
            "⟨{parent},{child},{},{},{},{parent},{},{},{},{},{}⟩",
            get(&fields[d.parent.as_str()], &d.parent_state)?,
            get(&fields[d.child.as_str()], &d.child_state)?,
            get(&fields[d.child.as_str()], &d.owner_field)?,
            literals(&d.parent_literals),
            literals(&d.child_literals),
            list(d.prohibited.iter().map(|m| bytes(m))),
            list(d.terminal.iter().map(|m| bytes(m))),
            list(controls)
        ));
    }
    let mut actions = vec![];
    for (name, action) in a.inventory.actions() {
        let params = action
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.as_str(), i))
            .collect::<BTreeMap<_, _>>();
        let mut parameters = vec![];
        let mut captures = vec![];
        for p in &action.params {
            match &p.ty {
                WType::Entity(e) => {
                    parameters.push(format!(
                        "({}, {})",
                        get(&entities, e)?,
                        probes.is_state_binding(name, &p.name, e)?
                    ));
                    captures.push((p.name.clone(), e.clone()));
                }
                _ => parameters.push(format!("({},false)", entities.len())),
            }
        }
        let mut ops = vec![];
        for op in &action.operations {
            let entity = get(&entities, op.entity())?;
            let (kind, field, binding, value, initializers) = match op {
                Operation::Assign {
                    entity,
                    field,
                    param,
                    value,
                    ..
                } => (
                    "assign",
                    get(&fields[entity.as_str()], field)?,
                    get(&params, param)?,
                    bytes(value),
                    "[]".into(),
                ),
                Operation::Create {
                    entity, fields: fs, ..
                } => (
                    "create",
                    0,
                    0,
                    "[]".into(),
                    list(
                        fs.iter()
                            .map(|(f, v)| {
                                Ok(format!(
                                    "({}, {})",
                                    get(&fields[entity.as_str()], f)?,
                                    bytes(v)
                                ))
                            })
                            .collect::<Result<Vec<_>, SafetyError>>()?,
                    ),
                ),
                Operation::Remove { param, .. } => {
                    ("remove", 0, get(&params, param)?, "[]".into(), "[]".into())
                }
            };
            ops.push(format!(
                "⟨⟨{entity},.{kind},{field}⟩,{binding},{value},{initializers}⟩"
            ));
        }
        let mut guards = vec![];
        for (di, d) in a.dependencies.iter().enumerate() {
            for (p, entity) in &captures {
                if entity != &d.parent || !probes.is_state_binding(name, p, entity)? {
                    continue;
                }
                // Only private exact-root matches from final-module analysis are authoritative.
                let matched = |family: &str,
                               required: &std::collections::BTreeSet<String>|
                 -> Result<(String, String), SafetyError> {
                    let found = a
                        .membership_matches
                        .iter()
                        .filter(|m| {
                            m.action == *name
                                && m.dependency == d.model_hash()
                                && m.parent == *p
                                && m.family == family
                        })
                        .min_by(|x, y| x.root.cmp(&y.root));
                    if let Some(m) = found {
                        let expected_entity = if family == "no_children" {
                            &d.child
                        } else {
                            &d.parent
                        };
                        let expected_field = if family == "no_children" {
                            &d.child_state
                        } else {
                            &d.parent_state
                        };
                        if m.action_hash != action.hash
                            || &m.required != required
                            || &m.entity != expected_entity
                            || &m.field != expected_field
                            || m.enumeration.is_empty()
                            || !action.preconditions.contains(&m.root)
                        {
                            return Err(SafetyError::new(
                                "SAFETY_ENCODING_FAILED",
                                "Membership witness does not bind the exact action/dependency/field/root",
                            ));
                        }
                        Ok((bytes(&m.root), list(m.ordered.iter().map(|v| bytes(v)))))
                    } else {
                        // Empty hash is a nonmatching sentinel, never an admitted root.
                        Ok(("[]".into(), list(required.iter().map(|v| bytes(v)))))
                    }
                };
                let (no_children, no_children_members) = matched("no_children", &d.terminal)?;
                let (permitted, permitted_members) = matched("permitted_parent", &d.prohibited)?;
                let parent_id = probes.expression(
                    &captures,
                    &CanonicalExpression::BoundId { binding: p.clone() },
                )?;
                // A creation has no existing child binding. Empty ownerEquality
                // is an unused sentinel, never an admitted expression hash/root.
                guards.push(format!(
                    "⟨{di},{},0,{}, {},[],{},{},{}⟩",
                    get(&params, p)?,
                    no_children,
                    permitted,
                    bytes(&parent_id),
                    no_children_members,
                    permitted_members
                ));
                for (c, child) in &captures {
                    if child != &d.child || !probes.is_state_binding(name, c, child)? {
                        continue;
                    }
                    let owner = probes.expression(
                        &captures,
                        &CanonicalExpression::OwnerEquality {
                            child: c.clone(),
                            owner_field: d.owner_field.clone(),
                            parent: p.clone(),
                        },
                    )?;
                    guards.push(format!(
                        "⟨{di},{},{},{},{},{},{},{},{}⟩",
                        get(&params, p)?,
                        get(&params, c)?,
                        no_children,
                        permitted,
                        bytes(&owner),
                        bytes(&parent_id),
                        no_children_members,
                        permitted_members
                    ));
                }
            }
        }
        actions.push(format!(
            "⟨{},{},{},{}⟩",
            list(parameters),
            list(action.preconditions.iter().map(|h| bytes(h))),
            list(ops),
            list(guards)
        ));
    }
    Ok(format!(
        "{{actions := {},dependencies := {}}}",
        list(actions),
        list(deps)
    ))
}

/// Recheck the opaque Rust witness at the encoding boundary. The set agreement is
/// also checked by Lean; exact order/root/captures remain public-Core correspondence.
fn validate_membership_matches(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
    probes: &CanonicalProbes,
) -> Result<(), SafetyError> {
    let invalid = || {
        SafetyError::new(
            "SAFETY_ENCODING_FAILED",
            "Membership witness no longer matches the exact admitted action, dependency and ordered root",
        )
    };
    for m in &a.membership_matches {
        let d = a
            .dependencies
            .iter()
            .find(|d| d.model_hash() == m.dependency)
            .ok_or_else(invalid)?;
        let action = a.inventory.actions().get(&m.action).ok_or_else(invalid)?;
        let captures = action
            .params
            .iter()
            .filter_map(|p| match &p.ty {
                WType::Entity(e) => Some((p.name.clone(), e.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let (entity, field, required, expression) = match m.family {
            "no_children" => (
                &d.child,
                &d.child_state,
                &d.terminal,
                CanonicalExpression::NoNonterminalChildren {
                    parent: m.parent.clone(),
                    child: d.child.clone(),
                    owner_field: d.owner_field.clone(),
                    child_state: d.child_state.clone(),
                    terminal: m.ordered.clone(),
                },
            ),
            "permitted_parent" => (
                &d.parent,
                &d.parent_state,
                &d.prohibited,
                CanonicalExpression::Membership {
                    binding: m.parent.clone(),
                    field: d.parent_state.clone(),
                    values: m.ordered.clone(),
                    negated: true,
                },
            ),
            _ => return Err(invalid()),
        };
        let Some(WType::Enum(enumeration)) = module
            .entity(entity)
            .and_then(|e| e.field_type(field))
            .map(|t| t.to_wire_type())
        else {
            return Err(invalid());
        };
        if m.action_hash != action.hash
            || &m.entity != entity
            || &m.field != field
            || m.enumeration != enumeration
            || &m.required != required
            || m.ordered
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
                != *required
            || !probes.is_state_binding(&m.action, &m.parent, &d.parent)?
            || !probes.mandatory_root(&m.action, &m.root)?
            || probes.expression_ordered(&captures, &expression)? != m.root
        {
            return Err(invalid());
        }
    }
    Ok(())
}
