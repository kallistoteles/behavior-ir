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

use crate::encode::{Encoder, Term, model_value};
use crate::smt::SmtValue;

/// What the evaluator must show for the counterexample to count.
#[derive(Debug, Clone)]
pub enum Expect {
    /// The `index`-th trace step of `phase` evaluates to `false` and the result is DENY.
    RuleFails { phase: &'static str, index: usize },
    /// The result is ERROR with exactly this message (`"<error> in <expression>"`).
    Error { message: String },
}

/// A confirmed counterexample: the request sections and the decision record.
#[derive(Debug, Clone)]
pub struct Confirmed {
    pub state: Json,
    pub input: Json,
    pub context: Json,
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
    let mut sections: [Map<String, Json>; 3] = Default::default();
    let mut ids: BTreeMap<(String, String), String> = BTreeMap::new();
    for var in &enc.inputs {
        let (present, val) = match &var.term {
            Term::Plain(t) => (true, model.get(t)?),
            Term::Opt { some, val } => (model.get(some)? == &SmtValue::Bool(true), model.get(val)?),
        };
        let json = if !present {
            Json::Null
        } else if let Some(entity) = id_entity(&var.ty) {
            let SmtValue::Str(raw) = val else { return None };
            let next = format!("e{}", ids.len());
            json!(
                ids.entry((entity.to_string(), raw.clone()))
                    .or_insert(next)
                    .clone()
            )
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
    let [state, input, context] = request_sections(enc, model)?;
    let request = json!({
        "action": action,
        "data_version": "verification",
        "state": state,
        "input": input,
        "context": context,
    });
    let record = behavior_core::evaluate(module, &request.to_string());
    let record = record.as_json().clone();
    reproduces(&record, expect).then_some(Confirmed {
        state,
        input,
        context,
        record,
    })
}
