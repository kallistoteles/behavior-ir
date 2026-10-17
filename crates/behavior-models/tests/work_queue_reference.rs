//! Feature 507, W20 (authoring conformance), the Rust half: the reference document-processing
//! module (`models/examples/work_queue/workflow.py` describes the same module) authored
//! independently with the Rust model authority: a RequestFlow `document_request` and the
//! WorkQueue `document_work`.
//!
//! The authoring identity (BehaviorHash, item hashes, model hashes and versions, generated
//! hashes; no source locations) is written as canonical JSON to
//! `$BEHAVIOR_CONFORMANCE_DIR/w20-rust.json` (default: the workspace's `target/conformance/`),
//! where the Python half compares it with its own.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node, SemanticProfile};
use behavior_engine::wire::{WField, WType};
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::work_queue::input::{
    CapabilityFragments, CapabilityInput, CapabilityScope, WorkQueueInput, WorkQueueSource,
    WorkQueueSpec,
};
use behavior_models::{
    AnyModel, BindingError, BuildOutput, CreationFragments, CreationInput, CreationScope,
    DisplayMeta, FragmentSource, RequestFlowInput, RequestFlowSource, RequestFlowSpec,
    TransitionFragments, TransitionInput, TransitionScope, compose_models, declare_models,
};
use serde_json::{Value, json};

use common::loc;

const REQUEST: &str = "DocumentRequest";

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

fn id(entity: &str) -> WType {
    WType::Id(entity.into())
}

fn actor() -> Vec<(String, WType)> {
    vec![("actor".into(), id("User"))]
}

/// P1: `Time`, `Worker { name, active }`, `User { name, admin }`, `Document { title, processed }`,
/// `RequestStatus { Open, Submitted }`.
fn authored() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_nominal("Time", WType::Int, vec!["order".into()], None, loc())
        .unwrap();
    b.declare_enum(
        "RequestStatus",
        vec!["Open".into(), "Submitted".into()],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Worker",
        vec![field("name", WType::String), field("active", WType::Bool)],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "User",
        vec![field("name", WType::String), field("admin", WType::Bool)],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Document",
        vec![
            field("title", WType::String),
            field("processed", WType::Bool),
        ],
        loc(),
    )
    .unwrap();
    b
}

// --- the RequestFlow `document_request` -----------------------------------------------------

fn flow_input() -> RequestFlowInput {
    RequestFlowInput {
        flow_id: "document_request".into(),
        actor_entity: "User".into(),
        status_enum: "RequestStatus".into(),
        assignee: false,
        payload: vec![("document".into(), id("Document"))],
        creations: vec![CreationInput {
            creation_id: "open".into(),
            to_state: "Open".into(),
            inputs: vec![("document".into(), id("Document"))],
            state: vec![],
            context: actor(),
            display: DisplayMeta::default(),
            source: None,
        }],
        transitions: vec![TransitionInput {
            transition_id: "submit".into(),
            from_states: vec!["Open".into()],
            to_state: "Submitted".into(),
            inputs: vec![],
            context: actor(),
            state: vec![],
            display: DisplayMeta::default(),
            source: None,
        }],
        display: DisplayMeta::default(),
        source: None,
    }
}

struct FlowSource(FragmentTable<'static>);

impl FragmentSource for FlowSource {
    fn trace(
        &mut self,
        b: &mut Builder,
        s: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        self.0.trace(b, s)
    }
}

impl RequestFlowSource for FlowSource {
    fn trace_creation(
        &mut self,
        b: &mut Builder,
        _: &CreationScope,
    ) -> Result<CreationFragments, BindingError> {
        Ok(CreationFragments {
            initializer: vec![
                ("requester".into(), b.param("actor", loc()).unwrap()),
                ("document".into(), b.param("document", loc()).unwrap()),
            ],
            ..CreationFragments::default()
        })
    }
}

fn flow() -> AnyModel<'static> {
    let mut t: FragmentTable<'static> = BTreeMap::new();
    // submit: the requester only; emits ProcessDocument { request, document }.
    t.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, s: &TransitionScope| {
            let r = s.params[0].name.clone();
            let actor = b.param("actor", loc()).unwrap();
            let requester = b.field(&r, "requester", loc()).unwrap();
            let guard = b.op("eq", vec![actor, requester], loc()).unwrap();
            let request = b.field(&r, "id", loc()).unwrap();
            let document = b.field(&r, "document", loc()).unwrap();
            Ok(TransitionFragments {
                guards: vec![(guard, loc())],
                commands: vec![CommandEmissionSpec {
                    command: "ProcessDocument".into(),
                    when: None,
                    payload: BTreeMap::from([
                        ("document".to_string(), document),
                        ("request".to_string(), request),
                    ]),
                    loc: loc(),
                }],
                ..TransitionFragments::default()
            })
        }),
    );
    AnyModel::RequestFlow(RequestFlowSpec {
        input: flow_input(),
        fragments: Box::new(FlowSource(t)),
    })
}

// --- the WorkQueue `document_work` -----------------------------------------------------------

fn capability(
    id_: &str,
    context: Vec<(String, WType)>,
    state: Vec<(String, String)>,
) -> CapabilityInput {
    CapabilityInput {
        capability_id: id_.into(),
        inputs: vec![],
        context,
        state,
        display: DisplayMeta::default(),
        source: None,
    }
}

fn queue_input() -> WorkQueueInput {
    WorkQueueInput {
        queue_id: "document_work".into(),
        worker_entity: "Worker".into(),
        time: WType::Nominal("Time".into()),
        payload: vec![
            ("document".into(), id("Document")),
            ("request".into(), id(REQUEST)),
        ],
        max_attempts: 3,
        capabilities: vec![
            capability("enqueue", actor(), vec![]),
            capability("claim", vec![], vec![]),
            capability("renew", vec![], vec![]),
            capability("retry", vec![], vec![]),
            capability("fail", vec![], vec![]),
            capability("recover", actor(), vec![]),
            capability("exhaust", actor(), vec![]),
            capability("cancel", actor(), vec![]),
            capability("complete", vec![], vec![("doc".into(), "Document".into())]),
        ],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// `any(select(entity), m → m.id == param ∧ m.flag)`
fn some_member(b: &mut Builder, entity: &str, member: &str, param: &str, flag: &str) -> Node {
    let all = b.select(entity, loc()).unwrap();
    b.push_lambda(member, entity).unwrap();
    let mid = b.field(member, "id", loc()).unwrap();
    let who = b.param(param, loc()).unwrap();
    let same = b.op("eq", vec![mid, who], loc()).unwrap();
    let f = b.field(member, flag, loc()).unwrap();
    let both = b.op("and", vec![same, f], loc()).unwrap();
    b.pop_scope();
    b.lambda("any", all, member, both, loc()).unwrap()
}

struct QueueSource;

impl WorkQueueSource for QueueSource {
    fn trace_capability(
        &mut self,
        b: &mut Builder,
        s: &CapabilityScope,
    ) -> Result<CapabilityFragments, BindingError> {
        let mut f = CapabilityFragments::default();
        match s.capability_id.as_str() {
            "enqueue" | "recover" | "exhaust" | "cancel" => {
                f.authorization = Some((some_member(b, "User", "u", "actor", "admin"), loc()));
            }
            "claim" => {
                f.authorization = Some((some_member(b, "Worker", "x", "worker", "active"), loc()));
            }
            "complete" => {
                let w = s.params[0].name.clone();
                let doc_id = b.field("doc", "id", loc()).unwrap();
                let document = b.field(&w, "document", loc()).unwrap();
                f.guards = vec![(b.op("eq", vec![doc_id, document], loc()).unwrap(), loc())];
                let t = b.lit(WType::Bool, json!(true), loc()).unwrap();
                f.effects = vec![("doc".into(), "processed".into(), t, loc())];
                let doc = b.field("doc", "id", loc()).unwrap();
                f.commands = vec![CommandEmissionSpec {
                    command: "DocumentProcessed".into(),
                    when: None,
                    payload: BTreeMap::from([("document".to_string(), doc)]),
                    loc: loc(),
                }];
            }
            _ => {}
        }
        Ok(f)
    }
}

fn queue() -> AnyModel<'static> {
    AnyModel::WorkQueue(WorkQueueSpec {
        input: queue_input(),
        fragments: Box::new(QueueSource),
    })
}

/// The full protocol: P1 → P2 → P2b (commands name the flow's entity) → P4.
fn build() -> BuildOutput {
    let mut b = authored();
    let ms = vec![flow(), queue()];
    declare_models(&mut b, &ms).unwrap_or_else(|e| panic!("{e:#?}"));
    b.add_command(
        "ProcessDocument",
        vec![
            field("request", id(REQUEST)),
            field("document", id("Document")),
        ],
        loc(),
    )
    .unwrap();
    b.add_command(
        "DocumentProcessed",
        vec![field("document", id("Document"))],
        loc(),
    )
    .unwrap();
    compose_models(&mut b, ms, None).unwrap_or_else(|e| panic!("{e:#?}"))
}

/// The authoring identity (506 comparison.md §1, applied to 507).
fn authoring_identity(out: &BuildOutput) -> Value {
    let admission = behavior_engine::admission_result(&out.module);
    let mut models = serde_json::Map::new();
    let mut generated = serde_json::Map::new();
    for r in &out.receipts {
        let r = &r.0;
        let model = &r["model"];
        let id_ = ["model_id", "flow_id", "approval_id", "queue_id"]
            .iter()
            .find_map(|k| model[*k].as_str())
            .unwrap()
            .to_string();
        models.insert(
            id_.clone(),
            json!({
                "kind": r["model_kind"],
                "model_semantic_version": r["model_semantic_version"],
                "lowerer_version": r["lowerer_version"],
                "model_hash": r["model_hash"],
            }),
        );
        if let Some(g) = r.get("generated") {
            generated.insert(id_, g.clone());
        }
    }
    json!({
        "behavior_hash": out.module.behavior_version(),
        "items": admission.items,
        "models": models,
        "generated": generated,
    })
}

fn conformance_dir() -> std::path::PathBuf {
    std::env::var_os("BEHAVIOR_CONFORMANCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../target/conformance"
            ))
        })
}

#[test]
fn the_reference_queue_authored_in_rust_has_a_stable_identity() {
    let first = authoring_identity(&build());
    let second = authoring_identity(&build());
    assert_eq!(first, second, "two Rust builds differ");
    let dir = conformance_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("w20-rust.json"), format!("{first}\n")).unwrap();
}
