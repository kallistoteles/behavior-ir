//! RequestFlow identity and provenance (data model §7–§8, research R9).
//!
//! ```text
//! RequestFlowHash = tagged_hash("behavior.model.v1", CanonicalRequestFlowDocument)
//! ```
//!
//! The document references the status enum by its Core hash and embeds the StateMachine's own
//! canonical document and ModelHash: no second encoding of transitions, no copy of enum cases.
//! The entity name and schema are derived from the hashed facts and are not hashed separately.
//! One receipt per supplied flow; the embedded machine appears as an identity record, never as a
//! receipt of its own.

use std::collections::BTreeMap;

use behavior_engine::semantic::Module;
use behavior_engine::semantic::types::hash_display;
use serde_json::{Map, Value, json};

use crate::error::ModelError;
use crate::identity::{canonical_document, entity_ref, identity_error, model_hash, typed_params};
use crate::request_flow::resolve::{ResolvedRequestFlow, STATUS};
use crate::source_map::{LoweringReceipt, RelativeSources, display_json};
use crate::{
    CORE_RELEASE, REQUEST_FLOW_BINDINGS_SEMANTIC_VERSION, REQUEST_FLOW_LOWERER_VERSION,
    REQUEST_FLOW_SEMANTIC_VERSION, STATE_MACHINE_LOWERER_VERSION,
};

/// The flow's language level (feature 507, research R13): 3 when its embedded machine is level
/// 3 (a transition binds further entities), otherwise the established level 2.
pub fn flow_version(f: &ResolvedRequestFlow) -> u32 {
    if f.machine.semantic_version() == crate::STATE_MACHINE_BINDINGS_SEMANTIC_VERSION {
        REQUEST_FLOW_BINDINGS_SEMANTIC_VERSION
    } else {
        REQUEST_FLOW_SEMANTIC_VERSION
    }
}

/// The canonical document of a resolved, lowered and admitted flow.
pub fn canonical_document_rf(
    f: &ResolvedRequestFlow,
    module: &Module,
) -> Result<Value, ModelError> {
    let version = flow_version(f);
    let enum_hash = module
        .enums()
        .get(&f.status_enum)
        .map(|e| hash_display(&e.hash))
        .ok_or_else(|| identity_error(format!("enum `{}` not in module", f.status_enum)))?;
    let mut creations = Vec::new();
    for c in f.creations.values() {
        let action = module
            .actions()
            .get(&c.action_name)
            .ok_or_else(|| identity_error(format!("action `{}` not in module", c.action_name)))?;
        let created = action
            .creates()
            .first()
            .ok_or_else(|| identity_error(format!("action `{}` creates nothing", c.action_name)))?;
        // Creation lowering generates no precondition, effect or postcondition: all are the
        // author's (data model §5). The initializer's values come from the creation, minus the
        // generated status.
        let initializer: BTreeMap<&str, String> = created
            .fields()
            .iter()
            .filter(|(n, _)| n != STATUS)
            .map(|(n, e)| (n.as_str(), hash_display(e.hash())))
            .collect();
        let state: Vec<Value> = c
            .state
            .iter()
            .map(|(n, e)| {
                let mut r = entity_ref(module, f.flow_id.as_str(), &format!("state `{n}`"), e)?;
                r["name"] = json!(n);
                Ok(r)
            })
            .collect::<Result<_, ModelError>>()?;
        creations.push(json!({
            "id": c.creation_id.as_str(),
            "to": c.to_state,
            "inputs": typed_params(&c.inputs, module, f.flow_id.as_str())?,
            "state": state,
            "context": typed_params(&c.context, module, f.flow_id.as_str())?,
            "initializer": initializer,
            "guards": action.preconditions().iter().map(|p| hash_display(p.expr().hash())).collect::<Vec<_>>(),
            "effects": action.effects().iter().map(|e| hash_display(e.hash())).collect::<Vec<_>>(),
            "postconditions": action.postconditions().iter().map(|p| hash_display(p.expr().hash())).collect::<Vec<_>>(),
            "commands": action.command_emissions().iter().map(|k| hash_display(k.hash())).collect::<Vec<_>>(),
        }));
    }
    let machine_doc = canonical_document(&f.machine, module)?;
    let machine_hash = model_hash(&machine_doc)?;
    Ok(json!({
        "format": if version == REQUEST_FLOW_BINDINGS_SEMANTIC_VERSION {
            "behavior.model.request_flow.v3"
        } else {
            "behavior.model.request_flow.v2"
        },
        "model_kind": "request_flow",
        "model_semantic_version": version,
        "flow_id": f.flow_id.as_str(),
        "actor": entity_ref(module, f.flow_id.as_str(), "actor", &f.actor)?,
        "status_enum": {"name": f.status_enum, "enum_hash": enum_hash},
        "assignee": f.assignee,
        "payload": typed_params(&f.payload, module, f.flow_id.as_str())?,
        "creations": creations,
        "state_machine": {"model_hash": machine_hash, "document": machine_doc},
    }))
}

/// The flow's source map (research R9): creations, transitions (with their embedded origin), the
/// generated entity and its fields. Locations are relative (Constitution IV).
fn source_map_rf(f: &ResolvedRequestFlow, module: &Module, root: Option<&str>) -> Value {
    let sources = RelativeSources::from_sources(
        f.creations
            .values()
            .filter_map(|c| c.source.as_ref())
            .chain(
                f.machine
                    .transitions
                    .values()
                    .filter_map(|t| t.source.as_ref()),
            )
            .chain(f.source.as_ref()),
        root,
    );
    let item_hash = |name: &str| module.actions().get(name).map(|a| hash_display(a.hash()));
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
    for c in f.creations.values() {
        entry(
            format!("action\0{}", c.action_name),
            json!({"kind": "action", "name": c.action_name, "item_hash": item_hash(&c.action_name)}),
            f.creation_ref(&c.creation_id),
            vec![
                ("display", display_json(&c.display)),
                ("source", sources.json(&c.source)),
            ],
        );
    }
    for t in f.machine.transitions.values() {
        entry(
            format!("action\0{}", t.action_name),
            json!({"kind": "action", "name": t.action_name, "item_hash": item_hash(&t.action_name)}),
            format!("{}/transition/{}", f.flow_id, t.transition_id),
            vec![
                (
                    "embedded",
                    Some(json!(format!(
                        "state_machine:{}",
                        f.machine.transition_ref(&t.transition_id)
                    ))),
                ),
                ("display", display_json(&t.display)),
                ("source", sources.json(&t.source)),
            ],
        );
    }
    entry(
        format!("entity\0{}", f.entity),
        json!({"kind": "entity", "name": f.entity}),
        format!("{}/request_entity", f.flow_id),
        vec![("source", sources.json(&f.source))],
    );
    for (field, _) in &f.schema {
        entry(
            format!("field\0{}\0{field}", f.entity),
            json!({"kind": "field", "entity": f.entity, "field": field}),
            format!("{}/request_entity/{field}", f.flow_id),
            vec![],
        );
    }
    json!({"entries": entries.into_values().collect::<Vec<_>>()})
}

/// The receipt of one supplied flow.
pub fn receipt_rf(
    f: &ResolvedRequestFlow,
    module: &Module,
    root: Option<&str>,
) -> Result<LoweringReceipt, ModelError> {
    let document = canonical_document_rf(f, module)?;
    let hash = model_hash(&document)?;
    let machine_hash = document["state_machine"]["model_hash"].clone();
    Ok(LoweringReceipt(json!({
        "format": "behavior.model_receipt.v1",
        "model_hash": hash,
        "model_kind": "request_flow",
        "model_semantic_version": flow_version(f),
        "lowerer_version": REQUEST_FLOW_LOWERER_VERSION,
        "core_release": {"version": CORE_RELEASE.version, "commit": CORE_RELEASE.commit},
        "behavior_hash": module.behavior_version(),
        "embedded": [{
            "kind": "state_machine",
            "id": f.flow_id.as_str(),
            "model_semantic_version": f.machine.semantic_version(),
            "lowerer_version": STATE_MACHINE_LOWERER_VERSION,
            "model_hash": machine_hash,
        }],
        "model": document,
        "source_map": source_map_rf(f, module, root),
    })))
}
