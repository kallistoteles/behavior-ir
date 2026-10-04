//! First-class reads (feature 010): a read observes one exact state and changes nothing. It
//! returns a value with a content-addressed read record as evidence; a read response carries only
//! the declared result and the record's identity (research R6, R7).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value as Json};

use crate::admit::hash::{self, TAG_READ_RECORD};
use crate::admit::resolve::Decls;
use crate::admit::{AdmissionError, AdmissionResult, decode_failure, typecheck};
use crate::canonical;
use crate::eval::InputProblem;
use crate::eval::{evaluate_read_inner, unknown_read_fields};
use crate::facts::EvaluationFacts;
use crate::intent::{IntentError, IntentRejection, check_params, problem, sections};
use crate::record::{ReplayResult, compare};
use crate::semantic::module::{Module, ParamRole, ReadItem};
use crate::semantic::types::hash_display;
use crate::serialize;
use crate::wire;

/// The read to evaluate: a module's declared read by name, or an ad-hoc read admitted against the
/// module (trusted hosts only).
#[derive(Debug, Clone)]
pub enum ReadSource {
    Declared(String),
    AdHoc(Box<ReadItem>),
}

/// Evaluates a read in plain mode: the request supplies the state's facts.
/// `request` is `{data_version, state, input, context, facts}`.
pub fn evaluate_read(module: &Module, source: &ReadSource, request: &str) -> ReadExecution {
    run(module, source, request, None)
}

/// Evaluates a read whose facts come from `facts` (a store as of one position); the request must
/// not have a `facts` section.
pub fn evaluate_read_with(
    module: &Module,
    source: &ReadSource,
    request: &str,
    facts: &dyn EvaluationFacts,
) -> ReadExecution {
    run(module, source, request, Some(facts))
}

/// Evaluates a plain read request in the `behavior read` format: the request's `read` is a
/// declared read's name or `{"definition": <read document>}` (an ad-hoc read admitted against the
/// module); the rest is `{data_version, state, input, context, facts}`.
pub fn evaluate_read_request(
    module: &Module,
    request: &str,
) -> Result<ReadExecution, AdmissionResult> {
    let refuse = |message: String| {
        AdmissionResult::failed(vec![AdmissionError::new("DECODE_ERROR", message, None)])
    };
    let mut req: Json =
        serde_json::from_str(request).map_err(|e| refuse(format!("invalid JSON: {e}")))?;
    let Some(obj) = req.as_object_mut() else {
        return Err(refuse("a read request is an object".into()));
    };
    let source = match obj.remove("read") {
        Some(Json::String(name)) => ReadSource::Declared(name),
        Some(Json::Object(mut o)) if o.len() == 1 && o.contains_key("definition") => {
            let doc = o.remove("definition").unwrap_or_default();
            ReadSource::AdHoc(Box::new(admit_read(module, &doc.to_string())?))
        }
        Some(_) => {
            return Err(refuse(
                "at read: expected a declared read's name or {\"definition\": <read document>}"
                    .into(),
            ));
        }
        None => return Err(refuse("missing key `read`".into())),
    };
    Ok(evaluate_read(module, &source, &req.to_string()))
}

/// The read a record names: a declared read of the module by name, or the ad-hoc read in its
/// `definition`, admitted again against the module.
pub fn record_source(module: &Module, record: &Json) -> Result<ReadSource, String> {
    let read = &record["read"];
    let name = read["name"].as_str().ok_or("read.name: missing")?;
    if read["declared"] == Json::Bool(true) {
        return Ok(ReadSource::Declared(name.to_string()));
    }
    let definition = read
        .get("definition")
        .ok_or("read.definition: missing for an ad-hoc read")?;
    admit_read(module, &definition.to_string())
        .map(|r| ReadSource::AdHoc(Box::new(r)))
        .map_err(|e| {
            let codes: Vec<&str> = e.errors.iter().map(|x| x.code.as_str()).collect();
            format!("read.definition: not admitted ({})", codes.join(", "))
        })
}

/// The request a record was evaluated with: its state, input, context and data version, and
/// its recorded facts.
pub fn record_request(record: &Json) -> Json {
    let mut req = Map::new();
    for key in ["data_version", "state", "input", "context", "facts"] {
        if let Some(v) = record.get(key) {
            req.insert(key.into(), v.clone());
        }
    }
    Json::Object(req)
}

/// Replays a read record from its own facts (FR-011): the read is evaluated again with the
/// recorded state, parameters and facts, and the result is compared byte for byte, `record_id`
/// included. Any difference is reported at its first path.
pub fn replay_read(module: &Module, record: &str) -> ReplayResult {
    let stored: Json = match serde_json::from_str(record) {
        Ok(v) => v,
        Err(e) => return ReplayResult::mismatch(format!("record is not valid JSON: {e}")),
    };
    if stored["format"] != Json::String(READ_RECORD_FORMAT.into()) {
        return ReplayResult::mismatch(format!(
            "format: record has {}, expected \"{READ_RECORD_FORMAT}\"",
            stored["format"]
        ));
    }
    let version = module.behavior_version();
    if stored["behavior_version"] != Json::String(version.clone()) {
        return ReplayResult::mismatch(format!(
            "behavior_version: record has {}, module is \"{version}\"",
            stored["behavior_version"]
        ));
    }
    let source = match record_source(module, &stored) {
        Ok(s) => s,
        Err(diff) => return ReplayResult::mismatch(diff),
    };
    let replayed = evaluate_read(module, &source, &record_request(&stored).to_string());
    compare(&stored, replayed.record.as_json())
}

/// A validated read intent (feature 010): the declared read an untrusted caller names, with the
/// identities of its bound entities and its input.
#[derive(Debug, Clone)]
pub struct ReadIntent {
    pub capability: String,
    pub targets: BTreeMap<String, String>,
    pub input: Json,
}

fn intent_error(code: &str, path: &str, message: String) -> IntentError {
    IntentError {
        code: code.to_string(),
        message,
        path: path.to_string(),
    }
}

/// Parses and validates a read intent `{capability, targets, input}` against the module's
/// declared reads, the capability boundary for untrusted callers (FR-016): only a declared read
/// can be named, nothing but `capability`, `targets` and `input` may be supplied, and every
/// problem is listed. `host_state` is the trusted host's bound entities, when it supplies them.
/// Returns the parsed intent whenever it names a declared read (so a store can add its own
/// problems), and every problem found.
pub fn check_read_intent(
    module: &Module,
    intent: &str,
    host_state: Option<&Map<String, Json>>,
) -> (Option<ReadIntent>, Vec<IntentError>) {
    let to_errors = |ps: Vec<InputProblem>| -> Vec<IntentError> {
        ps.into_iter()
            .map(|p| intent_error(p.code, &p.path, p.message))
            .collect()
    };
    let intent: Json = match serde_json::from_str(intent) {
        Ok(v) => v,
        Err(e) => {
            return (
                None,
                vec![intent_error(
                    "DECODE_ERROR",
                    "$",
                    format!("invalid JSON: {e}"),
                )],
            );
        }
    };
    let Some(obj) = intent.as_object() else {
        return (
            None,
            vec![intent_error(
                "DECODE_ERROR",
                "$",
                "expected an object".into(),
            )],
        );
    };
    let mut problems = Vec::new();
    for key in obj.keys() {
        if !matches!(key.as_str(), "capability" | "targets" | "input") {
            problems.push(problem(
                "EXTRA_ARGUMENT",
                key.clone(),
                format!(
                    "a read intent may not supply `{key}`; it names a declared read, and context \
                     comes from the host"
                ),
            ));
        }
    }
    let capability = match obj.get("capability") {
        Some(Json::String(s)) => s.clone(),
        Some(_) => {
            problems.push(problem("WRONG_TYPE", "capability", "expected a string"));
            return (None, to_errors(problems));
        }
        None => {
            problems.push(problem(
                "MISSING_ARGUMENT",
                "capability",
                "missing `capability`",
            ));
            return (None, to_errors(problems));
        }
    };
    let Some(read) = module.read(&capability) else {
        problems.push(problem(
            "UNKNOWN_CAPABILITY",
            "capability",
            format!("unknown read capability `{capability}`"),
        ));
        return (None, to_errors(problems));
    };
    let empty = Map::new();
    let (targets, input) = sections(obj, &empty, &mut problems);
    problems.extend(check_params(
        module,
        read.params(),
        &capability,
        targets,
        input,
        host_state,
    ));
    let parsed = ReadIntent {
        capability,
        targets: targets
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|id| (k.clone(), id.to_string())))
            .collect(),
        input: Json::Object(input.clone()),
    };
    (Some(parsed), to_errors(problems))
}

/// Evaluates a read intent with the trusted host's side (plain mode): `host` is
/// `{data_version, state, context, facts}`. Every problem is reported before evaluation; the
/// execution's `response` is what the caller may see.
pub fn evaluate_read_intent(
    module: &Module,
    intent: &str,
    host: &str,
) -> Result<ReadExecution, IntentRejection> {
    let host: Json = serde_json::from_str(host).map_err(|e| {
        IntentRejection::of(vec![intent_error(
            "DECODE_ERROR",
            "host",
            format!("invalid JSON: {e}"),
        )])
    })?;
    let Some(host) = host.as_object() else {
        return Err(IntentRejection::of(vec![intent_error(
            "DECODE_ERROR",
            "host",
            "expected an object".into(),
        )]));
    };
    let mut errors: Vec<IntentError> = host
        .keys()
        .filter(|k| !matches!(k.as_str(), "data_version" | "state" | "context" | "facts"))
        .map(|k| {
            intent_error(
                "DECODE_ERROR",
                &format!("host.{k}"),
                "unexpected host key".into(),
            )
        })
        .collect();
    let empty = Map::new();
    let host_state = host
        .get("state")
        .and_then(Json::as_object)
        .unwrap_or(&empty);
    let (parsed, problems) = check_read_intent(module, intent, Some(host_state));
    errors.extend(problems);
    let intent = match parsed {
        Some(i) if errors.is_empty() => i,
        _ => return Err(IntentRejection::of(errors)),
    };
    let mut request = Map::new();
    request.insert(
        "data_version".into(),
        host.get("data_version").cloned().unwrap_or(Json::Null),
    );
    // The host may supply its entities for every capability; the read takes those its `state`
    // parameters bind (an input or context of the same name is not an entity).
    let bound: Map<String, Json> = module
        .read(&intent.capability)
        .map(|r| {
            r.params()
                .iter()
                .filter(|p| p.role() == ParamRole::State)
                .filter_map(|p| {
                    host_state
                        .get(p.name())
                        .map(|v| (p.name().to_string(), v.clone()))
                })
                .collect()
        })
        .unwrap_or_default();
    request.insert("state".into(), Json::Object(bound));
    request.insert("input".into(), intent.input);
    request.insert(
        "context".into(),
        host.get("context")
            .cloned()
            .unwrap_or_else(|| Json::Object(Map::new())),
    );
    if let Some(f) = host.get("facts") {
        request.insert("facts".into(), f.clone());
    }
    Ok(evaluate_read(
        module,
        &ReadSource::Declared(intent.capability),
        &Json::Object(request).to_string(),
    ))
}

/// The read a source names, if the module has it.
pub fn resolve<'a>(module: &'a Module, source: &'a ReadSource) -> Option<&'a ReadItem> {
    match source {
        ReadSource::Declared(name) => module.read(name),
        ReadSource::AdHoc(r) => Some(r),
    }
}

/// The record's `read` section: name, hash and kind; an ad-hoc read carries its definition, so
/// the record replays without the host that issued it.
fn read_identity(r: &ReadItem) -> Json {
    let mut o = Map::new();
    o.insert("name".into(), Json::String(r.name().to_string()));
    o.insert("hash".into(), Json::String(hash_display(r.hash())));
    o.insert("declared".into(), Json::Bool(r.declared()));
    if !r.declared() {
        o.insert("definition".into(), serialize::read_document(r));
    }
    Json::Object(o)
}

fn run(
    module: &Module,
    source: &ReadSource,
    request: &str,
    provider: Option<&dyn EvaluationFacts>,
) -> ReadExecution {
    let (identity, fields) = match resolve(module, source) {
        Some(r) => (
            read_identity(r),
            evaluate_read_inner(module, r, request, provider),
        ),
        None => {
            let name = match source {
                ReadSource::Declared(n) => n.as_str(),
                ReadSource::AdHoc(r) => r.name(),
            };
            (
                serde_json::json!({"name": name, "declared": true}),
                unknown_read_fields(request, name),
            )
        }
    };
    let mut record = Map::new();
    record.insert("format".into(), Json::String(READ_RECORD_FORMAT.into()));
    record.insert(
        "behavior_version".into(),
        Json::String(module.behavior_version()),
    );
    record.insert("read".into(), identity);
    record.extend(fields);
    ReadExecution::of(ReadRecord::seal(record))
}

/// Admits a read document (an ad-hoc read) against `module`, with the same rules as a declared
/// read. The read never enters the module.
pub fn admit_read(module: &Module, doc: &str) -> Result<ReadItem, AdmissionResult> {
    let w = wire::decode_read_document(doc).map_err(decode_failure)?;
    let decls = Decls {
        enums: module.enums.clone(),
        nominals: module.nominals.clone(),
        entities: module.entities.clone(),
        reads: module.reads.keys().cloned().collect(),
    };
    let mut errors: Vec<AdmissionError> = Vec::new();
    match typecheck::read_item(&decls, &module.derived, &w, false, &mut errors) {
        Some(r) if errors.is_empty() => Ok(r),
        _ => Err(AdmissionResult::failed(errors)),
    }
}

/// The format tag of a read record.
pub const READ_RECORD_FORMAT: &str = TAG_READ_RECORD;

/// The identity of a read record: `read:` and the hash of its canonical JSON without
/// `record_id`. It binds the read, the behavior version, the exact state, the parameters, the
/// observations and the result (FR-009).
pub fn record_id(record: &Json) -> String {
    let mut r = record.clone();
    if let Json::Object(m) = &mut r {
        m.remove("record_id");
    }
    // Records hold no floats (inputs are sanitized), so canonicalization cannot fail.
    let text = canonical::to_canonical_string(&r).unwrap_or_default();
    format!("read:{}", hash_display(&hash::read_record(&text)))
}

/// The evidence of one read (research R6): canonical, content-addressed, self-contained. A store
/// never keeps it; a trusted host may.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadRecord {
    json: Json,
}

impl ReadRecord {
    /// Seals a record built by the engine: adds its identity.
    pub(crate) fn seal(mut fields: Map<String, Json>) -> ReadRecord {
        fields.remove("record_id");
        let id = record_id(&Json::Object(fields.clone()));
        fields.insert("record_id".into(), Json::String(id));
        ReadRecord {
            json: Json::Object(fields),
        }
    }

    /// Parses a read record and checks its format and identity.
    pub fn from_json(text: &str) -> Result<ReadRecord, String> {
        let json: Json = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
        if json.get("format").and_then(Json::as_str) != Some(READ_RECORD_FORMAT) {
            return Err(format!("format: expected \"{READ_RECORD_FORMAT}\""));
        }
        let stored = json
            .get("record_id")
            .and_then(Json::as_str)
            .ok_or("record_id: missing")?;
        let computed = record_id(&json);
        if stored != computed {
            return Err(format!(
                "record_id: the record has {stored}, its content gives {computed}"
            ));
        }
        Ok(ReadRecord { json })
    }

    pub fn record_id(&self) -> &str {
        self.json["record_id"].as_str().unwrap_or_default()
    }

    /// `VALUE`, `EVALUATION_ERROR`, `INVALID_INPUT` or `INVALID_BINDING`.
    pub fn result(&self) -> &str {
        self.json["result"].as_str().unwrap_or_default()
    }

    pub fn as_json(&self) -> &Json {
        &self.json
    }

    /// Canonical JSON.
    pub fn to_json_string(&self) -> String {
        canonical::to_canonical_string(&self.json).unwrap_or_default()
    }

    /// What a capability caller may see of this read: the declared result and the record's
    /// identity, never the trace or the observations (FR-016a).
    pub fn response(&self) -> ReadResponse {
        ReadResponse {
            result: self.result().to_string(),
            value: self.json.get("value").cloned(),
            reasons: self.json.get("reasons").cloned(),
            record_id: self.record_id().to_string(),
        }
    }
}

/// The capability response of a read (research R7): a distinct type from the record, with no
/// evidence fields. Only the engine builds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadResponse {
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasons: Option<Json>,
    record_id: String,
}

impl ReadResponse {
    pub fn result(&self) -> &str {
        &self.result
    }
    pub fn value(&self) -> Option<&Json> {
        self.value.as_ref()
    }
    pub fn reasons(&self) -> Option<&Json> {
        self.reasons.as_ref()
    }
    pub fn record_id(&self) -> &str {
        &self.record_id
    }
    /// Canonical JSON.
    pub fn to_json_string(&self) -> String {
        canonical::canonical(self).unwrap_or_default()
    }
}

/// What every read returns to the trusted host: the capability response, and the full record.
#[derive(Debug, Clone)]
pub struct ReadExecution {
    pub response: ReadResponse,
    pub record: ReadRecord,
}

impl ReadExecution {
    pub(crate) fn of(record: ReadRecord) -> ReadExecution {
        ReadExecution {
            response: record.response(),
            record,
        }
    }
}
