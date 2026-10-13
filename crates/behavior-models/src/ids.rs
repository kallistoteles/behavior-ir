//! Semantic ids and the generated action-name encoding (research R4, spec FR-013).
//!
//! A semantic id matches `ID = [a-z][a-z0-9]*(_[a-z0-9]+)*`: lowercase, starting with a letter,
//! single underscores only between alphanumeric runs. Because no id contains `"__"`, starts with
//! `_` or ends with `_`, the action name `encode(m, t) = m ‖ "__" ‖ t` contains exactly one
//! `"__"`, so splitting there recovers `(m, t)`: the encoding is injective. Every result is also
//! a Core identifier (`[A-Za-z_][A-Za-z0-9_]*`).

use std::fmt;

/// A validated semantic id (machine id or transition id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticId(String);

impl SemanticId {
    /// The id, if `s` matches the semantic-id grammar.
    pub fn parse(s: &str) -> Option<SemanticId> {
        is_semantic_id(s).then(|| SemanticId(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SemanticId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `s` matches `[a-z][a-z0-9]*(_[a-z0-9]+)*`.
pub fn is_semantic_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_lowercase) || bytes.last() == Some(&b'_') {
        return false;
    }
    let mut previous_underscore = false;
    for &b in bytes {
        match b {
            b'_' if previous_underscore => return false,
            b'_' => previous_underscore = true,
            b'a'..=b'z' | b'0'..=b'9' => previous_underscore = false,
            _ => return false,
        }
    }
    true
}

/// The generated action name of transition `t` of machine `m`.
pub fn encode(m: &SemanticId, t: &SemanticId) -> String {
    format!("{m}__{t}")
}

/// The inverse of [`encode`] on its image.
pub fn decode(name: &str) -> Option<(SemanticId, SemanticId)> {
    let (m, t) = name.split_once("__")?;
    Some((SemanticId::parse(m)?, SemanticId::parse(t)?))
}

/// The Core name of a RequestFlow's generated request entity (feature 502, research R3).
///
/// Split the id into its runs at `_`. The first run and every run that starts with a letter
/// are capitalized; every run that starts with a digit keeps its `_`:
/// `purchase_request` → `PurchaseRequest`, `a_1b` → `A_1b`, `a1b` → `A1b`.
///
/// Plain PascalCase would not be injective (`a_1b` and `a1b` would both give `A1b`). This
/// encoding is: runs contain only `[a-z0-9]`, so an uppercase letter in the output marks exactly
/// the start of a letter-led run and `_` exactly the start of a digit-led run. Splitting there
/// and lowercasing recovers the id ([`decode_entity_name`]). The output starts with `[A-Z]` and
/// contains only `[A-Za-z0-9_]`, so it is a Core identifier.
pub fn encode_entity_name(id: &SemanticId) -> String {
    let mut out = String::with_capacity(id.as_str().len());
    for (i, run) in id.as_str().split('_').enumerate() {
        let mut chars = run.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() => {
                out.push(c.to_ascii_uppercase());
                out.extend(chars);
            }
            Some(c) => {
                // A digit-led run (never the first: ids start with a letter).
                debug_assert!(i > 0);
                out.push('_');
                out.push(c);
                out.extend(chars);
            }
            None => {}
        }
    }
    out
}

/// The inverse of [`encode_entity_name`] on its image.
pub fn decode_entity_name(name: &str) -> Option<SemanticId> {
    let mut runs: Vec<String> = Vec::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            runs.push(c.to_ascii_lowercase().to_string());
        } else if c == '_' {
            runs.push(String::new());
        } else {
            runs.last_mut()?.push(c);
        }
    }
    let id = SemanticId::parse(&runs.join("_"))?;
    (encode_entity_name(&id) == name).then_some(id)
}
