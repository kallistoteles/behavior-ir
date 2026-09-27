//! Decision records and replay (research R14; contracts/engine-api.md).
//!
//! A record is self-contained: it holds the request as evaluated, so it can be replayed and
//! audited without the source. It never contains timestamps or host data.

use serde::Serialize;
use serde_json::{Value as Json, json};

use crate::canonical;
use crate::eval::evaluate;
use crate::semantic::module::Module;

#[derive(Debug, Clone)]
pub struct DecisionRecord {
    json: Json,
}

impl DecisionRecord {
    pub(crate) fn new(json: Json) -> Self {
        DecisionRecord { json }
    }

    /// `ALLOW`, `DENY`, `ERROR`, `INVALID_INPUT`, or `INVALID_STATE`.
    pub fn result(&self) -> &str {
        self.json["result"].as_str().unwrap_or("ERROR")
    }

    pub fn as_json(&self) -> &Json {
        &self.json
    }

    /// Canonical JSON. Records hold no floats (invalid input is sanitized), so this cannot fail.
    pub fn to_json_string(&self) -> String {
        canonical::to_canonical_string(&self.json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayResult {
    pub matches: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

impl ReplayResult {
    fn mismatch(diff: String) -> Self {
        ReplayResult {
            matches: false,
            diff: Some(diff),
        }
    }

    pub fn to_json_string(&self) -> String {
        canonical::canonical(self).unwrap_or_default()
    }
}

/// First differing JSON path between the stored record and the replayed one.
fn first_difference(path: &str, stored: &Json, replayed: &Json) -> Option<String> {
    match (stored, replayed) {
        (Json::Object(a), Json::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            keys.into_iter().find_map(|k| {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => first_difference(&p, x, y),
                    (x, y) => Some(format!(
                        "{p}: record has {}, replay has {}",
                        x.map_or("nothing".into(), Json::to_string),
                        y.map_or("nothing".into(), Json::to_string)
                    )),
                }
            })
        }
        (Json::Array(a), Json::Array(b)) => {
            for i in 0..a.len().max(b.len()) {
                let p = format!("{path}[{i}]");
                match (a.get(i), b.get(i)) {
                    (Some(x), Some(y)) => {
                        if let Some(d) = first_difference(&p, x, y) {
                            return Some(d);
                        }
                    }
                    (x, y) => {
                        return Some(format!(
                            "{p}: record has {}, replay has {}",
                            x.map_or("nothing".into(), Json::to_string),
                            y.map_or("nothing".into(), Json::to_string)
                        ));
                    }
                }
            }
            None
        }
        (a, b) if a == b => None,
        (a, b) => Some(format!("{path}: record has {a}, replay has {b}")),
    }
}

/// Re-evaluates the request stored in `record` and compares the outcome byte for byte.
pub fn replay(module: &Module, record: &str) -> ReplayResult {
    let stored: Json = match serde_json::from_str(record) {
        Ok(v) => v,
        Err(e) => return ReplayResult::mismatch(format!("record is not valid JSON: {e}")),
    };
    let version = module.behavior_version();
    if stored["behavior_version"] != json!(version) {
        return ReplayResult::mismatch(format!(
            "behavior_version: record has {}, module is \"{version}\"",
            stored["behavior_version"]
        ));
    }
    let mut request = json!({
        "action": stored["action"]["name"],
        "data_version": stored["data_version"],
        "state": stored["state"],
        "input": stored["input"],
        "context": stored["context"],
    });
    if let (Some(g), Json::Object(m)) = (stored.get("git_revision"), &mut request) {
        m.insert("git_revision".into(), g.clone());
    }
    // Recorded facts (feature 006) are the replay's facts: the observed facts of the evaluation,
    // or the refused facts section of an invalid request.
    if let (Some(f), Json::Object(m)) = (stored.get("facts"), &mut request) {
        m.insert("facts".into(), f.clone());
    }
    let replayed = evaluate(module, &request.to_string());
    match first_difference("", &stored, replayed.as_json()) {
        None => ReplayResult {
            matches: true,
            diff: None,
        },
        Some(d) => ReplayResult::mismatch(d),
    }
}
