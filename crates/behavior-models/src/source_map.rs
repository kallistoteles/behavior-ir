//! Lowering receipts, source maps and diagnostics (identity contract §3–4, data-model §9).
//!
//! All three are produced after the composed module is admitted and never feed back into it:
//! they are provenance and explanation, never semantics (law L9).

use std::collections::{BTreeMap, BTreeSet};

use behavior_engine::semantic::Module;
use behavior_engine::semantic::types::hash_display;
use serde_json::{Map, Value, json};

use crate::state_machine::input::{DisplayMeta, SourceLoc};
use crate::state_machine::resolve::ResolvedStateMachine;
use crate::{CORE_RELEASE, STATE_MACHINE_LOWERER_VERSION, STATE_MACHINE_SEMANTIC_VERSION};

/// A lowering receipt (`behavior.model_receipt.v1`), as canonical JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringReceipt(pub Value);

/// A non-blocking model diagnostic: a warning, never a verification claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub code: &'static str,
    pub model_ref: String,
    pub message: String,
}

pub(crate) fn display_json(d: &DisplayMeta) -> Option<Value> {
    let mut m = Map::new();
    if let Some(n) = &d.display_name {
        m.insert("display_name".into(), json!(n));
    }
    if let Some(n) = &d.description {
        m.insert("description".into(), json!(n));
    }
    (!m.is_empty()).then_some(Value::Object(m))
}

/// Whether `file` is a marker for code without a file (`<string>`, `<stdin>`, `<__main__>`),
/// not a path.
fn is_marker(file: &str) -> bool {
    file.starts_with('<') && file.ends_with('>')
}

/// Source locations relative to a base that does not depend on where the sources were checked
/// out (Constitution IV): the build `root` when one is given (as Core relativizes module
/// locations), otherwise the common directory of the model's own source files (markers excluded;
/// they are kept verbatim). Unrelated files of the module never influence a model's receipt.
pub(crate) struct RelativeSources {
    base: std::path::PathBuf,
}

impl RelativeSources {
    fn new(m: &ResolvedStateMachine, root: Option<&str>) -> RelativeSources {
        Self::from_sources(
            m.transitions
                .values()
                .filter_map(|t| t.source.as_ref())
                .chain(m.source.as_ref()),
            root,
        )
    }

    /// The base for a model whose own sources are `sources` (any model kind).
    pub(crate) fn from_sources<'a>(
        sources: impl Iterator<Item = &'a SourceLoc>,
        root: Option<&str>,
    ) -> RelativeSources {
        use std::path::{Path, PathBuf};
        if let Some(root) = root {
            return RelativeSources {
                base: PathBuf::from(root),
            };
        }
        // A marker (`<string>`, `<__main__>`: code without a file) names no directory; its
        // empty parent would empty the base and leave every real path checkout-dependent.
        let dirs = sources.filter(|s| !is_marker(&s.file)).map(|s| {
            Path::new(&s.file)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        });
        let mut base: Option<PathBuf> = None;
        for d in dirs {
            base = Some(match base {
                None => d,
                Some(b) => b
                    .components()
                    .zip(d.components())
                    .take_while(|(x, y)| x == y)
                    .map(|(x, _)| x)
                    .collect(),
            });
        }
        RelativeSources {
            base: base.unwrap_or_default(),
        }
    }

    /// `file` relative to the base, as `/`-joined normal components.
    fn file(&self, file: &str) -> String {
        use std::path::{Component, Path};
        let path = Path::new(file);
        let rel = path.strip_prefix(&self.base).unwrap_or(path);
        rel.components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                Component::ParentDir => Some("..".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    pub(crate) fn json(&self, s: &Option<SourceLoc>) -> Option<Value> {
        s.as_ref()
            .map(|s| json!({"file": self.file(&s.file), "line": s.line}))
    }
}

/// The source map of one machine: one entry per generated action and one for the owned
/// field, sorted by their Core reference. Source locations are relative (see
/// [`RelativeSources`]).
pub fn source_map(m: &ResolvedStateMachine, module: &Module, root: Option<&str>) -> Value {
    let sources = RelativeSources::new(m, root);
    let source_json = |s: &Option<SourceLoc>| sources.json(s);
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    for t in m.transitions.values() {
        let item_hash = module
            .actions()
            .get(&t.action_name)
            .map(|a| hash_display(a.hash()));
        let mut e = Map::new();
        e.insert(
            "core".into(),
            json!({"kind": "action", "name": t.action_name, "item_hash": item_hash}),
        );
        e.insert("model".into(), json!(m.transition_ref(&t.transition_id)));
        if let Some(d) = display_json(&t.display) {
            e.insert("display".into(), d);
        }
        if let Some(s) = source_json(&t.source) {
            e.insert("source".into(), s);
        }
        entries.insert(format!("action\0{}", t.action_name), Value::Object(e));
    }
    let mut f = Map::new();
    f.insert(
        "core".into(),
        json!({"kind": "field", "entity": m.subject, "field": m.field}),
    );
    f.insert("model".into(), json!(format!("{}/state_field", m.model_id)));
    if let Some(d) = display_json(&m.display) {
        f.insert("display".into(), d);
    }
    if let Some(s) = source_json(&m.source) {
        f.insert("source".into(), s);
    }
    entries.insert(
        format!("field\0{}\0{}", m.subject, m.field),
        Value::Object(f),
    );
    json!({"entries": entries.into_values().collect::<Vec<_>>()})
}

/// The receipt of one machine in a composed module.
pub(crate) fn receipt(
    module: &Module,
    m: &ResolvedStateMachine,
    document: Value,
    model_hash: String,
    root: Option<&str>,
) -> LoweringReceipt {
    LoweringReceipt(json!({
        "format": "behavior.model_receipt.v1",
        "model_hash": model_hash,
        "model_kind": "state_machine",
        "model_semantic_version": STATE_MACHINE_SEMANTIC_VERSION,
        "lowerer_version": STATE_MACHINE_LOWERER_VERSION,
        "core_release": {"version": CORE_RELEASE.version, "commit": CORE_RELEASE.commit},
        "behavior_hash": module.behavior_version(),
        "model": document,
        "source_map": source_map(m, module, root),
    }))
}

/// Graph diagnostics of a machine (state-machine contract §3). Reachability follows
/// `F_t → q_t` edges from the entry states and ignores guards: it is structural tooling, never
/// a claim about what Core can actually reach.
pub fn diagnostics(m: &ResolvedStateMachine, module: &Module) -> Vec<Diagnostic> {
    let cases: Vec<String> = module
        .enums()
        .get(&m.enum_type)
        .map(|e| e.values.clone())
        .unwrap_or_default();
    let mut outgoing: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut incoming: BTreeSet<&str> = BTreeSet::new();
    let mut used: BTreeSet<&str> = m.entry_states.iter().map(String::as_str).collect();
    for t in m.transitions.values() {
        for f in &t.from_states {
            outgoing.entry(f).or_default().insert(&t.to_state);
            used.insert(f);
        }
        incoming.insert(&t.to_state);
        used.insert(&t.to_state);
    }
    let mut reachable: BTreeSet<&str> = m.entry_states.iter().map(String::as_str).collect();
    let mut frontier: Vec<&str> = reachable.iter().copied().collect();
    while let Some(s) = frontier.pop() {
        for &next in outgoing.get(s).into_iter().flatten() {
            if reachable.insert(next) {
                frontier.push(next);
            }
        }
    }
    let mut out = Vec::new();
    // A transition can fire only from a reachable state: it is structurally unreachable when
    // `F_t ∩ Reach(Q_entry) = ∅`, whether or not its target is reachable some other way.
    for t in m.transitions.values() {
        if !t.from_states.iter().any(|s| reachable.contains(s.as_str())) {
            out.push(Diagnostic {
                code: "UNREACHABLE_TRANSITION",
                model_ref: m.transition_ref(&t.transition_id),
                message: format!(
                    "no source state of `{}` is reachable from an entry state (guards ignored)",
                    t.transition_id
                ),
            });
        }
    }
    let at = |case: &str| format!("{}/state/{case}", m.model_id);
    for case in &cases {
        let c = case.as_str();
        let entry = m.entry_states.contains(case);
        if !used.contains(c) {
            out.push(Diagnostic {
                code: "UNUSED_STATE",
                model_ref: at(c),
                message: format!("`{c}` is in no entry state set and no transition; only existing entities can hold it"),
            });
            continue;
        }
        if !outgoing.contains_key(c) {
            out.push(Diagnostic {
                code: "NO_OUTGOING_TRANSITIONS",
                model_ref: at(c),
                message: format!("no transition leaves `{c}` (a terminal state)"),
            });
        }
        if !entry && !incoming.contains(c) {
            out.push(Diagnostic {
                code: "NO_INCOMING_TRANSITIONS",
                model_ref: at(c),
                message: format!("`{c}` is not an entry state and no transition enters it"),
            });
        } else if !reachable.contains(c) {
            out.push(Diagnostic {
                code: "UNREACHABLE_STATE",
                model_ref: at(c),
                message: format!(
                    "no path of transitions from an entry state reaches `{c}` (guards ignored)"
                ),
            });
        }
    }
    out
}
