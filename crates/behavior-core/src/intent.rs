//! The capability boundary (research R13; contracts/engine-api.md → StructuredIntent).
//!
//! An intent carries only what an AI may choose: the capability, the ids of the state entities
//! it targets, and input arguments. State, context (such as the authenticated actor), and the
//! data version come from the trusted host. Every problem is reported before any evaluation.

use serde::Serialize;
use serde_json::{Map, Value as Json, json};

use crate::canonical;
use crate::eval::{InputProblem, decode_param, evaluate};
use crate::record::DecisionRecord;
use crate::semantic::module::{Module, ParamRole};

#[derive(Debug, Clone, Serialize)]
pub struct IntentError {
    pub code: String,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntentRejection {
    pub rejected: bool,
    pub errors: Vec<IntentError>,
}

impl IntentRejection {
    pub fn to_json_string(&self) -> String {
        canonical::canonical(self).unwrap_or_default()
    }
}

fn problem(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> InputProblem {
    InputProblem {
        code,
        path: path.into(),
        message: message.into(),
    }
}

fn object(v: &Json) -> Option<&Map<String, Json>> {
    match v {
        Json::Object(m) => Some(m),
        _ => None,
    }
}

fn reject(mut problems: Vec<InputProblem>) -> IntentRejection {
    problems.sort_by(|a, b| a.path.cmp(&b.path));
    IntentRejection {
        rejected: true,
        errors: problems
            .into_iter()
            .map(|p| IntentError {
                code: p.code.to_string(),
                message: p.message,
                path: p.path,
            })
            .collect(),
    }
}

/// Checks a structured intent against the capability, then evaluates it with the host's
/// state and context exactly like a direct EvaluationRequest.
pub fn evaluate_intent(
    module: &Module,
    intent: &str,
    host: &str,
) -> Result<DecisionRecord, IntentRejection> {
    let mut problems = Vec::new();
    let intent: Json = match serde_json::from_str(intent) {
        Ok(v) => v,
        Err(e) => {
            return Err(reject(vec![problem(
                "DECODE_ERROR",
                "$",
                format!("invalid JSON: {e}"),
            )]));
        }
    };
    let host: Json = match serde_json::from_str(host) {
        Ok(v) => v,
        Err(e) => {
            return Err(reject(vec![problem(
                "DECODE_ERROR",
                "host",
                format!("invalid JSON: {e}"),
            )]));
        }
    };
    let Some(obj) = object(&intent) else {
        return Err(reject(vec![problem(
            "DECODE_ERROR",
            "$",
            "expected an object",
        )]));
    };
    let Some(host_obj) = object(&host) else {
        return Err(reject(vec![problem(
            "DECODE_ERROR",
            "host",
            "expected an object",
        )]));
    };
    for key in host_obj.keys() {
        if !matches!(
            key.as_str(),
            "data_version" | "git_revision" | "state" | "context"
        ) {
            problems.push(problem(
                "DECODE_ERROR",
                format!("host.{key}"),
                "unexpected host key",
            ));
        }
    }

    for key in obj.keys() {
        if !matches!(key.as_str(), "capability" | "targets" | "input") {
            problems.push(problem(
                "EXTRA_ARGUMENT",
                key.clone(),
                format!("an intent may not supply `{key}`; state and context come from the host"),
            ));
        }
    }
    let capability = match obj.get("capability") {
        Some(Json::String(s)) => s.clone(),
        Some(_) => {
            problems.push(problem("WRONG_TYPE", "capability", "expected a string"));
            return Err(reject(problems));
        }
        None => {
            problems.push(problem(
                "MISSING_ARGUMENT",
                "capability",
                "missing `capability`",
            ));
            return Err(reject(problems));
        }
    };
    let Some(action) = module.action(&capability) else {
        problems.push(problem(
            "UNKNOWN_CAPABILITY",
            "capability",
            format!("unknown capability `{capability}`"),
        ));
        return Err(reject(problems));
    };

    let empty = Map::new();
    let targets = match obj.get("targets") {
        None => &empty,
        Some(v) => match object(v) {
            Some(m) => m,
            None => {
                problems.push(problem("WRONG_TYPE", "targets", "expected an object"));
                &empty
            }
        },
    };
    let input = match obj.get("input") {
        None => &empty,
        Some(v) => match object(v) {
            Some(m) => m,
            None => {
                problems.push(problem("WRONG_TYPE", "input", "expected an object"));
                &empty
            }
        },
    };
    let host_state = host_obj.get("state").and_then(object).unwrap_or(&empty);

    for p in action.params() {
        match p.role() {
            ParamRole::State => {
                let path = format!("targets.{}", p.name());
                match targets.get(p.name()) {
                    None => problems.push(problem(
                        "MISSING_TARGET",
                        path,
                        format!("missing target id for `{}`", p.name()),
                    )),
                    Some(Json::String(id)) => {
                        let supplied = host_state.get(p.name()).and_then(|e| e.get("id"));
                        if let Some(Json::String(actual)) = supplied
                            && actual != id
                        {
                            problems.push(problem(
                                "TARGET_MISMATCH",
                                path,
                                format!("intent targets `{id}` but the host supplied `{actual}`"),
                            ));
                        }
                    }
                    Some(_) => problems.push(problem("WRONG_TYPE", path, "expected an id string")),
                }
            }
            ParamRole::Input => {
                let path = format!("input.{}", p.name());
                match input.get(p.name()) {
                    None => problems.push(problem(
                        "MISSING_ARGUMENT",
                        path,
                        format!("missing input `{}`", p.name()),
                    )),
                    Some(raw) => {
                        let mut found = Vec::new();
                        let _ = decode_param(module, p.ty(), raw, &path, &mut found);
                        problems.extend(found);
                    }
                }
            }
            ParamRole::Context | ParamRole::Read => {}
        }
    }
    for key in targets.keys() {
        let is_state = action
            .params()
            .iter()
            .any(|p| p.name() == key && p.role() == ParamRole::State);
        if !is_state {
            problems.push(problem(
                "EXTRA_TARGET",
                format!("targets.{key}"),
                format!("`{key}` is not a state parameter of `{capability}`"),
            ));
        }
    }
    for key in input.keys() {
        let is_input = action
            .params()
            .iter()
            .any(|p| p.name() == key && p.role() == ParamRole::Input);
        if !is_input {
            problems.push(problem(
                "EXTRA_ARGUMENT",
                format!("input.{key}"),
                format!("`{key}` is not an input of `{capability}`"),
            ));
        }
    }
    if !problems.is_empty() {
        return Err(reject(problems));
    }

    let mut request = json!({
        "action": capability,
        "data_version": host_obj.get("data_version").cloned().unwrap_or(Json::Null),
        "state": host_obj.get("state").cloned().unwrap_or_else(|| json!({})),
        "input": Json::Object(input.clone()),
        "context": host_obj.get("context").cloned().unwrap_or_else(|| json!({})),
    });
    if let (Some(g), Json::Object(m)) = (host_obj.get("git_revision"), &mut request) {
        m.insert("git_revision".into(), g.clone());
    }
    Ok(evaluate(module, &request.to_string()))
}
