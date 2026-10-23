//! Untrusted finite merge choices. No supplied keys, source code or proof authority.
//! The generic Lean replay theorem establishes completeness and uniqueness.
use super::{Inventory, SafetyError};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Leaf,
    Branch {
        left: usize,
        right: usize,
        choices: Vec<bool>,
    },
}
#[derive(Clone, Debug)]
pub(super) struct MergeWitness {
    nodes: Vec<Node>,
}
fn invalid() -> SafetyError {
    SafetyError::new(
        "SAFETY_INVALID_WITNESS",
        "Expected a complete finite postorder merge tree with typed Boolean choices",
    )
}
fn exact(value: &Value, names: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|obj| obj.len() == names.len() && names.iter().all(|n| obj.contains_key(*n)))
}
fn index(value: &Value) -> Result<usize, SafetyError> {
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(invalid)
}
impl MergeWitness {
    pub(super) fn from_json(value: &Value) -> Result<Self, SafetyError> {
        if !exact(value, &["format", "nodes", "root"])
            || value["format"] != "behavior.safety_merge_trace.v1"
        {
            return Err(invalid());
        }
        let rows = value["nodes"].as_array().ok_or_else(invalid)?;
        if rows.is_empty() || index(&value["root"])? != rows.len() - 1 {
            return Err(invalid());
        }
        let mut parents = vec![0_u8; rows.len()];
        let mut nodes = Vec::with_capacity(rows.len());
        for (position, row) in rows.iter().enumerate() {
            let node = match row["kind"].as_str() {
                Some("empty") if exact(row, &["kind"]) => Node::Empty,
                Some("leaf") if exact(row, &["kind"]) => Node::Leaf,
                Some("branch") if exact(row, &["kind", "left", "right", "choices"]) => {
                    let left = index(&row["left"])?;
                    let right = index(&row["right"])?;
                    for child in [left, right] {
                        if child >= position || parents[child] != 0 {
                            return Err(invalid());
                        }
                        parents[child] = 1;
                    }
                    let choices = row["choices"]
                        .as_array()
                        .ok_or_else(invalid)?
                        .iter()
                        .map(|v| v.as_bool().ok_or_else(invalid))
                        .collect::<Result<Vec<_>, _>>()?;
                    Node::Branch {
                        left,
                        right,
                        choices,
                    }
                }
                _ => return Err(invalid()),
            };
            nodes.push(node);
        }
        if parents[..parents.len() - 1].iter().any(|p| *p != 1) {
            return Err(invalid());
        }
        Ok(Self { nodes })
    }
    /// Resource bounds depend on the independently expected input, not a producer count.
    pub(super) fn check_bounds(&self, actions: usize) -> Result<(), SafetyError> {
        let max_nodes = actions
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(invalid)?;
        let max_choices = actions
            .checked_mul(actions)
            .and_then(|v| v.checked_add(actions))
            .ok_or_else(invalid)?;
        let choices = self
            .nodes
            .iter()
            .try_fold(0_usize, |sum, node| match node {
                Node::Branch { choices, .. } => sum.checked_add(choices.len()),
                _ => Some(sum),
            })
            .ok_or_else(invalid)?;
        if self.nodes.len() > max_nodes || choices > max_choices {
            return Err(invalid());
        }
        Ok(())
    }
    pub(super) fn to_json(&self) -> Value {
        json!({"format":"behavior.safety_merge_trace.v1","root":self.nodes.len()-1,
            "nodes":self.nodes.iter().map(|node| match node {
                Node::Empty=>json!({"kind":"empty"}), Node::Leaf=>json!({"kind":"leaf"}),
                Node::Branch{left,right,choices}=>json!({"kind":"branch","left":left,"right":right,"choices":choices})
            }).collect::<Vec<_>>()})
    }
    /// A fallible/unverified producer. Its correctness is not a checker assumption.
    pub(super) fn produce(inventory: &Inventory) -> Self {
        fn build(keys: &[String], nodes: &mut Vec<Node>) -> (usize, Vec<String>) {
            let node = if keys.is_empty() {
                Node::Empty
            } else if keys.len() == 1 {
                Node::Leaf
            } else {
                let mid = keys.len() / 2;
                let (left, ls) = build(&keys[..mid], nodes);
                let (right, rs) = build(&keys[mid..], nodes);
                let (mut l, mut r) = (0, 0);
                let mut choices = Vec::with_capacity(keys.len());
                let mut sorted = Vec::with_capacity(keys.len());
                while l < ls.len() || r < rs.len() {
                    let take_left = r == rs.len() || (l < ls.len() && ls[l] <= rs[r]);
                    choices.push(take_left);
                    if take_left {
                        sorted.push(ls[l].clone());
                        l += 1;
                    } else {
                        sorted.push(rs[r].clone());
                        r += 1;
                    }
                }
                let result = nodes.len();
                nodes.push(Node::Branch {
                    left,
                    right,
                    choices,
                });
                return (result, sorted);
            };
            let result = nodes.len();
            nodes.push(node);
            (result, keys.to_vec())
        }
        let keys = inventory
            .actions()
            .values()
            .map(|a| a.hash.clone())
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        build(&keys, &mut nodes);
        Self { nodes }
    }
    /// Fixed source template, containing only validated finite constructors.
    pub(super) fn lean(&self) -> String {
        let mut text = String::new();
        for (position, node) in self.nodes.iter().enumerate() {
            let body = match node {
                Node::Empty => ".empty".to_owned(),
                Node::Leaf => ".leaf".to_owned(),
                Node::Branch {
                    left,
                    right,
                    choices,
                } => format!(
                    ".branch trace{left} trace{right} [{}]",
                    choices
                        .iter()
                        .map(bool::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            };
            text.push_str(&format!(
                "noncomputable def trace{position} : Safety.Candidate.MergeTrace := {body}\n"
            ));
        }
        text.push_str(&format!(
            "noncomputable def proposed : Safety.Candidate.MergeTrace := trace{}\n",
            self.nodes.len() - 1
        ));
        text
    }
}
