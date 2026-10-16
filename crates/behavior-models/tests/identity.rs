//! Identity, receipts, source maps and diagnostics (identity contract; T044, T046–T048).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::WType;
use behavior_models::{CORE_RELEASE, DisplayMeta, SourceLoc};
use common::*;
use serde_json::{Value, json};

fn receipt() -> Value {
    build_reference().receipts[0].0.clone()
}

fn model_hash(out: &behavior_models::BuildOutput) -> String {
    out.receipts[0].0["model_hash"]
        .as_str()
        .unwrap()
        .to_string()
}

// --- T044: canonical document -------------------------------------------------------------------

#[test]
fn canonical_document_has_exactly_the_semantic_facts() {
    let out = build_reference();
    let doc = &out.receipts[0].0["model"];
    let actions = out.module.actions();
    let approve = &actions["purchase_request__approve"];
    let h = |x: &[u8; 32]| hash_display(x);
    let enum_hash = h(&out.module.enums()["RequestStatus"].hash);
    let entities = behavior_engine::schema::declarations(&out.module);
    let expected = json!({
        "format": "behavior.model.state_machine.v2",
        "model_kind": "state_machine",
        "model_semantic_version": 2,
        "model_id": "purchase_request",
        "subject": {"entity": "PurchaseRequest", "entity_hash": entities["PurchaseRequest"]},
        "field": {"name": "status", "enum": "RequestStatus", "enum_hash": enum_hash},
        "subject_param": "purchase_request",
        "entry_states": ["Draft"],
        "transitions": [
            {
                "id": "approve", "from": ["InReview"], "to": "Approved",
                "inputs": [],
                "context": [{"name": "actor", "type": {"Id": "Actor"},
                             "types": {"Actor": entities["Actor"]}}],
                "guards": [h(approve.preconditions()[1].expr().hash())],
                "effects": [h(approve.effects()[1].hash())],
                "postconditions": [],
                "commands": [h(approve.command_emissions()[0].hash())]
            },
            {"id": "reject", "from": ["InReview"], "to": "Rejected", "inputs": [], "context": [],
             "guards": [], "effects": [], "postconditions": [], "commands": []},
            {"id": "start_review", "from": ["Submitted"], "to": "InReview", "inputs": [],
             "context": [], "guards": [], "effects": [], "postconditions": [], "commands": []},
            {"id": "submit", "from": ["Draft"], "to": "Submitted", "inputs": [], "context": [],
             "guards": [], "effects": [], "postconditions": [], "commands": []}
        ]
    });
    assert_eq!(doc, &expected);
    let mh = model_hash(&out);
    assert!(mh.starts_with("sha256:") && mh.len() == 71, "{mh}");
    assert_eq!(
        mh,
        behavior_engine::canonical::tagged_hash("behavior.model.v1", &expected).unwrap()
    );
}

// --- T046: source map ---------------------------------------------------------------------------

#[test]
fn source_map_is_total_one_to_one_and_sorted() {
    let out = build_reference();
    let entries = out.receipts[0].0["source_map"]["entries"]
        .as_array()
        .unwrap()
        .clone();
    let actions: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e["core"]["kind"] == "action")
        .map(|e| {
            (
                e["core"]["name"].as_str().unwrap(),
                e["model"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        actions,
        vec![
            (
                "purchase_request__approve",
                "purchase_request/transition/approve"
            ),
            (
                "purchase_request__reject",
                "purchase_request/transition/reject"
            ),
            (
                "purchase_request__start_review",
                "purchase_request/transition/start_review"
            ),
            (
                "purchase_request__submit",
                "purchase_request/transition/submit"
            ),
        ]
    );
    for e in entries.iter().filter(|e| e["core"]["kind"] == "action") {
        let name = e["core"]["name"].as_str().unwrap();
        assert_eq!(
            e["core"]["item_hash"],
            hash_display(out.module.actions()[name].hash())
        );
    }
    let fields: Vec<&Value> = entries
        .iter()
        .filter(|e| e["core"]["kind"] == "field")
        .collect();
    assert_eq!(
        fields,
        vec![&json!({
            "core": {"kind": "field", "entity": "PurchaseRequest", "field": "status"},
            "model": "purchase_request/state_field"
        })]
    );
}

#[test]
fn metadata_and_source_map_data_never_change_identity() {
    let plain = build_reference();
    let mut m = machine();
    m.display = DisplayMeta {
        display_name: Some("Purchase request".into()),
        description: Some("A request to buy".into()),
    };
    m.source = Some(SourceLoc {
        file: "elsewhere/model.py".into(),
        line: 99,
    });
    for t in &mut m.transitions {
        t.display.display_name = Some(format!("Do {}", t.transition_id));
        t.source = Some(SourceLoc {
            file: "x.py".into(),
            line: 7,
        });
    }
    let decorated = build(|_| {}, vec![spec(m)]).unwrap();
    assert_eq!(model_hash(&plain), model_hash(&decorated));
    assert_eq!(
        plain.module.behavior_version(),
        decorated.module.behavior_version()
    );
    // ...but the source map carries the metadata.
    let entries = decorated.receipts[0].0["source_map"]["entries"].clone();
    assert_eq!(entries[0]["display"]["display_name"], "Do approve");
    assert_eq!(entries[0]["source"], json!({"file": "x.py", "line": 7}));
}

// --- T047: receipt ------------------------------------------------------------------------------

#[test]
fn receipt_has_exactly_the_contract_fields() {
    let r = receipt();
    let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "behavior_hash",
            "core_release",
            "format",
            "lowerer_version",
            "model",
            "model_hash",
            "model_kind",
            "model_semantic_version",
            "source_map"
        ]
    );
    assert_eq!(r["format"], "behavior.model_receipt.v1");
    assert_eq!(r["model_kind"], "state_machine");
    assert_eq!(r["model_semantic_version"], 2);
    assert_eq!(r["lowerer_version"], 1);
    assert_eq!(
        r["core_release"],
        json!({"version": CORE_RELEASE.version, "commit": CORE_RELEASE.commit})
    );
    let out = build_reference();
    assert_eq!(r["behavior_hash"], out.module.behavior_version());
}

#[test]
fn receipts_are_byte_identical_across_builds() {
    let a = behavior_engine::canonical::to_canonical_string(&receipt()).unwrap();
    let b = behavior_engine::canonical::to_canonical_string(&receipt()).unwrap();
    assert_eq!(a, b);
}

// --- T048: diagnostics --------------------------------------------------------------------------

#[test]
fn diagnostics_are_warnings_and_never_block() {
    let mut m = machine();
    // Draft → Submitted, and InReview → Approved with nothing entering InReview:
    //   InReview: not an entry state, no incoming transition    → NO_INCOMING_TRANSITIONS
    //   Approved: entered only from unreachable InReview         → UNREACHABLE_STATE
    //   Submitted, Approved: nothing leaves them                 → NO_OUTGOING_TRANSITIONS
    //   Rejected: in no entry set and no transition              → UNUSED_STATE
    m.transitions = vec![
        transition("submit", &["Draft"], "Submitted"),
        transition("approve", &["InReview"], "Approved"),
    ];
    let out = build(|_| {}, vec![spec_without_fragments(m)]).unwrap();
    let got: Vec<(&str, &str)> = out
        .diagnostics
        .iter()
        .map(|d| (d.code, d.model_ref.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("NO_INCOMING_TRANSITIONS", "purchase_request/state/InReview"),
            ("NO_OUTGOING_TRANSITIONS", "purchase_request/state/Approved"),
            (
                "NO_OUTGOING_TRANSITIONS",
                "purchase_request/state/Submitted"
            ),
            ("UNREACHABLE_STATE", "purchase_request/state/Approved"),
            (
                "UNREACHABLE_TRANSITION",
                "purchase_request/transition/approve"
            ),
            ("UNUSED_STATE", "purchase_request/state/Rejected"),
        ]
    );
    for d in &out.diagnostics {
        assert!(!d.message.to_lowercase().contains("verif"), "{}", d.message);
    }
}

#[test]
fn the_reference_machine_only_reports_its_terminal_states() {
    let codes: Vec<(&str, String)> = build_reference()
        .diagnostics
        .iter()
        .map(|d| (d.code, d.model_ref.clone()))
        .collect();
    assert_eq!(
        codes,
        vec![
            (
                "NO_OUTGOING_TRANSITIONS",
                "purchase_request/state/Approved".into()
            ),
            (
                "NO_OUTGOING_TRANSITIONS",
                "purchase_request/state/Rejected".into()
            ),
        ]
    );
}

// --- T076: receipts do not depend on the checkout location ---------------------------------------

fn located(root: &str) -> Value {
    let mut m = machine();
    m.source = Some(SourceLoc {
        file: format!("{root}/models/procurement.py"),
        line: 10,
    });
    for (i, t) in m.transitions.iter_mut().enumerate() {
        t.source = Some(SourceLoc {
            file: format!("{root}/models/procurement.py"),
            line: 20 + i as u64,
        });
    }
    let mut b = declarations();
    let out = behavior_models::compose(&mut b, vec![spec(m)], Some(root)).unwrap();
    out.receipts[0].0.clone()
}

#[test]
fn receipts_are_identical_across_checkouts() {
    let a = located("/home/one/checkout");
    let b = located("/build/ci/elsewhere");
    assert_eq!(
        behavior_engine::canonical::to_canonical_string(&a).unwrap(),
        behavior_engine::canonical::to_canonical_string(&b).unwrap()
    );
    let entries = a["source_map"]["entries"].as_array().unwrap();
    for e in entries {
        let file = e["source"]["file"].as_str().unwrap();
        assert_eq!(file, "models/procurement.py", "{e}");
    }
}

// --- T077: transitions unreachable from entry states --------------------------------------------

/// `Draft → Submitted → Approved` and `InReview → Approved`, with nothing entering InReview.
/// Approved itself is reachable (through Submitted), but the transition from InReview can never
/// fire: `F_t ∩ Reach(Q_entry) = ∅`. A separate diagnostic from any state diagnostic.
#[test]
fn a_transition_from_unreachable_states_is_reported_even_if_its_target_is_reachable() {
    let mut m = machine();
    m.transitions = vec![
        transition("submit", &["Draft"], "Submitted"),
        transition("approve", &["Submitted"], "Approved"),
        transition("fast_track", &["InReview"], "Approved"),
        // Reachable through Draft although InReview is not: not reported.
        transition("withdraw", &["Draft", "InReview"], "Rejected"),
    ];
    let out = build(|_| {}, vec![spec_without_fragments(m)]).unwrap();
    let transitions: Vec<&str> = out
        .diagnostics
        .iter()
        .filter(|d| d.code == "UNREACHABLE_TRANSITION")
        .map(|d| d.model_ref.as_str())
        .collect();
    assert_eq!(transitions, vec!["purchase_request/transition/fast_track"]);
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.code == "UNREACHABLE_STATE" && d.model_ref.ends_with("/Approved"))
    );
}

// --- feature 505: exact declaration dependencies (contract `declaration-references.md`) ---------

/// The declarations of one build: the reference declarations plus `Mode` (an enum), `Amount` (a
/// nominal) and optionally an unrelated entity, with the variations the tests below apply.
#[derive(Clone)]
struct Decls {
    mode_name: &'static str,
    mode: Vec<&'static str>,
    amount: (WType, Vec<&'static str>, Option<u64>),
    subject_extra: bool,
    actor_extra: bool,
    unrelated: Option<WType>,
    reverse: bool,
}

impl Default for Decls {
    fn default() -> Self {
        Decls {
            mode_name: "Mode",
            mode: vec!["Standard", "Fast"],
            amount: (WType::Decimal, vec!["add"], Some(2)),
            subject_extra: false,
            actor_extra: false,
            unrelated: None,
            reverse: false,
        }
    }
}

fn field(name: &str, ty: WType) -> behavior_engine::wire::WField {
    behavior_engine::wire::WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}

fn builder(d: &Decls) -> behavior_engine::builder::Builder {
    use behavior_engine::builder::{Builder, SemanticProfile};
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    let status = |b: &mut Builder| {
        b.declare_enum(
            "RequestStatus",
            CASES.iter().map(|c| c.to_string()).collect(),
            loc(),
        )
        .unwrap();
    };
    let mode = |b: &mut Builder| {
        b.declare_enum(
            d.mode_name,
            d.mode.iter().map(|c| c.to_string()).collect(),
            loc(),
        )
        .unwrap();
    };
    let amount = |b: &mut Builder| {
        let (underlying, ops, scale) = &d.amount;
        b.declare_nominal(
            "Amount",
            underlying.clone(),
            ops.iter().map(|o| o.to_string()).collect(),
            *scale,
            loc(),
        )
        .unwrap();
    };
    if d.reverse {
        amount(&mut b);
        mode(&mut b);
        status(&mut b);
    } else {
        status(&mut b);
        mode(&mut b);
        amount(&mut b);
    }
    if let Some(ty) = &d.unrelated {
        b.declare_entity("Unrelated", vec![field("x", ty.clone())], loc())
            .unwrap();
    }
    let mut actor = vec![field("name", WType::String)];
    if d.actor_extra {
        actor.push(field("email", WType::String));
    }
    b.declare_entity("Actor", actor, loc()).unwrap();
    let mut request = vec![
        field("status", status_ty()),
        field("approver", WType::Id("Actor".into())),
        field(
            "approved_by",
            WType::Option(Box::new(WType::Id("Actor".into()))),
        ),
        field("note", WType::String),
        field("review", status_ty()),
    ];
    if d.subject_extra {
        request.push(field("internal_note", WType::String));
    }
    b.declare_entity("PurchaseRequest", request, loc()).unwrap();
    b.add_command(
        "Notify",
        vec![field("approver", WType::Id("Actor".into()))],
        loc(),
    )
    .unwrap();
    b
}

/// The reference machine whose `submit` has the unused context `mode: <mode_name>` and the
/// unused inputs `amount: Amount` and `who: Id<Actor>`; `reverse` lists them in reverse order.
fn hash_with(d: &Decls, reverse_params: bool) -> String {
    let mut m = machine();
    let submit = m
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "submit")
        .unwrap();
    submit.context = vec![("mode".into(), WType::Enum(d.mode_name.into()))];
    submit.inputs = vec![
        ("amount".into(), WType::Nominal("Amount".into())),
        ("who".into(), WType::Id("Actor".into())),
    ];
    if reverse_params {
        submit.inputs.reverse();
    }
    let mut b = builder(d);
    let out =
        behavior_models::compose(&mut b, vec![spec(m)], None).unwrap_or_else(|e| panic!("{e:#?}"));
    model_hash(&out)
}

/// Acceptance 1–3, 6, 7 (SC-001): every change to a referenced declaration changes the hash, even
/// where the machine does not use what changed.
#[test]
fn declaration_dependencies_change_the_machine_hash() {
    let base = hash_with(&Decls::default(), false);
    let changed = [
        (
            "the context enum's cases",
            Decls {
                mode: vec!["Standard", "Slow"],
                ..Decls::default()
            },
        ),
        (
            "the input nominal's operations",
            Decls {
                amount: (WType::Decimal, vec![], Some(2)),
                ..Decls::default()
            },
        ),
        (
            "the input nominal's scale",
            Decls {
                amount: (WType::Decimal, vec!["add"], Some(3)),
                ..Decls::default()
            },
        ),
        (
            "the input nominal's underlying type",
            Decls {
                amount: (WType::Int, vec!["add"], None),
                ..Decls::default()
            },
        ),
        (
            "an unused field of the subject",
            Decls {
                subject_extra: true,
                ..Decls::default()
            },
        ),
        (
            "the entity of an identity input",
            Decls {
                actor_extra: true,
                ..Decls::default()
            },
        ),
        (
            "the name of the context enum",
            Decls {
                mode_name: "Speed",
                ..Decls::default()
            },
        ),
    ];
    for (what, d) in changed {
        assert_ne!(hash_with(&d, false), base, "{what} must change the hash");
    }
}

/// Acceptance 4, 5 (SC-002; laws I2, I4): declarations the machine does not reference, and the
/// order of declarations and parameters, never change the hash.
#[test]
fn unrelated_declarations_and_order_keep_the_machine_hash() {
    let base = hash_with(&Decls::default(), false);
    let same = [
        (
            "an unrelated entity",
            Decls {
                unrelated: Some(WType::String),
                ..Decls::default()
            },
            false,
        ),
        (
            "declaration order",
            Decls {
                reverse: true,
                ..Decls::default()
            },
            false,
        ),
        ("parameter order", Decls::default(), true),
    ];
    for (what, d, reverse_params) in same {
        assert_eq!(
            hash_with(&d, reverse_params),
            base,
            "{what} must not change the hash"
        );
    }
    // An unrelated entity's fields change: still the same hash.
    let one = hash_with(
        &Decls {
            unrelated: Some(WType::String),
            ..Decls::default()
        },
        false,
    );
    let two = hash_with(
        &Decls {
            unrelated: Some(WType::Int),
            ..Decls::default()
        },
        false,
    );
    assert_eq!(one, two);
}
