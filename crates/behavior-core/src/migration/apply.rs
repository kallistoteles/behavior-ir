//! Applying a migration to a complete source universe (FR-011–FR-013, research R6). Pure: the
//! same universe gives the same result in any order. The steps, each refusing the whole
//! migration:
//! 1. the source universe is decoded under the source schema; retired types must be empty;
//! 2. the source state must satisfy the source module's rules, which verification assumes
//!    (`MIGRATION_SOURCE_INVALID`);
//! 3. every source requirement must hold (`MIGRATION_REQUIREMENT_FAILED`);
//! 4. every entity of a changed type is transformed, independently (`MIGRATION_TRANSFORM_ERROR`);
//! 5. the target state must satisfy every target rule: entity constraints and invariants,
//!    referential integrity and module invariants (`MIGRATION_INVALID_RESULT`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value as Json, json};

use super::{FieldSource, Migration, OLD};
use crate::eval::{
    Vals, check_global_invariants, decode_value, encode_param, entity_rule_failures, eval_in,
    violators,
};
use crate::facts::{EvaluationFacts, FactError, QueryRequest, RefEdge};
use crate::schema::schema;
use crate::semantic::module::Module;
use crate::semantic::types::Type;
use crate::semantic::value::Value;

/// One entity of the source universe: its type and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntity {
    pub entity: String,
    pub value: Json,
}

/// One entity of the target universe; `migrated` is false for an entity of an unchanged type,
/// carried over as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratedEntity {
    pub entity: String,
    pub id: String,
    pub value: Json,
    pub migrated: bool,
}

/// The outcome of one source requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequirementOutcome {
    pub name: String,
    pub held: bool,
}

/// A successful application: the complete target universe in canonical order (entity, id), the
/// requirement outcomes, and the report (the summary plus the number of migrated entities).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migrated {
    pub entities: Vec<MigratedEntity>,
    pub requirements: Vec<RequirementOutcome>,
    pub report: Json,
}

/// A refused application: the code, a message naming the rule and the entities, the rule (a
/// requirement, constraint or invariant name), the entities concerned (at most
/// [`MAX_EXAMPLES`]) and how many there are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRefusal {
    pub code: &'static str,
    pub message: String,
    pub rule: Option<String>,
    pub entities: Vec<(String, String)>,
    pub count: usize,
}

/// How many entities a refusal lists by name.
pub const MAX_EXAMPLES: usize = 10;

impl std::fmt::Display for MigrationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

fn refuse(code: &'static str, message: impl Into<String>) -> MigrationRefusal {
    MigrationRefusal {
        code,
        message: message.into(),
        rule: None,
        entities: Vec::new(),
        count: 0,
    }
}

type Key = (String, String);

fn listed(keys: &[Key]) -> String {
    let mut out: Vec<String> = keys
        .iter()
        .take(MAX_EXAMPLES)
        .map(|(e, id)| format!("{e}#{id}"))
        .collect();
    if keys.len() > MAX_EXAMPLES {
        out.push(format!("and {} more", keys.len() - MAX_EXAMPLES));
    }
    out.join(", ")
}

/// The evaluation facts of a complete universe: existence, references (from the module's `Ref`
/// fields), queries and fields, answered from the universe itself.
struct UniverseFacts<'a> {
    values: &'a BTreeMap<Key, Json>,
    incoming: BTreeMap<Key, Vec<RefEdge>>,
}

impl<'a> UniverseFacts<'a> {
    fn new(module: &Module, values: &'a BTreeMap<Key, Json>) -> Self {
        let mut incoming: BTreeMap<Key, Vec<RefEdge>> = BTreeMap::new();
        for ((entity, id), v) in values {
            if let Some(item) = module.entity(entity) {
                for (field, target) in item.reference_fields() {
                    if let Some(t) = v[field].as_str() {
                        incoming
                            .entry((target.to_string(), t.to_string()))
                            .or_default()
                            .push(RefEdge {
                                entity: entity.clone(),
                                id: id.clone(),
                                field: field.to_string(),
                            });
                    }
                }
            }
        }
        UniverseFacts { values, incoming }
    }
}

fn k(entity: &str, id: &str) -> Key {
    (entity.to_string(), id.to_string())
}

impl EvaluationFacts for UniverseFacts<'_> {
    fn exists(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        Ok(self.values.contains_key(&k(entity, id)))
    }
    fn used(&self, entity: &str, id: &str) -> Result<bool, FactError> {
        Ok(self.values.contains_key(&k(entity, id)))
    }
    fn incoming(&self, entity: &str, id: &str) -> Result<Vec<RefEdge>, FactError> {
        Ok(self
            .incoming
            .get(&k(entity, id))
            .cloned()
            .unwrap_or_default())
    }
    fn query(&self, q: &QueryRequest<'_>) -> Result<Vec<String>, FactError> {
        let t = q.node.entity();
        let mut out = Vec::new();
        for ((_, id), v) in self
            .values
            .range(k(t, "")..)
            .take_while(|((e, _), _)| e == t)
        {
            if q.matches(v)? {
                out.push(id.clone());
            }
        }
        Ok(out)
    }
    fn field(&self, entity: &str, id: &str, field: &str) -> Result<Json, FactError> {
        self.values
            .get(&k(entity, id))
            .and_then(|v| v.get(field))
            .cloned()
            .ok_or_else(|| FactError(format!("{entity}#{id}.{field} is not in the universe")))
    }
}

/// Every rule of every entity, then every module invariant, on a universe: the first violated
/// rule (in rule name order) with every entity that violates it.
fn first_violation(
    module: &Module,
    values: &BTreeMap<Key, Json>,
) -> Option<(String, Vec<Key>, String)> {
    let facts = UniverseFacts::new(module, values);
    let mut by_rule: BTreeMap<String, (Vec<Key>, String)> = BTreeMap::new();
    for ((entity, id), raw) in values {
        let value = match decode_value(module, entity, raw) {
            Ok(v) => v,
            Err(p) => {
                return Some((
                    "the schema".into(),
                    vec![k(entity, id)],
                    format!("{entity}#{id} is not a valid `{entity}`: {}", p.join("; ")),
                ));
            }
        };
        for (rule, message) in entity_rule_failures(module, entity, &value, &facts) {
            let slot = by_rule.entry(rule).or_insert_with(|| (Vec::new(), message));
            slot.0.push(k(entity, id));
        }
    }
    if let Some((rule, (keys, message))) = by_rule.into_iter().next() {
        return Some((rule, keys, message));
    }
    match check_global_invariants(module, &facts) {
        Ok(()) => None,
        Err(problems) => {
            let first = problems.into_iter().next().unwrap_or_default();
            let rule = first
                .split('`')
                .nth(1)
                .unwrap_or("a module invariant")
                .to_string();
            Some((rule, Vec::new(), first))
        }
    }
}

fn violation(
    code: &'static str,
    side: &str,
    (rule, keys, message): (String, Vec<Key>, String),
) -> MigrationRefusal {
    let text = if keys.is_empty() {
        format!("in the {side} state, {message}")
    } else {
        format!(
            "in the {side} state, {message} by {} {} ({})",
            keys.len(),
            if keys.len() == 1 {
                "entity"
            } else {
                "entities"
            },
            listed(&keys)
        )
    };
    MigrationRefusal {
        code,
        message: text,
        rule: Some(rule),
        count: keys.len(),
        entities: keys.into_iter().take(MAX_EXAMPLES).collect(),
    }
}

/// Transforms one source entity on its own (feature 009: how the verifier confirms a
/// counterexample): the source requirements must hold on the one-entity universe, the transform
/// must succeed, and the target value must satisfy its own entity rules (the constraints and
/// invariants of its type; reference constraints concern the universe and are not checked).
pub fn transform_one(
    migration: &Migration,
    source: &Module,
    target: &Module,
    entity: &str,
    value: &Json,
) -> Result<Json, MigrationRefusal> {
    let canonical = crate::eval::decode_entity(source, entity, value).map_err(|p| {
        refuse(
            "MIGRATION_SOURCE_INVALID",
            format!("a `{entity}` of the source state: {}", p.join("; ")),
        )
    })?;
    let id = canonical["id"].as_str().unwrap_or_default().to_string();
    let values = BTreeMap::from([(k(entity, &id), canonical.clone())]);
    let facts = UniverseFacts::new(source, &values);
    let constants: Vals = migration.constant_values();
    for r in &migration.requirements {
        if !matches!(
            eval_in(source, &r.body, &constants, &facts),
            Ok(Value::Bool(true))
        ) {
            return Err(MigrationRefusal {
                code: "MIGRATION_REQUIREMENT_FAILED",
                message: format!("requirement `{}` does not hold for {entity}#{id}", r.name),
                rule: Some(r.name.clone()),
                entities: vec![k(entity, &id)],
                count: 1,
            });
        }
    }
    let out = match migration.transforms.get(entity) {
        Some(t) => transform_entity(
            migration, source, target, t, entity, &id, &canonical, &constants,
        )?,
        // An unchanged type is carried over as it is, under the target's rules.
        None => canonical,
    };
    let decoded = decode_value(target, entity, &out).map_err(|p| {
        refuse(
            "MIGRATION_TRANSFORM_ERROR",
            format!("{entity}#{id}: {}", p.join("; ")),
        )
    })?;
    let target_values = BTreeMap::from([(k(entity, &id), out.clone())]);
    let target_facts = UniverseFacts::new(target, &target_values);
    let reference: Vec<&String> = target
        .constraints_for(entity)
        .filter(|(_, c)| c.reference().is_some())
        .map(|(n, _)| n)
        .collect();
    if let Some((rule, message)) = entity_rule_failures(target, entity, &decoded, &target_facts)
        .into_iter()
        .find(|(rule, _)| !reference.contains(&rule))
    {
        return Err(MigrationRefusal {
            code: "MIGRATION_INVALID_RESULT",
            message: format!("for the target value of {entity}#{id}, {message}"),
            rule: Some(rule),
            entities: vec![k(entity, &id)],
            count: 1,
        });
    }
    Ok(out)
}

/// The transform of one entity of a changed type: every target field, encoded and checked under
/// the target schema.
#[allow(clippy::too_many_arguments)]
fn transform_entity(
    migration: &Migration,
    source: &Module,
    target: &Module,
    t: &super::Transform,
    entity: &str,
    id: &str,
    raw: &Json,
    constants: &Vals,
) -> Result<Json, MigrationRefusal> {
    let _ = migration;
    let old = decode_value(source, entity, raw).map_err(|p| {
        refuse(
            "MIGRATION_SOURCE_INVALID",
            format!("{entity}#{id}: {}", p.join("; ")),
        )
    })?;
    let Value::Entity(old_fields) = &old else {
        return Err(refuse("MIGRATION_SOURCE_INVALID", format!("{entity}#{id}")));
    };
    let mut vals = constants.clone();
    vals.insert(OLD.to_string(), old.clone());
    let no_facts = crate::facts::Facts::default();
    let mut new_fields = BTreeMap::new();
    for (field, src) in &t.fields {
        let v = match src {
            FieldSource::Copy(from) => old_fields.get(from).cloned().ok_or_else(|| {
                refuse(
                    "MIGRATION_TRANSFORM_ERROR",
                    format!("{entity}#{id} has no field `{from}`"),
                )
            })?,
            FieldSource::Expr(e) => {
                eval_in(source, e, &vals, &no_facts).map_err(|msg| MigrationRefusal {
                    code: "MIGRATION_TRANSFORM_ERROR",
                    message: format!("{entity}#{id}.{field}: {msg}"),
                    rule: None,
                    entities: vec![k(entity, id)],
                    count: 1,
                })?
            }
        };
        new_fields.insert(field.clone(), v);
    }
    let encoded = encode_param(
        target,
        &Type::Entity(entity.to_string()),
        &Value::Entity(new_fields),
    );
    crate::eval::decode_entity(target, entity, &encoded).map_err(|p| MigrationRefusal {
        code: "MIGRATION_TRANSFORM_ERROR",
        message: format!("{entity}#{id}: the transformed value: {}", p.join("; ")),
        rule: None,
        entities: vec![k(entity, id)],
        count: 1,
    })
}

/// The outcome of a plain-mode application as JSON (the command line and the bindings): `result`
/// is `"MIGRATED"` with the target entities, requirement outcomes and report, or the refusal code
/// with its message, rule, entities and count.
pub fn outcome_json(r: &Result<Migrated, MigrationRefusal>) -> Json {
    match r {
        Ok(m) => json!({
            "result": "MIGRATED",
            "entities": m.entities.iter().map(|e| json!({
                "entity": e.entity, "id": e.id, "value": e.value, "migrated": e.migrated,
            })).collect::<Vec<_>>(),
            "requirements": m.requirements,
            "report": m.report,
        }),
        Err(r) => json!({
            "result": r.code,
            "message": r.message,
            "rule": r.rule,
            "entities": r.entities,
            "count": r.count,
        }),
    }
}

/// Applies `migration` to the complete source universe `entities` (every entity of the source
/// state, in any order).
pub fn apply_migration(
    migration: &Migration,
    source: &Module,
    target: &Module,
    entities: &[SourceEntity],
) -> Result<Migrated, MigrationRefusal> {
    let (sa, ta) = (schema(source), schema(target));
    if sa.hash != migration.source.hash || ta.hash != migration.target.hash {
        return Err(refuse(
            "MIGRATION_SCHEMA_MISMATCH",
            format!(
                "migration {} relates {} to {}; the modules declare {} and {}",
                migration.hash(),
                migration.source.hash,
                migration.target.hash,
                sa.hash,
                ta.hash
            ),
        ));
    }
    // 1. The source universe, canonical, under the source schema.
    let mut values: BTreeMap<Key, Json> = BTreeMap::new();
    for e in entities {
        let canonical = crate::eval::decode_entity(source, &e.entity, &e.value).map_err(|p| {
            refuse(
                "MIGRATION_SOURCE_INVALID",
                format!("a `{}` of the source state: {}", e.entity, p.join("; ")),
            )
        })?;
        let id = canonical["id"].as_str().unwrap_or_default().to_string();
        if values.insert(k(&e.entity, &id), canonical).is_some() {
            return Err(refuse(
                "MIGRATION_SOURCE_INVALID",
                format!("{}#{id} appears twice in the source state", e.entity),
            ));
        }
    }
    let retired: Vec<Key> = values
        .keys()
        .filter(|(e, _)| migration.retire.contains(e))
        .cloned()
        .collect();
    if !retired.is_empty() {
        return Err(MigrationRefusal {
            code: "RETIRED_TYPE_NOT_EMPTY",
            message: format!(
                "retired entity types must be empty when the migration is applied; {} remain ({}). \
                 Remove them with ordinary actions first",
                retired.len(),
                listed(&retired)
            ),
            rule: None,
            count: retired.len(),
            entities: retired.into_iter().take(MAX_EXAMPLES).collect(),
        });
    }
    // 2. The source state is valid under the source module's rules (they are behavior, not
    //    schema: data written under earlier behavior may break them).
    if let Some(v) = first_violation(source, &values) {
        return Err(violation("MIGRATION_SOURCE_INVALID", "source", v));
    }
    // 3. Source requirements, on the complete, immutable source state.
    let facts = UniverseFacts::new(source, &values);
    let constants: Vals = migration.constant_values();
    let mut outcomes = Vec::new();
    for r in &migration.requirements {
        let held = match eval_in(source, &r.body, &constants, &facts) {
            Ok(Value::Bool(b)) => b,
            Ok(_) => false,
            Err(e) => {
                return Err(MigrationRefusal {
                    code: "MIGRATION_REQUIREMENT_FAILED",
                    message: format!("requirement `{}` cannot be evaluated: {e}", r.name),
                    rule: Some(r.name.clone()),
                    entities: Vec::new(),
                    count: 0,
                });
            }
        };
        if !held {
            let (keys, detail) = match violators(source, &r.body, &constants, &facts) {
                Some(Ok((t, ids))) => {
                    let keys: Vec<Key> = ids.iter().map(|id| k(&t, id)).collect();
                    let detail = format!(
                        ": {} {t} {} ({})",
                        keys.len(),
                        if keys.len() == 1 {
                            "entity violates it"
                        } else {
                            "entities violate it"
                        },
                        listed(&keys)
                    );
                    (keys, detail)
                }
                _ => (Vec::new(), String::new()),
            };
            return Err(MigrationRefusal {
                code: "MIGRATION_REQUIREMENT_FAILED",
                message: format!(
                    "requirement `{}` does not hold for the source state{detail}; the migration \
                     applies once the data satisfies it",
                    r.name
                ),
                rule: Some(r.name.clone()),
                count: keys.len(),
                entities: keys.into_iter().take(MAX_EXAMPLES).collect(),
            });
        }
        outcomes.push(RequirementOutcome {
            name: r.name.clone(),
            held,
        });
    }
    // 4. Transform every entity of every changed type, independently and in canonical order.
    let mut out_values: BTreeMap<Key, Json> = BTreeMap::new();
    let mut migrated_keys: BTreeSet<Key> = BTreeSet::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for ((entity, id), raw) in &values {
        let Some(t) = migration.transforms.get(entity) else {
            out_values.insert(k(entity, id), raw.clone());
            continue;
        };
        let canonical =
            transform_entity(migration, source, target, t, entity, id, raw, &constants)?;
        out_values.insert(k(entity, id), canonical);
        migrated_keys.insert(k(entity, id));
        *counts.entry(entity.clone()).or_default() += 1;
    }
    // 5. The complete target state satisfies every target rule.
    if let Some(v) = first_violation(target, &out_values) {
        return Err(violation("MIGRATION_INVALID_RESULT", "target", v));
    }
    let mut report = migration.summary();
    if let Some(Json::Object(types)) = report.get_mut("types") {
        for (entity, t) in types.iter_mut() {
            if let Json::Object(o) = t {
                o.insert(
                    "entities".into(),
                    json!(counts.get(entity).copied().unwrap_or(0)),
                );
            }
        }
    }
    if let Json::Object(o) = &mut report {
        o.insert("migration".into(), json!(migration.hash()));
    }
    Ok(Migrated {
        entities: out_values
            .into_iter()
            .map(|((entity, id), value)| MigratedEntity {
                migrated: migrated_keys.contains(&(entity.clone(), id.clone())),
                entity,
                id,
                value,
            })
            .collect(),
        requirements: outcomes,
        report,
    })
}
