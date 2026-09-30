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
    /// The members of a query instance at the evaluated state (feature 007), in any order.
    fn query(&self, q: &QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        Err(FactError(format!(
            "no query fact for instance {}",
            q.instance
        )))
    }
    /// A field of an entity the action does not bind (feature 007), in its canonical encoding.
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        Err(FactError(format!(
            "no field fact for {}.{field}",
            describe(entity, id)
        )))
    }
}

/// A query instance to answer (feature 007): the query, its identity, and the captured values it
/// is evaluated with. `matches` decides the membership of one candidate value.
pub struct QueryRequest<'a> {
    pub module: &'a Module,
    pub node: &'a crate::semantic::expr::QueryNode,
    pub definition: String,
    pub instance: String,
    pub captures: &'a [(String, Json)],
    pub env: &'a BTreeMap<String, crate::semantic::value::Value>,
}

impl QueryRequest<'_> {
    /// Whether a candidate (an entity value in its canonical encoding) is a member.
    pub fn matches(&self, candidate: &Json) -> Result<bool, FactError> {
        crate::eval::query_matches(self.module, self.node, self.env, candidate).map_err(FactError)
    }
}

/// An observed query instance: definition, entity type, captured values and members (sorted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryFact {
    pub definition: String,
    pub entity: String,
    pub captures: Vec<(String, Json)>,
    pub members: Vec<String>,
}

type FieldKey = (String, String, String);

type Key = (String, String);

/// A `facts` section: supplied facts (request), observed facts (record, read set).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub existence: BTreeMap<Key, bool>,
    pub identities: BTreeMap<Key, bool>,
    /// Incoming references per target, sorted.
    pub references: BTreeMap<Key, Vec<RefEdge>>,
    /// Query instances by instance id (feature 007).
    pub queries: BTreeMap<String, QueryFact>,
    /// Fields of entities not bound by the action (feature 007).
    pub fields: BTreeMap<FieldKey, Json>,
    /// Supplied only (plain evaluation): the complete set of existing entities of a type, with
    /// their values, by id (feature 007). Never recorded; observations are recorded instead.
    pub universe: BTreeMap<String, BTreeMap<String, Json>>,
}

fn key(entity: &str, id: &str) -> Key {
    (entity.to_string(), id.to_string())
}

fn describe(entity: &str, id: &str) -> String {
    format!("{entity} {id}")
}

impl EvaluationFacts for Facts {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        if let Some(b) = self.existence.get(&key(entity, id)) {
            return Ok(*b);
        }
        // A universe is the complete set of existing entities of its type.
        if let Some(members) = self.universe.get(entity) {
            return Ok(members.contains_key(id));
        }
        Err(FactError(format!(
            "no existence fact for {}",
            describe(entity, id)
        )))
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
    fn query(&self, q: &QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        if let Some(members) = self.universe.get(q.node.entity()) {
            let mut out = Vec::new();
            for (id, value) in members {
                if q.matches(value)? {
                    out.push(id.clone());
                }
            }
            if let Some(f) = self.queries.get(&q.instance)
                && (f.definition != q.definition
                    || f.entity != q.node.entity()
                    || f.captures.as_slice() != q.captures
                    || f.members != out)
            {
                return Err(FactError(format!(
                    "query fact for instance {} contradicts the supplied universe of {}",
                    q.instance,
                    q.node.entity()
                )));
            }
            return Ok(out);
        }
        self.queries
            .get(&q.instance)
            .map(|f| f.members.clone())
            .ok_or_else(|| {
                FactError(format!(
                    "no query fact for instance {} and no universe of {}",
                    q.instance,
                    q.node.entity()
                ))
            })
    }
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        let k = (entity.to_string(), id.to_string(), field.to_string());
        if let Some(v) = self.fields.get(&k) {
            return Ok(v.clone());
        }
        self.universe
            .get(entity)
            .and_then(|m| m.get(id))
            .and_then(|v| v.get(field))
            .cloned()
            .ok_or_else(|| {
                FactError(format!(
                    "no field fact for {}.{field}",
                    describe(entity, id)
                ))
            })
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
        self.existence.is_empty()
            && self.identities.is_empty()
            && self.references.is_empty()
            && self.queries.is_empty()
            && self.fields.is_empty()
            && self.universe.is_empty()
    }

    /// Whether the section has query or field facts (a record 0.6, feature 007).
    pub fn has_query_facts(&self) -> bool {
        !self.queries.is_empty() || !self.fields.is_empty()
    }

    /// Parses a `facts` section. A fact given twice is `INCONSISTENT_FACTS`.
    pub fn from_json(v: &Json) -> Result<Facts, FactsProblem> {
        let root = object(v, "facts")?;
        only_keys(
            root,
            &[
                "existence",
                "identities",
                "references",
                "queries",
                "fields",
                "universe",
            ],
            "facts",
        )?;
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
        f.parse_queries(root)?;
        Ok(f)
    }

    fn parse_queries(&mut self, root: &Map<String, Json>) -> Result<(), FactsProblem> {
        for (i, item) in entries(root, "queries")?.iter().enumerate() {
            let path = format!("facts.queries[{i}]");
            let o = object(item, &path)?;
            only_keys(
                o,
                &["instance", "definition", "entity", "captures", "members"],
                &path,
            )?;
            let instance = string_field(o, "instance", &path)?;
            let Some(Json::Array(cs)) = o.get("captures") else {
                return Err(malformed(format!("{path}.captures: expected an array")));
            };
            let mut captures = Vec::new();
            for (j, c) in cs.iter().enumerate() {
                let cp = format!("{path}.captures[{j}]");
                let co = object(c, &cp)?;
                only_keys(co, &["read", "value"], &cp)?;
                let value = co
                    .get("value")
                    .cloned()
                    .ok_or_else(|| malformed(format!("{cp}.value: missing")))?;
                captures.push((string_field(co, "read", &cp)?, value));
            }
            let Some(Json::Array(ms)) = o.get("members") else {
                return Err(malformed(format!("{path}.members: expected an array")));
            };
            let mut members = BTreeSet::new();
            for (j, m) in ms.iter().enumerate() {
                let mp = format!("{path}.members[{j}]");
                let mo = object(m, &mp)?;
                only_keys(mo, &["id"], &mp)?;
                if !members.insert(string_field(mo, "id", &mp)?) {
                    return Err(inconsistent(format!("{mp}: a member is listed twice")));
                }
            }
            let fact = QueryFact {
                definition: string_field(o, "definition", &path)?,
                entity: string_field(o, "entity", &path)?,
                captures,
                members: members.into_iter().collect(),
            };
            if self.queries.insert(instance.clone(), fact).is_some() {
                return Err(inconsistent(format!(
                    "query instance {instance} is given more than once"
                )));
            }
        }
        for (i, item) in entries(root, "fields")?.iter().enumerate() {
            let path = format!("facts.fields[{i}]");
            let o = object(item, &path)?;
            only_keys(o, &["entity", "id", "field", "value"], &path)?;
            let k = (
                string_field(o, "entity", &path)?,
                string_field(o, "id", &path)?,
                string_field(o, "field", &path)?,
            );
            let value = o
                .get("value")
                .cloned()
                .ok_or_else(|| malformed(format!("{path}.value: missing")))?;
            if self.fields.insert(k.clone(), value).is_some() {
                return Err(inconsistent(format!(
                    "the field {}.{} is given more than once",
                    describe(&k.0, &k.1),
                    k.2
                )));
            }
        }
        for (i, item) in entries(root, "universe")?.iter().enumerate() {
            let path = format!("facts.universe[{i}]");
            let o = object(item, &path)?;
            only_keys(o, &["entity", "members"], &path)?;
            let entity = string_field(o, "entity", &path)?;
            let Some(Json::Array(ms)) = o.get("members") else {
                return Err(malformed(format!("{path}.members: expected an array")));
            };
            let mut members = BTreeMap::new();
            for (j, m) in ms.iter().enumerate() {
                let mp = format!("{path}.members[{j}]");
                let mo = object(m, &mp)?;
                let id = string_field(mo, "id", &mp)?;
                if members.insert(id.clone(), m.clone()).is_some() {
                    return Err(inconsistent(format!(
                        "{} appears twice in the universe",
                        describe(&entity, &id)
                    )));
                }
            }
            if self.universe.insert(entity.clone(), members).is_some() {
                return Err(inconsistent(format!(
                    "the universe of {entity} is given more than once"
                )));
            }
        }
        Ok(())
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
        if !self.queries.is_empty() {
            let items: Vec<Json> = self
                .queries
                .iter()
                .map(|(instance, q)| {
                    json!({
                        "instance": instance,
                        "definition": q.definition,
                        "entity": q.entity,
                        "captures": q.captures.iter()
                            .map(|(r, v)| json!({"read": r, "value": v}))
                            .collect::<Vec<_>>(),
                        "members": q.members.iter().map(|m| json!({"id": m})).collect::<Vec<_>>(),
                    })
                })
                .collect();
            o.insert("queries".into(), Json::Array(items));
        }
        if !self.fields.is_empty() {
            let items: Vec<Json> = self
                .fields
                .iter()
                .map(|((e, id, field), v)| json!({"entity": e, "id": id, "field": field, "value": v}))
                .collect();
            o.insert("fields".into(), Json::Array(items));
        }
        if !self.universe.is_empty() {
            let items: Vec<Json> = self
                .universe
                .iter()
                .map(|(e, members)| {
                    json!({"entity": e, "members": members.values().cloned().collect::<Vec<_>>()})
                })
                .collect();
            o.insert("universe".into(), Json::Array(items));
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
