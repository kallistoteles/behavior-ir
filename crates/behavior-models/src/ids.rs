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
