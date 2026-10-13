//! RequestFlow identity, receipts and source maps (data model §7–§8, research R9; T036–T038).

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

mod common;

use behavior_engine::builder::Builder;
use behavior_engine::canonical::tagged_hash;
use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::WType;
use behavior_models::{
    BuildOutput, CreationFragments, CreationScope, DisplayMeta, RequestFlowInput, SourceLoc,
};
use common::loc;
use common::rf::*;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;
use serde_json::{Value, json};

fn receipt(out: &BuildOutput) -> &Value {
    assert_eq!(out.receipts.len(), 1, "one supplied flow → one receipt");
    &out.receipts[0].0
}

fn flow_hash(out: &BuildOutput) -> String {
    receipt(out)["model_hash"].as_str().unwrap().to_string()
}

fn build(f: RequestFlowInput, s: Source) -> BuildOutput {
    build_with(|_| {}, vec![spec(f, s)]).unwrap_or_else(|e| panic!("{e:#?}"))
}

// --- T036: canonical document and RequestFlowHash -----------------------------------------

#[test]
fn canonical_document_references_instead_of_copying() {
    let out = build_reference();
    let doc = &receipt(&out)["model"];
    let h = |x: &[u8; 32]| hash_display(x);
    let creation = |name: &str| {
        let a = &out.module.actions()[name];
        let init: serde_json::Map<String, Value> = a.creates()[0]
            .fields()
            .iter()
            .filter(|(n, _)| n != "status")
            .map(|(n, e)| (n.clone(), json!(h(e.hash()))))
            .collect();
        init
    };
    let sm = &doc["state_machine"];
    let expected = json!({
        "format": "behavior.model.request_flow.v1",
        "model_kind": "request_flow",
        "model_semantic_version": 1,
        "flow_id": "purchase_request",
        "actor": {"entity": "User"},
        "status_enum": {
            "name": "PurchaseRequestStatus",
            "enum_hash": h(&out.module.enums()["PurchaseRequestStatus"].hash),
        },
        "assignee": true,
        "payload": [{"name": "amount", "type": "Int"}, {"name": "title", "type": "String"}],
        "creations": [
            {
                "id": "create", "to": "Draft",
                "inputs": [{"name": "amount", "type": "Int"}, {"name": "title", "type": "String"}],
                "state": [], "context": [{"name": "actor", "type": {"Id": "User"}}],
                "initializer": creation("purchase_request__create"),
                "guards": [], "effects": [], "postconditions": [], "commands": [],
            },
            {
                "id": "import_submitted", "to": "Submitted",
                "inputs": [{"name": "amount", "type": "Int"}, {"name": "title", "type": "String"}],
                "state": [], "context": [{"name": "actor", "type": {"Id": "User"}}],
                "initializer": creation("purchase_request__import_submitted"),
                "guards": [], "effects": [], "postconditions": [], "commands": [],
            },
        ],
        "state_machine": sm.clone(),
    });
    assert_eq!(doc, &expected);
    // The embedded machine contributes its own document and ModelHash: no second encoding.
    assert_eq!(sm["document"]["format"], "behavior.model.state_machine.v1");
    assert_eq!(
        sm["document"]["entry_states"],
        json!(["Draft", "Submitted"])
    );
    assert_eq!(
        sm["model_hash"],
        json!(tagged_hash("behavior.model.v1", &sm["document"]).unwrap())
    );
    // Neither the entity name nor its schema is hashed separately: outside the embedded
    // machine's own document, the flow document never names the entity.
    let mut own = doc.clone();
    own["state_machine"] = json!(null);
    assert!(!own.to_string().contains("\"PurchaseRequest\""));
    assert_eq!(
        flow_hash(&out),
        tagged_hash("behavior.model.v1", doc).unwrap()
    );
}

fn with_creation(
    s: &mut Source,
    id: &str,
    edit: impl Fn(&mut Builder, &CreationScope, &mut CreationFragments) + 'static,
) {
    s.creations.insert(
        id.into(),
        Box::new(move |b: &mut Builder, sc: &CreationScope| {
            let mut f = standard(b, sc)?;
            edit(b, sc, &mut f);
            Ok(f)
        }),
    );
}

#[test]
fn every_semantic_change_changes_the_flow_hash() {
    let base = flow_hash(&build_reference());
    let mut variants: Vec<(&str, BuildOutput)> = Vec::new();

    let mut f = flow();
    f.creations[1].to_state = "Draft".into();
    variants.push(("creation target", build(f, source())));

    let mut s = source();
    with_creation(&mut s, "create", |b, _, f| {
        let actor = b.param("actor", loc()).unwrap();
        let some = b.op("some", vec![actor], loc()).unwrap();
        f.initializer.retain(|(n, _)| n != "assignee");
        f.initializer.push(("assignee".into(), some));
    });
    variants.push(("initializer", build(flow(), s)));

    let mut s = source();
    with_creation(&mut s, "create", |b, _, f| {
        let amount = b.param("amount", loc()).unwrap();
        let zero = b.lit(WType::Int, json!(0), loc()).unwrap();
        f.guards
            .push((b.op("eq", vec![amount, zero], loc()).unwrap(), loc()));
    });
    variants.push(("creation guard", build(flow(), s)));

    let mut f = flow();
    let reject = f
        .transitions
        .iter_mut()
        .find(|t| t.transition_id == "reject")
        .unwrap();
    reject.to_state = "Cancelled".into();
    variants.push(("transition", build(f, source())));

    let mut f = flow();
    f.payload.push(("note".into(), WType::String));
    let mut s = source();
    for c in ["create", "import_submitted"] {
        with_creation(&mut s, c, |b, _, f| {
            f.initializer.push((
                "note".into(),
                b.lit(WType::String, json!(""), loc()).unwrap(),
            ));
        });
    }
    variants.push(("payload", build(f, s)));

    let mut f = flow();
    f.creations[0].state = vec![("budget".into(), "Budget".into())];
    variants.push(("state binding", build(f, source())));

    let mut f = flow();
    f.assignee = false;
    let mut s = source();
    for c in ["create", "import_submitted"] {
        with_creation(&mut s, c, |_, _, f| {
            f.initializer.retain(|(n, _)| n != "assignee")
        });
    }
    s.transitions.clear();
    variants.push(("assignee flag", build(f, s)));

    let mut seen = std::collections::BTreeSet::from([base.clone()]);
    for (what, out) in &variants {
        assert!(
            seen.insert(flow_hash(out)),
            "{what} did not change RequestFlowHash"
        );
    }
}

// --- T038: receipt and source map --------------------------------------------------------------

#[test]
fn one_receipt_with_an_embedded_identity_record() {
    let out = build_reference();
    let r = receipt(&out);
    assert_eq!(r["model_kind"], "request_flow");
    assert_eq!(r["behavior_hash"], json!(out.module.behavior_version()));
    assert_eq!(
        r["embedded"],
        json!([{
            "kind": "state_machine",
            "id": "purchase_request",
            "model_semantic_version": 1,
            "lowerer_version": 1,
            "model_hash": r["model"]["state_machine"]["model_hash"],
        }])
    );
}

#[test]
fn source_map_links_flow_machine_and_core() {
    let mut f = flow();
    f.source = Some(SourceLoc {
        file: "/abs/checkout/app/flows.py".into(),
        line: 3,
    });
    f.creations[0].source = Some(SourceLoc {
        file: "/abs/checkout/app/flows.py".into(),
        line: 9,
    });
    let out = build(f, source());
    let entries = receipt(&out)["source_map"]["entries"]
        .as_array()
        .unwrap()
        .clone();
    let model_refs: Vec<&str> = entries
        .iter()
        .map(|e| e["model"].as_str().unwrap())
        .collect();
    for want in [
        "purchase_request/creation/create",
        "purchase_request/creation/import_submitted",
        "purchase_request/transition/approve",
        "purchase_request/request_entity",
        "purchase_request/request_entity/requester",
        "purchase_request/request_entity/status",
    ] {
        assert!(
            model_refs.contains(&want),
            "{want} missing in {model_refs:?}"
        );
    }
    let approve = entries
        .iter()
        .find(|e| e["model"] == "purchase_request/transition/approve")
        .unwrap();
    assert_eq!(
        approve["embedded"],
        "state_machine:purchase_request/transition/approve"
    );
    assert_eq!(approve["core"]["name"], "purchase_request__approve");
    let create = entries
        .iter()
        .find(|e| e["model"] == "purchase_request/creation/create")
        .unwrap();
    assert_eq!(
        create["source"],
        json!({"file": "flows.py", "line": 9}),
        "relative to the model's own sources"
    );
}

fn decorate(f: &mut RequestFlowInput, n: u64) {
    let meta = |what: &str| DisplayMeta {
        display_name: Some(format!("{what} {n}")),
        description: Some(format!("about {what}")),
    };
    let at = |line| {
        Some(SourceLoc {
            file: format!("/elsewhere/{n}/flows.py"),
            line,
        })
    };
    f.display = meta("flow");
    f.source = at(1);
    for c in &mut f.creations {
        c.display = meta(&c.creation_id.clone());
        c.source = at(2);
    }
    for t in &mut f.transitions {
        t.display = meta(&t.transition_id.clone());
        t.source = at(3);
    }
}

#[test]
fn metadata_changes_neither_hash() {
    let plain = build_reference();
    let mut f = flow();
    decorate(&mut f, 7);
    let decorated = build(f, source());
    assert_eq!(flow_hash(&plain), flow_hash(&decorated));
    assert_eq!(
        plain.module.behavior_version(),
        decorated.module.behavior_version()
    );
}

// --- T037: permutation and metadata invariance (SC-005) -----------------------------------------

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 1000,
        rng_seed: RngSeed::Fixed(502),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn shuffled(n: usize) -> impl Strategy<Value = Vec<usize>> {
    Just((0..n).collect::<Vec<_>>()).prop_shuffle()
}

fn permute<T: Clone>(v: &mut Vec<T>, order: &[usize]) {
    let old = v.clone();
    *v = order.iter().map(|&i| old[i].clone()).collect();
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn order_and_metadata_never_reach_identity(
        payload in shuffled(2),
        creations in shuffled(2),
        transitions in shuffled(6),
        inputs in shuffled(2),
        cancel_from in shuffled(3),
        meta in 0u64..1000,
    ) {
        let reference = build_reference();
        let mut f = flow();
        permute(&mut f.payload, &payload);
        permute(&mut f.creations, &creations);
        permute(&mut f.transitions, &transitions);
        for c in &mut f.creations {
            permute(&mut c.inputs, &inputs);
        }
        let cancel = f.transitions.iter_mut().find(|t| t.transition_id == "cancel").unwrap();
        permute(&mut cancel.from_states, &cancel_from);
        decorate(&mut f, meta);
        let out = build(f, source());
        prop_assert_eq!(flow_hash(&out), flow_hash(&reference));
        prop_assert_eq!(out.module.behavior_version(), reference.module.behavior_version());
    }
}
