#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]
use behavior_engine::{
    builder::{Builder, Node, ScopeSite, SemanticProfile},
    semantic::Module,
    wire::{Loc, Role, WField, WLifecycle, WParam, WType},
};
use behavior_models::safety::{FieldOwnershipInput, LifecycleDependencyInput};
use serde_json::json;

pub fn loc() -> Loc {
    Loc {
        file: "lifecycle_fixture.rs".into(),
        line: 1,
    }
}
fn field(name: &str, ty: WType) -> WField {
    WField {
        name: name.into(),
        ty,
        loc: loc(),
    }
}
pub fn state(name: &str, entity: &str) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::State),
        ty: WType::Entity(entity.into()),
    }
}
fn input(name: &str, entity: &str) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::Input),
        ty: WType::Id(entity.into()),
    }
}

#[derive(Default, Clone, Debug)]
pub struct Options {
    pub two_children: bool,
    pub missing_guard: bool,
    pub missing_second_guard: bool,
    pub computed: bool,
    pub owner_assignment: bool,
    pub owner_noop: bool,
    pub double_relevant: bool,
    pub mixed_creation: bool,
    pub unrelated_write: bool,
}
pub fn declarations(owner: WType, two: bool) -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum(
        "ParentPhase",
        vec!["Open".into(), "Gone".into(), "Abandoned".into()],
        loc(),
    )
    .unwrap();
    b.declare_enum(
        "ChildPhase",
        vec!["Active".into(), "Terminal".into()],
        loc(),
    )
    .unwrap();
    b.declare_entity(
        "Parent",
        vec![
            field("phase", WType::Enum("ParentPhase".into())),
            field("note", WType::String),
        ],
        loc(),
    )
    .unwrap();
    for name in if two {
        vec!["Child", "OtherChild"]
    } else {
        vec!["Child"]
    } {
        b.declare_entity(
            name,
            vec![
                field("phase", WType::Enum("ChildPhase".into())),
                field("owner", owner.clone()),
            ],
            loc(),
        )
        .unwrap();
    }
    b
}
fn membership(b: &mut Builder, param: &str, field: &str, values: &[&str], negated: bool) -> Node {
    let value = b.field(param, field, loc()).unwrap();
    let test = b
        .in_(value, values.iter().map(|v| json!(v)).collect(), loc())
        .unwrap();
    if negated {
        b.op("not", vec![test], loc()).unwrap()
    } else {
        test
    }
}
pub fn no_children(b: &mut Builder, child: &str) -> Node {
    let query = b.select(child, loc()).unwrap();
    b.push_lambda("dependent", child).unwrap();
    let owner = b.field("dependent", "owner", loc()).unwrap();
    let id = b.field("p", "id", loc()).unwrap();
    let equals = b.op("eq", vec![owner, id], loc()).unwrap();
    let active = membership(b, "dependent", "phase", &["Terminal"], true);
    let body = b.op("and", vec![equals, active], loc()).unwrap();
    b.pop_scope();
    let any = b.lambda("any", query, "dependent", body, loc()).unwrap();
    b.op("not", vec![any], loc()).unwrap()
}
fn permitted(b: &mut Builder) -> Node {
    membership(b, "p", "phase", &["Abandoned", "Gone"], true)
}
fn child_fields(b: &mut Builder) -> Vec<(String, Node)> {
    // The test fixture alone uses Core's public lifecycle transport constructors.
    vec![
        ("owner".into(), b.field("p", "id", loc()).unwrap()),
        (
            "phase".into(),
            b.lit(WType::Enum("ChildPhase".into()), json!("Active"), loc())
                .unwrap(),
        ),
    ]
}

pub fn module(options: &Options) -> Module {
    let mut b = declarations(WType::Ref("Parent".into()), options.two_children);
    let families = if options.two_children {
        vec!["Child", "OtherChild"]
    } else {
        vec!["Child"]
    };
    let params = vec![state("p", "Parent")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let guards = families
        .iter()
        .enumerate()
        .filter(|(i, _)| !options.missing_guard && (!options.missing_second_guard || *i == 0))
        .map(|(_, child)| (no_children(&mut b, child), loc()))
        .collect();
    let gone = b
        .lit(WType::Enum("ParentPhase".into()), json!("Gone"), loc())
        .unwrap();
    let mut effects = vec![("p".into(), "phase".into(), gone, loc())];
    if options.unrelated_write {
        let note = b.lit(WType::String, json!("changed"), loc()).unwrap();
        effects.push(("p".into(), "note".into(), note, loc()));
    }
    b.add_action("stop", params, guards, effects, vec![], vec![], loc())
        .unwrap();
    b.pop_scope();
    let params = vec![state("p", "Parent")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let open = b
        .lit(WType::Enum("ParentPhase".into()), json!("Open"), loc())
        .unwrap();
    b.add_action(
        "reopen",
        params,
        vec![],
        vec![("p".into(), "phase".into(), open, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let params = vec![input("new_id", "Parent")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let id = b.param("new_id", loc()).unwrap().into_wire();
    let phase = b
        .lit(WType::Enum("ParentPhase".into()), json!("Open"), loc())
        .unwrap()
        .into_wire();
    let note = b.lit(WType::String, json!(""), loc()).unwrap().into_wire();
    b.add_action(
        "create_parent",
        params,
        vec![],
        vec![],
        vec![WLifecycle::Create {
            entity: "Parent".into(),
            id,
            fields: vec![("phase".into(), phase), ("note".into(), note)],
            loc: loc(),
        }],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let params = vec![state("p", "Parent")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    b.add_action(
        "remove_parent",
        params,
        vec![],
        vec![],
        vec![WLifecycle::Remove {
            param: "p".into(),
            loc: loc(),
        }],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    for (family, child) in families.iter().enumerate() {
        let prefix = if family == 0 { "" } else { "other_" };
        let params = vec![state("c", child), state("p", "Parent")];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let owner = b.field("c", "owner", loc()).unwrap();
        let id = b.field("p", "id", loc()).unwrap();
        let equals = b.op("eq", vec![owner, id], loc()).unwrap();
        let permission = permitted(&mut b);
        let mut active = b
            .lit(WType::Enum("ChildPhase".into()), json!("Active"), loc())
            .unwrap();
        if options.computed {
            let some = b.op("some", vec![active.clone()], loc()).unwrap();
            active = b.op("value_or", vec![some, active], loc()).unwrap();
        }
        b.add_action(
            &format!("{prefix}activate"),
            params,
            vec![(equals, loc()), (permission, loc())],
            vec![("c".into(), "phase".into(), active, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
        let params = vec![state("c", child)];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let terminal = b
            .lit(WType::Enum("ChildPhase".into()), json!("Terminal"), loc())
            .unwrap();
        b.add_action(
            &format!("{prefix}finish"),
            params,
            vec![],
            vec![("c".into(), "phase".into(), terminal, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
        let params = vec![state("p", "Parent"), input("new_id", child)];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let permission = permitted(&mut b);
        let id = b.param("new_id", loc()).unwrap().into_wire();
        let fields = child_fields(&mut b)
            .into_iter()
            .map(|(f, v)| (f, v.into_wire()))
            .collect();
        b.add_action(
            &format!("{prefix}create_child"),
            params,
            vec![(permission, loc())],
            vec![],
            vec![WLifecycle::Create {
                entity: (*child).into(),
                id,
                fields,
                loc: loc(),
            }],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
        let params = vec![state("c", child)];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        b.add_action(
            &format!("{prefix}remove_child"),
            params,
            vec![],
            vec![],
            vec![WLifecycle::Remove {
                param: "c".into(),
                loc: loc(),
            }],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    if options.owner_assignment || options.owner_noop {
        let params = vec![state("c", "Child"), state("p", "Parent")];
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let owner = b
            .field(
                if options.owner_noop { "c" } else { "p" },
                if options.owner_noop { "owner" } else { "id" },
                loc(),
            )
            .unwrap();
        b.add_action(
            "change_owner",
            params,
            vec![],
            vec![("c".into(), "owner".into(), owner, loc())],
            vec![],
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    if options.double_relevant || options.mixed_creation {
        let mut params = vec![state("p", "Parent")];
        if options.mixed_creation {
            params.push(input("new_id", "Child"));
        } else {
            params.push(state("c", "Child"));
        }
        b.push_scope(ScopeSite::Action, params.clone(), loc())
            .unwrap();
        let guard = no_children(&mut b, "Child");
        let gone = b
            .lit(WType::Enum("ParentPhase".into()), json!("Gone"), loc())
            .unwrap();
        let mut effects = vec![("p".into(), "phase".into(), gone, loc())];
        let mut creates = vec![];
        if options.mixed_creation {
            let id = b.param("new_id", loc()).unwrap().into_wire();
            let fields = child_fields(&mut b)
                .into_iter()
                .map(|(f, v)| (f, v.into_wire()))
                .collect();
            creates.push(WLifecycle::Create {
                entity: "Child".into(),
                id,
                fields,
                loc: loc(),
            });
        } else {
            let done = b
                .lit(WType::Enum("ChildPhase".into()), json!("Terminal"), loc())
                .unwrap();
            effects.push(("c".into(), "phase".into(), done, loc()));
        }
        b.add_action(
            "mixed",
            params,
            vec![(guard, loc())],
            effects,
            creates,
            vec![],
            loc(),
        )
        .unwrap();
        b.pop_scope();
    }
    b.finish(None).unwrap()
}

pub fn ownership(module: &Module) -> Vec<FieldOwnershipInput> {
    let actions = module.actions().keys().cloned().collect::<Vec<_>>();
    module.entities().keys().map(|entity|FieldOwnershipInput::from_json(&json!({
        "id":if entity=="Parent"{"parent_control"}else if entity=="Child"{"child_control"}else{"other_control"},
        "entity":entity,"writers":if entity=="Parent"{json!({"phase":actions})}else{json!({"phase":actions,"owner":[]})},
        "creators":actions,"removers":actions})).unwrap()).collect()
}
pub fn dependency(child: &str) -> LifecycleDependencyInput {
    LifecycleDependencyInput::from_json(&json!({"id":if child=="Child"{"children"}else{"other_children"},
        "parent":"Parent","child":child,"owner_field":"owner","parent_state":"phase","child_state":"phase",
        "prohibited":["Abandoned","Gone"],"terminal":["Terminal"],
        "ownership":["parent_control",if child=="Child"{"child_control"}else{"other_control"}]})).unwrap()
}

use behavior_engine::store::{InMemoryBackend, Store, documents::SeedEntity};
pub fn store(module: &Module, seed: Vec<SeedEntity>) -> Store<InMemoryBackend> {
    let policy=behavior_engine::verify::governance::EvidencePolicyV2::from_json(
        r#"{"execution_policies":[],"format":"behavior.evidence_policy.v2","require":"none","trusted_authorities":[]}"#).unwrap();
    let genesis = behavior_engine::store::store::genesis_v2_for(module, policy, seed).unwrap();
    Store::create(InMemoryBackend::default(), module, genesis).unwrap()
}
pub fn invoke(
    store: &Store<InMemoryBackend>,
    module: &Module,
    action: &str,
    bindings: serde_json::Value,
    input: serde_json::Value,
) -> behavior_engine::invocation::Invocation {
    let document = json!({"format":"behavior.invocation.v1","capability":action,"bindings":bindings,"input":input,"context":{}});
    let (request, errors) =
        behavior_engine::invocation::RequestedInvocation::decode(&document.to_string()).unwrap();
    store
        .invoke(
            module,
            &request.unwrap_or_else(|| panic!("{errors:?}")),
            "2026-10-18T00:00:00Z",
            None,
            None,
        )
        .unwrap()
}
pub fn snapshot(store: &Store<InMemoryBackend>, module: &Module) -> Vec<SeedEntity> {
    store
        .export_seed_at(
            module,
            &store.history_at(store.current().unwrap().position).unwrap(),
        )
        .unwrap()
        .seed
}
/// Independent oracle: evaluates the original R ∧ P directly, with no Safety evaluator.
pub fn property(seed: &[SeedEntity]) -> bool {
    let parents = seed
        .iter()
        .filter(|e| e.entity == "Parent")
        .map(|e| {
            (
                e.value["id"].as_str().unwrap(),
                e.value["phase"].as_str().unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    seed.iter()
        .filter(|e| e.entity == "Child" || e.entity == "OtherChild")
        .all(|e| {
            parents
                .get(e.value["owner"].as_str().unwrap())
                .is_some_and(|phase| e.value["phase"] == "Terminal" || *phase == "Open")
        })
}
pub fn parent(id: &str, phase: &str) -> SeedEntity {
    SeedEntity {
        entity: "Parent".into(),
        value: json!({"id":id,"phase":phase,"note":""}),
    }
}
pub fn child(entity: &str, id: &str, owner: &str, phase: &str) -> SeedEntity {
    SeedEntity {
        entity: entity.into(),
        value: json!({"id":id,"owner":owner,"phase":phase}),
    }
}
