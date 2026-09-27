//! Confirmation of counterexamples by evaluation (research R7).
//!
//! A solver model is turned into an ordinary evaluation request and run through the engine's own
//! evaluator. A counterexample is reported only if the evaluator reproduces the failure; the
//! decision record becomes part of the finding.

use std::collections::BTreeMap;

use serde_json::{Map, Value as Json, json};

use behavior_core::semantic::module::Module;
use behavior_core::semantic::types::Type;
use behavior_core::semantic::value::encode;

use crate::encode::{Encoder, FactKind, Term, model_value};
use crate::smt::SmtValue;

/// What the evaluator must show for the counterexample to count.
#[derive(Debug, Clone)]
pub enum Expect {
    /// The `index`-th trace step of `phase` evaluates to `false` and the result is DENY.
    RuleFails { phase: &'static str, index: usize },
    /// The result is ERROR with exactly this message (`"<error> in <expression>"`).
    Error { message: String },
    /// The result is DENY with this first reason code (feature 006: `DANGLING_REFERENCE`).
    Reason { code: &'static str },
}

/// A confirmed counterexample: the request sections and the decision record.
#[derive(Debug, Clone)]
pub struct Confirmed {
    pub state: Json,
    pub input: Json,
    pub context: Json,
    /// The evaluation facts of the counterexample (feature 006), if it needs any.
    pub facts: Option<Json>,
    pub record: Json,
}

/// The entity an identity type refers to.
fn id_entity(t: &Type) -> Option<&str> {
    match t {
        Type::Id(e) => Some(e),
        Type::Option(inner) => id_entity(inner),
        _ => None,
    }
}

/// Builds the request sections from a model: option flags become `null`, identities are renamed
/// `e0`, `e1`, … in order of first appearance (preserving equality between identities of the
/// same entity type, the only ones that can be compared), decimals must be exactly
/// representable.
pub fn request_sections(
    enc: &Encoder<'_>,
    model: &BTreeMap<String, SmtValue>,
) -> Option<[Json; 3]> {
    request_parts(enc, model).map(|(s, _)| s)
}

type Ids = BTreeMap<(String, String), String>;

fn renamed(ids: &mut Ids, entity: &str, raw: &str) -> String {
    let next = format!("e{}", ids.len());
    ids.entry((entity.to_string(), raw.to_string()))
        .or_insert(next)
        .clone()
}

/// The request sections and the facts section (feature 006) of a model. A `refd_T(x)` fact
/// becomes an incoming reference from an invented entity not bound by the action (`x0`, `x1`, …,
/// never an `e…` identity), plus the bound entities whose reference fields point at `x`.
pub fn request_parts(
    enc: &Encoder<'_>,
    model: &BTreeMap<String, SmtValue>,
) -> Option<([Json; 3], Option<Json>)> {
    let mut ids: Ids = BTreeMap::new();
    let sections = sections_with(enc, model, &mut ids)?;
    let mut facts = behavior_core::Facts::default();
    let mut invented = 0usize;
    for f in &enc.facts {
        let SmtValue::Str(raw) = model.get(&f.id)? else {
            return None;
        };
        let SmtValue::Bool(v) = model.get(&f.value)? else {
            return None;
        };
        let id = renamed(&mut ids, &f.entity, raw);
        let key = (f.entity.clone(), id.clone());
        match f.kind {
            FactKind::Exists => {
                facts.existence.insert(key, *v);
            }
            FactKind::Used => {
                facts.identities.insert(key, *v);
            }
            FactKind::Refd => {
                if facts.references.contains_key(&key) {
                    continue;
                }
                let mut edges = Vec::new();
                if *v {
                    let (entity, field) = enc.module.references_to(&f.entity).into_iter().next()?;
                    edges.push(behavior_core::RefEdge {
                        entity,
                        id: format!("x{invented}"),
                        field,
                    });
                    invented += 1;
                }
                for (param, source) in &enc.world.bound {
                    let Some(item) = enc.module.entity(source) else {
                        continue;
                    };
                    for (field, target) in item.reference_fields() {
                        if target == f.entity
                            && sections[0][param.as_str()][field] == Json::String(id.clone())
                        {
                            let bid = sections[0][param.as_str()]["id"].as_str()?.to_string();
                            edges.push(behavior_core::RefEdge {
                                entity: source.clone(),
                                id: bid,
                                field: field.to_string(),
                            });
                        }
                    }
                }
                edges.sort();
                facts.references.insert(key, edges);
            }
        }
    }
    let facts = (!facts.is_empty()).then(|| facts.to_json());
    Some((sections, facts))
}

fn sections_with(
    enc: &Encoder<'_>,
    model: &BTreeMap<String, SmtValue>,
    ids: &mut Ids,
) -> Option<[Json; 3]> {
    let mut sections: [Map<String, Json>; 3] = Default::default();
    for var in &enc.inputs {
        let (present, val) = match &var.term {
            Term::Plain(t) => (true, model.get(t)?),
            Term::Opt { some, val } => (model.get(some)? == &SmtValue::Bool(true), model.get(val)?),
        };
        let json = if !present {
            Json::Null
        } else if let Some(entity) = id_entity(&var.ty) {
            let SmtValue::Str(raw) = val else { return None };
            json!(renamed(ids, entity, raw))
        } else {
            encode(&var.ty, &model_value(&var.ty, val)?)
        };
        let idx = match var.section {
            "state" => 0,
            "input" => 1,
            _ => 2,
        };
        match &var.field {
            Some(f) => {
                let entry = sections[idx]
                    .entry(var.param.clone())
                    .or_insert_with(|| json!({}));
                if let Json::Object(m) = entry {
                    m.insert(f.clone(), json);
                }
            }
            None => {
                sections[idx].insert(var.param.clone(), json);
            }
        }
    }
    let [s, i, c] = sections;
    Some([Json::Object(s), Json::Object(i), Json::Object(c)])
}

fn reproduces(record: &Json, expect: &Expect) -> bool {
    match expect {
        Expect::RuleFails { phase, index } => {
            record["result"] == "DENY"
                && record["trace"]
                    .as_array()
                    .and_then(|t| t.iter().filter(|s| s["phase"] == *phase).nth(*index))
                    .is_some_and(|s| s["outcome"] == Json::Bool(false))
        }
        Expect::Error { message } => {
            record["result"] == "ERROR" && record["reasons"][0]["message"] == message.as_str()
        }
        Expect::Reason { code } => {
            record["result"] == "DENY" && record["reasons"][0]["code"] == *code
        }
    }
}

/// Evaluates the model as a request; `Some` only if the expected failure is reproduced.
pub fn confirm(
    module: &Module,
    enc: &Encoder<'_>,
    action: &str,
    model: &BTreeMap<String, SmtValue>,
    expect: &Expect,
) -> Option<Confirmed> {
    let ([state, input, context], facts) = request_parts(enc, model)?;
    let mut request = json!({
        "action": action,
        "data_version": "verification",
        "state": state,
        "input": input,
        "context": context,
    });
    if let (Some(f), Json::Object(m)) = (&facts, &mut request) {
        m.insert("facts".into(), f.clone());
    }
    let record = behavior_core::evaluate(module, &request.to_string());
    let record = record.as_json().clone();
    reproduces(&record, expect).then_some(Confirmed {
        state,
        input,
        context,
        facts,
        record,
    })
}
