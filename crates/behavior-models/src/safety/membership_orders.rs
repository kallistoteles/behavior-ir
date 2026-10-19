//! Untrusted literal-order hints only. No serialized operator/key has semantic authority.
use super::SafetyError;
use behavior_engine::semantic::Module;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    bytes: usize,
    nodes: usize,
    depth: usize,
    vectors: usize,
    values: usize,
    probes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 16 * 1024 * 1024,
            nodes: 1_000_000,
            depth: 128,
            vectors: 4096,
            values: 4096,
            probes: 4096,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub(super) struct MembershipOrders {
    vectors: BTreeSet<Vec<String>>,
    by_set: BTreeMap<BTreeSet<String>, Vec<Vec<String>>>,
    probes: usize,
    limits: Limits,
}
impl MembershipOrders {
    pub(super) fn discover(module: &Module) -> Result<Self, SafetyError> {
        Self::decode(
            &behavior_engine::serialize::to_wire_json(module),
            Limits::default(),
        )
    }
    fn decode(json: &str, limits: Limits) -> Result<Self, SafetyError> {
        let exhausted = || {
            SafetyError::new(
                "SAFETY_GUARD_SCAN_EXHAUSTED",
                "Literal-order discovery exceeded its cumulative resource budget",
            )
        };
        if json.len() > limits.bytes {
            return Err(exhausted());
        }
        // Core allocates the serialization String before the byte check. This allocation
        // is a process/build resource boundary, not a bounded streaming guarantee.
        let value: serde_json::Value = serde_json::from_str(json).map_err(|_| exhausted())?;
        let mut result = Self {
            limits,
            ..Self::default()
        };
        let mut stack = vec![(&value, 0usize)];
        let mut visited = 0usize;
        while let Some((value, depth)) = stack.pop() {
            if visited >= limits.nodes || depth > limits.depth {
                return Err(exhausted());
            }
            visited += 1;
            match value {
                serde_json::Value::Array(values) => {
                    if !values.is_empty() && values.iter().all(serde_json::Value::is_string) {
                        if values.len() > limits.values {
                            return Err(exhausted());
                        }
                        let vector = values
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>();
                        if !result.vectors.contains(&vector)
                            && result.vectors.len() >= limits.vectors
                        {
                            return Err(exhausted());
                        }
                        result.vectors.insert(vector);
                    }
                    // Check before extending the work queue, including scalar nodes.
                    if values.len() > limits.nodes.saturating_sub(visited + stack.len()) {
                        return Err(exhausted());
                    }
                    stack.extend(values.iter().rev().map(|v| (v, depth + 1)));
                }
                serde_json::Value::Object(values) => {
                    if values.len() > limits.nodes.saturating_sub(visited + stack.len()) {
                        return Err(exhausted());
                    }
                    stack.extend(values.values().rev().map(|v| (v, depth + 1)));
                }
                _ => {}
            }
        }
        for vector in &result.vectors {
            result
                .by_set
                .entry(vector.iter().cloned().collect())
                .or_default()
                .push(vector.clone());
        }
        Ok(result)
    }
    pub(super) fn candidates(&self, required: &BTreeSet<String>) -> Vec<Vec<String>> {
        self.by_set.get(required).cloned().unwrap_or_default()
    }
    pub(super) fn charge_probe(&mut self) -> Result<(), SafetyError> {
        if self.probes >= self.limits.probes {
            return Err(SafetyError::new(
                "SAFETY_GUARD_SCAN_EXHAUSTED",
                "Literal-order Core probe budget exhausted",
            ));
        }
        self.probes += 1;
        Ok(())
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn hints_retain_order_repetitions_and_exact_sets_only() {
        let scan = MembershipOrders::decode(
            r#"{"ignored":[["b","a","b"],["a","b"],["b","a","b"],["a"],["a","b","c"],[1,"a"]]}"#,
            Limits::default(),
        )
        .unwrap();
        let required = BTreeSet::from(["a".into(), "b".into()]);
        assert_eq!(
            scan.candidates(&required),
            vec![vec!["a", "b"], vec!["b", "a", "b"]]
        );
    }
    #[test]
    fn every_scan_budget_refuses_instead_of_truncating() {
        let json = r#"[["a","b"],["b","a"]]"#;
        for limits in [
            Limits {
                bytes: json.len() - 1,
                ..Limits::default()
            },
            Limits {
                nodes: 3,
                ..Limits::default()
            },
            Limits {
                depth: 1,
                ..Limits::default()
            },
            Limits {
                vectors: 1,
                ..Limits::default()
            },
            Limits {
                values: 1,
                ..Limits::default()
            },
        ] {
            let err = MembershipOrders::decode(json, limits).unwrap_err();
            assert_eq!(err.code(), "SAFETY_GUARD_SCAN_EXHAUSTED");
        }
        let mut scan = MembershipOrders::decode(
            json,
            Limits {
                probes: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        scan.charge_probe().unwrap();
        assert_eq!(
            scan.charge_probe().unwrap_err().code(),
            "SAFETY_GUARD_SCAN_EXHAUSTED"
        );
    }
}
