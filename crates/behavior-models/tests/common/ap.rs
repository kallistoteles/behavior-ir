//! Approval fixtures (feature 503): `doc_approval` over an authored `Document` with actor
//! `User`, and a Rust decision source that, like any binding, builds fragments only from the
//! names in the scope the model crate opens.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, SemanticProfile};
use behavior_engine::wire::{WField, WType};
use behavior_models::{
    AnyModel, ApprovalInput, ApprovalSource, ApprovalSpec, BindingError, BuildOutput,
    CapabilityInput, DecisionFragments, DecisionScope, DisplayMeta, ModelError, compose_models,
    declare_models,
};

use super::loc;

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

/// P1: `User { name }`, `Document { title }` (command profile).
pub fn authored() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_entity("User", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("Document", vec![field("title", WType::String)], loc())
        .unwrap();
    b
}

/// P2b: `Notify { document: Id<Document> }`.
pub fn commands(b: &mut Builder) {
    b.add_command(
        "Notify",
        vec![field("document", WType::Id("Document".into()))],
        loc(),
    )
    .unwrap();
}

/// A capability with context `actor: Id<User>` and no inputs.
pub fn capability(id: &str) -> CapabilityInput {
    CapabilityInput {
        capability_id: id.into(),
        inputs: vec![],
        context: vec![("actor".into(), WType::Id("User".into()))],
        display: DisplayMeta::default(),
        source: None,
    }
}

/// `doc_approval` over `Document`, actor `User`, thresholds `(k_a, k_r)`, no payload.
pub fn approval(k_a: i64, k_r: Option<i64>) -> ApprovalInput {
    ApprovalInput {
        approval_id: "doc_approval".into(),
        subject_entity: "Document".into(),
        actor_entity: "User".into(),
        approve_threshold: k_a,
        reject_threshold: k_r,
        payload: vec![],
        capabilities: vec![capability("approve"), capability("reject")],
        display: DisplayMeta::default(),
        source: None,
    }
}

pub type DecisionFn =
    Box<dyn FnMut(&mut Builder, &DecisionScope) -> Result<DecisionFragments, BindingError>>;

/// A Rust decision source: one tracer for both capabilities.
pub struct Source(pub DecisionFn);

impl ApprovalSource for Source {
    fn trace_decision(
        &mut self,
        b: &mut Builder,
        s: &DecisionScope,
    ) -> Result<DecisionFragments, BindingError> {
        (self.0)(b, s)
    }
}

/// The parameter named `name` must exist in the scope.
pub fn has(scope: &DecisionScope, name: &str) -> String {
    assert!(
        scope.params.iter().any(|p| p.name == name),
        "{name} not in scope"
    );
    name.to_string()
}

/// The standard fragments: actor = the context `actor`, no open window, no eligibility rule.
pub fn standard(b: &mut Builder, s: &DecisionScope) -> Result<DecisionFragments, BindingError> {
    let actor = b.param(&has(s, "actor"), loc()).unwrap();
    Ok(DecisionFragments {
        actor: (actor, loc()),
        open: None,
        eligible: None,
        guards: vec![],
        payload: vec![],
        commands: vec![],
    })
}

pub fn source() -> Source {
    Source(Box::new(standard))
}

pub fn spec(input: ApprovalInput, source: Source) -> AnyModel<'static> {
    AnyModel::Approval(ApprovalSpec {
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

pub fn build(k_a: i64, k_r: Option<i64>) -> BuildOutput {
    build_with(|_| {}, vec![spec(approval(k_a, k_r), source())])
        .unwrap_or_else(|e| panic!("{e:#?}"))
}

pub fn emit_notify(b: &mut Builder) -> CommandEmissionSpec {
    let id = b.field("subject", "id", loc()).unwrap();
    CommandEmissionSpec {
        command: "Notify".into(),
        when: None,
        payload: BTreeMap::from([("document".to_string(), id)]),
        loc: loc(),
    }
}

// --- Store helpers: the public engine's store with an in-memory backend ------------------------

use behavior_engine::builder::ScopeSite;
use behavior_engine::read::ReadSource;
use behavior_engine::semantic::Module;
use behavior_engine::store::documents::SeedEntity;
use behavior_engine::store::{InMemoryBackend, Store};
use behavior_engine::wire::{Role, WParam};
use serde_json::{Value, json};

pub const T: &str = "2026-10-14T00:00:00Z";

/// A decision seed: `(decision id, document, user, "APPROVE" | "REJECT")`.
pub type Seed<'a> = (&'a str, &'a str, &'a str, &'a str);

/// A store over `module` with users `u1..u6`, the given documents, and seeded decisions. Seeded
/// decisions are test fixtures only: a seed is not a valid Approval state by default (base-state
/// rule), which is exactly what the child-state tests need.
pub fn store(module: &Module, docs: &[&str], decisions: &[Seed<'_>]) -> Store<InMemoryBackend> {
    let mut seed = Vec::new();
    for u in ["u1", "u2", "u3", "u4", "u5", "u6"] {
        seed.push(SeedEntity {
            entity: "User".into(),
            value: json!({"id": u, "name": u}),
        });
    }
    for d in docs {
        seed.push(SeedEntity {
            entity: "Document".into(),
            value: json!({"id": d, "title": d}),
        });
    }
    for (id, doc, user, choice) in decisions {
        seed.push(SeedEntity {
            entity: "DocApprovalDecision".into(),
            value: json!({"id": id, "subject": doc, "actor": user, "choice": choice}),
        });
    }
    // The command profile (Wire 0.8) needs a v2 genesis; no evidence is required.
    let policy = behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#,
    )
    .unwrap();
    let genesis = behavior_engine::store::store::genesis_v2_for(module, policy, seed).unwrap();
    Store::create(InMemoryBackend::default(), module, genesis).unwrap()
}

/// The outcome of evaluating a decision: whether it was allowed, and its record.
pub struct Attempt {
    pub allowed: bool,
    pub record: Value,
}

/// Evaluates `doc_approval__<capability>` by `actor` on `doc`; if allowed, commits it on the
/// head it was evaluated against.
pub fn decide(
    store: &mut Store<InMemoryBackend>,
    module: &Module,
    capability: &str,
    decision_id: &str,
    doc: &str,
    actor: &str,
) -> Attempt {
    let (attempt, bundle) = evaluate(store, module, capability, decision_id, doc, actor);
    if let Some(bundle) = bundle {
        let head = store.current().unwrap();
        store.commit(module, &head, &bundle).unwrap();
    }
    attempt
}

/// Invokes without committing, through Core's unified capability intent (feature 012): the
/// attempt and its bundle (only for an allowed attempt at the captured head).
pub fn evaluate(
    store: &Store<InMemoryBackend>,
    module: &Module,
    capability: &str,
    decision_id: &str,
    doc: &str,
    actor: &str,
) -> (
    Attempt,
    Option<behavior_engine::store::documents::CommitBundle>,
) {
    let document = json!({
        "format": "behavior.invocation.v1",
        "capability": format!("doc_approval__{capability}"),
        "bindings": {"subject": {"entity": "Document", "id": doc}},
        "input": {"decision_id": decision_id},
        "context": {"actor": actor},
    });
    let (requested, errors) =
        behavior_engine::invocation::RequestedInvocation::decode(&document.to_string()).unwrap();
    let requested = requested.unwrap_or_else(|| panic!("{errors:?}"));
    let inv = store.invoke(module, &requested, T, None, None).unwrap();
    (
        Attempt {
            allowed: inv.bundle.is_some(),
            record: inv.record.as_json().clone(),
        },
        inv.bundle,
    )
}

/// The value of the derived definition `doc_approval__<local>` for `doc`, at the head.
pub fn derived(store: &Store<InMemoryBackend>, module: &Module, local: &str, doc: &str) -> Value {
    let mut b = Builder::for_module(module);
    let ps = vec![WParam {
        name: "doc".into(),
        role: Some(Role::State),
        ty: WType::Entity("Document".into()),
    }];
    b.push_scope(ScopeSite::Read, ps.clone(), loc()).unwrap();
    let body = b
        .derived_ref(&format!("doc_approval__{local}"), vec!["doc".into()], loc())
        .unwrap();
    b.pop_scope();
    let read = b
        .admit_read(&Builder::read_value("probe", ps, body, loc()))
        .unwrap();
    let out = store
        .read(
            module,
            &ReadSource::AdHoc(Box::new(read)),
            &BTreeMap::from([("doc".to_string(), doc.to_string())]),
            &json!({}),
            &json!({}),
            None,
        )
        .unwrap();
    out.response.value().cloned().unwrap_or(Value::Null)
}

/// `(approve_count, reject_count, outcome)` for `doc`, outcome one of PENDING, APPROVED,
/// REJECTED (asserting that exactly one predicate holds).
pub fn outcome(
    store: &Store<InMemoryBackend>,
    module: &Module,
    doc: &str,
) -> (i64, i64, &'static str) {
    let a = derived(store, module, "approve_count", doc)
        .as_i64()
        .unwrap();
    let r = derived(store, module, "reject_count", doc)
        .as_i64()
        .unwrap();
    let flags = [
        (
            "APPROVED",
            derived(store, module, "approved", doc) == json!(true),
        ),
        (
            "REJECTED",
            derived(store, module, "rejected", doc) == json!(true),
        ),
        (
            "PENDING",
            derived(store, module, "pending", doc) == json!(true),
        ),
    ];
    let holding: Vec<&'static str> = flags.iter().filter(|(_, v)| *v).map(|(n, _)| *n).collect();
    assert_eq!(
        holding.len(),
        1,
        "exactly one outcome must hold, got {holding:?}"
    );
    (a, r, holding[0])
}
