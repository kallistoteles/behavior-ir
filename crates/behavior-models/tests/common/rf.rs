//! RequestFlow fixtures (feature 502): the reference `purchase_request` flow and a Rust
//! fragment source that, like any binding, builds fragments only from the names in the scopes
//! the model crate opens.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, SemanticProfile};
use behavior_engine::wire::{WField, WType};
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{
    AnyModel, BindingError, BuildOutput, CreationFragments, CreationInput, CreationScope,
    DisplayMeta, FragmentSource, ModelError, RequestFlowInput, RequestFlowSource, RequestFlowSpec,
    TransitionFragments, TransitionScope, compose_models, declare_models,
};
use serde_json::json;

use super::{loc, transition};

pub const STATUSES: [&str; 7] = [
    "Draft",
    "Submitted",
    "InReview",
    "Approved",
    "Rejected",
    "NeedsChanges",
    "Cancelled",
];

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

/// P1: `PurchaseRequestStatus`, `User { name }`, `Budget { reserved }` (command profile).
pub fn authored() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum(
        "PurchaseRequestStatus",
        STATUSES.iter().map(|s| s.to_string()).collect(),
        loc(),
    )
    .unwrap();
    b.declare_entity("User", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("Budget", vec![field("reserved", WType::Int)], loc())
        .unwrap();
    b
}

/// P2b: `Notify { request: Id<PurchaseRequest> }`, which names the generated entity.
pub fn commands(b: &mut Builder) {
    b.add_command(
        "Notify",
        vec![field("request", WType::Id("PurchaseRequest".into()))],
        loc(),
    )
    .unwrap();
}

fn creation(id: &str, to: &str) -> CreationInput {
    CreationInput {
        creation_id: id.into(),
        to_state: to.into(),
        inputs: vec![
            ("title".into(), WType::String),
            ("amount".into(), WType::Int),
        ],
        state: vec![],
        context: vec![("actor".into(), WType::Id("User".into()))],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// The reference flow: two creations (Draft, Submitted), payload title + amount, assignee.
pub fn flow() -> RequestFlowInput {
    let mut start_review = transition("start_review", &["Submitted"], "InReview");
    start_review.inputs = vec![("reviewer".into(), WType::Id("User".into()))];
    RequestFlowInput {
        flow_id: "purchase_request".into(),
        actor_entity: "User".into(),
        status_enum: "PurchaseRequestStatus".into(),
        assignee: true,
        payload: vec![
            ("title".into(), WType::String),
            ("amount".into(), WType::Int),
        ],
        creations: vec![
            creation("create", "Draft"),
            creation("import_submitted", "Submitted"),
        ],
        transitions: vec![
            transition("submit", &["Draft", "NeedsChanges"], "Submitted"),
            start_review,
            transition("approve", &["InReview"], "Approved"),
            transition("reject", &["InReview"], "Rejected"),
            transition("return_for_changes", &["InReview"], "NeedsChanges"),
            transition(
                "cancel",
                &["Draft", "Submitted", "NeedsChanges"],
                "Cancelled",
            ),
        ],
        display: DisplayMeta::default(),
        source: None,
    }
}

pub type CreationFn =
    Box<dyn FnMut(&mut Builder, &CreationScope) -> Result<CreationFragments, BindingError>>;

/// A Rust fragment source: creations by id, transitions by id.
pub struct Source {
    pub creations: BTreeMap<String, CreationFn>,
    pub transitions: FragmentTable<'static>,
}

impl FragmentSource for Source {
    fn trace(
        &mut self,
        b: &mut Builder,
        s: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        self.transitions.trace(b, s)
    }
}

impl RequestFlowSource for Source {
    fn trace_creation(
        &mut self,
        b: &mut Builder,
        s: &CreationScope,
    ) -> Result<CreationFragments, BindingError> {
        match self.creations.get_mut(&s.creation_id) {
            Some(f) => f(b, s),
            None => Ok(CreationFragments::default()),
        }
    }
}

/// The parameter named `name` must exist in the scope (positional binding happens in bindings).
pub fn has(scope: &CreationScope, name: &str) -> String {
    assert!(
        scope.params.iter().any(|p| p.name == name),
        "{name} not in scope"
    );
    name.to_string()
}

/// The standard initializer: requester = actor, title, amount, assignee = none.
pub fn standard(b: &mut Builder, s: &CreationScope) -> Result<CreationFragments, BindingError> {
    let actor = b.param(&has(s, "actor"), loc()).unwrap();
    let title = b.param(&has(s, "title"), loc()).unwrap();
    let amount = b.param(&has(s, "amount"), loc()).unwrap();
    let none = b
        .lit(
            WType::Option(Box::new(WType::Id("User".into()))),
            json!(null),
            loc(),
        )
        .unwrap();
    Ok(CreationFragments {
        initializer: vec![
            ("requester".into(), actor),
            ("title".into(), title),
            ("amount".into(), amount),
            ("assignee".into(), none),
        ],
        ..CreationFragments::default()
    })
}

/// `start_review`: assignee = some(reviewer).
pub fn start_review(
    b: &mut Builder,
    s: &TransitionScope,
) -> Result<TransitionFragments, BindingError> {
    let subject = s.params[0].name.clone();
    let reviewer = b.param("reviewer", loc()).unwrap();
    let some = b.op("some", vec![reviewer], loc()).unwrap();
    Ok(TransitionFragments {
        effects: vec![(subject, "assignee".into(), some, loc())],
        ..TransitionFragments::default()
    })
}

pub fn source() -> Source {
    let mut creations: BTreeMap<String, CreationFn> = BTreeMap::new();
    creations.insert("create".into(), Box::new(standard));
    creations.insert("import_submitted".into(), Box::new(standard));
    let mut transitions: FragmentTable<'static> = BTreeMap::new();
    transitions.insert("start_review".into(), Box::new(start_review));
    Source {
        creations,
        transitions,
    }
}

pub fn spec(input: RequestFlowInput, source: Source) -> AnyModel<'static> {
    AnyModel::RequestFlow(RequestFlowSpec {
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
    build_with(|_| {}, vec![spec(flow(), source())]).unwrap_or_else(|e| panic!("{e:#?}"))
}

pub fn emit_notify(b: &mut Builder, id_param: &str) -> CommandEmissionSpec {
    let id = b.param(id_param, loc()).unwrap();
    CommandEmissionSpec {
        command: "Notify".into(),
        when: None,
        payload: BTreeMap::from([("request".to_string(), id)]),
        loc: loc(),
    }
}
