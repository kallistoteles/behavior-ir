//! Backend conformance (FR-015, research R12): wrappers that inject faults and interleavings at the
//! backend boundary (so they work for any backend without hooks), and the named conformance cases.

use std::cell::RefCell;

use crate::documents::{EntityKey, EntityVersion, Genesis, Head, RefChange, TransitionRecord};
use crate::{Backend, BackendError, CasOutcome, RefEdge};

/// A fault injected into the next commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Fail before calling the inner commit (nothing is written).
    BeforeWrite,
    /// Call the inner commit, then fail: a durable write whose acknowledgement is lost.
    AfterWrite,
}

/// Wraps any backend and injects a fault into the next commit.
pub struct FaultInjector<B> {
    pub inner: B,
    pub next: Option<Fault>,
}

impl<B> FaultInjector<B> {
    pub fn new(inner: B) -> Self {
        FaultInjector { inner, next: None }
    }
}

impl<B: Backend> Backend for FaultInjector<B> {
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
        refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed, refs)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        self.inner.keys_at(t, p)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        self.inner.keys_by_field_at(t, f, v, p)
    }
    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(key)
    }
    fn incoming_at(&self, t: &EntityKey, pos: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(t, pos)
    }
    fn used_at(&self, key: &EntityKey, pos: u64) -> Result<bool, BackendError> {
        self.inner.used_at(key, pos)
    }
    fn version_at(&self, key: &EntityKey, pos: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(key, pos)
    }
    fn version(&self, key: &EntityKey, rev: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(key, rev)
    }
    fn record(&self, pos: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.inner.record(pos)
    }
    fn commit(
        &mut self,
        expected: &str,
        versions: &[EntityVersion],
        removals: &[EntityKey],
        refs: &[RefChange],
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        match self.next.take() {
            Some(Fault::BeforeWrite) => Err(BackendError("injected fault before the write".into())),
            Some(Fault::AfterWrite) => {
                self.inner
                    .commit(expected, versions, removals, refs, record, head)?;
                Err(BackendError(
                    "injected fault after the write (acknowledgement lost)".into(),
                ))
            }
            None => self
                .inner
                .commit(expected, versions, removals, refs, record, head),
        }
    }
}

/// A commit prepared elsewhere, to be landed on a backend.
#[derive(Debug, Clone)]
pub struct Landing {
    pub expected: String,
    pub versions: Vec<EntityVersion>,
    pub removals: Vec<EntityKey>,
    pub ref_changes: Vec<RefChange>,
    pub record: TransitionRecord,
    pub head: Head,
}

impl Landing {
    /// The commit that wrote `record` (and moved the head to `head`), replayed on another copy
    /// whose last record is `expected`.
    pub fn of(expected: String, record: TransitionRecord, head: Head) -> Self {
        let mut versions = record.new_versions.clone();
        versions.extend(record.created.iter().cloned());
        Landing {
            expected,
            versions,
            removals: record
                .removed
                .iter()
                .map(|r| EntityKey {
                    entity: r.entity.clone(),
                    id: r.id.clone(),
                })
                .collect(),
            ref_changes: record.ref_changes.clone(),
            record,
            head,
        }
    }
}

/// When an [`Interleave`] lands its prepared commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandOn {
    /// During the first entity read: between an evaluation's head read and its entity reads.
    FirstRead,
    /// At the start of the next commit: between the store's head check and the compare-and-set.
    Commit,
}

/// Wraps any backend and lands a prepared commit at a chosen moment, simulating a concurrent host.
pub struct Interleave<B> {
    pub inner: RefCell<B>,
    pub landing: RefCell<Option<Landing>>,
    pub on: LandOn,
}

impl<B> Interleave<B> {
    pub fn new(inner: B, landing: Landing) -> Self {
        Interleave::at(inner, landing, LandOn::FirstRead)
    }

    pub fn at(inner: B, landing: Landing, on: LandOn) -> Self {
        Interleave {
            inner: RefCell::new(inner),
            landing: RefCell::new(Some(landing)),
            on,
        }
    }
}

impl<B: Backend> Interleave<B> {
    fn land(&self) -> Result<(), BackendError> {
        if let Some(l) = self.landing.borrow_mut().take() {
            self.inner.borrow_mut().commit(
                &l.expected,
                &l.versions,
                &l.removals,
                &l.ref_changes,
                &l.record,
                &l.head,
            )?;
        }
        Ok(())
    }
}

impl<B: Backend> Backend for Interleave<B> {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError> {
        self.inner.borrow().genesis()
    }
    fn head(&self) -> Result<Option<Head>, BackendError> {
        self.inner.borrow().head()
    }
    fn create(
        &mut self,
        g: &Genesis,
        h: &Head,
        seed: &[EntityVersion],
        refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.inner.borrow_mut().create(g, h, seed, refs)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().keys_at(t, p)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().keys_by_field_at(t, f, v, p)
    }
    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().removed_at(key)
    }
    fn incoming_at(&self, t: &EntityKey, pos: u64) -> Result<Vec<RefEdge>, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().incoming_at(t, pos)
    }
    fn used_at(&self, key: &EntityKey, pos: u64) -> Result<bool, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().used_at(key, pos)
    }
    fn version_at(&self, key: &EntityKey, pos: u64) -> Result<Option<EntityVersion>, BackendError> {
        if self.on == LandOn::FirstRead {
            self.land()?;
        }
        self.inner.borrow().version_at(key, pos)
    }
    fn version(&self, key: &EntityKey, rev: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.borrow().version(key, rev)
    }
    fn record(&self, pos: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.inner.borrow().record(pos)
    }
    fn commit(
        &mut self,
        expected: &str,
        versions: &[EntityVersion],
        removals: &[EntityKey],
        refs: &[RefChange],
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        if self.on == LandOn::Commit {
            self.land()?;
        }
        self.inner
            .borrow_mut()
            .commit(expected, versions, removals, refs, record, head)
    }
}

// --- the named cases (contracts/store-api.md) --------------------------------------------------

use std::collections::BTreeMap;

use behavior_core::semantic::module::Module;
use serde_json::{Value as Json, json};

use crate::documents::{CommitBundle, EvidencePolicy, Require, SeedEntity, StateRef, StoreError};
use crate::replay::{replay_behavior, replay_data};
use crate::store::{Evaluation, Store, genesis_for};

/// The ledger module the cases run against (tests/fixtures/wire/valid/ledger.json).
pub const LEDGER_WIRE: &str = include_str!("../../../tests/fixtures/wire/valid/ledger.json");
const T0: &str = "2026-09-27T12:00:00Z";

/// One conformance case's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    pub name: &'static str,
    pub ok: bool,
    pub message: String,
}

/// The outcome of every case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub cases: Vec<CaseResult>,
}

impl ConformanceReport {
    pub fn ok(&self) -> bool {
        self.cases.iter().all(|c| c.ok)
    }

    pub fn case(&self, name: &str) -> Option<&CaseResult> {
        self.cases.iter().find(|c| c.name == name)
    }
}

type Case<B> = fn(&dyn Fn() -> B) -> Result<(), String>;

fn ledger() -> Result<Module, String> {
    behavior_core::admit(LEDGER_WIRE).map_err(|r| format!("{:?}", r.errors))
}

fn seed() -> Vec<SeedEntity> {
    [
        ("a1", true, "100.00"),
        ("a2", true, "5.00"),
        ("a3", true, "50.00"),
    ]
    .into_iter()
    .map(|(id, active, balance)| SeedEntity {
        entity: "Account".into(),
        value: json!({"id": id, "active": active, "balance": balance}),
    })
    .collect()
}

fn e(err: StoreError) -> String {
    err.to_string()
}

fn create<B: Backend>(backend: B, policy: EvidencePolicy) -> Result<(Store<B>, Module), String> {
    let m = ledger()?;
    let s = Store::create(backend, &m, genesis_for(&m, policy, seed())).map_err(e)?;
    Ok((s, m))
}

fn bind(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(p, id)| (p.to_string(), id.to_string()))
        .collect()
}

fn transfer<B: Backend>(
    s: &Store<B>,
    m: &Module,
    from: &str,
    to: &str,
    amount: &str,
) -> Result<Evaluation, String> {
    s.evaluate(
        m,
        "transfer",
        &bind(&[("from_", from), ("to", to)]),
        &json!({"amount": amount}),
        &json!({}),
        T0,
        None,
    )
    .map_err(e)
}

fn unary<B: Backend>(
    s: &Store<B>,
    m: &Module,
    action: &str,
    id: &str,
) -> Result<Evaluation, String> {
    s.evaluate(
        m,
        action,
        &bind(&[("account", id)]),
        &json!({}),
        &json!({}),
        T0,
        None,
    )
    .map_err(e)
}

fn bundle(ev: Evaluation) -> Result<CommitBundle, String> {
    ev.bundle
        .ok_or_else(|| format!("expected an allowed decision, got {}", ev.record["result"]))
}

fn commit<B: Backend>(
    s: &mut Store<B>,
    m: &Module,
    b: &CommitBundle,
) -> Result<crate::store::Committed, String> {
    s.commit(m, &b.evaluated_state.clone(), b).map_err(e)
}

fn key(id: &str) -> EntityKey {
    EntityKey {
        entity: "Account".into(),
        id: id.into(),
    }
}

fn balance<B: Backend>(s: &Store<B>, id: &str, at: &StateRef) -> Result<Json, String> {
    Ok(s.load(&key(id), at).map_err(e)?.value["balance"].clone())
}

fn ensure(cond: bool, msg: impl Into<String>) -> Result<(), String> {
    if cond { Ok(()) } else { Err(msg.into()) }
}

fn create_and_open<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (s, _) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    ensure(s0.position == 0, "the genesis state is at position 0")?;
    let reopened = Store::open(s.into_backend()).map_err(e)?;
    ensure(
        reopened.current().map_err(e)? == s0,
        "reopening keeps the head",
    )?;
    let (other, _) = create(f(), EvidencePolicy::none())?;
    ensure(
        other.current().map_err(e)?.state == s0.state,
        "the same genesis gives the same S0",
    )
}

fn commit_new_state<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    let c = {
        let b = bundle(transfer(&s, &m, "a1", "a2", "20.00")?)?;
        commit(&mut s, &m, &b)?
    };
    ensure(
        c.result_state.position == 1 && s.current().map_err(e)? == c.result_state,
        "the head moved to position 1",
    )?;
    ensure(
        balance(&s, "a1", &c.result_state)? == json!("80.00"),
        "a1 is 80.00",
    )?;
    ensure(
        s.load(&key("a1"), &c.result_state).map_err(e)?.revision == 2,
        "a1 is at revision 2",
    )?;
    let r = s
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record 1 is missing")?;
    ensure(
        r.committed_on == s0 && r.evaluated_against == s0,
        "the record names its parent",
    )?;
    ensure(
        r.previous_record == s.store_id().map_err(e)?,
        "the record chains to the genesis",
    )
}

fn conflict_on_outdated_parent<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let first = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    let second = bundle(unary(&s, &m, "freeze", "a3")?)?;
    commit(&mut s, &m, &first)?;
    let head = s.current().map_err(e)?;
    match s.commit(&m, &second.evaluated_state.clone(), &second) {
        Err(StoreError::StateConflict { current, changed }) => {
            ensure(current == head, "the conflict names the current state")?;
            ensure(
                changed.len() == 2,
                format!("the conflict names the changed entities: {changed:?}"),
            )?;
        }
        other => return Err(format!("an outdated parent must conflict: {other:?}")),
    }
    ensure(
        s.current().map_err(e)? == head,
        "a refused commit leaves the head",
    )?;
    ensure(
        s.backend().record(2).map_err(|x| x.0)?.is_none(),
        "a refused commit adds no record",
    )?;
    // The race: another host commits between the store's head check and the compare-and-set.
    let (mut other, _) = create(f(), EvidencePolicy::none())?;
    let ob = bundle(transfer(&other, &m, "a1", "a2", "10.00")?)?;
    commit(&mut other, &m, &ob)?;
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record 1 missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let (base, _) = create(f(), EvidencePolicy::none())?;
    let landing = Landing::of(base.store_id().map_err(e)?, rec, head);
    let mut raced =
        Store::open(Interleave::at(base.into_backend(), landing, LandOn::Commit)).map_err(e)?;
    let mine = bundle(unary(&raced, &m, "freeze", "a3")?)?;
    match raced.commit(&m, &mine.evaluated_state.clone(), &mine) {
        Err(StoreError::StateConflict { .. }) => {}
        other => {
            return Err(format!(
                "a head that moved before the compare-and-set must conflict: {other:?}"
            ));
        }
    }
    ensure(
        raced.current().map_err(e)?.position == 1,
        "only the other host's commit is applied",
    )
}

fn no_partial_application<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    // At the primitive: a compare-and-set with a stale expected head writes nothing.
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let mut other = {
        let (o, _) = create(f(), EvidencePolicy::none())?;
        o
    };
    let b = bundle(transfer(&other, &m, "a1", "a2", "10.00")?)?;
    commit(&mut other, &m, &b)?;
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record 1 missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let out = s
        .backend_mut()
        .commit("sha256:stale", &rec.new_versions, &[], &[], &rec, &head)
        .map_err(|x| x.0)?;
    ensure(
        out == CasOutcome::HeadMoved,
        "a stale compare-and-set reports that the head moved",
    )?;
    ensure(
        s.current().map_err(e)?.position == 0,
        "the head did not move",
    )?;
    ensure(
        s.backend().record(1).map_err(|x| x.0)?.is_none(),
        "no record was written",
    )?;
    for v in &rec.new_versions {
        ensure(
            s.backend()
                .version(&v.key(), v.revision)
                .map_err(|x| x.0)?
                .is_none(),
            format!("no version of {} was written", v.key()),
        )?;
    }
    Ok(())
}

fn crash_retry<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(FaultInjector::new(f()), EvidencePolicy::none())?;
    let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    s.backend_mut().next = Some(Fault::BeforeWrite);
    ensure(
        s.commit(&m, &b.evaluated_state.clone(), &b).is_err(),
        "the injected fault is reported",
    )?;
    ensure(
        s.current().map_err(e)?.position == 0,
        "a crash before the write shows nothing",
    )?;
    ensure(!commit(&mut s, &m, &b)?.already, "the retry applies once")?;
    let b2 = bundle(transfer(&s, &m, "a3", "a2", "1.00")?)?;
    s.backend_mut().next = Some(Fault::AfterWrite);
    ensure(
        s.commit(&m, &b2.evaluated_state.clone(), &b2).is_err(),
        "the lost acknowledgement is reported",
    )?;
    ensure(
        s.current().map_err(e)?.position == 2,
        "the durable write is visible",
    )?;
    let b3 = bundle(unary(&s, &m, "touch", "a1")?)?;
    commit(&mut s, &m, &b3)?;
    let mut retry = b2.clone();
    retry.commit_time = "2026-09-27T12:30:00Z".into();
    let r = commit(&mut s, &m, &retry)?;
    ensure(
        r.already,
        "a retry after a lost acknowledgement is recognized",
    )?;
    ensure(
        s.current().map_err(e)?.position == 3,
        "nothing was applied twice",
    )
}

fn snapshot_consistency<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut other, m) = create(f(), EvidencePolicy::none())?;
    let b = bundle(transfer(&other, &m, "a1", "a2", "20.00")?)?;
    commit(&mut other, &m, &b)?;
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record 1 missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let (base, _) = create(f(), EvidencePolicy::none())?;
    let landing = Landing::of(base.store_id().map_err(e)?, rec, head);
    let s = Store::open(Interleave::new(base.into_backend(), landing)).map_err(e)?;
    let ev = transfer(&s, &m, "a1", "a3", "5.00")?;
    ensure(
        s.current().map_err(e)?.position == 1,
        "the concurrent commit landed during the reads",
    )?;
    let b = bundle(ev)?;
    ensure(
        b.evaluated_state.position == 0,
        "the evaluation is at the state whose head it read",
    )?;
    ensure(
        b.record["state"]["from_"]["balance"] == json!("100.00"),
        format!(
            "reads come from that state, not a newer one: {}",
            b.record["state"]["from_"]
        ),
    )
}

fn store_binding<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (a, m) = create(f(), EvidencePolicy::none())?;
    let strict = EvidencePolicy {
        require: Require::None,
        trusted_execution_policies: Some(vec![]),
        ..EvidencePolicy::none()
    };
    let (mut b, _) = create(f(), strict)?;
    ensure(
        a.current().map_err(e)?.state == b.current().map_err(e)?.state,
        "identical content, identical state",
    )?;
    let foreign = bundle(transfer(&a, &m, "a1", "a2", "1.00")?)?;
    match b.commit(&m, &foreign.evaluated_state.clone(), &foreign) {
        Err(err) if err.code() == "BUNDLE_INVALID" || err.code() == "EVIDENCE_MISMATCH" => Ok(()),
        other => Err(format!(
            "a bundle from another store must be refused: {other:?}"
        )),
    }
}

fn derived_dependencies<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let good = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    let refused =
        |s: &mut Store<B>, b: &CommitBundle| s.commit(&m, &b.evaluated_state.clone(), b).is_err();
    let mut b = good.clone();
    b.read_set.pop();
    ensure(refused(&mut s, &b), "an altered read set is refused")?;
    let mut b = good.clone();
    b.write_set[0].new = json!("0.00");
    ensure(refused(&mut s, &b), "an altered write set is refused")?;
    let mut b = good.clone();
    b.write_set[0].id = "zz".into();
    ensure(
        refused(&mut s, &b),
        "a write outside the entity universe is refused",
    )?;
    ensure(
        !commit(&mut s, &m, &good)?.already,
        "the unaltered bundle commits",
    )
}

fn load_at_past_state<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    let c1 = {
        let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
        commit(&mut s, &m, &b)?
    };
    {
        let b = bundle(transfer(&s, &m, "a1", "a3", "10.00")?)?;
        commit(&mut s, &m, &b)?
    };
    ensure(balance(&s, "a1", &s0)? == json!("100.00"), "a1 at S0")?;
    ensure(
        balance(&s, "a1", &c1.result_state)? == json!("90.00"),
        "a1 at S1",
    )?;
    ensure(
        balance(&s, "a2", &c1.result_state)? == json!("15.00"),
        "a2 at S1",
    )
}

fn history_between_states<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    for (a, b) in [("a1", "a2"), ("a3", "a1"), ("a2", "a3")] {
        {
            let b = bundle(transfer(&s, &m, a, b, "1.00")?)?;
            commit(&mut s, &m, &b)?
        };
    }
    let s3 = s.current().map_err(e)?;
    let recs = s.transitions(&s0, &s3).map_err(e)?;
    ensure(
        recs.iter().map(|r| r.position).collect::<Vec<_>>() == vec![1, 2, 3],
        "records in order",
    )?;
    let d = replay_data(&s, &s0, &s3);
    ensure(d.ok, format!("the chain verifies: {:?}", d.divergence))
}

fn idempotent_resubmission<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let t1 = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    let c1 = commit(&mut s, &m, &t1)?;
    let mut again = t1.clone();
    again.commit_time = "2026-09-27T12:05:00Z".into();
    ensure(
        commit(&mut s, &m, &again)?.already,
        "a resubmission with a new commit time is already committed",
    )?;
    {
        let b = bundle(transfer(&s, &m, "a3", "a2", "1.00")?)?;
        commit(&mut s, &m, &b)?
    };
    let late = commit(&mut s, &m, &t1)?;
    ensure(
        late.already && late.result_state == c1.result_state,
        "a late retry is recognized",
    )?;
    ensure(s.current().map_err(e)?.position == 2, "no record was added")
}

fn no_op_transition<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    let c = {
        let b = bundle(unary(&s, &m, "touch", "a1")?)?;
        commit(&mut s, &m, &b)?
    };
    ensure(
        c.result_state.state == s0.state && c.result_state.position == 1,
        "a no-op keeps the identity and advances",
    )
}

fn identity_is_content<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    let there = {
        let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
        commit(&mut s, &m, &b)?
    };
    let back = {
        let b = bundle(transfer(&s, &m, "a2", "a1", "10.00")?)?;
        commit(&mut s, &m, &b)?
    };
    ensure(
        there.result_state.state != s0.state,
        "changed content changes the identity",
    )?;
    ensure(
        back.result_state.state == s0.state,
        "returning content repeats the identity",
    )?;
    ensure(
        s.load(&key("a1"), &back.result_state).map_err(e)?.revision == 3,
        "revisions are history, not content",
    )?;
    let (fresh, _) = create(f(), EvidencePolicy::none())?;
    ensure(
        fresh.current().map_err(e)?.state == back.result_state.state,
        "another store with that content agrees",
    )
}

fn evidence_policy<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let strict = EvidencePolicy {
        require: Require::CommitAuthorization,
        ..EvidencePolicy::none()
    };
    let (mut s, m) = create(f(), strict.clone())?;
    let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    match s.commit(&m, &b.evaluated_state.clone(), &b) {
        Err(err) if err.code() == "EVIDENCE_REQUIRED" => {}
        other => {
            return Err(format!(
                "a commit without authorization must be refused: {other:?}"
            ));
        }
    }
    let (mut open, m) = create(f(), EvidencePolicy::none())?;
    let ob = bundle(transfer(&open, &m, "a1", "a2", "10.00")?)?;
    commit(&mut open, &m, &ob)?;
    let r = open
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record 1 missing")?;
    ensure(
        r.evidence_policy == EvidencePolicy::none().hash().map_err(e)? && r.authorization.is_none(),
        "the record cites the evidence policy",
    )
}

/// A well-formed commit authorization for `b` (synthetic: evidence checking is structural, so no
/// verifier run is needed to exercise how a backend stores it).
fn synthetic_authorization(b: &CommitBundle) -> Result<Json, String> {
    let mut auth = json!({
        "authorization_version": "1",
        "behavior_version": b.behavior_version,
        "transition_hash": b.transition_hash,
        "policy_hash": format!("sha256:{}", "0".repeat(64)),
        "verification": null,
        "waivers_used": [],
        "now": T0,
        "decision": "allow",
        "reasons": [],
    });
    let h =
        behavior_verify::hashing::document_hash(behavior_verify::hashing::TAG_AUTHORIZATION, &auth)
            .map_err(|x| x.to_string())?;
    auth["hash"] = json!(h);
    Ok(auth)
}

/// The storage side of evidence: the authorization reference is committed atomically with the
/// record, the versions and the head, and survives a crash and retry (the engine's acceptance of
/// evidence is tested separately, with real attestations).
fn evidence_atomicity<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let strict = EvidencePolicy {
        require: Require::CommitAuthorization,
        ..EvidencePolicy::none()
    };
    let (mut s, m) = create(FaultInjector::new(f()), strict)?;
    let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    let auth = synthetic_authorization(&b)?;
    let auth_hash = auth["hash"].as_str().unwrap_or_default().to_string();
    let bound = b.with_evidence(crate::documents::Evidence {
        authorization: auth,
        execution_policy: None,
        attestation: None,
        waivers: vec![],
    });
    s.backend_mut().next = Some(Fault::AfterWrite);
    ensure(
        s.commit(&m, &bound.evaluated_state.clone(), &bound)
            .is_err(),
        "the lost acknowledgement is reported",
    )?;
    let head = s.backend().head().map_err(|x| x.0)?.ok_or("head missing")?;
    let r = s
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("the record was not written with the head")?;
    ensure(
        r.authorization.as_deref() == Some(auth_hash.as_str()),
        "the record cites the authorization",
    )?;
    ensure(
        r.bundle.as_ref().is_some_and(|b| b.evidence.is_some()),
        "the record keeps the evidence",
    )?;
    ensure(
        head.last_record == r.hash().map_err(e)?,
        "the head points at that record",
    )?;
    for v in &r.new_versions {
        ensure(
            s.backend()
                .version(&v.key(), v.revision)
                .map_err(|x| x.0)?
                .as_ref()
                == Some(v),
            format!(
                "{} revision {} was written with the record",
                v.key(),
                v.revision
            ),
        )?;
    }
    let retry = commit(&mut s, &m, &bound)?;
    ensure(
        retry.already && retry.evidence_trust == Some("structural"),
        "the retry returns the bound commit",
    )
}

fn replay_detects_tamper<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create(f(), EvidencePolicy::none())?;
    let s0 = s.current().map_err(e)?;
    for (a, b) in [("a1", "a2"), ("a3", "a1"), ("a2", "a3")] {
        {
            let b = bundle(transfer(&s, &m, a, b, "1.00")?)?;
            commit(&mut s, &m, &b)?
        };
    }
    let s3 = s.current().map_err(e)?;
    let modules = BTreeMap::from([(m.behavior_version(), m)]);
    ensure(
        replay_data(&s, &s0, &s3).ok && replay_behavior(&s, &modules, &s0, &s3).ok,
        "a clean history replays",
    )?;
    let t = Store::open(AlterRecord {
        inner: s.into_backend(),
        position: 2,
    })
    .map_err(e)?;
    let d = replay_data(&t, &s0, &s3);
    match d.divergence {
        Some(div) if div.position == 2 => Ok(()),
        other => Err(format!(
            "a tampered record must be found at position 2: {other:?}"
        )),
    }
}

fn observed_read_set<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (s, m) = create(f(), EvidencePolicy::none())?;
    let b = bundle(transfer(&s, &m, "a1", "a2", "10.00")?)?;
    let fields = |id: &str| {
        b.read_set
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.fields.clone())
    };
    ensure(
        fields("a1") == Some(vec!["active".into(), "balance".into()]),
        format!("from_ reads: {:?}", fields("a1")),
    )?;
    ensure(
        fields("a2") == Some(vec!["balance".into()]),
        format!("to reads only balance: {:?}", fields("a2")),
    )
}

/// A backend that alters the result state of one record.
struct AlterRecord<B> {
    inner: B,
    position: u64,
}

impl<B: Backend> Backend for AlterRecord<B> {
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
        refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed, refs)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        self.inner.keys_at(t, p)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        self.inner.keys_by_field_at(t, f, v, p)
    }
    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(key)
    }
    fn incoming_at(&self, t: &EntityKey, pos: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(t, pos)
    }
    fn used_at(&self, key: &EntityKey, pos: u64) -> Result<bool, BackendError> {
        self.inner.used_at(key, pos)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        let mut r = self.inner.record(p)?;
        if p == self.position
            && let Some(r) = r.as_mut()
        {
            r.result_state.state = "sha256:00".into();
        }
        Ok(r)
    }
    fn commit(
        &mut self,
        x: &str,
        v: &[EntityVersion],
        rm: &[EntityKey],
        refs: &[RefChange],
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.inner.commit(x, v, rm, refs, r, h)
    }
}

// --- feature 006: lifecycle cases (the accounts module) ------------------------------------------

/// The lifecycle module the feature-006 cases run against (tests/fixtures/wire/valid/accounts.json).
pub const ACCOUNTS_WIRE: &str = include_str!("../../../tests/fixtures/wire/valid/accounts.json");

fn accounts() -> Result<Module, String> {
    behavior_core::admit(ACCOUNTS_WIRE).map_err(|r| format!("{:?}", r.errors))
}

fn accounts_seed() -> Vec<SeedEntity> {
    let c = |id: &str| SeedEntity {
        entity: "Customer".into(),
        value: json!({"id": id, "name": id}),
    };
    vec![
        c("c1"),
        c("c2"),
        c("c3"),
        SeedEntity {
            entity: "Account".into(),
            value: json!({"id": "a1", "owner": "c1", "balance": "0.00"}),
        },
        SeedEntity {
            entity: "AuditNote".into(),
            value: json!({"id": "n3", "about": "c3", "text": "t"}),
        },
    ]
}

fn create_accounts<B: Backend>(backend: B) -> Result<(Store<B>, Module), String> {
    let m = accounts()?;
    let s = Store::create(
        backend,
        &m,
        genesis_for(&m, EvidencePolicy::none(), accounts_seed()),
    )
    .map_err(e)?;
    Ok((s, m))
}

fn run_action<B: Backend>(
    s: &Store<B>,
    m: &Module,
    action: &str,
    bindings: &[(&str, &str)],
    input: Json,
) -> Result<Evaluation, String> {
    s.evaluate(m, action, &bind(bindings), &input, &json!({}), T0, None)
        .map_err(e)
}

fn open_account<B: Backend>(
    s: &Store<B>,
    m: &Module,
    id: &str,
    owner: &str,
) -> Result<Evaluation, String> {
    run_action(
        s,
        m,
        "open_account",
        &[("owner", owner)],
        json!({"account_id": id, "initial": "0.00"}),
    )
}

fn apply<B: Backend>(s: &mut Store<B>, m: &Module, ev: Evaluation) -> Result<StateRef, String> {
    let b = bundle(ev)?;
    Ok(commit(s, m, &b)?.result_state)
}

fn akey(entity: &str, id: &str) -> EntityKey {
    EntityKey {
        entity: entity.into(),
        id: id.into(),
    }
}

/// FR-014: the universe changes only through `commit`. Every entity key the backend reports is
/// justified by the seed or a recorded creation, every absence by a recorded removal.
fn create_and_remove_entity<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    let s0 = s.current().map_err(e)?;
    let s1 = {
        let ev = open_account(&s, &m, "n1", "c2")?;
        apply(&mut s, &m, ev)?
    };
    let created = s.load(&akey("Account", "n1"), &s1).map_err(e)?;
    ensure(
        created.revision == 1 && created.created_at == 1,
        "a creation is revision 1 at its position",
    )?;
    ensure(
        s.load(&akey("Account", "n1"), &s0).is_err(),
        "absent before its creation",
    )?;
    let s2 = {
        let ev = run_action(&s, &m, "close_account", &[("account", "n1")], json!({}))?;
        apply(&mut s, &m, ev)?
    };
    ensure(
        s.load(&akey("Account", "n1"), &s2).is_err(),
        "absent after its removal",
    )?;
    ensure(
        s.load(&akey("Account", "n1"), &s1).is_ok(),
        "present in between",
    )?;
    // Every key the history names, at every position: existence as the records say.
    let mut keys: Vec<(EntityKey, Option<u64>, Option<u64>)> = accounts_seed()
        .iter()
        .map(|x| {
            (
                akey(&x.entity, x.value["id"].as_str().unwrap_or_default()),
                Some(0),
                None,
            )
        })
        .collect();
    keys.push((akey("Account", "zz"), None, None));
    for pos in 1..=s2.position {
        let r = s
            .backend()
            .record(pos)
            .map_err(|x| x.0)?
            .ok_or("record missing")?;
        for v in &r.created {
            keys.push((v.key(), Some(pos), None));
        }
        for x in &r.removed {
            for k in keys
                .iter_mut()
                .filter(|k| k.0.entity == x.entity && k.0.id == x.id)
            {
                k.2 = Some(pos);
            }
        }
    }
    for (k, born, died) in &keys {
        let removed_at = s.backend().removed_at(k).map_err(|x| x.0)?;
        ensure(
            removed_at == *died,
            format!("{k}: removed_at {removed_at:?}, records say {died:?}"),
        )?;
        for pos in 0..=s2.position {
            let expect = born.is_some_and(|b| b <= pos) && died.is_none_or(|d| d > pos);
            let got = crate::store::exists_at(s.backend(), k, pos).map_err(|x| x.0)?;
            ensure(
                got == expect,
                format!("{k} at {pos}: exists {got}, history says {expect}"),
            )?;
        }
    }
    Ok(())
}

fn identity_never_reused<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    {
        let ev = open_account(&s, &m, "n1", "c2")?;
        apply(&mut s, &m, ev)?
    };
    {
        let ev = run_action(&s, &m, "close_account", &[("account", "n1")], json!({}))?;
        apply(&mut s, &m, ev)?
    };
    let at = s.current().map_err(e)?.position;
    ensure(
        s.backend()
            .used_at(&akey("Account", "n1"), at)
            .map_err(|x| x.0)?,
        "a removed identity stays used",
    )?;
    let again = open_account(&s, &m, "n1", "c2")?;
    ensure(
        again.record["result"] == "ENTITY_ID_ALREADY_USED",
        format!("re-creating a removed identity: {}", again.record["result"]),
    )
}

fn removed_entity_history<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    let s0 = s.current().map_err(e)?;
    let before = s.load(&akey("Account", "a1"), &s0).map_err(e)?;
    let s1 = {
        let ev = run_action(&s, &m, "close_account", &[("account", "a1")], json!({}))?;
        apply(&mut s, &m, ev)?
    };
    ensure(
        s.load(&akey("Account", "a1"), &s0).map_err(e)? == before,
        "the removed entity's last version is still loadable at earlier states",
    )?;
    ensure(
        s.backend()
            .version(&akey("Account", "a1"), 1)
            .map_err(|x| x.0)?
            == Some(before),
        "removal keeps the versions",
    )?;
    let d = replay_data(&s, &s0, &s1);
    ensure(d.ok, format!("data replay: {:?}", d.divergence))?;
    let mods = BTreeMap::from([(m.behavior_version(), m.clone())]);
    let b = replay_behavior(&s, &mods, &s0, &s1);
    ensure(b.ok, format!("behavior replay: {:?}", b.divergence))
}

fn referential_integrity_on_resulting_state<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    let refused = run_action(
        &s,
        &m,
        "remove_customer_unchecked",
        &[("customer", "c1")],
        json!({}),
    )?;
    ensure(
        refused.record["reasons"][0]["code"] == "DANGLING_REFERENCE",
        format!(
            "removing a referenced customer: {}",
            refused.record["result"]
        ),
    )?;
    let ev = run_action(
        &s,
        &m,
        "switch_and_remove",
        &[("account", "a1"), ("old", "c1"), ("new", "c2")],
        json!({}),
    )?;
    let s1 = apply(&mut s, &m, ev)?;
    ensure(
        s.backend()
            .removed_at(&akey("Customer", "c1"))
            .map_err(|x| x.0)?
            == Some(s1.position),
        "retarget and remove commit together",
    )
}

fn reference_index_consistency<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    let s0 = s.current().map_err(e)?;
    {
        let ev = open_account(&s, &m, "n1", "c2")?;
        apply(&mut s, &m, ev)?
    };
    {
        let ev = open_account(&s, &m, "n2", "c3")?;
        apply(&mut s, &m, ev)?
    };
    let ev = run_action(
        &s,
        &m,
        "switch_and_remove",
        &[("account", "a1"), ("old", "c1"), ("new", "c3")],
        json!({}),
    )?;
    apply(&mut s, &m, ev)?;
    {
        let ev = run_action(&s, &m, "close_account", &[("account", "n1")], json!({}))?;
        apply(&mut s, &m, ev)?
    };
    {
        let ev = run_action(&s, &m, "remove_customer", &[("customer", "c2")], json!({}))?;
        apply(&mut s, &m, ev)?
    };
    let end = s.current().map_err(e)?;
    let r = crate::replay::replay_index(&s, &m, &s0, &end);
    ensure(
        r.ok,
        format!(
            "the index diverges from the entity content: {:?}",
            r.divergence
        ),
    )
}

fn concurrent_creation_same_identity<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_accounts(f())?;
    let first = bundle(open_account(&s, &m, "n1", "c1")?)?;
    let second = bundle(open_account(&s, &m, "n1", "c2")?)?;
    commit(&mut s, &m, &first)?;
    match s.commit(&m, &second.evaluated_state.clone(), &second) {
        Err(StoreError::StateConflict { .. }) => {}
        other => {
            return Err(format!(
                "the second creation of n1 must conflict: {other:?}"
            ));
        }
    }
    let again = open_account(&s, &m, "n1", "c2")?;
    ensure(
        again.record["result"] == "ENTITY_ID_ALREADY_USED",
        "re-evaluation sees the creation",
    )?;
    // A creation landing between the store's checks and the compare-and-set.
    let (mut other, _) = create_accounts(f())?;
    {
        let ev = open_account(&other, &m, "n7", "c1")?;
        apply(&mut other, &m, ev)?
    };
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let (base, _) = create_accounts(f())?;
    let landing = Landing::of(base.store_id().map_err(e)?, rec, head);
    let mut raced =
        Store::open(Interleave::at(base.into_backend(), landing, LandOn::Commit)).map_err(e)?;
    let mine = bundle(open_account(&raced, &m, "n7", "c2")?)?;
    match raced.commit(&m, &mine.evaluated_state.clone(), &mine) {
        Err(StoreError::StateConflict { .. }) => Ok(()),
        other => Err(format!("a racing creation must conflict: {other:?}")),
    }
}

/// Evaluation facts come from the evaluated position even while commits land (FR-018).
fn existence_snapshot<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let m = accounts()?;
    // Prepared elsewhere: an account referencing c3 (adds an index edge at position 1).
    let (mut other, _) = create_accounts(f())?;
    {
        let ev = open_account(&other, &m, "n9", "c3")?;
        apply(&mut other, &m, ev)?
    };
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let (base, _) = create_accounts(f())?;
    let landing = Landing::of(base.store_id().map_err(e)?, rec, head);
    let s = Store::open(Interleave::new(base.into_backend(), landing)).map_err(e)?;
    // The landing happens at the first entity read, after the head was read at position 0.
    let ev = run_action(&s, &m, "remove_customer", &[("customer", "c3")], json!({}))?;
    ensure(
        ev.record["result"] == "ALLOW",
        format!(
            "`referenced(c3)` must be answered at position 0: {}",
            ev.record["result"]
        ),
    )?;
    ensure(
        ev.record["facts"]["references"][0]["incoming"] == json!([]),
        "the observed incoming references are those of position 0",
    )
}

// --- feature 007: query cases (the orders module) -------------------------------------------------

pub const ORDERS_WIRE: &str = include_str!("../../../tests/fixtures/wire/valid/orders.json");

fn orders() -> Result<Module, String> {
    behavior_core::admit(ORDERS_WIRE).map_err(|r| format!("{:?}", r.errors))
}

fn orders_seed() -> Vec<SeedEntity> {
    let s = |entity: &str, value: Json| SeedEntity {
        entity: entity.into(),
        value,
    };
    vec![
        s(
            "Customer",
            json!({"id": "c1", "name": "c1", "credit_limit": "100.00", "region": "north"}),
        ),
        s(
            "Customer",
            json!({"id": "c2", "name": "c2", "credit_limit": "1000.00", "region": "south"}),
        ),
        s(
            "Order",
            json!({"id": "ob", "customer": "c2", "amount": "150.00", "status": "open", "region": "south"}),
        ),
        s("Employee", json!({"id": "e1", "personnel_number": "N1"})),
    ]
}

fn create_orders<B: Backend>(backend: B) -> Result<(Store<B>, Module), String> {
    let m = orders()?;
    let s = Store::create(
        backend,
        &m,
        genesis_for(&m, EvidencePolicy::none(), orders_seed()),
    )
    .map_err(e)?;
    Ok((s, m))
}

/// A backend whose type and field indexes list keys in reverse: backend order is never semantic.
struct ReversedKeys<B> {
    inner: B,
}

impl<B: Backend> Backend for ReversedKeys<B> {
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
        refs: &[RefChange],
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed, refs)
    }
    fn keys_at(&self, t: &str, p: u64) -> Result<Vec<EntityKey>, BackendError> {
        let mut keys = self.inner.keys_at(t, p)?;
        keys.reverse();
        Ok(keys)
    }
    fn keys_by_field_at(
        &self,
        t: &str,
        f: &str,
        v: &serde_json::Value,
        p: u64,
    ) -> Result<Option<Vec<EntityKey>>, BackendError> {
        Ok(self.inner.keys_by_field_at(t, f, v, p)?.map(|mut keys| {
            keys.reverse();
            keys
        }))
    }
    fn removed_at(&self, key: &EntityKey) -> Result<Option<u64>, BackendError> {
        self.inner.removed_at(key)
    }
    fn incoming_at(&self, t: &EntityKey, pos: u64) -> Result<Vec<RefEdge>, BackendError> {
        self.inner.incoming_at(t, pos)
    }
    fn used_at(&self, key: &EntityKey, pos: u64) -> Result<bool, BackendError> {
        self.inner.used_at(key, pos)
    }
    fn version_at(&self, k: &EntityKey, p: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version_at(k, p)
    }
    fn version(&self, k: &EntityKey, r: u64) -> Result<Option<EntityVersion>, BackendError> {
        self.inner.version(k, r)
    }
    fn record(&self, p: u64) -> Result<Option<TransitionRecord>, BackendError> {
        self.inner.record(p)
    }
    fn commit(
        &mut self,
        x: &str,
        v: &[EntityVersion],
        rm: &[EntityKey],
        refs: &[RefChange],
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.inner.commit(x, v, rm, refs, r, h)
    }
}

/// An action, its bindings and its input.
type Step = (&'static str, Vec<(&'static str, &'static str)>, Json);

/// A small history touching every query-relevant change: creations, removals, and field changes
/// of queried types. Returns the records of every evaluation, committed or not.
fn orders_history<B: Backend>(s: &mut Store<B>, m: &Module) -> Result<Vec<Json>, String> {
    let steps: [Step; 9] = [
        (
            "place_order",
            vec![("customer", "c1")],
            json!({"order_id": "o1", "amount": "20.00"}),
        ),
        (
            "place_order",
            vec![("customer", "c1")],
            json!({"order_id": "o2", "amount": "20.00"}),
        ),
        (
            "place_order",
            vec![("customer", "c1")],
            json!({"order_id": "o3", "amount": "30.00"}),
        ),
        ("check_orders", vec![("customer", "c1")], json!({})),
        (
            "remove_cheapest",
            vec![("order", "o1"), ("customer", "c1")],
            json!({}),
        ),
        (
            "raise_limit",
            vec![("customer", "c1")],
            json!({"limit": "200.00"}),
        ),
        ("hire", vec![], json!({"employee_id": "e2", "number": "N2"})),
        (
            "renumber",
            vec![("employee", "e1")],
            json!({"number": "N3"}),
        ),
        ("check_orders", vec![("customer", "c2")], json!({})),
    ];
    let mut records = Vec::new();
    for (action, bindings, input) in steps {
        let ev = run_action(s, m, action, &bindings, input)?;
        records.push(ev.record.clone());
        if ev.bundle.is_some() {
            apply(s, m, ev)?;
        }
    }
    Ok(records)
}

/// Query results come from the evaluated position even while commits land (FR-012).
fn query_snapshot<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let m = orders()?;
    // Prepared elsewhere: the over-limit order `ob` is removed at position 1.
    let (mut other, _) = create_orders(f())?;
    {
        let ev = run_action(
            &other,
            &m,
            "remove_cheapest",
            &[("order", "ob"), ("customer", "c2")],
            json!({}),
        )?;
        apply(&mut other, &m, ev)?
    };
    let rec = other
        .backend()
        .record(1)
        .map_err(|x| x.0)?
        .ok_or("record missing")?;
    let head = other
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("head missing")?;
    let (base, _) = create_orders(f())?;
    let landing = Landing::of(base.store_id().map_err(e)?, rec, head);
    let s = Store::open(Interleave::new(base.into_backend(), landing)).map_err(e)?;
    // `count(orders over c1's limit) == 0` must see `ob` (it exists at position 0).
    let ev = run_action(
        &s,
        &m,
        "raise_limit",
        &[("customer", "c1")],
        json!({"limit": "120.00"}),
    )?;
    ensure(
        ev.record["result"] == "DENY",
        format!(
            "the query must be answered at position 0: {}",
            ev.record["result"]
        ),
    )?;
    let members = ev.record["facts"]["queries"][0]["members"].clone();
    ensure(
        members == json!([{"id": "ob"}]),
        format!("the observed members are those of position 0: {members}"),
    )
}

/// The type and field indexes equal a scan of the history at every position (FR-016).
fn query_index_consistency<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut s, m) = create_orders(f())?;
    orders_history(&mut s, &m)?;
    let end = s.current().map_err(e)?.position;
    // Every (type, field, value) that occurs anywhere in the history.
    let mut probes: std::collections::BTreeSet<(String, String, String)> = Default::default();
    for pos in 0..=end {
        let (content, _) = crate::replay::content_at(&s, pos).map_err(e)?;
        for v in content.values() {
            if let Some(o) = v.value.as_object() {
                for (field, value) in o {
                    probes.insert((v.entity.clone(), field.clone(), value.to_string()));
                }
            }
        }
    }
    for pos in 0..=end {
        let (content, _) = crate::replay::content_at(&s, pos).map_err(e)?;
        for t in ["Customer", "Order", "Employee"] {
            let scan: Vec<EntityKey> = content.keys().filter(|k| k.entity == t).cloned().collect();
            let mut keys = s.backend().keys_at(t, pos).map_err(|x| x.0)?;
            keys.sort();
            ensure(
                keys == scan,
                format!("keys_at({t}, {pos}) = {keys:?}, a scan gives {scan:?}"),
            )?;
        }
        for (t, field, value) in &probes {
            let v: Json = serde_json::from_str(value).map_err(|x| x.to_string())?;
            let Some(mut keys) = s
                .backend()
                .keys_by_field_at(t, field, &v, pos)
                .map_err(|x| x.0)?
            else {
                continue;
            };
            let scan: Vec<EntityKey> = content
                .values()
                .filter(|x| x.entity == *t && x.value.get(field) == Some(&v))
                .map(EntityVersion::key)
                .collect();
            keys.sort();
            ensure(
                keys == scan,
                format!(
                    "keys_by_field_at({t}.{field} = {value}, {pos}) = {keys:?}, a scan gives {scan:?}"
                ),
            )?;
        }
    }
    Ok(())
}

/// Results and records do not depend on the backend's iteration order (FR-020a).
fn query_order_independence<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (mut a, m) = create_orders(f())?;
    let (mut b, _) = create_orders(ReversedKeys { inner: f() })?;
    let ra = orders_history(&mut a, &m)?;
    let rb = orders_history(&mut b, &m)?;
    for (x, y) in ra.iter().zip(&rb) {
        ensure(
            x == y,
            format!("a record depends on the key order:\n{x}\n{y}"),
        )?;
    }
    ensure(
        a.current().map_err(e)? == b.current().map_err(e)?,
        "the resulting states differ",
    )
}

/// A transition breaking a module invariant is refused, and so is a genesis (FR-024).
fn module_invariant_preserved<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (s, m) = create_orders(f())?;
    let ev = run_action(
        &s,
        &m,
        "hire_unchecked",
        &[],
        json!({"employee_id": "e2", "number": "N1"}),
    )?;
    ensure(
        ev.record["result"] == "DENY"
            && ev.record["reasons"][0]["code"] == "INVARIANT_VIOLATED"
            && ev.bundle.is_none(),
        format!(
            "a duplicate personnel number must be refused: {}",
            ev.record
        ),
    )?;
    let mut seed = orders_seed();
    seed.push(SeedEntity {
        entity: "Employee".into(),
        value: json!({"id": "e9", "personnel_number": "N1"}),
    });
    match Store::create(f(), &m, genesis_for(&m, EvidencePolicy::none(), seed)) {
        Err(err) if err.code() == "GENESIS_INVALID" => Ok(()),
        Err(err) => Err(format!("a genesis breaking a module invariant: {err}")),
        Ok(_) => Err("a genesis breaking a module invariant must be refused".into()),
    }
}

// --- feature 009: schema evolution -------------------------------------------------------------

pub const CULTURES_V1_WIRE: &str =
    include_str!("../../../tests/fixtures/migration/modules/cultures_v1.json");
pub const CULTURES_V2_WIRE: &str =
    include_str!("../../../tests/fixtures/migration/modules/cultures_v2.json");
pub const CULTURES_V1_TO_V2: &str =
    include_str!("../../../tests/fixtures/migration/valid/cultures_v1_to_v2.json");

/// The cultures modules V1 and V2 and the migration between them.
fn cultures() -> Result<(Module, Module, behavior_core::migration::Migration), String> {
    let v1 = behavior_core::admit(CULTURES_V1_WIRE).map_err(|r| format!("{:?}", r.errors))?;
    let v2 = behavior_core::admit(CULTURES_V2_WIRE).map_err(|r| format!("{:?}", r.errors))?;
    let m = behavior_core::migration::admit_migration(&v1, &v2, CULTURES_V1_TO_V2)
        .map_err(|r| format!("{:?}", r.errors))?;
    Ok((v1, v2, m))
}

fn cultures_seed(note: bool) -> Vec<SeedEntity> {
    let mut seed = vec![
        SeedEntity {
            entity: "Customer".into(),
            value: json!({"id": "c1", "name": "Ada", "email": "ada@x"}),
        },
        SeedEntity {
            entity: "Culture".into(),
            value: json!({"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                          "legacy_code": "L1", "price": "1.50", "fee": "0.1235"}),
        },
        SeedEntity {
            entity: "Order".into(),
            value: json!({"id": "o1", "customer": "c1", "region": null, "qty": 3}),
        },
    ];
    if note {
        seed.push(SeedEntity {
            entity: "AuditNote".into(),
            value: json!({"id": "n1", "text": "keep"}),
        });
    }
    seed
}

fn cultures_store<B: Backend>(backend: B, note: bool) -> Result<Store<B>, String> {
    let (v1, _, _) = cultures()?;
    Store::create(
        backend,
        &v1,
        genesis_for(&v1, EvidencePolicy::none(), cultures_seed(note)),
    )
    .map_err(e)
}

/// A migration is all-or-nothing (FR-011): a refused one writes nothing; a crash before the
/// write leaves the store under its old schema, a crash after it leaves the store fully migrated.
fn migration_atomicity<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (v1, v2, m) = cultures()?;
    const T: &str = "2026-10-02T10:00:00Z";
    // A refusal (a retired type still holds an entity) writes nothing.
    let mut s = cultures_store(f(), true)?;
    let before = s.backend().head().map_err(|x| x.0)?;
    match s.migrate(&m, &v1, &v2, T, None) {
        Err(err) if err.code() == "RETIRED_TYPE_NOT_EMPTY" => {}
        other => return Err(format!("expected RETIRED_TYPE_NOT_EMPTY, got {other:?}")),
    }
    ensure(
        s.backend().head().map_err(|x| x.0)? == before,
        "a refused migration leaves the head as it was",
    )?;
    ensure(
        s.backend().record(1).map_err(|x| x.0)?.is_none(),
        "a refused migration writes no record",
    )?;
    // A crash before the write: nothing changed.
    let mut s = cultures_store(FaultInjector::new(f()), false)?;
    s.backend_mut().next = Some(Fault::BeforeWrite);
    ensure(
        s.migrate(&m, &v1, &v2, T, None).is_err(),
        "the injected fault is reported",
    )?;
    let s = Store::open(s.into_backend()).map_err(e)?;
    ensure(
        s.current().map_err(e)?.position == 0 && s.schema_history().map_err(e)?.len() == 1,
        "after a crash before the write the store is unmigrated",
    )?;
    // A crash after the write: everything is there.
    let mut s = cultures_store(FaultInjector::new(f()), false)?;
    s.backend_mut().next = Some(Fault::AfterWrite);
    ensure(
        s.migrate(&m, &v1, &v2, T, None).is_err(),
        "the injected fault is reported",
    )?;
    let s = Store::open(s.into_backend()).map_err(e)?;
    let at = s.current().map_err(e)?;
    ensure(
        at.position == 1,
        "after a crash after the write the store is migrated",
    )?;
    for (entity, id) in [("Customer", "c1"), ("Culture", "k1"), ("Order", "o1")] {
        let v = s.load(&akey(entity, id), &at).map_err(e)?;
        ensure(
            v.created_at == 1,
            format!("{entity}#{id} has its migrated version"),
        )?;
    }
    s.backend()
        .record(1)
        .map_err(|x| x.0)?
        .filter(|r| r.is_migration())
        .ok_or("the migration record was written with the versions")?;
    Ok(())
}

/// The schema at every position is recoverable (FR-002): after a migration and a reopen, the head
/// names the new schema, the history walks back to the genesis schema, and the store binds
/// behavior to the schema of the position it evaluates.
fn schema_history_consistency<B: Backend>(f: &dyn Fn() -> B) -> Result<(), String> {
    let (v1, v2, m) = cultures()?;
    let mut s = cultures_store(f(), false)?;
    let c = s
        .migrate(&m, &v1, &v2, "2026-10-02T10:00:00Z", None)
        .map_err(e)?;
    let s = Store::open(s.into_backend()).map_err(e)?;
    let head = s
        .backend()
        .head()
        .map_err(|x| x.0)?
        .ok_or("the store has no head")?;
    let named = head
        .schema
        .ok_or("the head of a migrated store names its schema")?;
    ensure(
        named.hash == behavior_core::schema(&v2).hash
            && named.since == 1
            && named.migration_record == c.record_id,
        "the head names the migrated schema and the record that introduced it",
    )?;
    let history: Vec<(u64, String)> = s
        .schema_history()
        .map_err(e)?
        .into_iter()
        .map(|h| (h.since, h.hash))
        .collect();
    ensure(
        history
            == vec![
                (0, behavior_core::schema(&v1).hash),
                (1, behavior_core::schema(&v2).hash),
            ],
        format!("the schema history is the genesis and the migration: {history:?}"),
    )?;
    let ev = s
        .evaluate(
            &v1,
            "kill",
            &bind(&[("culture", "k1")]),
            &json!({}),
            &json!({}),
            "2026-10-02T11:00:00Z",
            None,
        )
        .err()
        .map(|err| err.code());
    ensure(
        ev == Some("SCHEMA_MISMATCH"),
        "behavior of the old schema is refused after the migration",
    )
}

/// Runs every named case against backends from `factory`.
pub fn run<B: Backend>(factory: impl Fn() -> B) -> ConformanceReport {
    let cases: [(&'static str, Case<B>); 30] = [
        ("create_and_open", create_and_open),
        ("commit_new_state", commit_new_state),
        ("conflict_on_outdated_parent", conflict_on_outdated_parent),
        ("no_partial_application", no_partial_application),
        ("crash_retry", crash_retry),
        ("snapshot_consistency", snapshot_consistency),
        ("store_binding", store_binding),
        ("derived_dependencies", derived_dependencies),
        ("load_at_past_state", load_at_past_state),
        ("history_between_states", history_between_states),
        ("idempotent_resubmission", idempotent_resubmission),
        ("no_op_transition", no_op_transition),
        ("identity_is_content", identity_is_content),
        ("evidence_policy", evidence_policy),
        ("evidence_atomicity", evidence_atomicity),
        ("replay_detects_tamper", replay_detects_tamper),
        ("observed_read_set", observed_read_set),
        ("create_and_remove_entity", create_and_remove_entity),
        ("identity_never_reused", identity_never_reused),
        ("removed_entity_history", removed_entity_history),
        (
            "referential_integrity_on_resulting_state",
            referential_integrity_on_resulting_state,
        ),
        ("reference_index_consistency", reference_index_consistency),
        (
            "concurrent_creation_same_identity",
            concurrent_creation_same_identity,
        ),
        ("existence_snapshot", existence_snapshot),
        ("query_snapshot", query_snapshot),
        ("query_index_consistency", query_index_consistency),
        ("query_order_independence", query_order_independence),
        ("module_invariant_preserved", module_invariant_preserved),
        ("migration_atomicity", migration_atomicity),
        ("schema_history_consistency", schema_history_consistency),
    ];
    let cases = cases
        .into_iter()
        .map(|(name, case)| {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case(&factory)))
                .unwrap_or_else(|_| Err("the case panicked".into()));
            CaseResult {
                name,
                ok: r.is_ok(),
                message: r.err().unwrap_or_default(),
            }
        })
        .collect();
    ConformanceReport { cases }
}
