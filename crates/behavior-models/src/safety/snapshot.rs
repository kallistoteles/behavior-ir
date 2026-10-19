//! Exact-history state validation; no persistent state or runtime Safety registry.
//!
//! Only acquiring an export through Core can establish provenance. Detached data can
//! be inspected for diagnostics, but can never establish an actual Store's validity.
use super::{UncertifiedSafetyAnalysis, evidence::StateValidationRecord, identity::tagged};
use behavior_engine::{
    semantic::{Module, types::hash_display},
    store::{
        Backend, Store,
        documents::{HistoryRef, SnapshotExport},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn validate_store_snapshot<B: Backend>(
    module: &Module,
    analysis: &UncertifiedSafetyAnalysis,
    store: &Store<B>,
    at: &HistoryRef,
) -> StateValidationRecord {
    if analysis.inventory.verify_module(module).is_err() {
        return record(
            module,
            analysis,
            at,
            None,
            false,
            vec![json!({"code":"MODULE_ANALYSIS_MISMATCH"})],
        );
    }
    match store.export_seed_at(module, at) {
        Ok(export) => {
            let findings = inspect(module, analysis, &export);
            record(module, analysis, at, Some(&export), true, findings)
        }
        Err(e) => record(
            module,
            analysis,
            at,
            None,
            false,
            vec![json!({"code":e.code(),"detail":e.to_string()})],
        ),
    }
}
/// Diagnostic inspection only: neither a hash nor a caller flag establishes provenance.
pub fn inspect_snapshot(
    module: &Module,
    analysis: &UncertifiedSafetyAnalysis,
    export: &SnapshotExport,
) -> StateValidationRecord {
    let mut findings = inspect(module, analysis, export);
    findings.push(json!({"code":"DETACHED_EXPORT","detail":"Completeness and provenance not established by acquisition from Core Store"}));
    record(
        module,
        analysis,
        &export.source_history,
        Some(export),
        false,
        findings,
    )
}
fn inspect(module: &Module, a: &UncertifiedSafetyAnalysis, e: &SnapshotExport) -> Vec<Value> {
    let mut out = vec![];
    if a.inventory.verify_module(module).is_err() {
        out.push(json!({"code":"MODULE_ANALYSIS_MISMATCH"}));
    }
    if e.format != "behavior.snapshot_export.v1"
        || e.schema != behavior_engine::schema(module).hash
        || e.state != e.source_history.state
        || e.source_history.validate().is_err()
    {
        out.push(json!({"code":"INVALID_EXPORT_METADATA"}));
    }
    // Typed keys: equal textual IDs in distinct entity types never alias.
    let mut rows = BTreeMap::new();
    for row in &e.seed {
        let Some(id) = row
            .value
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            out.push(json!({"code":"INVALID_ENTITY_ID","entity":row.entity}));
            continue;
        };
        if !a.inventory.entities().contains_key(&row.entity) {
            out.push(json!({"code":"UNKNOWN_ENTITY","entity":row.entity}));
        }
        if rows.insert((row.entity.as_str(), id), &row.value).is_some() {
            out.push(json!({"code":"DUPLICATE_ENTITY","entity":row.entity,"id":id}));
        }
    }
    for d in &a.dependencies {
        for ((entity, id), value) in &rows {
            if *entity == d.parent
                && !value
                    .get(&d.parent_state)
                    .and_then(Value::as_str)
                    .is_some_and(|v| d.parent_literals.contains_key(v))
            {
                out.push(
                    json!({"code":"INVALID_PARENT_STATUS","dependency":d.model_hash(),"id":id}),
                );
            }
            if *entity != d.child {
                continue;
            }
            let status = value.get(&d.child_state).and_then(Value::as_str);
            if !status.is_some_and(|v| d.child_literals.contains_key(v)) {
                out.push(
                    json!({"code":"INVALID_CHILD_STATUS","dependency":d.model_hash(),"id":id}),
                );
                continue;
            }
            let Some(owner) = value.get(&d.owner_field).and_then(Value::as_str) else {
                out.push(json!({"code":"INVALID_OWNER","dependency":d.model_hash(),"id":id}));
                continue;
            };
            let Some(parent) = rows.get(&(d.parent.as_str(), owner)) else {
                out.push(json!({"code":"MISSING_PARENT","dependency":d.model_hash(),"id":id,"owner":owner}));
                continue;
            };
            let parent_status = parent.get(&d.parent_state).and_then(Value::as_str);
            if !status.is_some_and(|s| d.terminal.contains(s))
                && parent_status.is_some_and(|s| d.prohibited.contains(s))
            {
                out.push(json!({"code":"NONTERMINAL_CHILD_PROHIBITED_PARENT","dependency":d.model_hash(),"id":id,"owner":owner}));
            }
        }
    }
    out.sort_by_cached_key(|v| v.to_string());
    out
}
fn record(
    module: &Module,
    a: &UncertifiedSafetyAnalysis,
    at: &HistoryRef,
    export: Option<&SnapshotExport>,
    acquired: bool,
    findings: Vec<Value>,
) -> StateValidationRecord {
    let status = if !acquired || (a.dependencies.is_empty() && a.policies.is_empty()) {
        "Unestablished"
    } else if findings.is_empty() {
        "Valid"
    } else {
        "Invalid"
    };
    let export_hash =
        export.and_then(|e| tagged("behavior.safety_snapshot_export.v1", &json!(e)).ok());
    let validator_identity = super::evidence::dependency_identity()
        .and_then(|dependencies| {
            tagged(
                "behavior.safety_state_validator.v1",
                &json!({"source":include_str!("snapshot.rs"),
            "records":include_str!("evidence.rs"),"dependencies":dependencies}),
            )
        })
        .ok();
    let status = if acquired && (export_hash.is_none() || validator_identity.is_none()) {
        "Unestablished"
    } else {
        status
    };
    let mut properties=a.dependencies.iter().map(|d|json!({"model_hash":d.model_hash(),"property":"R: every child has an existing parent; P: every nonterminal child has a nonprohibited parent","configuration":d.document(),"status":if !acquired {"Unestablished"} else if findings.iter().any(|v|v["dependency"]==d.model_hash()){ "Invalid" }else if findings.is_empty(){"Valid"}else{"Unestablished"}})).collect::<Vec<_>>();
    properties.sort_by_cached_key(|v| v["model_hash"].as_str().unwrap_or("").to_owned());
    StateValidationRecord::new(
        json!({"format":"behavior.safety_state_validation.v1","validator_version":1,
        "status":status,"behavior_hash":hash_display(module.hash()),"policy_set_hash":a.policy_set_hash(),
        "properties":properties,"validator_identity":validator_identity,"history":at,"schema":export.map(|e|e.schema.as_str()),"state":export.map(|e|e.state.as_str()),
        "export_hash":export_hash,"acquisition":if acquired{"CoreStoreExport"}else{"Unestablished"},
        "initial_validity_scope":"OnlyThisHistoryAnchor","findings":findings,
        "evidence_class":"Executable original-property validation, not a universal proof",
        "assumptions":["Core export validates the exact full history, schema, content identity and entity types",
            "Backend provides complete consistent keys and versions; snapshot provenance is trusted",
            "Rust validator faithfully evaluates the original R and P predicates"],
        "exclusions":["Other history positions","Transition preservation certificate","Host module authorization","Migration preservation","Atomic activation"]}),
    )
}
