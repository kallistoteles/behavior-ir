//! Approval identity, receipts and source maps (feature 503, US5: T043–T046; data model §8–§9).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::{Builder, ScopeSite, SemanticProfile};
use behavior_engine::wire::{Role, WField, WParam, WType};
use behavior_models::{
    AnyModel, ApprovalInput, BuildOutput, DisplayMeta, ModelInput, ModelSpec, NoFragments,
    SourceLoc, compose_models, declare_models,
};
use common::ap::{self, Source, approval, source, spec, standard};
use common::{loc, rf};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;
use serde_json::{Value, json};

fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

/// P1 with optional extra fields on `Document` and `User` (to change the subject or actor).
fn authored(document_extra: &[&str], user_extra: &[&str]) -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    let mut user = vec![field("name", WType::String)];
    user.extend(user_extra.iter().map(|f| field(f, WType::String)));
    let mut document = vec![field("title", WType::String)];
    document.extend(document_extra.iter().map(|f| field(f, WType::String)));
    b.declare_entity("User", user, loc()).unwrap();
    b.declare_entity("Document", document, loc()).unwrap();
    b
}

fn build_in(mut b: Builder, models: Vec<AnyModel<'static>>) -> BuildOutput {
    declare_models(&mut b, &models).unwrap_or_else(|e| panic!("{e:#?}"));
    ap::commands(&mut b);
    compose_models(&mut b, models, None).unwrap_or_else(|e| panic!("{e:#?}"))
}

fn build(input: ApprovalInput, s: Source) -> BuildOutput {
    build_in(authored(&[], &[]), vec![spec(input, s)])
}

/// The receipt of approval `id`.
fn receipt<'a>(out: &'a BuildOutput, id: &str) -> &'a Value {
    out.receipts
        .iter()
        .map(|r| &r.0)
        .find(|r| r["model"]["approval_id"] == json!(id))
        .unwrap_or_else(|| panic!("no receipt for {id}"))
}

fn approval_hash(out: &BuildOutput) -> String {
    receipt(out, "doc_approval")["model_hash"]
        .as_str()
        .unwrap()
        .to_string()
}

fn with_payload(mut input: ApprovalInput, payload: &[(&str, WType)]) -> ApprovalInput {
    input.payload = payload
        .iter()
        .map(|(n, t)| (n.to_string(), t.clone()))
        .collect();
    input
}

/// A source that sets every payload field to a literal of its type.
fn payload_source(payload: Vec<(&'static str, WType, Value)>) -> Source {
    Source(Box::new(move |b, s| {
        let mut f = standard(b, s)?;
        for (n, t, v) in &payload {
            let lit = b.lit(t.clone(), v.clone(), loc()).unwrap();
            f.payload.push((n.to_string(), lit));
        }
        Ok(f)
    }))
}

// --- T043: the canonical document -----------------------------------------------------------

#[test]
fn canonical_document_references_types_and_reads_fragments_by_position() {
    let input = with_payload(
        approval(2, None),
        &[("reason", WType::String), ("amount", WType::Int)],
    );
    let out = build(
        input,
        payload_source(vec![
            ("reason", WType::String, json!("ok")),
            ("amount", WType::Int, json!(1)),
        ]),
    );
    let doc = &receipt(&out, "doc_approval")["model"];
    let declarations = behavior_engine::schema::declarations(&out.module);
    assert_eq!(doc["format"], json!("behavior.model.approval.v1"));
    assert_eq!(doc["model_kind"], json!("approval"));
    assert_eq!(
        doc["subject"],
        json!({"entity": "Document", "entity_hash": declarations["Document"]})
    );
    assert_eq!(
        doc["actor"],
        json!({"entity": "User", "entity_hash": declarations["User"]})
    );
    assert_eq!(doc["approve_threshold"], json!(2));
    assert_eq!(doc["reject_threshold"], Value::Null, "⊥ is null");
    let payload: Vec<&str> = doc["payload"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(payload, vec!["amount", "reason"], "payload sorted by name");
    let caps: Vec<&str> = doc["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(caps, vec!["approve", "reject"]);
    for c in doc["capabilities"].as_array().unwrap() {
        let keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "actor", "commands", "context", "eligible", "guards", "id", "inputs", "open",
                "payload"
            ]
        );
        assert_eq!(
            c["payload"].as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["amount", "reason"]
        );
        // Fragment hashes are the admitted action's own, at the layout positions.
        let action = &out.module.actions()[&format!("doc_approval__{}", c["id"].as_str().unwrap())];
        let pre = action.preconditions();
        let h = |e: &[u8; 32]| behavior_engine::semantic::types::hash_display(e);
        assert_eq!(c["open"], json!(h(pre[0].expr().hash())));
        assert_eq!(c["eligible"], json!(h(pre[2].expr().hash())));
        assert_eq!(c["guards"], json!([]));
    }
    let k_r = build(approval(2, Some(1)), source());
    assert_eq!(
        receipt(&k_r, "doc_approval")["model"]["reject_threshold"],
        json!(1)
    );
}

// --- T044: what changes the hash, and what does not -------------------------------------------

/// A source whose `approve` fragments are changed by `change`.
fn approve_changed(
    change: impl Fn(&mut Builder, &mut behavior_models::DecisionFragments) + 'static,
) -> Source {
    Source(Box::new(move |b, s| {
        let mut f = standard(b, s)?;
        if s.capability_id == "approve" {
            change(b, &mut f);
        }
        Ok(f)
    }))
}

fn truth(b: &mut Builder, v: bool) -> behavior_engine::builder::Node {
    b.lit(WType::Bool, json!(v), loc()).unwrap()
}

#[test]
fn approval_hash_changes_with_every_semantic_fact() {
    let base = approval_hash(&build(approval(2, Some(1)), source()));
    let mut changed: Vec<(&str, String)> = vec![
        ("k_A", approval_hash(&build(approval(3, Some(1)), source()))),
        ("k_R", approval_hash(&build(approval(2, Some(2)), source()))),
        ("k_R ⊥", approval_hash(&build(approval(2, None), source()))),
        (
            "subject entity",
            approval_hash(&build_in(
                authored(&["summary"], &[]),
                vec![spec(approval(2, Some(1)), source())],
            )),
        ),
        (
            "actor entity",
            approval_hash(&build_in(
                authored(&[], &["email"]),
                vec![spec(approval(2, Some(1)), source())],
            )),
        ),
        (
            "open",
            approval_hash(&build(
                approval(2, Some(1)),
                approve_changed(|b, f| f.open = Some((truth(b, false), loc()))),
            )),
        ),
        (
            "eligible",
            approval_hash(&build(
                approval(2, Some(1)),
                approve_changed(|b, f| f.eligible = Some((truth(b, false), loc()))),
            )),
        ),
        (
            "guard",
            approval_hash(&build(
                approval(2, Some(1)),
                approve_changed(|b, f| f.guards.push((truth(b, true), loc()))),
            )),
        ),
        (
            "actor expression",
            approval_hash(&build(
                approval(2, Some(1)),
                approve_changed(|b, f| {
                    let other = b.lit(WType::Id("User".into()), json!("u9"), loc()).unwrap();
                    f.actor = (other, loc());
                }),
            )),
        ),
        (
            "command",
            approval_hash(&build(
                approval(2, Some(1)),
                approve_changed(|b, f| f.commands.push(ap::emit_notify(b))),
            )),
        ),
    ];
    let payload = |ty: WType, v: Value| {
        approval_hash(&build(
            with_payload(approval(2, Some(1)), &[("note", ty.clone())]),
            payload_source(vec![("note", ty, v)]),
        ))
    };
    let string_payload = payload(WType::String, json!("a"));
    changed.push(("payload type", payload(WType::Int, json!(1))));
    changed.push(("payload initializer", payload(WType::String, json!("b"))));
    for (what, hash) in &changed {
        let reference = if what.starts_with("payload") {
            &string_payload
        } else {
            &base
        };
        assert_ne!(hash, reference, "{what} must change ApprovalHash");
    }
}

/// Named types in the interface are referenced by their Core declaration identity: changing an
/// unused context enum, an input nominal or a payload enum changes ApprovalHash, because each
/// changes the accepted invocations or the decision entity (review 2026-10-15).
#[test]
fn approval_hash_follows_the_declarations_of_named_parameter_and_payload_types() {
    let hash = |mode: &[&str], amount_ops: &[&str], kind: &[&str]| {
        let mut b = authored(&[], &[]);
        b.declare_enum("Mode", mode.iter().map(|c| c.to_string()).collect(), loc())
            .unwrap();
        b.declare_enum("Kind", kind.iter().map(|c| c.to_string()).collect(), loc())
            .unwrap();
        b.declare_nominal(
            "Amount",
            WType::Int,
            amount_ops.iter().map(|o| o.to_string()).collect(),
            None,
            loc(),
        )
        .unwrap();
        let mut input = with_payload(
            approval(2, Some(1)),
            &[("kind", WType::Enum("Kind".into()))],
        );
        for c in &mut input.capabilities {
            c.context.push(("mode".into(), WType::Enum("Mode".into())));
            c.inputs
                .push(("amount".into(), WType::Nominal("Amount".into())));
        }
        let first = kind[0].to_string();
        let src = payload_source(vec![("kind", WType::Enum("Kind".into()), json!(first))]);
        approval_hash(&build_in(b, vec![spec(input, src)]))
    };
    let base = hash(&["Standard", "Fast"], &["add"], &["A", "B"]);
    assert_eq!(base, hash(&["Standard", "Fast"], &["add"], &["A", "B"]));
    for (what, changed) in [
        (
            "context enum",
            hash(&["Standard", "Slow"], &["add"], &["A", "B"]),
        ),
        (
            "input nominal",
            hash(&["Standard", "Fast"], &[], &["A", "B"]),
        ),
        (
            "payload enum",
            hash(&["Standard", "Fast"], &["add"], &["A", "C"]),
        ),
    ] {
        assert_ne!(changed, base, "{what} must change ApprovalHash");
    }
}

#[test]
fn approval_hash_ignores_display_names_descriptions_and_locations() {
    let base = approval_hash(&build(approval(2, Some(1)), source()));
    let mut decorated = approval(2, Some(1));
    decorated.display = DisplayMeta {
        display_name: Some("Document approval".into()),
        description: Some("Two approvals".into()),
    };
    decorated.source = Some(SourceLoc {
        file: "/elsewhere/doc.py".into(),
        line: 42,
    });
    for c in &mut decorated.capabilities {
        c.display.display_name = Some(format!("{} it", c.capability_id));
        c.source = Some(SourceLoc {
            file: "/elsewhere/doc.py".into(),
            line: 7,
        });
    }
    assert_eq!(approval_hash(&build(decorated, source())), base);
}

/// A changed RequestFlow payload changes the ApprovalHash of an approval over its entity.
#[test]
fn a_changed_flow_payload_changes_an_approval_over_its_entity() {
    let hash = |extra: bool| {
        let mut input = rf::flow();
        if extra {
            input.payload.push(("cost_center".into(), WType::String));
        }
        let mut a = approval(1, None);
        a.approval_id = "purchase_request_approval".into();
        a.subject_entity = "PurchaseRequest".into();
        let mut flow_source = rf::source();
        if extra {
            for creation in ["create", "import_submitted"] {
                flow_source.creations.insert(
                    creation.into(),
                    Box::new(|b, s| {
                        let mut f = rf::standard(b, s)?;
                        let cc = b.lit(WType::String, json!("cc"), loc()).unwrap();
                        f.initializer.push(("cost_center".into(), cc));
                        Ok(f)
                    }),
                );
            }
        }
        let mut b = rf::authored();
        let models = vec![rf::spec(input, flow_source), spec(a, source())];
        declare_models(&mut b, &models).unwrap_or_else(|e| panic!("{e:#?}"));
        rf::commands(&mut b);
        let out = compose_models(&mut b, models, None).unwrap_or_else(|e| panic!("{e:#?}"));
        receipt(&out, "purchase_request_approval")["model_hash"].clone()
    };
    assert_ne!(hash(false), hash(true));
}

// --- T045: permutations -------------------------------------------------------------------------

/// Everything that identifies the build: per-approval hashes, the module, and the generated
/// declarations.
fn fingerprint(out: &BuildOutput) -> Value {
    let hashes: Vec<Value> = ["doc_approval", "doc_review"]
        .iter()
        .map(|id| receipt(out, id)["model_hash"].clone())
        .collect();
    let declarations = behavior_engine::schema::declarations(&out.module);
    let actions: Vec<&String> = out.module.actions().keys().collect();
    json!({
        "actions": actions,
        "approvals": hashes,
        "behavior": out.module.behavior_version(),
        "decisions": [declarations["DocApprovalDecision"], declarations["DocReviewDecision"]],
        "choices": [
            behavior_engine::semantic::types::hash_display(&out.module.enums()["DocApprovalDecisionChoice"].hash),
            behavior_engine::semantic::types::hash_display(&out.module.enums()["DocReviewDecisionChoice"].hash),
        ],
    })
}

/// The environment of the permutation property: the RequestFlow fixture's P1 (`User { name }`,
/// `Budget`, the flow's status enum) plus `Document { title }` and a StateMachine subject
/// `Ticket { status: TicketStatus }`.
fn mixed_authored() -> Builder {
    let mut b = rf::authored();
    b.declare_entity("Document", vec![field("title", WType::String)], loc())
        .unwrap();
    b.declare_enum("TicketStatus", vec!["Open".into(), "Closed".into()], loc())
        .unwrap();
    b.declare_entity(
        "Ticket",
        vec![field("status", WType::Enum("TicketStatus".into()))],
        loc(),
    )
    .unwrap();
    b
}

/// A StateMachine `ticket` over the authored `Ticket`: `close` Open → Closed.
fn ticket_machine() -> AnyModel<'static> {
    AnyModel::StateMachine(ModelSpec {
        input: ModelInput {
            model_id: "ticket".into(),
            subject_entity: "Ticket".into(),
            state_field: "status".into(),
            subject_param: None,
            entry_states: vec!["Open".into()],
            transitions: vec![common::transition("close", &["Open"], "Closed")],
            display: DisplayMeta::default(),
            source: None,
        },
        fragments: Box::new(NoFragments),
    })
}

/// Hand-written items over `doc_approval`'s decision entity: a derived count of all decisions,
/// a declared read of it and an action guarded by "the document has a decision". `reverse`
/// adds them in the opposite order.
fn hand_written(b: &mut Builder, reverse: bool) {
    const X: &str = "DocApprovalDecision";
    let derived = |b: &mut Builder| {
        b.push_scope(ScopeSite::Derived, vec![], loc()).unwrap();
        let all = b.select(X, loc()).unwrap();
        let n = b.op("count", vec![all], loc()).unwrap();
        b.pop_scope();
        b.add_derived(
            "decision_total",
            behavior_engine::wire::DerivedKind::Derived,
            vec![],
            n,
            Some(WType::Int),
            loc(),
        )
        .unwrap();
    };
    let ps = vec![WParam {
        name: "doc".into(),
        role: Some(Role::State),
        ty: WType::Entity("Document".into()),
    }];
    let read = |b: &mut Builder| {
        b.push_scope(ScopeSite::Read, ps.clone(), loc()).unwrap();
        let all = b.select(X, loc()).unwrap();
        let n = b.op("count", vec![all], loc()).unwrap();
        b.pop_scope();
        b.add_read(Builder::read_value("decisions", ps.clone(), n, loc()))
            .unwrap();
    };
    let action = |b: &mut Builder| {
        b.push_scope(ScopeSite::Action, ps.clone(), loc()).unwrap();
        let all = b.select(X, loc()).unwrap();
        b.push_lambda("d", X).unwrap();
        let subject = b.field("d", "subject", loc()).unwrap();
        let id = b.field("doc", "id", loc()).unwrap();
        let same = b.op("eq", vec![subject, id], loc()).unwrap();
        b.pop_scope();
        let any = b.lambda("any", all, "d", same, loc()).unwrap();
        let title = b.lit(WType::String, json!("decided"), loc()).unwrap();
        b.add_action(
            "retitle",
            ps.clone(),
            vec![(any, loc())],
            vec![("doc".into(), "title".into(), title, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    };
    if reverse {
        action(b);
        read(b);
        derived(b);
    } else {
        derived(b);
        read(b);
        action(b);
    }
}

/// One permutation: payload order (a permutation of 0..3), capability inputs and context
/// swapped or not, the supply order of the four models (a permutation of 0..4: the two
/// approvals, the RequestFlow and the StateMachine) and the order of the hand-written items.
type Permutation = (Vec<usize>, bool, bool, Vec<usize>, bool);

fn permuted_build(
    (payload, swap_inputs, swap_context, models, hand_reverse): &Permutation,
) -> BuildOutput {
    let fields = [
        ("amount", WType::Int, json!(1)),
        ("reason", WType::String, json!("r")),
        ("code", WType::String, json!("c")),
    ];
    let order: Vec<(&'static str, WType, Value)> =
        payload.iter().map(|&i| fields[i].clone()).collect();
    let mut first = approval(2, Some(1));
    first.payload = order
        .iter()
        .map(|(n, t, _)| (n.to_string(), t.clone()))
        .collect();
    for c in &mut first.capabilities {
        c.inputs = vec![("note".into(), WType::String), ("rank".into(), WType::Int)];
        c.context.push(("at".into(), WType::String));
        if *swap_inputs {
            c.inputs.reverse();
        }
        if *swap_context {
            c.context.reverse();
        }
    }
    let mut second = approval(1, None);
    second.approval_id = "doc_review".into();
    let mut supplied: Vec<Option<AnyModel<'static>>> = vec![
        Some(spec(first, payload_source(order))),
        Some(spec(second, source())),
        Some(rf::spec(rf::flow(), rf::source())),
        Some(ticket_machine()),
    ];
    let models: Vec<AnyModel<'static>> = models
        .iter()
        .map(|&i| supplied[i].take().unwrap())
        .collect();
    let mut b = mixed_authored();
    declare_models(&mut b, &models).unwrap_or_else(|e| panic!("{e:#?}"));
    rf::commands(&mut b);
    hand_written(&mut b, *hand_reverse);
    compose_models(&mut b, models, None).unwrap_or_else(|e| panic!("{e:#?}"))
}

thread_local! {
    /// One build per distinct permutation: a build is a pure function of its inputs, and the
    /// determinism of repeated builds is the determinism gate's subject, not this property's.
    static FINGERPRINTS: std::cell::RefCell<std::collections::BTreeMap<Permutation, Value>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

fn fingerprint_of(p: Permutation) -> Value {
    if let Some(v) = FINGERPRINTS.with(|m| m.borrow().get(&p).cloned()) {
        return v;
    }
    let v = fingerprint(&permuted_build(&p));
    FINGERPRINTS.with(|m| m.borrow_mut().insert(p, v.clone()));
    v
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: common::property_cases(),
        rng_seed: RngSeed::Fixed(503),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    /// Payload order, input order, context order, the supply order of approvals, a RequestFlow
    /// and a StateMachine, and the order of hand-written items never change ApprovalHash, the
    /// generated declarations, the module or BehaviorHash (SC-006).
    #[test]
    fn supply_and_mapping_order_never_change_identity(
        payload in Just(vec![0usize, 1, 2]).prop_shuffle(),
        swap_inputs in any::<bool>(),
        swap_context in any::<bool>(),
        models in Just(vec![0usize, 1, 2, 3]).prop_shuffle(),
        hand_reverse in any::<bool>(),
    ) {
        let reference = fingerprint_of((vec![0, 1, 2], false, false, vec![0, 1, 2, 3], false));
        for name in ["ticket__close", "purchase_request__start_review", "retitle"] {
            prop_assert!(reference["actions"].as_array().unwrap().contains(&json!(name)), "{name}");
        }
        prop_assert_eq!(
            fingerprint_of((payload, swap_inputs, swap_context, models, hand_reverse)),
            reference
        );
    }
}

// --- T046: receipts and source maps ---------------------------------------------------------

#[test]
fn one_receipt_per_approval_with_its_generated_declarations() {
    let mut second = approval(1, None);
    second.approval_id = "doc_review".into();
    let out = build_in(
        authored(&[], &[]),
        vec![spec(approval(2, Some(1)), source()), spec(second, source())],
    );
    let approvals: Vec<&Value> = out
        .receipts
        .iter()
        .map(|r| &r.0)
        .filter(|r| r["model_kind"] == json!("approval"))
        .collect();
    assert_eq!(approvals.len(), 2);
    let r = receipt(&out, "doc_approval");
    assert_eq!(r["format"], json!("behavior.model_receipt.v1"));
    assert_eq!(r["behavior_hash"], json!(out.module.behavior_version()));
    let g = &r["generated"];
    assert_eq!(g["enum"]["name"], json!("DocApprovalDecisionChoice"));
    assert_eq!(g["entity"]["name"], json!("DocApprovalDecision"));
    let derived: Vec<&String> = g["derived"].as_object().unwrap().keys().collect();
    assert_eq!(
        derived,
        vec![
            "approve_count",
            "approved",
            "pending",
            "reject_count",
            "rejected"
        ]
    );
    for (local, hash) in g["derived"].as_object().unwrap() {
        let item = &out.module.derived_items()[&format!("doc_approval__{local}")];
        assert_eq!(
            hash,
            &json!(behavior_engine::semantic::types::hash_display(item.hash()))
        );
    }
}

#[test]
fn source_map_names_actions_predicates_checks_and_declarations_with_relative_paths() {
    let mut input = approval(2, Some(1));
    input.source = Some(SourceLoc {
        file: "/work/app/approvals.py".into(),
        line: 3,
    });
    input.capabilities[0].source = Some(SourceLoc {
        file: "/work/app/approvals.py".into(),
        line: 9,
    });
    let mut b = authored(&[], &[]);
    let models = vec![spec(input, source())];
    declare_models(&mut b, &models).unwrap();
    ap::commands(&mut b);
    let out = compose_models(&mut b, models, Some("/work")).unwrap();
    let entries = receipt(&out, "doc_approval")["source_map"]["entries"]
        .as_array()
        .unwrap()
        .clone();
    let by_model = |model: &str| {
        entries
            .iter()
            .find(|e| e["model"] == json!(model))
            .unwrap_or_else(|| panic!("no entry for {model}"))
            .clone()
    };
    let approve = by_model("doc_approval/capability/approve");
    assert_eq!(approve["core"]["name"], json!("doc_approval__approve"));
    assert_eq!(
        approve["source"],
        json!({"file": "app/approvals.py", "line": 9})
    );
    assert_eq!(
        by_model("doc_approval/capability/reject")["core"]["name"],
        json!("doc_approval__reject")
    );
    for local in [
        "approve_count",
        "reject_count",
        "approved",
        "rejected",
        "pending",
    ] {
        assert_eq!(
            by_model(&format!("doc_approval/{local}"))["core"]["name"],
            json!(format!("doc_approval__{local}"))
        );
    }
    let check = |key: &str| by_model(&format!("doc_approval/{key}"))["core"].clone();
    assert_eq!(check("uniqueness")["kind"], json!("postcondition"));
    assert_eq!(check("uniqueness")["position"], json!(0));
    assert_eq!(check("threshold_consistency")["position"], json!(1));
    assert_eq!(check("terminal_guard")["kind"], json!("precondition"));
    assert_eq!(check("terminal_guard")["position"], json!(1));
    assert_eq!(
        by_model("doc_approval/decision")["core"],
        json!({"kind": "entity", "name": "DocApprovalDecision"})
    );
    assert_eq!(
        by_model("doc_approval/decision")["source"],
        json!({"file": "app/approvals.py", "line": 3})
    );
    assert_eq!(
        by_model("doc_approval/choice")["core"],
        json!({"kind": "enum", "name": "DocApprovalDecisionChoice"})
    );
    let text = serde_json::to_string(&entries).unwrap();
    assert!(!text.contains("/work/"), "every path is relative");

    // Without a reject threshold there is no threshold-consistency entry.
    let plain = build(approval(1, None), source());
    let entries = receipt(&plain, "doc_approval")["source_map"]["entries"].to_string();
    assert!(!entries.contains("threshold_consistency"));
}
