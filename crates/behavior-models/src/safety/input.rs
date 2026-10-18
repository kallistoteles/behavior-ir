//! Typed analysis boundaries; certification is a separate operation.

use behavior_engine::semantic::Module;
use serde_json::{Value, json};

use super::inventory::Inventory;
use std::collections::{BTreeMap, BTreeSet};

/// A selector is resolved against final admitted actions, never parameter spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionSelector {
    Authored(String),
    Generated {
        model_id: String,
        local_action_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct FieldOwnershipInput {
    pub author_id: String,
    pub entity: String,
    pub writers: BTreeMap<String, Vec<ActionSelector>>,
    pub creators: Vec<ActionSelector>,
    pub removers: Vec<ActionSelector>,
}

/// A recording of the restricted v1 relationship. All meaning is resolved against Core.
#[derive(Debug, Clone)]
pub struct LifecycleDependencyInput {
    pub author_id: String,
    pub parent: String,
    pub child: String,
    pub owner_field: String,
    pub parent_state: String,
    pub child_state: String,
    pub prohibited: BTreeSet<String>,
    pub terminal: BTreeSet<String>,
    pub ownership: BTreeSet<String>,
}

impl LifecycleDependencyInput {
    pub fn from_json(v: &Value) -> Result<Self, SafetyError> {
        let bad = || {
            SafetyError::new(
                "SAFETY_INVALID_DEPENDENCY",
                "Expected an exact LifecycleDependency record with unique member/control lists",
            )
        };
        let o = v.as_object().ok_or_else(bad)?;
        let keys = [
            "id",
            "parent",
            "child",
            "owner_field",
            "parent_state",
            "child_state",
            "prohibited",
            "terminal",
            "ownership",
        ];
        if o.len() != keys.len() || keys.iter().any(|k| !o.contains_key(*k)) {
            return Err(bad());
        }
        let text = |k| {
            v.get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or_else(bad)
        };
        let members = |k| -> Result<BTreeSet<String>, SafetyError> {
            let mut out = BTreeSet::new();
            for item in v.get(k).and_then(Value::as_array).ok_or_else(bad)? {
                let s = item.as_str().filter(|s| !s.is_empty()).ok_or_else(bad)?;
                if !out.insert(s.to_owned()) {
                    return Err(bad());
                }
            }
            Ok(out)
        };
        Ok(Self {
            author_id: text("id")?,
            parent: text("parent")?,
            child: text("child")?,
            owner_field: text("owner_field")?,
            parent_state: text("parent_state")?,
            child_state: text("child_state")?,
            prohibited: members("prohibited")?,
            terminal: members("terminal")?,
            ownership: members("ownership")?,
        })
    }
}

impl FieldOwnershipInput {
    pub fn from_json(v: &Value) -> Result<Self, SafetyError> {
        let bad = || {
            SafetyError::new(
                "SAFETY_INVALID_POLICY",
                "Expected a FieldOwnership record with explicit selector lists",
            )
        };
        let obj = v.as_object().ok_or_else(bad)?;
        if obj
            .keys()
            .any(|k| !["id", "entity", "writers", "creators", "removers"].contains(&k.as_str()))
        {
            return Err(bad());
        }
        let text = |k| {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(bad)
        };
        let mut writers = BTreeMap::new();
        if let Some(fields) = v.get("writers") {
            for (f, s) in fields.as_object().ok_or_else(bad)? {
                writers.insert(f.clone(), selectors(s)?);
            }
        }
        Ok(Self {
            author_id: text("id")?,
            entity: text("entity")?,
            writers,
            creators: v
                .get("creators")
                .map(selectors)
                .transpose()?
                .unwrap_or_default(),
            removers: v
                .get("removers")
                .map(selectors)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

fn selectors(v: &Value) -> Result<Vec<ActionSelector>, SafetyError> {
    let bad = || {
        SafetyError::new(
            "SAFETY_INVALID_SELECTOR",
            "Expected unique exact action names or generated action pairs",
        )
    };
    let mut out = BTreeSet::new();
    for s in v.as_array().ok_or_else(bad)? {
        let selector = if let Some(name) = s.as_str() {
            ActionSelector::Authored(name.into())
        } else {
            let o = s.as_object().ok_or_else(bad)?;
            if o.len() != 2 {
                return Err(bad());
            }
            ActionSelector::Generated {
                model_id: s
                    .get("model_id")
                    .and_then(Value::as_str)
                    .ok_or_else(bad)?
                    .into(),
                local_action_id: s
                    .get("local_action_id")
                    .and_then(Value::as_str)
                    .ok_or_else(bad)?
                    .into(),
            }
        };
        if !out.insert(selector) {
            return Err(bad());
        }
    }
    Ok(out.into_iter().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct SafetyError {
    pub(crate) code: &'static str,
    pub(crate) detail: String,
    pub(crate) action: Option<String>,
    pub(crate) entity: Option<String>,
    pub(crate) field: Option<String>,
}

impl SafetyError {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
            action: None,
            entity: None,
            field: None,
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn to_json(&self) -> Value {
        json!({"code": self.code, "detail": self.detail, "action": self.action,
            "entity": self.entity, "field": self.field})
    }
}

/// A successful structural build. This type cannot be converted to a certificate by a flag.
#[derive(Debug, Clone)]
pub struct UncertifiedSafetyAnalysis {
    pub(crate) inventory: Inventory,
    pub(crate) policies: Vec<super::resolve::ResolvedOwnership>,
    pub(crate) policy_set_hash: String,
    pub(crate) dependencies: Vec<super::lifecycle_dependency::ResolvedDependency>,
    pub(crate) issues: Vec<SafetyError>,
    pub(crate) lifecycle_actions: BTreeMap<String, super::lifecycle_dependency::ActionProfile>,
}

impl UncertifiedSafetyAnalysis {
    /// Only an admitted Core module can establish this lowering boundary. Transported
    /// inventory data alone cannot claim a successful Core build.
    pub fn new(module: &Module) -> Result<Self, SafetyError> {
        Ok(Self {
            inventory: Inventory::extract(module)?,
            policies: vec![],
            policy_set_hash: super::identity::policy_set_hash(&[])?,
            dependencies: vec![],
            issues: vec![],
            lifecycle_actions: BTreeMap::new(),
        })
    }
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }
    pub fn status(&self) -> &'static str {
        "Lowered"
    }
    pub fn is_certified(&self) -> bool {
        false
    }
    pub fn policy_set_hash(&self) -> &str {
        &self.policy_set_hash
    }
    pub(crate) fn policy_records(&self) -> Vec<Value> {
        let mut records = self
            .policies
            .iter()
            .map(|p| json!({"model_hash":p.model_hash(),"document":p.document()}))
            .chain(
                self.dependencies
                    .iter()
                    .map(|p| json!({"model_hash":p.model_hash(),"document":p.document()})),
            )
            .collect::<Vec<_>>();
        records.sort_by(|a, b| a["model_hash"].as_str().cmp(&b["model_hash"].as_str()));
        records
    }
    pub(crate) fn policy_documents(&self) -> Vec<Value> {
        self.policy_records()
            .iter()
            .map(|r| r["document"].clone())
            .collect()
    }
    pub fn to_json(&self) -> Value {
        json!({"status":self.status(),"is_certified":false,"analysis_version":1,"checker_version":super::SAFETY_CHECKER_VERSION,
            "policy_set_hash":self.policy_set_hash,"inventory":self.inventory.to_json(),"policies":self.policy_records(),
            "issues":self.issues.iter().map(|e|e.to_json()).collect::<Vec<_>>(),
            "dependencies":self.dependencies.iter().map(|d|json!({"model_hash":d.model_hash(),"document":d.document()})).collect::<Vec<_>>()})
    }
    pub fn policies(&self) -> &[super::resolve::ResolvedOwnership] {
        &self.policies
    }
    pub fn dependencies(&self) -> &[super::lifecycle_dependency::ResolvedDependency] {
        &self.dependencies
    }
    pub fn issues(&self) -> &[SafetyError] {
        &self.issues
    }
    pub fn lifecycle_actions(
        &self,
    ) -> &BTreeMap<String, super::lifecycle_dependency::ActionProfile> {
        &self.lifecycle_actions
    }
}
