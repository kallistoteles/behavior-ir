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

use documents::{EntityKey, EntityVersion, Genesis, Head, TransitionRecord};

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
    fn create(
        &mut self,
        genesis: &Genesis,
        head: &Head,
        seed: &[EntityVersion],
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
    fn commit(
        &mut self,
        expected_last_record: &str,
        versions: &[EntityVersion],
        record: &TransitionRecord,
        new_head: &Head,
    ) -> Result<CasOutcome, BackendError>;
}
