//! Feature 506, C16a (authoring conformance), the Rust half: the purchase approval reference
//! workflow authored independently with the Rust model authority, from data-model §1 of
//! specs/506-e2e-conformance, never from the Python construction's output.
//!
//! The build follows the protocol the Python binding follows: P1 authored declarations, P2
//! model declarations, P2b commands, P3 hand-written items (invariant, action, reads), P4
//! composition. The authoring identity (contract `comparison.md` §1: BehaviorHash, item hashes,
//! model hashes and versions, Approval's generated hashes; no source locations) is written as
//! canonical JSON to `$BEHAVIOR_CONFORMANCE_DIR/c16a-rust.json` (default: the workspace's
//! `target/conformance/`), where the Python half compares it with its own.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use std::collections::BTreeMap;

use behavior_engine::builder::{Builder, CommandEmissionSpec, Node, ScopeSite, SemanticProfile};
use behavior_engine::wire::{Role, WField, WItem, WParam, WType};
use behavior_models::state_machine::input::FragmentTable;
use behavior_models::{
    AnyModel, ApprovalInput, ApprovalSource, ApprovalSpec, BindingError, BuildOutput,
    CapabilityInput, CreationFragments, CreationInput, CreationScope, DecisionFragments,
    DecisionScope, DisplayMeta, FragmentSource, ModelInput, ModelSpec, NoFragments,
    RequestFlowInput, RequestFlowSource, RequestFlowSpec, TransitionFragments, TransitionInput,
    TransitionScope, compose_models, declare_models,
};
use serde_json::{Value, json};

use common::loc;

const STATUS: &str = "PurchaseRequestStatus";
const REQUEST: &str = "PurchaseRequest";
const DECISION: &str = "PurchaseRequestApprovalDecision";
const APPROVAL: &str = "purchase_request_approval";

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

fn param(name: &str, role: Option<Role>, ty: WType) -> WParam {
    WParam {
        name: name.into(),
        role,
        ty,
    }
}

fn id(entity: &str) -> WType {
    WType::Id(entity.into())
}

fn en(name: &str) -> WType {
    WType::Enum(name.into())
}

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

fn case(b: &mut Builder, enum_name: &str, value: &str) -> Node {
    b.lit(en(enum_name), json!(value), loc()).unwrap()
}

// --- P1: authored declarations (data-model §1) ---------------------------------------------

fn authored() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum("Role", strings(&["Requester", "Approver"]), loc())
        .unwrap();
    b.declare_enum("BudgetStatus", strings(&["Open", "Closed"]), loc())
        .unwrap();
    b.declare_enum(
        STATUS,
        strings(&["Draft", "Submitted", "InReview", "Approved", "Rejected"]),
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "User",
        vec![field("name", WType::String), field("role", en("Role"))],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Budget",
        vec![
            field("status", en("BudgetStatus")),
            field("limit", WType::Int),
        ],
        loc(),
    )
    .unwrap();
    b
}

// --- the RequestFlow `purchase_request` ---------------------------------------------------

fn transition(
    id_: &str,
    from: &[&str],
    to: &str,
    inputs: Vec<(String, WType)>,
    context: Vec<(String, WType)>,
) -> TransitionInput {
    TransitionInput {
        transition_id: id_.into(),
        from_states: strings(from),
        to_state: to.into(),
        inputs,
        context,
        display: DisplayMeta::default(),
        source: None,
    }
}

fn actor_context() -> Vec<(String, WType)> {
    vec![("actor".into(), id("User"))]
}

fn flow_input() -> RequestFlowInput {
    RequestFlowInput {
        flow_id: "purchase_request".into(),
        actor_entity: "User".into(),
        status_enum: STATUS.into(),
        assignee: true,
        payload: vec![
            ("title".into(), WType::String),
            ("amount".into(), WType::Int),
        ],
        creations: vec![CreationInput {
            creation_id: "create".into(),
            to_state: "Draft".into(),
            inputs: vec![
                ("title".into(), WType::String),
                ("amount".into(), WType::Int),
            ],
            state: vec![],
            context: actor_context(),
            display: DisplayMeta::default(),
            source: None,
        }],
        transitions: vec![
            transition("submit", &["Draft"], "Submitted", vec![], actor_context()),
            transition(
                "start_review",
                &["Submitted"],
                "InReview",
                vec![("reviewer".into(), id("User"))],
                vec![],
            ),
            transition(
                "finalize_approved",
                &["InReview"],
                "Approved",
                vec![],
                actor_context(),
            ),
            transition(
                "finalize_rejected",
                &["InReview"],
                "Rejected",
                vec![],
                vec![],
            ),
        ],
        display: DisplayMeta::default(),
        source: None,
    }
}

struct FlowSource {
    transitions: FragmentTable<'static>,
}

impl FragmentSource for FlowSource {
    fn trace(
        &mut self,
        b: &mut Builder,
        s: &TransitionScope,
    ) -> Result<TransitionFragments, BindingError> {
        self.transitions.trace(b, s)
    }
}

impl RequestFlowSource for FlowSource {
    /// `create`: requester = actor, title, amount, assignee = none.
    fn trace_creation(
        &mut self,
        b: &mut Builder,
        _: &CreationScope,
    ) -> Result<CreationFragments, BindingError> {
        let none = b
            .lit(WType::Option(Box::new(id("User"))), json!(null), loc())
            .unwrap();
        Ok(CreationFragments {
            initializer: vec![
                ("requester".into(), b.param("actor", loc()).unwrap()),
                ("title".into(), b.param("title", loc()).unwrap()),
                ("amount".into(), b.param("amount", loc()).unwrap()),
                ("assignee".into(), none),
            ],
            ..CreationFragments::default()
        })
    }
}

fn subject(s: &TransitionScope) -> String {
    s.params[0].name.clone()
}

fn flow_source() -> FlowSource {
    let mut t: FragmentTable<'static> = BTreeMap::new();
    // submit: the requester only.
    t.insert(
        "submit".into(),
        Box::new(|b: &mut Builder, s: &TransitionScope| {
            let actor = b.param("actor", loc()).unwrap();
            let requester = b.field(&subject(s), "requester", loc()).unwrap();
            let guard = b.op("eq", vec![actor, requester], loc()).unwrap();
            Ok(TransitionFragments {
                guards: vec![(guard, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    // start_review: assignee = some(reviewer).
    t.insert(
        "start_review".into(),
        Box::new(|b: &mut Builder, s: &TransitionScope| {
            let reviewer = b.param("reviewer", loc()).unwrap();
            let some = b.op("some", vec![reviewer], loc()).unwrap();
            Ok(TransitionFragments {
                effects: vec![(subject(s), "assignee".into(), some, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    // finalize_approved: requires approved(request); emits RequestApproved{request, by: actor}.
    t.insert(
        "finalize_approved".into(),
        Box::new(|b: &mut Builder, s: &TransitionScope| {
            let r = subject(s);
            let guard = b
                .derived_ref(&format!("{APPROVAL}__approved"), vec![r.clone()], loc())
                .unwrap();
            let when = b.lit(WType::Bool, json!(true), loc()).unwrap();
            let request = b.field(&r, "id", loc()).unwrap();
            let by = b.param("actor", loc()).unwrap();
            Ok(TransitionFragments {
                guards: vec![(guard, loc())],
                commands: vec![CommandEmissionSpec {
                    command: "RequestApproved".into(),
                    when: Some(when),
                    payload: BTreeMap::from([
                        ("by".to_string(), by),
                        ("request".to_string(), request),
                    ]),
                    loc: loc(),
                }],
                ..TransitionFragments::default()
            })
        }),
    );
    // finalize_rejected: requires rejected(request).
    t.insert(
        "finalize_rejected".into(),
        Box::new(|b: &mut Builder, s: &TransitionScope| {
            let guard = b
                .derived_ref(&format!("{APPROVAL}__rejected"), vec![subject(s)], loc())
                .unwrap();
            Ok(TransitionFragments {
                guards: vec![(guard, loc())],
                ..TransitionFragments::default()
            })
        }),
    );
    FlowSource { transitions: t }
}

// --- the StateMachine `budget` -------------------------------------------------------------

fn budget_machine() -> AnyModel<'static> {
    AnyModel::StateMachine(ModelSpec {
        input: ModelInput {
            model_id: "budget".into(),
            subject_entity: "Budget".into(),
            state_field: "status".into(),
            subject_param: None,
            entry_states: strings(&["Open"]),
            transitions: vec![transition("close", &["Open"], "Closed", vec![], vec![])],
            display: DisplayMeta::default(),
            source: None,
        },
        fragments: Box::new(NoFragments),
    })
}

// --- the Approval `purchase_request_approval` ---------------------------------------------

fn capability(id_: &str) -> CapabilityInput {
    CapabilityInput {
        capability_id: id_.into(),
        inputs: vec![],
        context: actor_context(),
        display: DisplayMeta::default(),
        source: None,
    }
}

struct DecisionSource;

impl ApprovalSource for DecisionSource {
    /// Actor: the context `actor`. Open: the request is InReview. Eligible: some User with
    /// `id == actor` and `role == Approver` (a query).
    fn trace_decision(
        &mut self,
        b: &mut Builder,
        _: &DecisionScope,
    ) -> Result<DecisionFragments, BindingError> {
        let actor = b.param("actor", loc()).unwrap();
        let status = b.field("subject", "status", loc()).unwrap();
        let in_review = case(b, STATUS, "InReview");
        let open = b.op("eq", vec![status, in_review], loc()).unwrap();
        let users = b.select("User", loc()).unwrap();
        b.push_lambda("u", "User").unwrap();
        let uid = b.field("u", "id", loc()).unwrap();
        let who = b.param("actor", loc()).unwrap();
        let same = b.op("eq", vec![uid, who], loc()).unwrap();
        let role = b.field("u", "role", loc()).unwrap();
        let approver = case(b, "Role", "Approver");
        let is_approver = b.op("eq", vec![role, approver], loc()).unwrap();
        let both = b.op("and", vec![same, is_approver], loc()).unwrap();
        b.pop_scope();
        let eligible = b.lambda("any", users, "u", both, loc()).unwrap();
        Ok(DecisionFragments {
            actor: (actor, loc()),
            open: Some((open, loc())),
            eligible: Some((eligible, loc())),
            guards: vec![],
            payload: vec![],
            commands: vec![],
        })
    }
}

fn approval() -> AnyModel<'static> {
    AnyModel::Approval(ApprovalSpec {
        input: ApprovalInput {
            approval_id: APPROVAL.into(),
            subject_entity: REQUEST.into(),
            actor_entity: "User".into(),
            approve_threshold: 2,
            reject_threshold: Some(1),
            payload: vec![],
            capabilities: vec![capability("approve"), capability("reject")],
            display: DisplayMeta::default(),
            source: None,
        },
        fragments: Box::new(DecisionSource),
    })
}

fn models() -> Vec<AnyModel<'static>> {
    vec![
        AnyModel::RequestFlow(RequestFlowSpec {
            input: flow_input(),
            fragments: Box::new(flow_source()),
        }),
        budget_machine(),
        approval(),
    ]
}

// --- P3: hand-written items ------------------------------------------------------------------

fn hand_written(b: &mut Builder) {
    // invariant budget_limit_is_not_negative(budget: Budget): budget.limit >= 0
    let ps = vec![param("budget", None, WType::Entity("Budget".into()))];
    b.push_scope(ScopeSite::Derived, ps, loc()).unwrap();
    let limit = b.field("budget", "limit", loc()).unwrap();
    let zero = b.lit(WType::Int, json!(0), loc()).unwrap();
    let body = b.op("ge", vec![limit, zero], loc()).unwrap();
    b.add_invariant(
        "budget_limit_is_not_negative",
        "Budget",
        "budget",
        body,
        loc(),
    )
    .unwrap();
    b.pop_scope();

    // action set_role(user: User, role: Input Role): user.role := role
    let ps = vec![
        param("user", Some(Role::State), WType::Entity("User".into())),
        param("role", Some(Role::Input), en("Role")),
    ];
    b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
    let role = b.param("role", loc()).unwrap();
    b.add_action(
        "set_role",
        ps,
        vec![],
        vec![("user".into(), "role".into(), role, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();

    // read decisions(): project(select(Decision), d → [subject, actor, choice])
    b.push_scope(ScopeSite::Read, vec![], loc()).unwrap();
    let all = b.select(DECISION, loc()).unwrap();
    b.pop_scope();
    let items = ["subject", "actor", "choice"]
        .iter()
        .map(|f| WItem::Field(f.to_string()))
        .collect();
    b.add_read(Builder::read_projection(
        "decisions",
        vec![],
        Ok(all),
        "d",
        items,
        loc(),
    ))
    .unwrap();

    // read request_outcome(request: PurchaseRequest): project(request, r → [status, approved,
    // rejected, pending])
    let ps = vec![param(
        "request",
        Some(Role::State),
        WType::Entity(REQUEST.into()),
    )];
    let mut items = vec![WItem::Field("status".into())];
    for local in ["approved", "rejected", "pending"] {
        items.push(WItem::Derived(format!("{APPROVAL}__{local}")));
    }
    b.add_read(Builder::read_projection(
        "request_outcome",
        ps,
        Err("request".into()),
        "r",
        items,
        loc(),
    ))
    .unwrap();
}

/// The full protocol: P1 → P2 → P2b (the command names the flow's entity) → P3 → P4.
fn build() -> BuildOutput {
    let mut b = authored();
    let ms = models();
    declare_models(&mut b, &ms).unwrap_or_else(|e| panic!("{e:#?}"));
    b.add_command(
        "RequestApproved",
        vec![field("by", id("User")), field("request", id(REQUEST))],
        loc(),
    )
    .unwrap();
    hand_written(&mut b);
    compose_models(&mut b, ms, None).unwrap_or_else(|e| panic!("{e:#?}"))
}

/// The authoring identity `N_A` (comparison.md §1).
fn authoring_identity(out: &BuildOutput) -> Value {
    let admission = behavior_engine::admission_result(&out.module);
    let mut models = serde_json::Map::new();
    let mut generated = serde_json::Map::new();
    for r in &out.receipts {
        let r = &r.0;
        let model = &r["model"];
        let id_ = ["model_id", "flow_id", "approval_id"]
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
fn the_reference_workflow_authored_in_rust_has_a_stable_identity() {
    let first = authoring_identity(&build());
    let second = authoring_identity(&build());
    assert_eq!(first, second, "two Rust builds differ");
    assert!(first["items"].as_object().unwrap().len() > 20, "{first}");
    let dir = conformance_dir();
    std::fs::create_dir_all(&dir).unwrap();
    // serde_json maps are ordered by key, so this is canonical JSON.
    std::fs::write(dir.join("c16a-rust.json"), format!("{first}\n")).unwrap();
}
