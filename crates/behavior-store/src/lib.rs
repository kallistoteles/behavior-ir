//! The persistence contract (feature 005): canonical state, transition and commit documents, a
//! content-addressed state identity, and the store semantics over a host-provided backend.
//!
//! The engine defines what must be preserved for behavior to be reproducible; hosts choose how it
//! is stored (specs/005-persistence-contract).

#![forbid(unsafe_code)]

pub mod conformance;
pub mod documents;
pub mod memory;
pub mod muhash;
pub mod replay;
pub mod store;

use documents::{EntityKey, EntityVersion, Genesis, Head, RefChange, TransitionRecord};

pub use behavior_core::RefEdge;

pub use documents::StoreError;
pub use memory::InMemoryBackend;
pub use store::{Committed, Evaluation, Store};

/// A failure of the host's storage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct BackendError(pub String);

/// The result of the atomic commit primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasOutcome {
    /// Everything was written and the head moved.
    Applied,
    /// The head was not the expected one; nothing was written.
    HeadMoved,
}

/// Host-implemented persistence primitives (research R2). Stored versions and records never
/// change; `commit` is the only write after `create`, and it is atomic and crash-safe: it writes
/// the versions, the record (with its idempotency information) and the new head as one unit,
/// only if the head's last record is still `expected_last_record`.
pub trait Backend {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError>;
    fn head(&self) -> Result<Option<Head>, BackendError>;
    /// Creates the store: genesis, head, seed versions, and the seed's reference index (edges
    /// added at position 0).
    fn create(
        &mut self,
        genesis: &Genesis,
        head: &Head,
        seed: &[EntityVersion],
        seed_refs: &[RefChange],
    ) -> Result<(), BackendError>;
    /// The version of `key` with the newest `created_at <= position` (an as-of read: reads at one
    /// position form a consistent snapshot even while commits land).
    fn version_at(
        &self,
        key: &EntityKey,
        position: u64,
    ) -> Result<Option<EntityVersion>, BackendError>;
    fn version(
        &self,
        key: &EntityKey,
        revision: u64,
    ) -> Result<Option<EntityVersion>, BackendError>;
    fn record(&self, position: u64) -> Result<Option<TransitionRecord>, BackendError>;
    /// The position at which `key` was removed, if it was (feature 006). Set once, inside the
    /// commit that removes it, and never changed; versions of a removed entity are kept.
    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError>;
    /// The surviving references to `target` as of `position` (feature 006): the derived
    /// reverse-reference index, folded from its edge events (`added_at <= position` and not
    /// dropped at or before `position`).
    fn incoming_at(&self, target: &EntityKey, position: u64) -> Result<Vec<RefEdge>, BackendError>;
    /// Whether the store has used `key` as of `position` (an identity names one lifetime,
    /// feature 006): some version of it exists at or before `position`.
    fn used_at(&self, key: &EntityKey, position: u64) -> Result<bool, BackendError> {
        Ok(self.version_at(key, position)?.is_some())
    }
    /// The atomic compare-and-set: versions (updates and creations), removals (`removed_at` =
    /// the new head's position), index changes, the record and the head, as one unit.
    #[allow(clippy::too_many_arguments)]
    fn commit(
        &mut self,
        expected_last_record: &str,
        versions: &[EntityVersion],
        removals: &[EntityKey],
        ref_changes: &[RefChange],
        record: &TransitionRecord,
        new_head: &Head,
    ) -> Result<CasOutcome, BackendError>;
}
