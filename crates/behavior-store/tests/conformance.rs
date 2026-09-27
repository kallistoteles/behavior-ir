#![allow(clippy::unwrap_used, clippy::expect_used)]

//! US3: the conformance suite. The reference backend passes every case; each broken backend fails
//! its designated case (SC-003).

use behavior_store::conformance::run;
use behavior_store::documents::{EntityKey, EntityVersion, Genesis, Head, TransitionRecord};
use behavior_store::{Backend, BackendError, CasOutcome, InMemoryBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Broken {
    IgnoresHead,
    PartialWrites,
    ReordersRecords,
    DropsVersions,
    LatestReads,
    NonAtomicHead,
}

/// A deliberately broken backend around the reference backend.
struct Mutant {
    inner: InMemoryBackend,
    kind: Broken,
    /// NonAtomicHead: positions whose record was lost (the head moved, then a crash lost the record).
    lost: Vec<u64>,
    /// DropsVersions: versions forgotten at commit.
    dropped: Vec<(EntityKey, u64)>,
}

impl Mutant {
    fn new(kind: Broken) -> Self {
        Mutant {
            inner: InMemoryBackend::new(),
            kind,
            lost: Vec::new(),
            dropped: Vec::new(),
        }
    }
}

impl Backend for Mutant {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.inner.genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.inner.head()
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        seed: &[EntityVersion],
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        match self.kind {
            Broken::LatestReads => self.inner.version_at(k, u64::MAX),
            Broken::DropsVersions => Ok(self
                .inner
                .version_at(k, p)?
                .filter(|v| !self.dropped.contains(&(k.clone(), v.revision)))),
            _ => self.inner.version_at(k, p),
        }
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        if self.lost.contains(&p) {
            return Ok(None);
        }
        match (self.kind, p) {
            (Broken::ReordersRecords, 1) => self.inner.record(2),
            (Broken::ReordersRecords, 2) => self.inner.record(1),
            _ => self.inner.record(p),
        }
    }
    fn commit(
        &mut self,
        expected: &str,
        versions: &[EntityVersion],
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        match self.kind {
            Broken::IgnoresHead => {
                let actual = self.inner.head()?.unwrap().last_record;
                self.inner.commit(&actual, versions, record, head)
            }
            Broken::PartialWrites => {
                let out = self.inner.commit(expected, versions, record, head)?;
                if out == CasOutcome::HeadMoved {
                    // Writes the versions anyway (a torn write), without moving the head.
                    let actual = self.inner.head()?.unwrap();
                    self.inner
                        .commit(&actual.last_record.clone(), versions, record, &actual)?;
                }
                Ok(out)
            }
            Broken::DropsVersions => {
                for v in versions {
                    if v.revision > 1 {
                        self.dropped.push((v.key(), v.revision - 1));
                    }
                }
                self.inner.commit(expected, versions, record, head)
            }
            Broken::NonAtomicHead => {
                // Moves the head (and versions) now, writes the record only later.
                // Moves the head and writes the versions, but the record is not part of the same
                // atomic unit: a crash between the two (modelled as always happening) loses it.
                let out = self.inner.commit(expected, versions, record, head)?;
                if out == CasOutcome::Applied {
                    self.lost.push(record.position);
                }
                Ok(out)
            }
            _ => self.inner.commit(expected, versions, record, head),
        }
    }
}

#[test]
fn the_reference_backend_passes_every_case() {
    let report = run(InMemoryBackend::new);
    assert_eq!(report.cases.len(), 17);
    let failed: Vec<_> = report.cases.iter().filter(|c| !c.ok).collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn each_broken_backend_fails_its_designated_case() {
    for (kind, case) in [
        (Broken::IgnoresHead, "conflict_on_outdated_parent"),
        (Broken::PartialWrites, "no_partial_application"),
        (Broken::ReordersRecords, "history_between_states"),
        (Broken::DropsVersions, "load_at_past_state"),
        (Broken::LatestReads, "snapshot_consistency"),
        (Broken::NonAtomicHead, "crash_retry"),
    ] {
        let report = run(|| Mutant::new(kind));
        let c = report.case(case).unwrap();
        assert!(!c.ok, "{kind:?} must fail {case}");
        assert!(
            !c.message.is_empty(),
            "{kind:?}: the failure names the violated rule"
        );
    }
}
