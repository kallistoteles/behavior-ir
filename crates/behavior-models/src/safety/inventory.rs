//! Complete public-admitted-module mutation inventory. No serialized guard AST inspection.

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::{Module, types::hash_display};
use behavior_engine::wire::WType;
use serde_json::{Map, Value, json};

use super::identity::{INVENTORY_FORMAT, tagged, valid_hash};
use super::input::SafetyError;
use crate::CORE_RELEASE;

fn invalid(detail: impl Into<String>) -> SafetyError {
    SafetyError::new("SAFETY_INVALID_DATA", detail)
}

fn object<'a>(v: &'a Value, keys: &[&str]) -> Result<&'a Map<String, Value>, SafetyError> {
    let m = v.as_object().ok_or_else(|| invalid("expected object"))?;
    if m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)) {
        return Err(invalid(format!("expected exact fields {keys:?}")));
    }
    Ok(m)
}

fn text(v: &Value) -> Result<String, SafetyError> {
    v.as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid("expected nonempty string"))
}

fn hash(v: &Value) -> Result<String, SafetyError> {
    let s = text(v)?;
    if !valid_hash(&s) {
        return Err(invalid("invalid semantic hash"));
    }
    Ok(s)
}

fn array(v: &Value) -> Result<&Vec<Value>, SafetyError> {
    v.as_array().ok_or_else(|| invalid("expected array"))
}

fn wire_type(v: &Value) -> Result<WType, SafetyError> {
    // This is the inventory's transport codec for public WType constructors, not Core
    // type checking or its wire-module decoder. Core admission alone establishes meaning.
    if let Some(s) = v.as_str() {
        return match s {
            "Bool" => Ok(WType::Bool),
            "Int" => Ok(WType::Int),
            "Decimal" => Ok(WType::Decimal),
            "String" => Ok(WType::String),
            _ => Err(invalid("unknown inventory type constructor")),
        };
    }
    let m = v
        .as_object()
        .filter(|m| m.len() == 1)
        .ok_or_else(|| invalid("expected inventory type constructor"))?;
    let (kind, value) = m
        .iter()
        .next()
        .ok_or_else(|| invalid("empty type constructor"))?;
    match kind.as_str() {
        "Option" => Ok(WType::Option(Box::new(wire_type(value)?))),
        "Enum" => Ok(WType::Enum(text(value)?)),
        "Nominal" => Ok(WType::Nominal(text(value)?)),
        "Id" => Ok(WType::Id(text(value)?)),
        "Ref" => Ok(WType::Ref(text(value)?)),
        "Entity" => Ok(WType::Entity(text(value)?)),
        "Exact" if value.is_null() => Ok(WType::Exact(None)),
        "Exact" => Ok(WType::Exact(Some(text(value)?))),
        _ => Err(invalid("unknown inventory type constructor")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldData {
    pub ty: WType,
    pub reference: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityData {
    pub hash: String,
    pub fields: BTreeMap<String, FieldData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterData {
    pub name: String,
    pub ty: WType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Assign {
        index: usize,
        hash: String,
        param: String,
        entity: String,
        field: String,
        value: String,
    },
    Create {
        index: usize,
        hash: String,
        entity: String,
        id: String,
        fields: BTreeMap<String, String>,
    },
    Remove {
        index: usize,
        hash: String,
        param: String,
        entity: String,
    },
}

impl Operation {
    pub fn entity(&self) -> &str {
        match self {
            Self::Assign { entity, .. }
            | Self::Create { entity, .. }
            | Self::Remove { entity, .. } => entity,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Assign { .. } => "assign",
            Self::Create { .. } => "create",
            Self::Remove { .. } => "remove",
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Self::Assign {
                index,
                hash,
                param,
                entity,
                field,
                value,
            } => {
                json!({"kind":"assign","index":index,"hash":hash,"param":param,"entity":entity,"field":field,"value":value})
            }
            Self::Create {
                index,
                hash,
                entity,
                id,
                fields,
            } => {
                json!({"kind":"create","index":index,"hash":hash,"entity":entity,"id":id,"fields":fields})
            }
            Self::Remove {
                index,
                hash,
                param,
                entity,
            } => json!({"kind":"remove","index":index,"hash":hash,"param":param,"entity":entity}),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionData {
    pub name: String,
    pub hash: String,
    pub params: Vec<ParameterData>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub operations: Vec<Operation>,
}

impl ActionData {
    pub fn to_json(&self) -> Value {
        json!({"name":self.name,"hash":self.hash,
            "params":self.params.iter().map(|p| json!({"name":p.name,"type":p.ty})).collect::<Vec<_>>(),
            "preconditions":self.preconditions,"postconditions":self.postconditions,
            "operations":self.operations.iter().map(Operation::to_json).collect::<Vec<_>>()})
    }

    fn target(&self, param: &str) -> Result<String, SafetyError> {
        match self.params.iter().find(|p| p.name == param).map(|p| &p.ty) {
            Some(WType::Entity(entity)) => Ok(entity.clone()),
            _ => Err(invalid(format!(
                "{}: mutation target {param} is not an entity binding",
                self.name
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    behavior_hash: String,
    entities: BTreeMap<String, EntityData>,
    actions: BTreeMap<String, ActionData>,
    inventory_hash: String,
}

impl Inventory {
    pub fn behavior_hash(&self) -> &str {
        &self.behavior_hash
    }
    pub fn entities(&self) -> &BTreeMap<String, EntityData> {
        &self.entities
    }
    pub fn actions(&self) -> &BTreeMap<String, ActionData> {
        &self.actions
    }
    pub fn hash(&self) -> &str {
        &self.inventory_hash
    }

    fn payload(&self) -> Value {
        let declarations: BTreeMap<_, _> = self
            .entities
            .iter()
            .map(|(name, e)| {
                let fields: BTreeMap<_, _> = e
                    .fields
                    .iter()
                    .map(|(name, f)| (name, json!({"type":f.ty,"reference":f.reference})))
                    .collect();
                (name, json!({"hash":e.hash,"fields":fields}))
            })
            .collect();
        json!({"format":INVENTORY_FORMAT,
            "core_release":{"version":CORE_RELEASE.version,"commit":CORE_RELEASE.commit},
            "behavior_hash":self.behavior_hash,"declarations":declarations,
            "actions":self.actions.values().map(ActionData::to_json).collect::<Vec<_>>()})
    }

    pub fn to_json(&self) -> Value {
        let mut value = self.payload();
        value["inventory_hash"] = json!(self.inventory_hash);
        value
    }

    pub fn extract(module: &Module) -> Result<Self, SafetyError> {
        let mut entities = BTreeMap::new();
        for (name, entity) in module.entities() {
            let declaration_hash = module
                .name_table()
                .iter()
                .find(|((kind, n), _)| kind.as_str() == "entity" && n == name)
                .map(|(_, h)| hash_display(h))
                .ok_or_else(|| invalid("entity declaration identity missing"))?;
            let fields = entity
                .fields()
                .iter()
                .map(|(field, ty)| {
                    (
                        field.clone(),
                        FieldData {
                            ty: ty.to_wire_type(),
                            reference: entity.is_reference(field),
                        },
                    )
                })
                .collect();
            entities.insert(
                name.clone(),
                EntityData {
                    hash: declaration_hash,
                    fields,
                },
            );
        }
        let mut actions = BTreeMap::new();
        for (name, action) in module.actions() {
            let mut row = ActionData {
                name: name.clone(),
                hash: hash_display(action.hash()),
                params: action
                    .params()
                    .iter()
                    .map(|p| ParameterData {
                        name: p.name().into(),
                        ty: p.ty().to_wire_type(),
                    })
                    .collect(),
                preconditions: action
                    .preconditions()
                    .iter()
                    .map(|c| hash_display(c.expr().hash()))
                    .collect(),
                postconditions: action
                    .postconditions()
                    .iter()
                    .map(|c| hash_display(c.expr().hash()))
                    .collect(),
                operations: Vec::new(),
            };
            for (index, effect) in action.effects().iter().enumerate() {
                row.operations.push(Operation::Assign {
                    index,
                    hash: hash_display(effect.hash()),
                    param: effect.param().into(),
                    entity: row.target(effect.param())?,
                    field: effect.field().into(),
                    value: hash_display(effect.value().hash()),
                });
            }
            for (index, effect) in action.creates().iter().enumerate() {
                row.operations.push(Operation::Create {
                    index,
                    hash: hash_display(effect.hash()),
                    entity: effect.entity().into(),
                    id: hash_display(effect.id().hash()),
                    fields: effect
                        .fields()
                        .iter()
                        .map(|(f, v)| (f.clone(), hash_display(v.hash())))
                        .collect(),
                });
            }
            for (index, effect) in action.removes().iter().enumerate() {
                row.operations.push(Operation::Remove {
                    index,
                    hash: hash_display(effect.hash()),
                    param: effect.param().into(),
                    entity: row.target(effect.param())?,
                });
            }
            actions.insert(name.clone(), row);
        }
        let mut inventory = Self {
            behavior_hash: module.behavior_version(),
            entities,
            actions,
            inventory_hash: String::new(),
        };
        inventory.validate()?;
        inventory.inventory_hash = tagged(INVENTORY_FORMAT, &inventory.payload())?;
        Ok(inventory)
    }

    fn validate(&self) -> Result<(), SafetyError> {
        let mut hashes = BTreeSet::new();
        for action in self.actions.values() {
            if !hashes.insert(&action.hash) {
                return Err(invalid(format!(
                    "ambiguous duplicate action identity {}",
                    action.hash
                )));
            }
            let mut names = BTreeSet::new();
            for p in &action.params {
                if !names.insert(&p.name) {
                    return Err(invalid("duplicate action parameter"));
                }
                if let WType::Entity(e) = &p.ty
                    && !self.entities.contains_key(e)
                {
                    return Err(invalid("unknown parameter entity"));
                }
            }
            let mut counts = BTreeMap::<&str, usize>::new();
            for op in &action.operations {
                let entity = self
                    .entities
                    .get(op.entity())
                    .ok_or_else(|| invalid("unknown operation entity"))?;
                let index = match op {
                    Operation::Assign { index, .. }
                    | Operation::Create { index, .. }
                    | Operation::Remove { index, .. } => *index,
                };
                let next = counts.entry(op.kind()).or_default();
                if index != *next {
                    return Err(invalid("duplicate or missing operation occurrence index"));
                }
                *next += 1;
                match op {
                    Operation::Assign {
                        param,
                        field,
                        entity: target,
                        ..
                    } => {
                        if action.target(param)? != *target || !entity.fields.contains_key(field) {
                            return Err(invalid("invalid assignment target"));
                        }
                    }
                    Operation::Remove {
                        param,
                        entity: target,
                        ..
                    } => {
                        if action.target(param)? != *target {
                            return Err(invalid("invalid removal target"));
                        }
                    }
                    Operation::Create { fields, .. } => {
                        let expected: BTreeSet<_> = entity
                            .fields
                            .keys()
                            .filter(|f| f.as_str() != "id")
                            .collect();
                        if fields.keys().collect::<BTreeSet<_>>() != expected {
                            return Err(invalid("incomplete creation initializer"));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse typed canonical inventory data, without granting correspondence or certification.
    pub fn from_json(data: &Value) -> Result<Self, SafetyError> {
        object(
            data,
            &[
                "format",
                "core_release",
                "behavior_hash",
                "declarations",
                "actions",
                "inventory_hash",
            ],
        )?;
        if data["format"] != INVENTORY_FORMAT {
            return Err(invalid("unsupported inventory format"));
        }
        object(&data["core_release"], &["version", "commit"])?;
        if data["core_release"]["version"] != CORE_RELEASE.version
            || data["core_release"]["commit"] != CORE_RELEASE.commit
        {
            return Err(invalid("wrong Core release"));
        }
        let declarations = data["declarations"]
            .as_object()
            .ok_or_else(|| invalid("expected declarations"))?;
        let mut entities = BTreeMap::new();
        for (name, e) in declarations {
            object(e, &["hash", "fields"])?;
            let fs = e["fields"]
                .as_object()
                .ok_or_else(|| invalid("expected field map"))?;
            let mut fields = BTreeMap::new();
            for (field, f) in fs {
                object(f, &["type", "reference"])?;
                fields.insert(
                    field.clone(),
                    FieldData {
                        ty: wire_type(&f["type"])?,
                        reference: f["reference"]
                            .as_bool()
                            .ok_or_else(|| invalid("expected reference Boolean"))?,
                    },
                );
            }
            entities.insert(
                name.clone(),
                EntityData {
                    hash: hash(&e["hash"])?,
                    fields,
                },
            );
        }
        let mut actions = BTreeMap::new();
        for a in array(&data["actions"])? {
            object(
                a,
                &[
                    "name",
                    "hash",
                    "params",
                    "preconditions",
                    "postconditions",
                    "operations",
                ],
            )?;
            let mut params = Vec::new();
            for p in array(&a["params"])? {
                object(p, &["name", "type"])?;
                params.push(ParameterData {
                    name: text(&p["name"])?,
                    ty: wire_type(&p["type"])?,
                });
            }
            let mut operations = Vec::new();
            for o in array(&a["operations"])? {
                let index = o["index"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| invalid("expected integer occurrence index"))?;
                let effect_hash = hash(&o["hash"])?;
                let entity = text(&o["entity"])?;
                operations.push(match o["kind"].as_str() {
                    Some("assign") => {
                        object(
                            o,
                            &["kind", "index", "hash", "entity", "param", "field", "value"],
                        )?;
                        Operation::Assign {
                            index,
                            hash: effect_hash,
                            entity,
                            param: text(&o["param"])?,
                            field: text(&o["field"])?,
                            value: hash(&o["value"])?,
                        }
                    }
                    Some("remove") => {
                        object(o, &["kind", "index", "hash", "entity", "param"])?;
                        Operation::Remove {
                            index,
                            hash: effect_hash,
                            entity,
                            param: text(&o["param"])?,
                        }
                    }
                    Some("create") => {
                        object(o, &["kind", "index", "hash", "entity", "id", "fields"])?;
                        let fs = o["fields"]
                            .as_object()
                            .ok_or_else(|| invalid("expected initializer map"))?;
                        let fields = fs
                            .iter()
                            .map(|(f, h)| Ok((f.clone(), hash(h)?)))
                            .collect::<Result<_, SafetyError>>()?;
                        Operation::Create {
                            index,
                            hash: effect_hash,
                            entity,
                            id: hash(&o["id"])?,
                            fields,
                        }
                    }
                    _ => return Err(invalid("unknown operation form")),
                });
            }
            let row = ActionData {
                name: text(&a["name"])?,
                hash: hash(&a["hash"])?,
                params,
                preconditions: array(&a["preconditions"])?
                    .iter()
                    .map(hash)
                    .collect::<Result<_, _>>()?,
                postconditions: array(&a["postconditions"])?
                    .iter()
                    .map(hash)
                    .collect::<Result<_, _>>()?,
                operations,
            };
            if actions.insert(row.name.clone(), row).is_some() {
                return Err(invalid("duplicate action declaration"));
            }
        }
        let inventory = Self {
            behavior_hash: hash(&data["behavior_hash"])?,
            entities,
            actions,
            inventory_hash: hash(&data["inventory_hash"])?,
        };
        inventory.validate()?;
        if tagged(INVENTORY_FORMAT, &inventory.payload())? != inventory.inventory_hash
            || inventory.to_json() != *data
        {
            return Err(invalid(
                "inventory identity or canonical representation differs",
            ));
        }
        Ok(inventory)
    }

    /// Audit directly against the admitted module instead of trusting extraction's output.
    /// This separate check remains trusted Rust correspondence, not a formal proof of Rust.
    pub fn verify_module(&self, module: &Module) -> Result<(), SafetyError> {
        let mismatch = || {
            SafetyError::new(
                "SAFETY_INVENTORY_MISMATCH",
                "inventory does not cover the exact admitted module",
            )
        };
        if self.behavior_hash != module.behavior_version()
            || self.entities.len() != module.entities().len()
            || self.actions.len() != module.actions().len()
        {
            return Err(mismatch());
        }
        for (name, actual) in module.entities() {
            let row = self.entities.get(name).ok_or_else(mismatch)?;
            let declaration = module
                .name_table()
                .iter()
                .find(|((k, n), _)| k.as_str() == "entity" && n == name)
                .map(|(_, h)| hash_display(h));
            if declaration.as_deref() != Some(row.hash.as_str())
                || row.fields.len() != actual.fields().len()
            {
                return Err(mismatch());
            }
            for (field, ty) in actual.fields() {
                let f = row.fields.get(field).ok_or_else(mismatch)?;
                if f.ty != ty.to_wire_type() || f.reference != actual.is_reference(field) {
                    return Err(mismatch());
                }
            }
        }
        for (name, actual) in module.actions() {
            let row = self.actions.get(name).ok_or_else(mismatch)?;
            let params: Vec<_> = actual
                .params()
                .iter()
                .map(|p| ParameterData {
                    name: p.name().into(),
                    ty: p.ty().to_wire_type(),
                })
                .collect();
            let pre: Vec<_> = actual
                .preconditions()
                .iter()
                .map(|c| hash_display(c.expr().hash()))
                .collect();
            let post: Vec<_> = actual
                .postconditions()
                .iter()
                .map(|c| hash_display(c.expr().hash()))
                .collect();
            if row.hash != hash_display(actual.hash())
                || row.params != params
                || row.preconditions != pre
                || row.postconditions != post
                || row.operations.len()
                    != actual.effects().len() + actual.creates().len() + actual.removes().len()
            {
                return Err(mismatch());
            }
            let mut recorded = row.operations.iter();
            for (i, e) in actual.effects().iter().enumerate() {
                match recorded.next() {
                    Some(Operation::Assign {
                        index,
                        hash,
                        param,
                        entity,
                        field,
                        value,
                    }) if *index == i
                        && *hash == hash_display(e.hash())
                        && param == e.param()
                        && *entity == row.target(e.param())?
                        && field == e.field()
                        && *value == hash_display(e.value().hash()) => {}
                    _ => return Err(mismatch()),
                }
            }
            for (i, e) in actual.creates().iter().enumerate() {
                match recorded.next() {
                    Some(Operation::Create {
                        index,
                        hash,
                        entity,
                        id,
                        fields,
                    }) if *index == i
                        && *hash == hash_display(e.hash())
                        && entity == e.entity()
                        && *id == hash_display(e.id().hash())
                        && fields.len() == e.fields().len()
                        && e.fields().iter().all(|(f, v)| {
                            fields.get(f).is_some_and(|h| *h == hash_display(v.hash()))
                        }) => {}
                    _ => return Err(mismatch()),
                }
            }
            for (i, e) in actual.removes().iter().enumerate() {
                match recorded.next() {
                    Some(Operation::Remove {
                        index,
                        hash,
                        param,
                        entity,
                    }) if *index == i
                        && *hash == hash_display(e.hash())
                        && param == e.param()
                        && *entity == row.target(e.param())? => {}
                    _ => return Err(mismatch()),
                }
            }
        }
        Ok(())
    }
}
