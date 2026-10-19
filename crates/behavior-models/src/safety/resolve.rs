//! Final-module policy resolution. Display references grant no authority.
use super::input::{ActionSelector, FieldOwnershipInput};
use super::{Inventory, SafetyError};
use crate::ids::{SemanticId, encode};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct ResolvedOwnership {
    pub(crate) author_id: String,
    pub(crate) entity: String,
    pub(crate) entity_hash: String,
    pub(crate) writers: BTreeMap<String, BTreeSet<String>>,
    pub(crate) creators: BTreeSet<String>,
    pub(crate) removers: BTreeSet<String>,
    pub(crate) model_hash: String,
}
impl ResolvedOwnership {
    pub fn model_hash(&self) -> &str {
        &self.model_hash
    }
    pub fn document(&self) -> Value {
        json!({"kind":"FieldOwnership","model_semantic_version":super::FIELD_OWNERSHIP_SEMANTIC_VERSION,
            "entity":self.entity_hash,"writers":self.writers,"creators":self.creators,"removers":self.removers})
    }
}

pub(crate) fn resolve(
    inventory: &Inventory,
    inputs: &[FieldOwnershipInput],
) -> Result<Vec<ResolvedOwnership>, SafetyError> {
    let mut ids = BTreeSet::new();
    let mut documents = BTreeSet::new();
    let mut out = Vec::new();
    for p in inputs {
        if SemanticId::parse(&p.author_id).is_none() || !ids.insert(&p.author_id) {
            return Err(SafetyError::new(
                "SAFETY_INVALID_POLICY",
                format!("Invalid or repeated policy reference `{}`", p.author_id),
            ));
        }
        let entity = inventory.entities().get(&p.entity).ok_or_else(|| {
            SafetyError::new(
                "SAFETY_UNKNOWN_ENTITY",
                format!("Unknown entity `{}`", p.entity),
            )
        })?;
        let actions = |sels: &[ActionSelector]| -> Result<BTreeSet<String>, SafetyError> {
            let mut seen = BTreeSet::new();
            let mut hashes = BTreeSet::new();
            for s in sels {
                if !seen.insert(s) {
                    return Err(SafetyError::new(
                        "SAFETY_INVALID_SELECTOR",
                        "Duplicate selector",
                    ));
                }
                let name = match s {
                    ActionSelector::Authored(n) => n.clone(),
                    ActionSelector::Generated {
                        model_id,
                        local_action_id,
                    } => {
                        let id = |v: &str| {
                            SemanticId::parse(v).ok_or_else(|| {
                                SafetyError::new(
                                    "SAFETY_INVALID_SELECTOR",
                                    format!("Invalid generated selector `{v}`"),
                                )
                            })
                        };
                        encode(&id(model_id)?, &id(local_action_id)?)
                    }
                };
                let a = inventory.actions().get(&name).ok_or_else(|| {
                    SafetyError::new("SAFETY_UNKNOWN_ACTION", format!("No final action `{name}`"))
                })?;
                if !hashes.insert(a.hash.clone()) {
                    return Err(SafetyError::new(
                        "SAFETY_AMBIGUOUS_SELECTOR",
                        format!("Repeated resolved action `{name}`"),
                    ));
                }
            }
            Ok(hashes)
        };
        let mut writers = BTreeMap::new();
        for (f, selectors) in &p.writers {
            if f == "id" || !entity.fields.contains_key(f) {
                return Err(SafetyError::new(
                    "SAFETY_UNKNOWN_FIELD",
                    format!("No assignable field `{}.{f}`", p.entity),
                ));
            }
            writers.insert(f.clone(), actions(selectors)?);
        }
        let mut r = ResolvedOwnership {
            author_id: p.author_id.clone(),
            entity: p.entity.clone(),
            entity_hash: entity.hash.clone(),
            writers,
            creators: actions(&p.creators)?,
            removers: actions(&p.removers)?,
            model_hash: String::new(),
        };
        r.model_hash = super::identity::tagged(crate::identity::MODEL_HASH_TAG, &r.document())?;
        if !documents.insert(r.model_hash.clone()) {
            return Err(SafetyError::new(
                "SAFETY_DUPLICATE_POLICY",
                "Duplicate normalized ownership policy",
            ));
        }
        out.push(r);
    }
    out.sort_by(|a, b| a.model_hash.cmp(&b.model_hash));
    Ok(out)
}

/// Resolve canonical imported policy records back through the same authoring
/// authority. Synthetic display IDs never become semantic permission grants.
pub(crate) fn imported(
    module: &behavior_engine::semantic::Module,
    records: &Value,
) -> Result<super::UncertifiedSafetyAnalysis, SafetyError> {
    let invalid = |detail: &str| SafetyError::new("SAFETY_INVALID_BUNDLE", detail);
    let records = records
        .as_array()
        .ok_or_else(|| invalid("Expected canonical policy records"))?;
    let inventory = Inventory::extract(module)?;
    let entity_name = |hash: &Value| -> Result<String, SafetyError> {
        let hash = hash
            .as_str()
            .ok_or_else(|| invalid("Expected entity identity"))?;
        let names = inventory
            .entities()
            .iter()
            .filter(|(_, e)| e.hash == hash)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        if names.len() != 1 {
            return Err(invalid("Unknown or ambiguous entity identity"));
        }
        Ok(names[0].clone())
    };
    let action_names = |values: &Value| -> Result<Value, SafetyError> {
        let values = values
            .as_array()
            .ok_or_else(|| invalid("Expected action identity set"))?;
        Ok(Value::Array(
            values
                .iter()
                .map(|h| {
                    let h = h
                        .as_str()
                        .ok_or_else(|| invalid("Expected action identity"))?;
                    let names = inventory
                        .actions()
                        .values()
                        .filter(|a| a.hash == h)
                        .map(|a| a.name.clone())
                        .collect::<Vec<_>>();
                    if names.len() != 1 {
                        return Err(invalid("Unknown or ambiguous action identity"));
                    }
                    Ok(json!(names[0]))
                })
                .collect::<Result<Vec<_>, SafetyError>>()?,
        ))
    };
    let mut ownership = Vec::new();
    let mut dependencies = Vec::new();
    let mut ids = BTreeMap::new();
    for (i, record) in records.iter().enumerate() {
        let obj = record
            .as_object()
            .ok_or_else(|| invalid("Expected policy record"))?;
        if obj.len() != 2 || !obj.contains_key("model_hash") || !obj.contains_key("document") {
            return Err(invalid("Unknown policy record fields"));
        }
        if record["model_hash"]
            != super::identity::tagged(crate::identity::MODEL_HASH_TAG, &record["document"])?
        {
            return Err(invalid("Policy document identity differs"));
        }
        if record["document"]["kind"] == "FieldOwnership" {
            let d = &record["document"];
            let id = format!("imported_fo_{i}");
            let writers = d["writers"]
                .as_object()
                .ok_or_else(|| invalid("Missing ownership writers"))?
                .iter()
                .map(|(f, hashes)| Ok((f.clone(), action_names(hashes)?)))
                .collect::<Result<serde_json::Map<_, _>, SafetyError>>()?;
            ownership.push(FieldOwnershipInput::from_json(
                &json!({"id":id,"entity":entity_name(&d["entity"])?,"writers":writers,
                "creators":action_names(&d["creators"])?,"removers":action_names(&d["removers"])?}),
            )?);
            if ids
                .insert(
                    record["model_hash"]
                        .as_str()
                        .ok_or_else(|| invalid("Expected ModelHash"))?
                        .to_owned(),
                    id,
                )
                .is_some()
            {
                return Err(invalid("Duplicate policy identity"));
            }
        }
    }
    for (i, record) in records.iter().enumerate() {
        let d = &record["document"];
        match d["kind"].as_str() {
            Some("FieldOwnership") => {}
            Some("LifecycleDependency") => {
                let refs = d["ownership"]
                    .as_array()
                    .ok_or_else(|| invalid("Missing ownership policy references"))?
                    .iter()
                    .map(|h| {
                        ids.get(
                            h.as_str()
                                .ok_or_else(|| invalid("Expected control identity"))?,
                        )
                        .cloned()
                        .ok_or_else(|| invalid("Unknown, cyclic or non-ownership policy reference"))
                    })
                    .collect::<Result<Vec<_>, SafetyError>>()?;
                dependencies.push(super::LifecycleDependencyInput::from_json(&json!({"id":format!("imported_ld_{i}"),"parent":entity_name(&d["parent"])?,"child":entity_name(&d["child"])?,
                    "owner_field":d["owner_field"],"parent_state":d["parent_state"],"child_state":d["child_state"],"prohibited":d["prohibited"],"terminal":d["terminal"],"ownership":refs}))?);
            }
            _ => return Err(invalid("Unknown Safety Model kind")),
        }
    }
    let a = super::analyze_with_dependencies(module, &ownership, &dependencies)?;
    if a.policy_records() != *records {
        return Err(invalid(
            "Policy records are not exact canonical supported policies",
        ));
    }
    Ok(a)
}
