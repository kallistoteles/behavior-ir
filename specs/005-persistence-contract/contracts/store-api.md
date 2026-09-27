# Contract: Store API

Documents and errors: [data-model.md](../data-model.md). Decisions: [research.md](../research.md).

## Rust (`behavior-store`)

```rust
/// Host-implemented persistence primitives (R2). Every method is deterministic given the stored
/// data. `commit` is the only write after `create`, and it must be atomic.
pub trait Backend {
    fn genesis(&self) -> Result<Option<Genesis>, BackendError>;
    fn head(&self) -> Result<Option<Head>, BackendError>;
    fn create(&mut self, genesis: &Genesis, head: &Head, seed: &[EntityVersion])
        -> Result<(), BackendError>;
    /// Newest version with created_at <= position: immutable, so reads at one position form a
    /// consistent snapshot even while commits land (FR-018).
    fn version_at(&self, key: &EntityKey, position: u64) -> Result<Option<EntityVersion>, BackendError>;
    fn version(&self, key: &EntityKey, revision: u64) -> Result<Option<EntityVersion>, BackendError>;
    fn record(&self, position: u64) -> Result<Option<TransitionRecord>, BackendError>;
    fn commit(&mut self, expected_last_record: &Hash, versions: &[EntityVersion],
              record: &TransitionRecord, new_head: &Head) -> Result<CasOutcome, BackendError>;
}
pub enum CasOutcome { Applied, HeadMoved }

/// Engine-owned semantics over any backend.
pub struct Store<B: Backend> { /* backend */ }

impl<B: Backend> Store<B> {
    pub fn create(backend: B, genesis: Genesis) -> Result<Self, StoreError>;   // validates the seed
    pub fn open(backend: B) -> Result<Self, StoreError>;
    pub fn current(&self) -> Result<StateRef, StoreError>;
    pub fn load(&self, key: &EntityKey, at: &StateRef) -> Result<EntityVersion, StoreError>;
    pub fn evaluate(&self, module: &Module, action: &str, bindings: &BTreeMap<String, String>,
                    input: &Json, context: &Json, commit_time: &str, evidence: Option<Evidence>)
        -> Result<Evaluation, StoreError>;          // Evaluation { record, bundle: Option<CommitBundle> }
    pub fn commit(&mut self, expected_parent: &StateRef, bundle: &CommitBundle)
        -> Result<Committed, StoreError>;           // Committed { record_id, result_state, already: bool }
    pub fn transitions(&self, from: &StateRef, to: &StateRef)
        -> Result<Vec<TransitionRecord>, StoreError>;
}

pub fn replay_data<B: Backend>(store: &Store<B>, from: &StateRef, to: &StateRef) -> ReplayReport;
pub fn replay_behavior<B: Backend>(store: &Store<B>, modules: &BTreeMap<String, Module>,
                                   from: &StateRef, to: &StateRef) -> ReplayReport;
pub fn verify_snapshot<B: Backend>(store: &Store<B>, at: &StateRef,
                                   entities: &[EntityVersion]) -> ReplayReport;

pub struct InMemoryBackend { /* BTreeMaps; deterministic */ }

pub mod conformance {
    pub fn run<B: Backend>(factory: impl Fn() -> B) -> ConformanceReport; // named cases, pass/fail
}
```

Rules:
- **No I/O:** `Store` never reads the clock or the environment. `commit_time` and evidence are
  inputs.
- **Snapshot evaluation (FR-018):** `evaluate` reads the head once and every bound entity
  `version_at` that position. The bundle's expected parent is exactly that state.
- **Engine-derived sets:** read and write sets are derived by the engine and re-derived at commit
  (FR-020).
- **Trust:** `Committed` reports `evidence_trust: "structural"` when an authorization was bound.
  It is a structural guarantee within the current trust boundary, not proof of who issued the
  authorization (FR-023).
- **Evidence attachment:** `evaluate` attaches the evidence to the bundle only if supplied.
  Evidence can also be added before committing, because the authorization is made from the record.
  `CommitBundle::with_evidence(evidence)` returns a new bundle; the bundle hash covers the
  evidence.
- **Error mapping:** `StoreError` carries the codes of data-model.md → Store results and errors.
  `STATE_CONFLICT` carries the head and the changed keys.

## Python (`behavior`)

```python
from behavior import Store, InMemoryBackend, StateConflict, CommitRefused, run_conformance

store = Store.create(InMemoryBackend(), genesis={...})   # or Store.open(backend)
ref = store.current()                                    # StateRef(state=..., position=...)
ev = store.evaluate(model, "transfer", bindings={"from_": "a1", "to": "a2"},
                    input={"amount": Decimal("20.00")}, context={}, commit_time="2026-...Z")
ev.decision                                              # Decision (as `evaluate` returns today)
ev.bundle                                                # dict | None
result = store.commit(ref, ev.bundle)                    # CommitResult(record_id, result_state, already)
store.transitions(from_ref, to_ref)                      # list[dict]
replay_data(store) ; replay_behavior(store, [model])     # ReplayReport(ok, divergence)
run_conformance(lambda: MyBackend())                     # ConformanceReport(cases=[(name, ok, msg)])
```

- **Python backends:** a Python backend is any object with the methods `genesis`, `head`, `create`,
  `version_at`, `version`, `record` and `commit`, taking and returning canonical-JSON-compatible
  dicts. `commit` returns `"applied"` or `"head_moved"`.
- **Errors:** `StateConflict` has `current` and `changed`. `CommitRefused` has `code` and
  `message`.

## Conformance cases (minimum; FR-015, SC-003)

| Case | Checks |
|---|---|
| `create_and_open` | The genesis round-trips; `current()` is S0 with the documented identity |
| `commit_new_state` | The versions, revisions, state identity and record chain after a commit |
| `conflict_on_outdated_parent` | `STATE_CONFLICT` with the changed keys; the head is unchanged |
| `no_partial_application` | A backend failure injected mid-commit leaves the old head and no new versions visible |
| `crash_retry` | Faults are injected at the wrapper boundary, so this works for any backend without hooks. After an error before the inner commit, a retry applies once. After an error following a durable inner commit (a lost acknowledgement), a retry, even with a new commit time and after an intervening commit, returns `already = true`. No duplicate records |
| `snapshot_consistency` | Commits interleaved between the reads of an evaluation never mix states: every read and the evaluated state belong to one position |
| `store_binding` | A bundle or authorization from another store with identical content and behavior is refused (`EVIDENCE_MISMATCH` / `BUNDLE_INVALID`) |
| `derived_dependencies` | A bundle with an altered read or write set, or a write to an entity outside the parent state, is refused |
| `load_at_past_state` | `load(key, S_k)` returns the version valid at position k |
| `history_between_states` | `transitions` returns exactly the records in order, and the chain verifies |
| `idempotent_resubmission` | Resubmitting the same transition (same semantic hash and parent, even with a new commit time) returns `already = true` and adds no record, also when other commits followed (late retry) |
| `no_op_transition` | An empty effective change keeps the identity and advances the position |
| `identity_is_content` | Two stores reaching the same content through different histories have equal state identities. A store returning to an earlier content has the same identity at both positions, with different revisions. Swapping values between ids changes the identity |
| `observed_read_set` | The read set holds exactly the fields actually evaluated: reads inside derived values are included; a short-circuited operand (`active and balance > amount` with `active = false`) is excluded; the re-derivation at commit agrees |
| `evidence_policy` | `EVIDENCE_REQUIRED` and `EVIDENCE_MISMATCH` are enforced; records cite the policy and authorization hashes |
| `evidence_atomicity` | With a synthetic but well-formed authorization, its reference is committed atomically with the record, the versions and the head, and a crash after the durable write followed by a retry returns the bound commit (`evidence_trust: "structural"`). The engine's acceptance of real evidence is tested in `evidence.rs` |
| `replay_detects_tamper` | Data and behavior replay flag altered changes, inputs, identities and reordered records at the first position |

Broken reference backends in the test suite:
- `IgnoresHead` must fail `conflict_on_outdated_parent`.
- `PartialWrites` must fail `no_partial_application`.
- `ReordersRecords` must fail `history_between_states`.
- `DropsVersions` must fail `load_at_past_state`.
- `LatestReads` (serves newest versions regardless of position) must fail `snapshot_consistency`.
- `NonAtomicHead` (moves the head before writing the record) must fail `crash_retry`.

Internal atomicity of a backend's commit cannot be tested from outside. It is the host's
documented responsibility, checked for the reference backend by the `PartialWrites` mutant.
