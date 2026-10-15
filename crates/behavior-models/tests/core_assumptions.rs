//! Assumptions about the pinned Core Release that the model crate's design relies on
//! (feature 501, research R5 and R8; analysis U1 and U2). They are kept as permanent tests: a
//! Core Release that breaks one fails here first, with a clear name.

#![allow(clippy::unwrap_used, clippy::expect_used)] // tests (constitution: Technology Constraints)

use behavior_engine::builder::{Builder, ScopeSite};
use behavior_engine::semantic::types::hash_display;
use behavior_engine::wire::{Loc, Role, WField, WLifecycle, WParam, WType};
use serde_json::json;

fn loc() -> Loc {
    Loc {
        file: "core_assumptions.rs".into(),
        line: 1,
    }
}

fn status() -> WType {
    WType::Enum("RequestStatus".into())
}

/// Declares `RequestStatus` and `PurchaseRequest { status, note }`.
fn declarations() -> Builder {
    let mut b = Builder::new();
    b.declare_enum(
        "RequestStatus",
        vec!["Draft".into(), "Submitted".into(), "Approved".into()],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "PurchaseRequest",
        vec![
            WField {
                name: "status".into(),
                ty: status(),
                loc: loc(),
            },
            WField {
                name: "note".into(),
                ty: WType::String,
                loc: loc(),
            },
        ],
        loc(),
    )
    .unwrap();
    b
}

fn state_param() -> WParam {
    WParam {
        name: "purchase_request".into(),
        role: Some(Role::State),
        ty: WType::Entity("PurchaseRequest".into()),
    }
}

/// An action whose preconditions, effects and postconditions are distinguishable by shape:
/// precondition 0 is `In` (one child), precondition 1 is `==` (two children); effect 0 writes
/// `status`, effect 1 writes `note`; postcondition 0 is `==`, postcondition 1 is `In`.
fn add_ordered_action(b: &mut Builder) {
    b.push_scope(ScopeSite::Action, vec![state_param()], loc())
        .unwrap();
    let f = b.field("purchase_request", "status", loc()).unwrap();
    let pre0 = b.in_(f.clone(), vec![json!("Draft")], loc()).unwrap();
    let note = b.field("purchase_request", "note", loc()).unwrap();
    let empty = b.lit(WType::String, json!(""), loc()).unwrap();
    let pre1 = b.op("eq", vec![note, empty], loc()).unwrap();
    let submitted = b.lit(status(), json!("Submitted"), loc()).unwrap();
    let text = b.lit(WType::String, json!("x"), loc()).unwrap();
    let post0 = b
        .op("eq", vec![f.clone(), submitted.clone()], loc())
        .unwrap();
    let post1 = b.in_(f, vec![json!("Submitted")], loc()).unwrap();
    b.add_action(
        "ordered",
        vec![state_param()],
        vec![(pre0, loc()), (pre1, loc())],
        vec![
            ("purchase_request".into(), "status".into(), submitted, loc()),
            ("purchase_request".into(), "note".into(), text, loc()),
        ],
        vec![],
        vec![(post0, loc()), (post1, loc())],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// A creating action `open(id: Input Id<PurchaseRequest>)` with `status = Draft`.
fn add_create_action(b: &mut Builder, initial: &str) {
    let id_param = WParam {
        name: "id".into(),
        role: Some(Role::Input),
        ty: WType::Id("PurchaseRequest".into()),
    };
    b.push_scope(ScopeSite::Action, vec![id_param.clone()], loc())
        .unwrap();
    let id = b.param("id", loc()).unwrap();
    let initial = b.lit(status(), json!(initial), loc()).unwrap();
    let note = b.lit(WType::String, json!(""), loc()).unwrap();
    let create = WLifecycle::Create {
        entity: "PurchaseRequest".into(),
        id: id.into_wire(),
        fields: vec![
            ("status".into(), initial.into_wire()),
            ("note".into(), note.into_wire()),
        ],
        loc: loc(),
    };
    b.add_action(
        "open",
        vec![id_param],
        vec![],
        vec![],
        vec![create],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

/// U2: an admitted action keeps the given order of its preconditions, effects and
/// postconditions (fragment identities are read by position, identity contract §2).
#[test]
fn admitted_action_keeps_fragment_order() {
    let mut b = declarations();
    add_ordered_action(&mut b);
    let module = b.finish(None).unwrap();
    let action = &module.actions()["ordered"];

    let pre: Vec<usize> = action
        .preconditions()
        .iter()
        .map(|c| c.expr().children().len())
        .collect();
    assert_eq!(pre, vec![1, 2], "precondition order");

    let effects: Vec<&str> = action.effects().iter().map(|e| e.field()).collect();
    assert_eq!(effects, vec!["status", "note"], "effect order");

    let post: Vec<usize> = action
        .postconditions()
        .iter()
        .map(|c| c.expr().children().len())
        .collect();
    assert_eq!(post, vec![2, 1], "postcondition order");
}

/// U1: Core's semantic hash of an enum literal can be obtained from a throwaway probe module
/// that re-declares the same enum, and equals the hash of the same literal inside a real
/// creation (research R8, fallback 1). `Builder::for_module` cannot be used: its `finish`
/// admits only the builder's own declarations, so the enum is unknown there. Enum literals hash
/// through the enum declaration's hash, so equal declarations give equal literal hashes; the
/// probe checks that equality explicitly.
#[test]
fn entry_literal_probe_matches_creation_literal() {
    let mut b = declarations();
    add_create_action(&mut b, "Draft");
    let module = b.finish(None).unwrap();
    let created = &module.actions()["open"].creates()[0];
    let (_, initial) = created
        .fields()
        .iter()
        .find(|(name, _)| name == "status")
        .unwrap();

    let info = &module.enums()["RequestStatus"];
    let mut probe = Builder::new();
    probe
        .declare_enum(&info.name, info.values.clone(), loc())
        .unwrap();
    probe.push_scope(ScopeSite::Closed, vec![], loc()).unwrap();
    for (i, case) in ["Draft", "Submitted"].iter().enumerate() {
        let a = probe.lit(status(), json!(case), loc()).unwrap();
        let b2 = probe.lit(status(), json!(case), loc()).unwrap();
        let eq = probe.op("eq", vec![a, b2], loc()).unwrap();
        probe
            .add_global_invariant(&format!("__probe_{i}"), eq, loc())
            .unwrap();
    }
    probe.pop_scope();
    let probed = probe
        .finish(None)
        .unwrap_or_else(|r| panic!("probe module not admitted: {:?}", r.errors));
    assert_eq!(
        probed.enums()["RequestStatus"].hash,
        info.hash,
        "the probe declares the identical enum"
    );
    let hashes: Vec<_> = (0..2)
        .map(|i| {
            *probed.global_invariants()[&format!("__probe_{i}")]
                .body()
                .children()[0]
                .hash()
        })
        .collect();
    assert_eq!(
        hash_display(&hashes[0]),
        hash_display(initial.hash()),
        "Draft literal"
    );
    assert_ne!(hashes[0], hashes[1], "distinct cases hash differently");
}

/// U1 (negative): the first probe design, `Builder::for_module`, does not admit literals of the
/// module's enums. Kept so a Core Release that changes this is noticed (the simpler probe would
/// then become possible).
#[test]
fn for_module_probe_does_not_know_module_enums() {
    let module = declarations().finish(None).unwrap();
    let mut probe = Builder::for_module(&module);
    probe.push_scope(ScopeSite::Closed, vec![], loc()).unwrap();
    let a = probe.lit(status(), json!("Draft"), loc()).unwrap();
    let b2 = probe.lit(status(), json!("Draft"), loc()).unwrap();
    let eq = probe.op("eq", vec![a, b2], loc()).unwrap();
    probe.add_global_invariant("__probe", eq, loc()).unwrap();
    probe.pop_scope();
    let errors = probe.finish(None).err().unwrap().errors;
    assert!(errors.iter().all(|e| e.code == "UNKNOWN_TYPE"));
}

// --- Feature 504 (US2, research R7): command-profile actions that observe a query commit ------

use behavior_engine::builder::SemanticProfile;
use behavior_engine::semantic::Module;
use behavior_engine::store::documents::{CommitBundle, SeedEntity};
use behavior_engine::store::{InMemoryBackend, Store};

const T: &str = "2026-10-15T00:00:00Z";

/// `Item { name }`, `Mark { item: Id<Item> }`, command `Ping { item: Id<Item> }`, and
/// `mark(item: State Item, mark_id: Input Id<Mark>)` with the single precondition
/// `¬any(select(Mark), m → m.item == item.id)`, creating `Mark { item: item.id }`.
fn query_module() -> Module {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    let field = |name: &str, ty: WType| WField {
        name: name.into(),
        ty,
        loc: loc(),
    };
    b.declare_entity("Item", vec![field("name", WType::String)], loc())
        .unwrap();
    b.declare_entity("Mark", vec![field("item", WType::Id("Item".into()))], loc())
        .unwrap();
    b.add_command("Ping", vec![field("item", WType::Id("Item".into()))], loc())
        .unwrap();
    let params = vec![
        WParam {
            name: "item".into(),
            role: Some(Role::State),
            ty: WType::Entity("Item".into()),
        },
        WParam {
            name: "mark_id".into(),
            role: Some(Role::Input),
            ty: WType::Id("Mark".into()),
        },
    ];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let marks = b.select("Mark", loc()).unwrap();
    b.push_lambda("m", "Mark").unwrap();
    let marked = b.field("m", "item", loc()).unwrap();
    let id = b.field("item", "id", loc()).unwrap();
    let same = b.op("eq", vec![marked, id], loc()).unwrap();
    b.pop_scope();
    let any = b.lambda("any", marks, "m", same, loc()).unwrap();
    let fresh = b.op("not", vec![any], loc()).unwrap();
    let mark_id = b.param("mark_id", loc()).unwrap();
    let item_id = b.field("item", "id", loc()).unwrap();
    let create = WLifecycle::Create {
        entity: "Mark".into(),
        id: mark_id.into_wire(),
        fields: vec![("item".into(), item_id.into_wire())],
        loc: loc(),
    };
    b.add_action(
        "mark",
        params,
        vec![(fresh, loc())],
        vec![],
        vec![create],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    b.finish(None)
        .unwrap_or_else(|r| panic!("not admitted: {:?}", r.errors))
}

/// A v2 store (evidence policy `none`) with item `i1`.
fn query_store(module: &Module) -> Store<InMemoryBackend> {
    let policy = behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#,
    )
    .unwrap();
    let seed = vec![SeedEntity {
        entity: "Item".into(),
        value: json!({"id": "i1", "name": "i1"}),
    }];
    let genesis = behavior_engine::store::store::genesis_v2_for(module, policy, seed).unwrap();
    Store::create(InMemoryBackend::default(), module, genesis).unwrap()
}

/// Invokes `mark` on `i1` creating `mark_id`: the candidate bundle, if the action is allowed.
fn invoke_mark(
    store: &Store<InMemoryBackend>,
    module: &Module,
    mark_id: &str,
) -> Option<CommitBundle> {
    let document = json!({
        "format": "behavior.invocation.v1",
        "capability": "mark",
        "bindings": {"item": {"entity": "Item", "id": "i1"}},
        "input": {"mark_id": mark_id},
        "context": {},
    });
    let (requested, errors) =
        behavior_engine::invocation::RequestedInvocation::decode(&document.to_string()).unwrap();
    let requested = requested.unwrap_or_else(|| panic!("{errors:?}"));
    store
        .invoke(module, &requested, T, None, None)
        .unwrap()
        .bundle
}

fn commit(
    store: &mut Store<InMemoryBackend>,
    module: &Module,
    bundle: &CommitBundle,
) -> Result<(), String> {
    let head = store.current().unwrap();
    store
        .commit(module, &head, bundle)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// US2 scenario 1: allowed, commits, and the committed state shows the mark. Core 0.12.0
/// refused this candidate with `INVALID_CANDIDATE: invalid observed facts` (503 research R13);
/// fixed by Core 023, released in 0.13.0.
#[test]
fn command_profile_query_action_commits() {
    let module = query_module();
    let mut store = query_store(&module);
    let bundle = invoke_mark(&store, &module, "m1").expect("the first mark is allowed");
    assert_eq!(commit(&mut store, &module, &bundle), Ok(()));
    let head = store.current().unwrap();
    assert_ne!(head, bundle.evaluated_state, "the commit advanced the head");
}

/// US2 scenario 2: once marked, the query makes the precondition false: no candidate, and the
/// head does not move.
#[test]
fn command_profile_query_action_refused_without_commit() {
    let module = query_module();
    let mut store = query_store(&module);
    let first = invoke_mark(&store, &module, "m1").unwrap();
    commit(&mut store, &module, &first).unwrap();
    let head = store.current().unwrap();
    assert!(
        invoke_mark(&store, &module, "m2").is_none(),
        "second mark refused"
    );
    assert_eq!(store.current().unwrap(), head);
}

/// US2 scenario 3: a candidate whose query evidence was altered is refused at commit, and the
/// head does not move.
#[test]
fn command_profile_altered_query_evidence_refused() {
    let module = query_module();
    let mut store = query_store(&module);
    let mut bundle = invoke_mark(&store, &module, "m1").unwrap();
    let queries = bundle.read_facts["queries"]
        .as_array_mut()
        .expect("the candidate carries query evidence");
    let hash = queries[0]["result_hash"]
        .as_str()
        .expect("query evidence has a result hash")
        .to_string();
    let flipped = if hash.ends_with('0') { "1" } else { "0" };
    queries[0]["result_hash"] = json!(format!("{}{flipped}", &hash[..hash.len() - 1]));
    let head = store.current().unwrap();
    assert!(
        commit(&mut store, &module, &bundle).is_err(),
        "altered evidence refused"
    );
    assert_eq!(store.current().unwrap(), head);
}
