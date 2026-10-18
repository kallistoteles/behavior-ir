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
                let no_children = probes.expression(
                    &captures,
                    &CanonicalExpression::NoNonterminalChildren {
                        parent: p.clone(),
                        child: d.child.clone(),
                        owner_field: d.owner_field.clone(),
                        child_state: d.child_state.clone(),
                        terminal: d.terminal.iter().cloned().collect(),
                    },
                )?;
                let permitted = probes.expression(
                    &captures,
                    &CanonicalExpression::Membership {
                        binding: p.clone(),
                        field: d.parent_state.clone(),
                        values: d.prohibited.iter().cloned().collect(),
                        negated: true,
                    },
                )?;
                let parent_id = probes.expression(
                    &captures,
                    &CanonicalExpression::BoundId { binding: p.clone() },
                )?;
                // A creation has no existing child binding. Empty ownerEquality
                // is an unused sentinel, never an admitted expression hash/root.
                guards.push(format!(
                    "⟨{di},{},0,{}, {},[],{}⟩",
                    get(&params, p)?,
                    bytes(&no_children),
                    bytes(&permitted),
                    bytes(&parent_id)
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
                        "⟨{di},{},{},{},{},{},{}⟩",
                        get(&params, p)?,
                        get(&params, c)?,
                        bytes(&no_children),
                        bytes(&permitted),
                        bytes(&owner),
                        bytes(&parent_id)
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
