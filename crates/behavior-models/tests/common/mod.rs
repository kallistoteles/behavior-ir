//! Shared fixtures (feature 501, T014): the `purchase_request` reference declarations, its
//! machine input, and a Rust fragment source that, like any binding, builds fragments only from
//! the names in the scope the model crate opened.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)] // shared test helpers

pub mod rf;

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, ScopeSite, SemanticProfile};
use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, Role, WField, WLifecycle, WParam, WType};
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{
    BuildOutput, DisplayMeta, ModelError, ModelInput, ModelSpec, NoFragments, TransitionFragments,
    TransitionInput, TransitionScope, compose,
};
use serde_json::json;

pub fn loc() -> Loc {
    Loc {
        file: "fixture.rs".into(),
        line: 1,
    }
}

pub fn status_ty() -> WType {
    WType::Enum("RequestStatus".into())
}

pub const CASES: [&str; 5] = ["Draft", "Submitted", "InReview", "Approved", "Rejected"];

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

/// `RequestStatus`, `Actor`, `PurchaseRequest { status, approver, approved_by, note, review }`
/// (`review` is a second `RequestStatus` field, for a second machine on the same entity) and the
/// command `Notify { approver }`, under the command profile.
pub fn declarations() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum(
        "RequestStatus",
        CASES.iter().map(|c| c.to_string()).collect(),
        loc(),
    )
    .unwrap();
    b.declare_entity("Actor", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity(
        "PurchaseRequest",
        vec![
            field("status", status_ty()),
            field("approver", WType::Id("Actor".into())),
            field(
                "approved_by",
                WType::Option(Box::new(WType::Id("Actor".into()))),
            ),
            field("note", WType::String),
            field("review", status_ty()),
        ],
        loc(),
    )
    .unwrap();
    b.add_command(
        "Notify",
        vec![field("approver", WType::Id("Actor".into()))],
        loc(),
    )
    .unwrap();
    b
}

pub fn transition(id: &str, from: &[&str], to: &str) -> TransitionInput {
    TransitionInput {
        transition_id: id.into(),
        from_states: from.iter().map(|s| s.to_string()).collect(),
        to_state: to.into(),
        inputs: vec![],
        context: vec![],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// The reference machine: Draft → Submitted → InReview → {Approved, Rejected}, entry Draft,
/// `approve` with the context parameter `actor: Id<Actor>`.
pub fn machine() -> ModelInput {
    let mut approve = transition("approve", &["InReview"], "Approved");
    approve.context = vec![("actor".into(), WType::Id("Actor".into()))];
    ModelInput {
        model_id: "purchase_request".into(),
        subject_entity: "PurchaseRequest".into(),
        state_field: "status".into(),
        subject_param: None,
        entry_states: vec!["Draft".into()],
        transitions: vec![
            transition("submit", &["Draft"], "Submitted"),
            transition("start_review", &["Submitted"], "InReview"),
            approve,
            transition("reject", &["InReview"], "Rejected"),
        ],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// The subject parameter name of a scope (its first parameter).
pub fn subject(scope: &TransitionScope) -> String {
    scope.params[0].name.clone()
}

/// `approve`: guard `actor == subject.approver`, effect `approved_by = some(actor)`, command
/// `Notify { approver: actor }`. Names come only from the scope.
pub fn approve_fragments(
    b: &mut Builder,
    scope: &TransitionScope,
) -> Result<TransitionFragments, behavior_models::BindingError> {
    let s = subject(scope);
    let actor = scope.params[1].name.clone();
    let a = b.param(&actor, loc()).unwrap();
    let approver = b.field(&s, "approver", loc()).unwrap();
    let guard = b.op("eq", vec![a.clone(), approver], loc()).unwrap();
    let some = b.op("some", vec![a.clone()], loc()).unwrap();
    Ok(TransitionFragments {
        guards: vec![(guard, loc())],
        effects: vec![(s, "approved_by".into(), some, loc())],
        postconditions: vec![],
        commands: vec![CommandEmissionSpec {
            command: "Notify".into(),
            when: None,
            payload: BTreeMap::from([("approver".to_string(), a)]),
            loc: loc(),
        }],
    })
}

pub fn fragments() -> FragmentTable<'static> {
    let mut t: FragmentTable<'static> = BTreeMap::new();
    t.insert("approve".into(), Box::new(approve_fragments));
    t
}

pub fn spec(input: ModelInput) -> ModelSpec<'static> {
    ModelSpec {
        input,
        fragments: Box::new(fragments()),
    }
}

pub fn spec_without_fragments(input: ModelInput) -> ModelSpec<'static> {
    ModelSpec {
        input,
        fragments: Box::new(NoFragments),
    }
}

/// Builds the reference module: declarations, then `extra` hand-written behavior, then models.
pub fn build(
    extra: impl FnOnce(&mut Builder),
    models: Vec<ModelSpec<'static>>,
) -> Result<BuildOutput, Vec<ModelError>> {
    let mut b = declarations();
    extra(&mut b);
    compose(&mut b, models, None)
}

pub fn build_reference() -> BuildOutput {
    build(|_| {}, vec![spec(machine())]).unwrap_or_else(|e| panic!("{e:#?}"))
}

pub fn codes(errors: &[ModelError]) -> Vec<&'static str> {
    errors.iter().map(ModelError::code).collect()
}

pub fn state_param(name: &str, entity: &str) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::State),
        ty: WType::Entity(entity.into()),
    }
}

/// A hand-written action `name(request: state PurchaseRequest)` that sets `field := case`.
pub fn hand_written_set(
    b: &mut Builder,
    name: &str,
    field: &str,
    value: serde_json::Value,
    ty: WType,
) {
    let p = state_param("request", "PurchaseRequest");
    b.push_scope(ScopeSite::Action, vec![p.clone()], loc())
        .unwrap();
    let v = b.lit(ty, value, loc()).unwrap();
    b.add_action(
        name,
        vec![p],
        vec![],
        vec![("request".into(), field.into(), v, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// A hand-written creation of a `PurchaseRequest` whose status comes from `status`, built in the
/// creating action's scope (`id: Input Id<PurchaseRequest>`, `approver: Input Id<Actor>`,
/// `initial: Input RequestStatus`).
pub fn hand_written_create(
    b: &mut Builder,
    name: &str,
    status: impl FnOnce(&mut Builder) -> behavior_engine::builder::Node,
) {
    let ps = vec![
        WParam {
            name: "id".into(),
            role: Some(Role::Input),
            ty: WType::Id("PurchaseRequest".into()),
        },
        WParam {
            name: "approver".into(),
            role: Some(Role::Input),
            ty: WType::Id("Actor".into()),
        },
        WParam {
            name: "initial".into(),
            role: Some(Role::Input),
            ty: status_ty(),
        },
    ];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let id = b.param("id", loc()).unwrap();
    let approver = b.param("approver", loc()).unwrap();
    let st = status(b);
    let none = b
        .lit(
            WType::Option(Box::new(WType::Id("Actor".into()))),
            json!(null),
            loc(),
        )
        .unwrap();
    let note = b.lit(WType::String, json!(""), loc()).unwrap();
    let review = b.lit(status_ty(), json!("Draft"), loc()).unwrap();
    let create = WLifecycle::Create {
        entity: "PurchaseRequest".into(),
        id: id.into_wire(),
        fields: vec![
            ("status".into(), st.into_wire()),
            ("approver".into(), approver.into_wire()),
            ("approved_by".into(), none.into_wire()),
            ("note".into(), note.into_wire()),
            ("review".into(), review.into_wire()),
        ],
        loc: loc(),
    };
    b.add_action_with_commands(
        name,
        ps,
        vec![],
        vec![],
        vec![create],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// The names of every item in an admitted module's name table.
pub fn item_names(module: &Module) -> Vec<String> {
    module
        .name_table()
        .keys()
        .map(|(_, name)| name.clone())
        .collect()
}

/// Adds `Invoice { status: RequestStatus }`.
pub fn with_invoice(b: &mut Builder) {
    b.declare_entity(
        "Invoice",
        vec![WField {
            name: "status".into(),
            ty: status_ty(),
            loc: loc(),
        }],
        loc(),
    )
    .unwrap();
}

pub fn invoice_machine() -> ModelInput {
    ModelInput {
        model_id: "invoice".into(),
        subject_entity: "Invoice".into(),
        transitions: vec![transition("submit", &["Draft"], "Submitted")],
        ..machine()
    }
}
