//! Canonical persistence documents (data-model.md): entity content and versions, state references,
//! the store genesis and its evidence policy, commit bundles, transition records, and replay
//! reports. Every document is canonical JSON identified by a domain-tagged SHA-256.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use behavior_verify::hashing::document_hash;

pub const TAG_ENTITY_CONTENT: &str = "behavior.entity_content.v1";
pub const TAG_ENTITY_VERSION: &str = "behavior.entity_version.v1";
pub const TAG_STATE: &str = "behavior.state.v1";
pub const TAG_EVIDENCE_POLICY: &str = "behavior.evidence_policy.v1";
pub const TAG_GENESIS: &str = "behavior.store_genesis.v1";
pub const TAG_COMMIT_BUNDLE: &str = "behavior.commit_bundle.v1";
pub const TAG_TRANSITION_RECORD: &str = "behavior.transition_record.v1";
pub const TAG_REPLAY_REPORT: &str = "behavior.replay_report.v1";

/// Every store document tag (feature 008: reported by `behavior engine-info`).
pub const DOCUMENT_TAGS: &[&str] = &[
    TAG_ENTITY_CONTENT,
    TAG_ENTITY_VERSION,
    TAG_STATE,
    TAG_EVIDENCE_POLICY,
    TAG_GENESIS,
    TAG_COMMIT_BUNDLE,
    TAG_TRANSITION_RECORD,
    TAG_REPLAY_REPORT,
];

/// Errors of the store and its documents (data-model.md → Store results and errors).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("STATE_CONFLICT: the store is at {current:?}, not the expected parent")]
    StateConflict {
        current: StateRef,
        changed: Vec<EntityKey>,
    },
    #[error("NOTHING_TO_COMMIT: {0}")]
    NothingToCommit(String),
    #[error("ENTITY_NOT_FOUND: {0}")]
    EntityNotFound(String),
    #[error("ENTITY_DECLARATION_MISMATCH: {0}")]
    EntityDeclarationMismatch(String),
    #[error("BUNDLE_INVALID: {0}")]
    BundleInvalid(String),
    #[error("EVIDENCE_REQUIRED: {0}")]
    EvidenceRequired(String),
    #[error("EVIDENCE_MISMATCH: {0}")]
    EvidenceMismatch(String),
    #[error("ENTITY_UNIVERSE_CHANGED: {0}")]
    EntityUniverseChanged(String),
    #[error("GENESIS_INVALID: {0}")]
    GenesisInvalid(String),
    #[error("ENTITY_ID_ALREADY_USED: {0}")]
    EntityIdAlreadyUsed(String),
    #[error("DANGLING_REFERENCE: {0}")]
    DanglingReference(String),
    #[error("BACKEND_ERROR: {0}")]
    Backend(String),
}

impl StoreError {
    /// The stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            StoreError::StateConflict { .. } => "STATE_CONFLICT",
            StoreError::NothingToCommit(_) => "NOTHING_TO_COMMIT",
            StoreError::EntityNotFound(_) => "ENTITY_NOT_FOUND",
            StoreError::EntityDeclarationMismatch(_) => "ENTITY_DECLARATION_MISMATCH",
            StoreError::BundleInvalid(_) => "BUNDLE_INVALID",
            StoreError::EvidenceRequired(_) => "EVIDENCE_REQUIRED",
            StoreError::EvidenceMismatch(_) => "EVIDENCE_MISMATCH",
            StoreError::EntityUniverseChanged(_) => "ENTITY_UNIVERSE_CHANGED",
            StoreError::GenesisInvalid(_) => "GENESIS_INVALID",
            StoreError::EntityIdAlreadyUsed(_) => "ENTITY_ID_ALREADY_USED",
            StoreError::DanglingReference(_) => "DANGLING_REFERENCE",
            StoreError::Backend(_) => "BACKEND_ERROR",
        }
    }
}

pub type R<T> = Result<T, StoreError>;

/// The domain-tagged hash of a serializable document.
pub fn hash_of<T: Serialize>(tag: &str, doc: &T) -> R<String> {
    let v = serde_json::to_value(doc).map_err(|e| StoreError::BundleInvalid(e.to_string()))?;
    document_hash(tag, &v).map_err(|e| StoreError::BundleInvalid(e.to_string()))
}

/// Decodes a document strictly (unknown fields are refused).
pub fn decode<T: for<'de> Deserialize<'de>>(what: &str, v: &Json) -> R<T> {
    serde_json::from_value(v.clone()).map_err(|e| StoreError::BundleInvalid(format!("{what}: {e}")))
}

/// An entity instance: type name and id (string form).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityKey {
    pub entity: String,
    pub id: String,
}

impl std::fmt::Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.entity, self.id)
    }
}

/// What an entity *is* (enters the state identity): type, declaration, id, canonical value.
/// Revisions and positions are history, not content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityContent {
    pub entity: String,
    pub declaration: String,
    pub id: String,
    pub value: Json,
}

impl EntityContent {
    pub fn hash(&self) -> R<String> {
        hash_of(TAG_ENTITY_CONTENT, self)
    }
}

/// One historical incarnation of an entity. `created_at` is the position of the commit that
/// created it (0 for the seed); stored versions never change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityVersion {
    pub content_hash: String,
    pub entity: String,
    pub id: String,
    pub revision: u64,
    pub created_at: u64,
    pub value: Json,
}

impl EntityVersion {
    pub fn key(&self) -> EntityKey {
        EntityKey {
            entity: self.entity.clone(),
            id: self.id.clone(),
        }
    }

    pub fn content(&self, declaration: &str) -> EntityContent {
        EntityContent {
            entity: self.entity.clone(),
            declaration: declaration.to_string(),
            id: self.id.clone(),
            value: self.value.clone(),
        }
    }

    pub fn hash(&self) -> R<String> {
        hash_of(TAG_ENTITY_VERSION, self)
    }
}

/// A state (content identity) at a history position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRef {
    pub state: String,
    pub position: u64,
}

/// The store's head: current state, the state accumulator (hex), and the last record (or the
/// genesis hash before the first commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Head {
    pub state_ref: StateRef,
    pub acc_num: String,
    pub acc_den: String,
    pub last_record: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Require {
    None,
    CommitAuthorization,
}

/// What governance evidence a commit needs (research R9); fixed in the genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePolicy {
    pub format: String,
    pub require: Require,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_execution_policies: Option<Vec<String>>,
}

impl EvidencePolicy {
    pub fn none() -> Self {
        EvidencePolicy {
            format: TAG_EVIDENCE_POLICY.into(),
            require: Require::None,
            trusted_execution_policies: None,
        }
    }

    pub fn hash(&self) -> R<String> {
        hash_of(TAG_EVIDENCE_POLICY, self)
    }

    pub fn validate(&self) -> R<()> {
        if self.format != TAG_EVIDENCE_POLICY {
            return Err(StoreError::GenesisInvalid(format!(
                "evidence policy format `{}`",
                self.format
            )));
        }
        Ok(())
    }
}

/// One seed entity of the genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedEntity {
    pub entity: String,
    pub value: Json,
}

/// The document a store is created from; its hash is the store identity and the chain root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    pub format: String,
    pub evidence_policy: EvidencePolicy,
    pub entity_declarations: BTreeMap<String, String>,
    pub seed: Vec<SeedEntity>,
}

impl Genesis {
    pub fn hash(&self) -> R<String> {
        hash_of(TAG_GENESIS, self)
    }
}

/// An observed read of one entity: the revision at the evaluated state and the fields read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadEntry {
    pub entity: String,
    pub id: String,
    pub revision: u64,
    pub fields: Vec<String>,
}

/// A written cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteEntry {
    pub entity: String,
    pub id: String,
    pub field: String,
    pub old: Json,
    pub new: Json,
}

/// A lifecycle write (feature 006): the existence of `entity#id` changes (`create` also writes
/// every field).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleWrite {
    pub op: String,
    pub entity: String,
    pub id: String,
}

/// A removal in a transition record: the last version the entity had.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovedEntity {
    pub entity: String,
    pub id: String,
    pub last_revision: u64,
    pub last_content_hash: String,
}

/// A change of the derived reverse-reference index (feature 006): `source.field` starts (`add`)
/// or stops (`drop`) pointing at `target`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefChange {
    pub target: EntityKey,
    pub source: EntityKey,
    pub field: String,
    pub op: String,
}

fn is_null(v: &Json) -> bool {
    v.is_null()
}

/// Governance evidence for one commit (feature 002 documents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub authorization: Json,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_policy: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<Json>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waivers: Vec<Json>,
}

/// What an allowed evaluation hands to a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitBundle {
    pub format: String,
    pub evaluated_state: StateRef,
    pub behavior_version: String,
    pub store: String,
    pub record: Json,
    pub transition_hash: String,
    pub entity_declarations: BTreeMap<String, String>,
    pub read_set: Vec<ReadEntry>,
    pub write_set: Vec<WriteEntry>,
    /// The evaluation facts the decision observed (feature 006; the record's `facts` section):
    /// existence, identity and reference reads. Omitted when there are none.
    #[serde(default, skip_serializing_if = "is_null")]
    pub read_facts: Json,
    /// Creations and removals (feature 006), omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_lifecycle: Vec<LifecycleWrite>,
    pub commit_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
}

impl CommitBundle {
    pub fn hash(&self) -> R<String> {
        hash_of(TAG_COMMIT_BUNDLE, self)
    }

    /// The same bundle with evidence attached (the transition hash is unchanged).
    pub fn with_evidence(&self, evidence: Evidence) -> CommitBundle {
        let mut b = self.clone();
        b.evidence = Some(evidence);
        b
    }
}

/// A committed bundle: the commit object. Records form a hash chain from the genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionRecord {
    pub format: String,
    pub position: u64,
    pub previous_record: String,
    pub bundle: CommitBundle,
    pub bundle_hash: String,
    pub evaluated_against: StateRef,
    pub committed_on: StateRef,
    pub result_state: StateRef,
    pub new_versions: Vec<EntityVersion>,
    pub evidence_policy: String,
    pub authorization: Option<String>,
    /// Entities created by this transition, at revision 1 (feature 006).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<EntityVersion>,
    /// Entities removed by this transition; their versions are kept (feature 006).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<RemovedEntity>,
    /// Changes of the derived reverse-reference index (feature 006).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ref_changes: Vec<RefChange>,
}

impl TransitionRecord {
    pub fn hash(&self) -> R<String> {
        hash_of(TAG_TRANSITION_RECORD, self)
    }
}

/// The first divergence found by a replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Divergence {
    pub position: u64,
    pub kind: String,
    pub expected: String,
    pub found: String,
}

/// The result of a data or behavior replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayReport {
    pub format: String,
    pub kind: String,
    pub from: StateRef,
    pub to: StateRef,
    pub checked: u64,
    pub ok: bool,
    pub divergence: Option<Divergence>,
}

/// `store:<genesis>;state:<state id>;position:<n>`: binds a decision record to its store and
/// evaluated state (FR-019).
pub fn data_version(store: &str, at: &StateRef) -> String {
    format!("store:{store};state:{};position:{}", at.state, at.position)
}

/// An RFC 3339 UTC timestamp `YYYY-MM-DDTHH:MM:SSZ` (as in feature 002 governance).
pub fn valid_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    b.len() == 20
        && digits.iter().all(|&i| b[i].is_ascii_digit())
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
}
