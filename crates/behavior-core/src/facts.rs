//! Evaluation facts (feature 006, research R3): what evaluation observes about the entity universe
//! beyond the bound entities' fields.
//!
//! - **Current-state facts** are properties of the evaluated state: `exists(T, id)` and
//!   `incoming(T, id)`, the surviving `Ref` fields that point at an identity.
//! - **History facts** are properties of the store's history: `used(T, id)`, whether the identity
//!   was ever created (an identity names one lifetime).
//!
//! Together with the state they form the evaluation snapshot. Facts are obtained lazily through
//! [`EvaluationFacts`], and every answer evaluation actually obtains is an observed fact, recorded
//! in the decision record's `facts` section in canonical order. Facts about the resulting state S'
//! are never asked: evaluation derives them from S's facts and ΔS.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json, json};

use crate::semantic::module::Module;

/// A surviving reference: entity `entity#id` holds `field` = the target identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefEdge {
    pub entity: String,
    pub id: String,
    pub field: String,
}

impl RefEdge {
    pub fn to_json(&self) -> Json {
        json!({"entity": self.entity, "id": self.id, "field": self.field})
    }
}

/// A fact the provider cannot answer (plain evaluation: not supplied; replay: not recorded;
/// a store: a backend failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactError(pub String);

/// Current-state facts (`exists`, `incoming`) and history facts (`used`) of one evaluation
/// snapshot. Implementations answer as of one position; evaluation asks each question at most
/// once.
pub trait EvaluationFacts {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError>;
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError>;
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError>;
}

type Key = (String, String);

/// A `facts` section: supplied facts (request), observed facts (record, read set).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub existence: BTreeMap<Key, bool>,
    pub identities: BTreeMap<Key, bool>,
    /// Incoming references per target, sorted.
    pub references: BTreeMap<Key, Vec<RefEdge>>,
}

fn key(entity: &str, id: &str) -> Key {
    (entity.to_string(), id.to_string())
}

fn describe(entity: &str, id: &str) -> String {
    format!("{entity} {id}")
}

impl EvaluationFacts for Facts {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        self.existence
            .get(&key(entity, id))
            .copied()
            .ok_or_else(|| FactError(format!("no existence fact for {}", describe(entity, id))))
    }
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        self.identities
            .get(&key(entity, id))
            .copied()
            .ok_or_else(|| FactError(format!("no identity fact for {}", describe(entity, id))))
    }
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError> {
        self.references
            .get(&key(entity, id))
            .cloned()
            .ok_or_else(|| FactError(format!("no reference fact for {}", describe(entity, id))))
    }
}

/// Why a supplied `facts` section is refused: malformed (`DECODE_ERROR`) or not a valid snapshot
/// (`INCONSISTENT_FACTS`, FR-010g).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactsProblem {
    pub code: &'static str,
    pub message: String,
}

fn malformed(message: impl Into<String>) -> FactsProblem {
    FactsProblem {
        code: "DECODE_ERROR",
        message: message.into(),
    }
}

fn inconsistent(message: impl Into<String>) -> FactsProblem {
    FactsProblem {
        code: "INCONSISTENT_FACTS",
        message: message.into(),
    }
}

fn string_field(o: &Map<String, Json>, k: &str, path: &str) -> Result<String, FactsProblem> {
    match o.get(k) {
        Some(Json::String(s)) => Ok(s.clone()),
        _ => Err(malformed(format!("{path}.{k}: expected a string"))),
    }
}

fn only_keys(o: &Map<String, Json>, keys: &[&str], path: &str) -> Result<(), FactsProblem> {
    match o.keys().find(|k| !keys.contains(&k.as_str())) {
        Some(k) => Err(malformed(format!("{path}: unexpected key `{k}`"))),
        None => Ok(()),
    }
}

fn entries<'a>(root: &'a Map<String, Json>, section: &str) -> Result<&'a [Json], FactsProblem> {
    match root.get(section) {
        None => Ok(&[]),
        Some(Json::Array(items)) => Ok(items),
        Some(_) => Err(malformed(format!("facts.{section}: expected an array"))),
    }
}

fn object<'a>(v: &'a Json, path: &str) -> Result<&'a Map<String, Json>, FactsProblem> {
    match v {
        Json::Object(o) => Ok(o),
        _ => Err(malformed(format!("{path}: expected an object"))),
    }
}

fn boolean(o: &Map<String, Json>, k: &str, path: &str) -> Result<bool, FactsProblem> {
    match o.get(k) {
        Some(Json::Bool(b)) => Ok(*b),
        _ => Err(malformed(format!("{path}.{k}: expected a boolean"))),
    }
}

impl Facts {
    pub fn is_empty(&self) -> bool {
        self.existence.is_empty() && self.identities.is_empty() && self.references.is_empty()
    }

    /// Parses a `facts` section. A fact given twice is `INCONSISTENT_FACTS`.
    pub fn from_json(v: &Json) -> Result<Facts, FactsProblem> {
        let root = object(v, "facts")?;
        only_keys(root, &["existence", "identities", "references"], "facts")?;
        let mut f = Facts::default();
        let duplicate = |what: &str, e: &str, id: &str| {
            inconsistent(format!(
                "the {what} of {} is given more than once",
                describe(e, id)
            ))
        };
        for (i, item) in entries(root, "existence")?.iter().enumerate() {
            let path = format!("facts.existence[{i}]");
            let o = object(item, &path)?;
            only_keys(o, &["entity", "id", "exists"], &path)?;
            let (e, id) = (
                string_field(o, "entity", &path)?,
                string_field(o, "id", &path)?,
            );
            let b = boolean(o, "exists", &path)?;
            if f.existence.insert(key(&e, &id), b).is_some() {
                return Err(duplicate("existence", &e, &id));
            }
        }
        for (i, item) in entries(root, "identities")?.iter().enumerate() {
            let path = format!("facts.identities[{i}]");
            let o = object(item, &path)?;
            only_keys(o, &["entity", "id", "used"], &path)?;
            let (e, id) = (
                string_field(o, "entity", &path)?,
                string_field(o, "id", &path)?,
            );
            let b = boolean(o, "used", &path)?;
            if f.identities.insert(key(&e, &id), b).is_some() {
                return Err(duplicate("identity", &e, &id));
            }
        }
        for (i, item) in entries(root, "references")?.iter().enumerate() {
            let path = format!("facts.references[{i}]");
            let o = object(item, &path)?;
            only_keys(o, &["entity", "id", "incoming"], &path)?;
            let (e, id) = (
                string_field(o, "entity", &path)?,
                string_field(o, "id", &path)?,
            );
            let Some(Json::Array(list)) = o.get("incoming") else {
                return Err(malformed(format!("{path}.incoming: expected an array")));
            };
            let mut edges = BTreeSet::new();
            for (j, edge) in list.iter().enumerate() {
                let ep = format!("{path}.incoming[{j}]");
                let eo = object(edge, &ep)?;
                only_keys(eo, &["entity", "id", "field"], &ep)?;
                let edge = RefEdge {
                    entity: string_field(eo, "entity", &ep)?,
                    id: string_field(eo, "id", &ep)?,
                    field: string_field(eo, "field", &ep)?,
                };
                if !edges.insert(edge) {
                    return Err(inconsistent(format!(
                        "{ep}: a reference to {} is listed twice",
                        describe(&e, &id)
                    )));
                }
            }
            if f.references
                .insert(key(&e, &id), edges.into_iter().collect())
                .is_some()
            {
                return Err(duplicate("incoming references", &e, &id));
            }
        }
        Ok(f)
    }

    /// The canonical `facts` section: sections sorted by (entity, id), edges by (entity, id,
    /// field); empty sections are omitted.
    pub fn to_json(&self) -> Json {
        let mut o = Map::new();
        if !self.existence.is_empty() {
            let items: Vec<Json> = self
                .existence
                .iter()
                .map(|((e, id), b)| json!({"entity": e, "id": id, "exists": b}))
                .collect();
            o.insert("existence".into(), Json::Array(items));
        }
        if !self.identities.is_empty() {
            let items: Vec<Json> = self
                .identities
                .iter()
                .map(|((e, id), b)| json!({"entity": e, "id": id, "used": b}))
                .collect();
            o.insert("identities".into(), Json::Array(items));
        }
        if !self.references.is_empty() {
            let items: Vec<Json> = self
                .references
                .iter()
                .map(|((e, id), edges)| {
                    json!({"entity": e, "id": id,
                           "incoming": edges.iter().map(RefEdge::to_json).collect::<Vec<_>>()})
                })
                .collect();
            o.insert("references".into(), Json::Array(items));
        }
        Json::Object(o)
    }
}

/// An entity bound by the action to a state parameter: it exists in the evaluated state, with the
/// given reference-field values (`field → target id`, `None` for an absent optional reference).
#[derive(Debug, Clone)]
pub struct BoundEntity {
    pub entity: String,
    pub id: String,
    pub references: Vec<(String, Option<String>)>,
}

/// Checks that supplied facts, together with the bound entities, can describe one valid state and
/// history (FR-010g). The rules are closed under subsets, so the observed facts of a record pass
/// whenever the supplied facts did:
/// - an existing identity is used;
/// - an identity with incoming references exists (so is used), and so does every source;
/// - an edge is a declared `Ref` field of its source's type that targets the fact's entity type,
///   and one source field points at one target;
/// - bound entities exist and are used, and so do the targets of their reference fields;
/// - an edge from a bound entity matches its field's value,
///   and an incoming fact lists every bound entity whose reference field points at the target.
pub fn check_snapshot(
    module: &Module,
    facts: &Facts,
    bound: &[BoundEntity],
) -> Result<(), FactsProblem> {
    let says_absent =
        |k: &Key| facts.existence.get(k) == Some(&false) || facts.identities.get(k) == Some(&false);
    for (k, exists) in &facts.existence {
        if *exists && facts.identities.get(k) == Some(&false) {
            return Err(inconsistent(format!(
                "{} exists but is marked as never used",
                describe(&k.0, &k.1)
            )));
        }
    }
    let mut sources: BTreeMap<(String, String, String), Key> = BTreeMap::new();
    for (target, edges) in &facts.references {
        if !edges.is_empty() && says_absent(target) {
            return Err(inconsistent(format!(
                "{} has incoming references but is marked absent",
                describe(&target.0, &target.1)
            )));
        }
        for edge in edges {
            let declared = module
                .entity(&edge.entity)
                .and_then(|e| e.reference_fields().find(|(f, _)| *f == edge.field))
                .map(|(_, t)| t == target.0);
            if declared != Some(true) {
                return Err(inconsistent(format!(
                    "`{}.{}` is not a reference to {}",
                    edge.entity, edge.field, target.0
                )));
            }
            let source = key(&edge.entity, &edge.id);
            if says_absent(&source) {
                return Err(inconsistent(format!(
                    "{} holds a reference but is marked absent",
                    describe(&edge.entity, &edge.id)
                )));
            }
            let slot = (edge.entity.clone(), edge.id.clone(), edge.field.clone());
            if let Some(other) = sources.insert(slot, target.clone())
                && other != *target
            {
                return Err(inconsistent(format!(
                    "`{}.{}` of {} points at two identities",
                    edge.entity,
                    edge.field,
                    describe(&edge.entity, &edge.id)
                )));
            }
        }
    }
    for b in bound {
        let k = key(&b.entity, &b.id);
        if says_absent(&k) {
            return Err(inconsistent(format!(
                "{} is bound by the action but marked absent",
                describe(&b.entity, &b.id)
            )));
        }
        for (field, value) in &b.references {
            let target_type = module
                .entity(&b.entity)
                .and_then(|e| e.reference_fields().find(|(f, _)| f == field))
                .map(|(_, t)| t.to_string())
                .unwrap_or_default();
            // A bound entity's reference points at an existing (so used) entity: the store keeps
            // referential integrity, and the verifier assumes it.
            if let Some(target) = value
                && says_absent(&key(&target_type, target))
            {
                return Err(inconsistent(format!(
                    "bound {} refers to {} (`{field}`), which is marked absent",
                    describe(&b.entity, &b.id),
                    describe(&target_type, target)
                )));
            }
            for ((te, tid), edges) in &facts.references {
                let listed = edges
                    .iter()
                    .any(|e| e.entity == b.entity && e.id == b.id && e.field == *field);
                let points = *te == target_type && value.as_deref() == Some(tid.as_str());
                if listed != points {
                    return Err(inconsistent(format!(
                        "the incoming references of {} disagree with bound {} (`{field}`)",
                        describe(te, tid),
                        describe(&b.entity, &b.id)
                    )));
                }
            }
        }
    }
    Ok(())
}
