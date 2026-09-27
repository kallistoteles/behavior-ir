//! Backend conformance (FR-015, research R12): wrappers that inject faults and interleavings at the
//! backend boundary (so they work for any backend without hooks), and the named conformance cases.

use std::cell::RefCell;

use crate::documents::{EntityKey, EntityVersion, Genesis, Head, TransitionRecord};
use crate::{Backend, BackendError, CasOutcome};

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
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed)
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
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        match self.next.take() {
            Some(Fault::BeforeWrite) => Err(BackendError("injected fault before the write".into())),
            Some(Fault::AfterWrite) => {
                self.inner.commit(expected, versions, record, head)?;
                Err(BackendError(
                    "injected fault after the write (acknowledgement lost)".into(),
                ))
            }
            None => self.inner.commit(expected, versions, record, head),
        }
    }
}

/// A commit prepared elsewhere, to be landed on a backend: `(expected, versions, record, head)`.
pub type Landing = (String, Vec<EntityVersion>, TransitionRecord, Head);

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
        if let Some((expected, versions, record, head)) = self.landing.borrow_mut().take() {
            self.inner
                .borrow_mut()
                .commit(&expected, &versions, &record, &head)?;
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
    ) -> Result<(), BackendError> {
        self.inner.borrow_mut().create(g, h, seed)
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
        record: &TransitionRecord,
        head: &Head,
    ) -> Result<CasOutcome, BackendError> {
        if self.on == LandOn::Commit {
            self.land()?;
        }
        self.inner
            .borrow_mut()
            .commit(expected, versions, record, head)
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
    let landing = (
        base.store_id().map_err(e)?,
        rec.new_versions.clone(),
        rec,
        head,
    );
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
        .commit("sha256:stale", &rec.new_versions, &rec, &head)
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
    let landing = (
        base.store_id().map_err(e)?,
        rec.new_versions.clone(),
        rec,
        head,
    );
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
    ensure(r.bundle.evidence.is_some(), "the record keeps the evidence")?;
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
    ) -> Result<(), BackendError> {
        self.inner.create(g, h, seed)
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
        r: &TransitionRecord,
        h: &Head,
    ) -> Result<CasOutcome, BackendError> {
        self.inner.commit(x, v, r, h)
    }
}

/// Runs every named case against backends from `factory`.
pub fn run<B: Backend>(factory: impl Fn() -> B) -> ConformanceReport {
    let cases: [(&'static str, Case<B>); 17] = [
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
