#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use behavior_engine::builder::{Builder, ScopeSite, SemanticProfile};
use behavior_engine::semantic::Module;
use behavior_engine::wire::{Loc, Role, WField, WLifecycle, WParam, WType};
use serde_json::json;

pub fn loc() -> Loc {
    Loc {
        file: "safety_fixture.rs".into(),
        line: 1,
    }
}

pub fn declarations() -> Builder {
    let mut b = Builder::with_semantic_profile(SemanticProfile::CommandIntents);
    b.declare_enum("Phase", vec!["Idle".into(), "Done".into()], loc())
        .unwrap();
    b.declare_entity(
        "Item",
        vec![
            WField {
                name: "phase".into(),
                ty: WType::Enum("Phase".into()),
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

pub fn state(name: &str) -> WParam {
    WParam {
        name: name.into(),
        role: Some(Role::State),
        ty: WType::Entity("Item".into()),
    }
}

pub fn edit(b: &mut Builder, name: &str, binding: &str, no_op: bool) {
    let params = vec![state(binding)];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let value = if no_op {
        b.field(binding, "note", loc()).unwrap()
    } else {
        b.lit(WType::String, json!("changed"), loc()).unwrap()
    };
    b.add_action(
        name,
        params,
        vec![],
        vec![(binding.into(), "note".into(), value, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
}

pub fn fixture() -> Module {
    let mut b = declarations();
    edit(&mut b, "edit", "renamed_subject", false);
    let params = vec![WParam {
        name: "new_id".into(),
        role: Some(Role::Input),
        ty: WType::Id("Item".into()),
    }];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let id = b.param("new_id", loc()).unwrap().into_wire();
    let phase = b
        .lit(WType::Enum("Phase".into()), json!("Idle"), loc())
        .unwrap()
        .into_wire();
    let note = b.lit(WType::String, json!(""), loc()).unwrap().into_wire();
    b.add_action(
        "create",
        params,
        vec![],
        vec![],
        vec![WLifecycle::Create {
            entity: "Item".into(),
            id,
            fields: vec![("phase".into(), phase), ("note".into(), note)],
            loc: loc(),
        }],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let params = vec![state("doomed")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    b.add_action(
        "remove",
        params,
        vec![],
        vec![],
        vec![WLifecycle::Remove {
            param: "doomed".into(),
            loc: loc(),
        }],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    let params = vec![state("left"), state("right")];
    b.push_scope(ScopeSite::Action, params.clone(), loc())
        .unwrap();
    let note = b.field("right", "note", loc()).unwrap();
    b.add_action(
        "copy",
        params,
        vec![],
        vec![("left".into(), "note".into(), note, loc())],
        vec![],
        vec![],
        loc(),
    )
    .unwrap();
    b.pop_scope();
    b.finish(None).unwrap()
}

pub fn action<'a>(data: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    data["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == name)
        .unwrap()
}

pub fn rehash(data: &mut serde_json::Value) {
    data.as_object_mut().unwrap().remove("inventory_hash");
    let hash =
        behavior_engine::canonical::tagged_hash("behavior.safety_inventory.v1", data).unwrap();
    data["inventory_hash"] = json!(hash);
}
