//! Approval identity and provenance (data model §8–§9).
//!
//! ```text
//! ApprovalHash = tagged_hash("behavior.model.v1", CanonicalApprovalDocument)
//! ```
//!
//! The document holds only the facts the author chose: the subject and actor types (by name and
//! Core declaration hash, so a change to either entity changes the approval), the thresholds,
//! the payload and each capability's fragments. Every named type in the payload, inputs and
//! context is likewise carried with its Core declaration hash: an unused context enum still
//! shapes the accepted invocations, and a payload enum shapes the decision entity. Every fragment is represented by Core's own
//! semantic hash, read from the admitted action at the positions the layout fixes (data model
//! §6): preconditions `[0]` Open, `[2]` Eligible, `[4..]` the guards; the creation's `actor` and
//! payload fields; the command emissions. Generated names, the decision schema, the derived
//! bodies and the generated pre- and postconditions are functions of these facts and are not
//! hashed separately. One receipt per supplied approval.

use std::collections::BTreeMap;

use behavior_engine::semantic::Module;
use behavior_engine::semantic::types::hash_display;
use serde_json::{Map, Value, json};

use crate::approval::decls::DEFINITIONS;
use crate::approval::resolve::{ACTOR, ResolvedApproval};
use crate::error::ModelError;
use crate::identity::{entity_ref, identity_error, model_hash, typed_params};
use crate::source_map::{LoweringReceipt, RelativeSources, display_json};
use crate::{APPROVAL_LOWERER_VERSION, APPROVAL_SEMANTIC_VERSION, CORE_RELEASE};

/// Precondition positions of a decision action (data model §6).
const OPEN: usize = 0;
const TERMINAL_GUARD: usize = 1;
const ELIGIBLE: usize = 2;
const FIRST_GUARD: usize = 4;
/// Postcondition positions.
const UNIQUENESS: usize = 0;
const THRESHOLD_CONSISTENCY: usize = 1;

/// The canonical document of a resolved, lowered and admitted approval.
pub fn canonical_document_ap(a: &ResolvedApproval, module: &Module) -> Result<Value, ModelError> {
    let mut capabilities = Vec::new();
    for c in a.capabilities.values() {
        let action = module
            .actions()
            .get(&c.action_name)
            .ok_or_else(|| identity_error(format!("action `{}` not in module", c.action_name)))?;
        let pre = action.preconditions();
        if pre.len() < FIRST_GUARD {
            return Err(identity_error(format!(
                "action `{}` lacks the decision layout",
                c.action_name
            )));
        }
        let created = action
            .creates()
            .first()
            .ok_or_else(|| identity_error(format!("action `{}` creates nothing", c.action_name)))?;
        let fields: BTreeMap<&str, String> = created
            .fields()
            .iter()
            .map(|(n, e)| (n.as_str(), hash_display(e.hash())))
            .collect();
        let actor = fields
            .get(ACTOR)
            .cloned()
            .ok_or_else(|| identity_error(format!("action `{}`: no actor", c.action_name)))?;
        let payload: BTreeMap<&str, &String> = fields
            .iter()
            .filter(|(n, _)| a.payload.contains_key(**n))
            .map(|(n, h)| (*n, h))
            .collect();
        capabilities.push(json!({
            "id": c.capability_id.as_str(),
            "inputs": typed_params(&c.inputs, module, a.approval_id.as_str())?,
            "context": typed_params(&c.context, module, a.approval_id.as_str())?,
            "actor": actor,
            "open": hash_display(pre[OPEN].expr().hash()),
            "eligible": hash_display(pre[ELIGIBLE].expr().hash()),
            "guards": pre[FIRST_GUARD..].iter().map(|p| hash_display(p.expr().hash())).collect::<Vec<_>>(),
            "payload": payload,
            "commands": action.command_emissions().iter().map(|k| hash_display(k.hash())).collect::<Vec<_>>(),
        }));
    }
    Ok(json!({
        "format": "behavior.model.approval.v1",
        "model_kind": "approval",
        "model_semantic_version": APPROVAL_SEMANTIC_VERSION,
        "approval_id": a.approval_id.as_str(),
        "subject": entity_ref(module, a.approval_id.as_str(), "subject", &a.subject)?,
        "actor": entity_ref(module, a.approval_id.as_str(), "actor", &a.actor)?,
        "approve_threshold": a.approve_threshold,
        "reject_threshold": a.reject_threshold,
        "payload": typed_params(&a.payload, module, a.approval_id.as_str())?,
        "capabilities": capabilities,
    }))
}

/// The names and hashes of what the approval generated (data model §9).
fn generated(a: &ResolvedApproval, module: &Module) -> Result<Value, ModelError> {
    let enum_hash = module
        .enums()
        .get(&a.enum_name)
        .map(|e| hash_display(&e.hash))
        .ok_or_else(|| identity_error(format!("enum `{}` not in module", a.enum_name)))?;
    let entity = entity_ref(module, a.approval_id.as_str(), "decision entity", &a.entity)?;
    let mut derived = Map::new();
    for local in DEFINITIONS {
        let name = a.definition_name(local);
        let item = module
            .derived_items()
            .get(&name)
            .ok_or_else(|| identity_error(format!("derived `{name}` not in module")))?;
        derived.insert(local.to_string(), json!(hash_display(item.hash())));
    }
    Ok(json!({
        "enum": {"name": a.enum_name, "hash": enum_hash},
        "entity": {"name": a.entity, "hash": entity["entity_hash"]},
        "derived": derived,
    }))
}

/// The approval's source map (data model §9): capabilities → actions, predicates → derived
/// items, the generated checks → their condition positions, and the generated declarations.
/// Locations are relative (Constitution IV).
fn source_map_ap(a: &ResolvedApproval, module: &Module, root: Option<&str>) -> Value {
    let sources = RelativeSources::from_sources(
        a.capabilities
            .values()
            .filter_map(|c| c.source.as_ref())
            .chain(a.source.as_ref()),
        root,
    );
    let p = a.approval_id.as_str();
    let actions: Vec<&str> = a
        .capabilities
        .values()
        .map(|c| c.action_name.as_str())
        .collect();
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    let mut entry = |key: String, core: Value, model: String, extra: Vec<(&str, Option<Value>)>| {
        let mut e = Map::new();
        e.insert("core".into(), core);
        e.insert("model".into(), json!(model));
        for (k, v) in extra {
            if let Some(v) = v {
                e.insert(k.into(), v);
            }
        }
        entries.insert(key, Value::Object(e));
    };
    for c in a.capabilities.values() {
        let item_hash = module
            .actions()
            .get(&c.action_name)
            .map(|x| hash_display(x.hash()));
        entry(
            format!("action\0{}", c.action_name),
            json!({"kind": "action", "name": c.action_name, "item_hash": item_hash}),
            a.capability_ref(&c.capability_id),
            vec![
                ("display", display_json(&c.display)),
                ("source", sources.json(&c.source)),
            ],
        );
    }
    for local in DEFINITIONS {
        let name = a.definition_name(local);
        let item_hash = module
            .derived_items()
            .get(&name)
            .map(|d| hash_display(d.hash()));
        entry(
            format!("derived\0{name}"),
            json!({"kind": "derived", "name": name, "item_hash": item_hash}),
            format!("{p}/{local}"),
            vec![],
        );
    }
    let mut checks = vec![
        ("terminal_guard", "precondition", TERMINAL_GUARD),
        ("uniqueness", "postcondition", UNIQUENESS),
    ];
    if a.reject_threshold.is_some() {
        checks.push((
            "threshold_consistency",
            "postcondition",
            THRESHOLD_CONSISTENCY,
        ));
    }
    for (key, kind, position) in checks {
        entry(
            format!("check\0{key}"),
            json!({"kind": kind, "actions": actions, "position": position}),
            format!("{p}/{key}"),
            vec![],
        );
    }
    entry(
        format!("entity\0{}", a.entity),
        json!({"kind": "entity", "name": a.entity}),
        format!("{p}/decision"),
        vec![("source", sources.json(&a.source))],
    );
    for (field, _) in &a.schema {
        entry(
            format!("field\0{}\0{field}", a.entity),
            json!({"kind": "field", "entity": a.entity, "field": field}),
            format!("{p}/decision/{field}"),
            vec![],
        );
    }
    entry(
        format!("enum\0{}", a.enum_name),
        json!({"kind": "enum", "name": a.enum_name}),
        format!("{p}/choice"),
        vec![],
    );
    json!({"entries": entries.into_values().collect::<Vec<_>>()})
}

/// The receipt of one supplied approval: the 501 receipt fields plus `generated`.
pub fn receipt_ap(
    a: &ResolvedApproval,
    module: &Module,
    root: Option<&str>,
) -> Result<LoweringReceipt, ModelError> {
    let document = canonical_document_ap(a, module)?;
    let hash = model_hash(&document)?;
    Ok(LoweringReceipt(json!({
        "format": "behavior.model_receipt.v1",
        "model_hash": hash,
        "model_kind": "approval",
        "model_semantic_version": APPROVAL_SEMANTIC_VERSION,
        "lowerer_version": APPROVAL_LOWERER_VERSION,
        "core_release": {"version": CORE_RELEASE.version, "commit": CORE_RELEASE.commit},
        "behavior_hash": module.behavior_version(),
        "generated": generated(a, module)?,
        "model": document,
        "source_map": source_map_ap(a, module, root),
    })))
}
