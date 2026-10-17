//! WorkQueue identity and provenance (research R6; data model §6).
//!
//! ```text
//! WorkQueueHash = tagged_hash("behavior.model.v1", CanonicalWorkQueueDocument)
//! ```
//!
//! The document holds the facts the author chose: `M`, the worker and time types and the payload
//! (each with its Core declaration identity, feature 505), and per capability the authored
//! parameters and bindings with their declaration identities and the Core semantic hash of the
//! admitted action, which covers the authored guards, effects, postconditions and commands as
//! well as the generated parts. The embedded machine's own canonical document and hash are
//! included, as RequestFlow does. Display metadata and source locations never enter it.

use std::collections::BTreeMap;

use behavior_engine::semantic::Module;
use behavior_engine::semantic::types::hash_display;
use serde_json::{Map, Value, json};

use crate::approval::resolve::definition_name;
use crate::error::ModelError;
use crate::identity::{canonical_document, entity_ref, identity_error, model_hash, typed_params};
use crate::source_map::{LoweringReceipt, RelativeSources, display_json};
use crate::work_queue::decls::{EXPIRED_LOCAL, READY_LOCAL, TERMINAL_LOCAL, VALID_WORK_LOCAL};
use crate::work_queue::resolve::{ResolvedWorkQueue, type_name};
use crate::{
    CORE_RELEASE, STATE_MACHINE_LOWERER_VERSION, WORK_QUEUE_LOWERER_VERSION,
    WORK_QUEUE_SEMANTIC_VERSION,
};

fn time_ref(q: &ResolvedWorkQueue, module: &Module) -> Result<Value, ModelError> {
    let name = type_name(&q.time);
    let hash = module
        .nominals()
        .get(&name)
        .map(|n| hash_display(&n.hash))
        .ok_or_else(|| identity_error(format!("nominal `{name}` not in module")))?;
    Ok(json!({"name": name, "hash": hash}))
}

/// The canonical document of a resolved, lowered and admitted work queue.
pub fn canonical_document_wq(q: &ResolvedWorkQueue, module: &Module) -> Result<Value, ModelError> {
    let id = q.queue_id.as_str();
    let mut capabilities = Vec::new();
    for c in q.capabilities.values() {
        let action = module
            .actions()
            .get(&c.action_name)
            .ok_or_else(|| identity_error(format!("action `{}` not in module", c.action_name)))?;
        let state: Vec<Value> = c
            .state
            .iter()
            .map(|(n, e)| {
                let mut r = entity_ref(module, id, &format!("state `{n}`"), e)?;
                r["name"] = json!(n);
                Ok(r)
            })
            .collect::<Result<_, ModelError>>()?;
        capabilities.push(json!({
            "id": c.capability_id.as_str(),
            "inputs": typed_params(&c.inputs, module, id)?,
            "context": typed_params(&c.context, module, id)?,
            "state": state,
            "action": hash_display(action.hash()),
        }));
    }
    let machine_doc = canonical_document(&q.machine, module)?;
    let machine_hash = model_hash(&machine_doc)?;
    Ok(json!({
        "format": "behavior.model.work_queue.v1",
        "model_kind": "work_queue",
        "model_semantic_version": WORK_QUEUE_SEMANTIC_VERSION,
        "queue_id": id,
        "worker": entity_ref(module, id, "worker", &q.worker)?,
        "time": time_ref(q, module)?,
        "max_attempts": q.max_attempts,
        "payload": typed_params(&q.payload, module, id)?,
        "capabilities": capabilities,
        "state_machine": {"model_hash": machine_hash, "document": machine_doc},
    }))
}

/// The names and hashes of what the queue generated (data model §6).
fn generated(q: &ResolvedWorkQueue, module: &Module) -> Result<Value, ModelError> {
    let enum_hash = module
        .enums()
        .get(&q.enum_name)
        .map(|e| hash_display(&e.hash))
        .ok_or_else(|| identity_error(format!("enum `{}` not in module", q.enum_name)))?;
    let entity = entity_ref(module, q.queue_id.as_str(), "work entity", &q.entity)?;
    let name = |local: &str| definition_name(&q.queue_id, local);
    let terminal = module
        .derived_items()
        .get(&name(TERMINAL_LOCAL))
        .map(|d| hash_display(d.hash()))
        .ok_or_else(|| identity_error("derived `terminal` not in module".into()))?;
    let invariant = module
        .invariants()
        .get(&name(VALID_WORK_LOCAL))
        .map(|i| hash_display(i.hash()))
        .ok_or_else(|| identity_error("invariant `valid_work` not in module".into()))?;
    let mut reads = Map::new();
    for local in [READY_LOCAL, EXPIRED_LOCAL] {
        let h = module
            .reads()
            .get(&name(local))
            .map(|r| hash_display(r.hash()))
            .ok_or_else(|| identity_error(format!("read `{local}` not in module")))?;
        reads.insert(local.to_string(), json!(h));
    }
    Ok(json!({
        "enum": {"name": q.enum_name, "hash": enum_hash},
        "entity": {"name": q.entity, "hash": entity["entity_hash"]},
        "derived": {TERMINAL_LOCAL: terminal},
        "invariant": {"name": name(VALID_WORK_LOCAL), "hash": invariant},
        "reads": reads,
    }))
}

/// The queue's source map: capabilities → actions, the generated declarations and items.
fn source_map_wq(q: &ResolvedWorkQueue, module: &Module, root: Option<&str>) -> Value {
    let sources = RelativeSources::from_sources(
        q.capabilities
            .values()
            .filter_map(|c| c.source.as_ref())
            .chain(q.source.as_ref()),
        root,
    );
    let id = q.queue_id.as_str();
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
    for c in q.capabilities.values() {
        let item_hash = module
            .actions()
            .get(&c.action_name)
            .map(|x| hash_display(x.hash()));
        let embedded = (c.capability_id.as_str() != "enqueue").then(|| {
            json!(format!(
                "state_machine:{}",
                q.machine.transition_ref(&c.capability_id)
            ))
        });
        entry(
            format!("action\0{}", c.action_name),
            json!({"kind": "action", "name": c.action_name, "item_hash": item_hash}),
            q.capability_ref(c.capability_id.as_str()),
            vec![
                ("embedded", embedded),
                ("display", display_json(&c.display)),
                ("source", sources.json(&c.source)),
            ],
        );
    }
    entry(
        format!("entity\0{}", q.entity),
        json!({"kind": "entity", "name": q.entity}),
        format!("{id}/work_entity"),
        vec![("source", sources.json(&q.source))],
    );
    for (field, _) in &q.schema {
        entry(
            format!("field\0{}\0{field}", q.entity),
            json!({"kind": "field", "entity": q.entity, "field": field}),
            format!("{id}/work_entity/{field}"),
            vec![],
        );
    }
    entry(
        format!("enum\0{}", q.enum_name),
        json!({"kind": "enum", "name": q.enum_name}),
        format!("{id}/status"),
        vec![],
    );
    for (kind, local) in [
        ("derived", TERMINAL_LOCAL),
        ("invariant", VALID_WORK_LOCAL),
        ("read", READY_LOCAL),
        ("read", EXPIRED_LOCAL),
    ] {
        let name = definition_name(&q.queue_id, local);
        entry(
            format!("{kind}\0{name}"),
            json!({"kind": kind, "name": name}),
            format!("{id}/{local}"),
            vec![],
        );
    }
    json!({"entries": entries.into_values().collect::<Vec<_>>()})
}

/// The receipt of one supplied work queue: the 501 receipt fields plus `embedded` and
/// `generated`.
pub fn receipt_wq(
    q: &ResolvedWorkQueue,
    module: &Module,
    root: Option<&str>,
) -> Result<LoweringReceipt, ModelError> {
    let document = canonical_document_wq(q, module)?;
    let hash = model_hash(&document)?;
    let machine_hash = document["state_machine"]["model_hash"].clone();
    Ok(LoweringReceipt(json!({
        "format": "behavior.model_receipt.v1",
        "model_hash": hash,
        "model_kind": "work_queue",
        "model_semantic_version": WORK_QUEUE_SEMANTIC_VERSION,
        "lowerer_version": WORK_QUEUE_LOWERER_VERSION,
        "core_release": {"version": CORE_RELEASE.version, "commit": CORE_RELEASE.commit},
        "behavior_hash": module.behavior_version(),
        "embedded": [{
            "kind": "state_machine",
            "id": q.queue_id.as_str(),
            "model_semantic_version": q.machine.semantic_version(),
            "lowerer_version": STATE_MACHINE_LOWERER_VERSION,
            "model_hash": machine_hash,
        }],
        "generated": generated(q, module)?,
        "model": document,
        "source_map": source_map_wq(q, module, root),
    })))
}
