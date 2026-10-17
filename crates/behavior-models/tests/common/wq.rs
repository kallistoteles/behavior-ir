//! WorkQueue fixtures (feature 507): `document_work` over authored `Worker`, `Document` and
//! `User`, time `Time` (an Int nominal with `order`), and a Rust capability source that, like any
//! binding, builds fragments only from the names in the scope the model crate opens.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, SemanticProfile};
use behavior_engine::wire::{WField, WType};
use behavior_models::work_queue::input::{
    CapabilityFragments, CapabilityInput, CapabilityScope, WorkQueueInput, WorkQueueSource,
    WorkQueueSpec,
};
use behavior_models::{AnyModel, BindingError, BuildOutput, DisplayMeta, ModelError};
use behavior_models::{compose_models, declare_models};
use serde_json::json;

use super::loc;

pub const CAPABILITIES: [&str; 9] = [
    "enqueue", "claim", "renew", "complete", "retry", "fail", "recover", "exhaust", "cancel",
];
pub const NEEDS_AUTH: [&str; 5] = ["enqueue", "claim", "recover", "exhaust", "cancel"];

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

pub fn time() -> WType {
    WType::Nominal("Time".into())
}

/// P1: `Time` (Int, order), `Worker { name }`, `User { name }`, `Document { processed }`.
pub fn authored() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_nominal("Time", WType::Int, vec!["order".into()], None, loc())
        .unwrap();
    b.declare_nominal("Plain", WType::Int, vec![], None, loc())
        .unwrap();
    b.declare_entity("Worker", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("User", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("Document", vec![field("processed", WType::Bool)], loc())
        .unwrap();
    b
}

/// P2b: `DocumentProcessed { document: Id<Document> }`.
pub fn commands(b: &mut Builder) {
    b.add_command(
        "DocumentProcessed",
        vec![field("document", WType::Id("Document".into()))],
        loc(),
    )
    .unwrap();
}

pub fn capability(id: &str) -> CapabilityInput {
    CapabilityInput {
        capability_id: id.into(),
        inputs: vec![],
        context: vec![],
        state: vec![],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// `document_work`: payload `{document: Id<Document>}`, `M`, all nine capabilities.
pub fn queue(m: i64) -> WorkQueueInput {
    WorkQueueInput {
        queue_id: "document_work".into(),
        worker_entity: "Worker".into(),
        time: time(),
        payload: vec![("document".into(), WType::Id("Document".into()))],
        max_attempts: m,
        capabilities: CAPABILITIES.iter().map(|c| capability(c)).collect(),
        display: DisplayMeta::default(),
        source: None,
    }
}

pub type CapFn =
    Box<dyn FnMut(&mut Builder, &CapabilityScope) -> Result<CapabilityFragments, BindingError>>;

/// Authorization `true` for every capability that needs one, plus per-capability extras.
pub struct Source {
    pub authorize: bool,
    pub extra: BTreeMap<String, CapFn>,
}

impl WorkQueueSource for Source {
    fn trace_capability(
        &mut self,
        b: &mut Builder,
        s: &CapabilityScope,
    ) -> Result<CapabilityFragments, BindingError> {
        let mut f = match self.extra.get_mut(&s.capability_id) {
            Some(f) => f(b, s)?,
            None => CapabilityFragments::default(),
        };
        if self.authorize
            && f.authorization.is_none()
            && NEEDS_AUTH.contains(&s.capability_id.as_str())
        {
            let t = b.lit(WType::Bool, json!(true), loc()).unwrap();
            f.authorization = Some((t, loc()));
        }
        Ok(f)
    }
}

pub fn source() -> Source {
    Source {
        authorize: true,
        extra: BTreeMap::new(),
    }
}

pub fn spec(input: WorkQueueInput, source: Source) -> AnyModel<'static> {
    AnyModel::WorkQueue(WorkQueueSpec {
        input,
        fragments: Box::new(source),
    })
}

/// Full protocol: P1 authored → P2 declare_models → P2b commands → P3 `extra` → P4 compose.
pub fn build_with(
    extra: impl FnOnce(&mut Builder),
    models: Vec<AnyModel<'static>>,
) -> Result<BuildOutput, Vec<ModelError>> {
    let mut b = authored();
    declare_models(&mut b, &models)?;
    commands(&mut b);
    extra(&mut b);
    compose_models(&mut b, models, None)
}

pub fn build_reference() -> BuildOutput {
    build_with(|_| {}, vec![spec(queue(3), source())]).unwrap_or_else(|e| panic!("{e:#?}"))
}
