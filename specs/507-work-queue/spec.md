# Feature Specification: WorkQueue Model

**Feature Branch**: `507-work-queue`

**Created**: 2026-10-16

**Status**: Draft (clarified 2026-10-17)

**Input**: User description: "Feature 507 — WorkQueue Model. A declarative Behavior Model for
persistent, claimable units of work that a worker can lease, complete, fail or retry. Every
change is an ordinary Core transition: `Meaning(WorkQueue) = Meaning_Core(Lower(WorkQueue))`.
WorkQueue is not a queue engine and does not duplicate Core's durable command outbox. v1 promises
no FIFO, no automatic scheduling and no exactly-once execution. StateMachine owns the lifecycle,
the Store provides atomic concurrency control, and a monotonic attempt number is the fencing
token. No Core change."

**Depends on**: features 501 (StateMachine), 502 (RequestFlow: generated entity with an embedded
StateMachine), 503 (Approval: structural ownership over the final module), 505 (declaration
identity) and 506 (end-to-end conformance infrastructure), all on `main`, and the pinned Core
release 0.13.0. **Core changes**: none. A capability the pinned public Core cannot express is a
planning blocker owned by Core, never an ecosystem workaround (models/README.md Rule 3).

## Clarifications

### Session 2026-10-17

Design review in rounds (Q1–Q37). Each decision is applied in the sections below.

- Q1: Which capabilities are in v1? → All nine: enqueue, claim, renew, complete, retry, fail,
  recover, exhaust, cancel.
- Q2: Name of the expired-lease, attempts = M capability? → `exhaust` (input called it
  `expire_terminal`); `recover` is the attempts < M case.
- Q3: Generated or authored status enum? → Generated, with fixed cases and terminal set.
- Q4: Generated names? → Entity `EncodeEntityName(id)`, enum `<Entity>Status`, actions
  `<id>__<capability>`; collisions are build errors.
- Q5: Where does M live? → A model constant in the ModelHash, not a per-item field.
- Q6: Payload shape? → A typed schema of named fields, immutable after enqueue.
- Q7, Q20: Trust boundary and parameter names? → `worker`, `now`, `lease_until` (claim, renew)
  and `available_at` (retry, recover) are fixed-name host context; `generation` and
  `expected_generation` are caller input and only compared. Authored guards may add context;
  a name collision is a build error.
- Q8, Q25: Authorization? → enqueue, claim, cancel, recover and exhaust require an authored
  authorization guard (an explicit `true` to allow everyone); a missing one is a build error.
  Current-based capabilities have built-in ownership and optional guards.
- Q9: Removal? → Never in v1; terminal items remain as history.
- Q10: Past `available_at` at enqueue? → Allowed, unconstrained (immediately available).
- Q11: Lease length and backoff? → Host policy; the model requires only u > now, u' >
  lease_until and r ≥ now.
- Q12: Cancel a live lease? → Authority only, no ownership or generation; the running worker is
  fenced.
- Q13: RequestFlow in the reference example? → Yes, asynchronously (emitted command, separate
  enqueue).
- Q14: Non-PROVEN verifier results? → As 506: reported verbatim; a Core limitation becomes a Core
  issue and `blocked`; the gate may pass with the blocker recorded, completion does not; the model
  is never changed to make a result green.
- Q15, Q23: Conformance? → Reuse the 506 infrastructure and conventions with a separate 507
  report, completion status, readiness registry and scenario ids; 506's acceptance never depends
  on 507.
- Q16: Option ordering? → Every ordering against `lease_until` is an explicit `is_some(...)`
  conjunction; no sentinel value; `value_or` never decides an outcome.
- Q17: How is ValidWork established? → A model-contributed per-entity invariant, checked by Core
  at genesis and on every resulting state of every action.
- Q18, Q35: Ownership? → A WorkQueue-specific field-and-action ownership check over every
  WorkItem-typed binding of every action, including the queue's own; WorkQueue actions bind no
  WorkItem beyond their subject; no reliance on another model's exemption.
- Q19, Q28: Reads? → Generated reads `ready(now)` and `expired(now)` projecting identities only,
  and the per-entity derived value `terminal(w)`; time- and worker-dependent conditions stay in
  guards; no new derived-value semantics or binding-side evaluation.
- Q21: Time type? → A nominal type declared with the `order` op; plain `Int` is rejected.
- Q22: Lost acknowledgement? → Recovery resubmits the original bundle (Core's original-event
  contract); re-evaluation is a new attempt with an ordinary outcome.
- Q26: Commands and effects? → Authored guards, effects on other bound entities and command
  intents on all nine capabilities.
- Q27, Q31: Several queues per module? → Yes; W25 checks their independence.
- Q24, Q29, Q30, Q36: Evidence? → Four classes (SMT, structural/inductive argument, conformance
  test, Core/Store contract); each VO lists its dependencies and their statuses; structural
  arguments are reviewable with premises tied to executable enforcement; local SMT results are
  never labelled as the global theorem.
- Q32: Non-empty genesis? → Allowed when every item satisfies ValidWork; validity is not
  provenance (an initial LEASED item is trusted imported state, and its deadline is not assumed
  unexpired).
- Q33: Time guarantee? → Relative to the trusted evaluation time; commit-time freshness and clock
  monotonicity are explicit host assumptions; no physical deadline enforcement is claimed.
- Q34: Generation postconditions? → Generated `ensures attempts == generation` on Current-based
  capabilities and `expected_generation` with `ensures attempts == expected_generation + 1` on
  claim.
- Q37: Migrations? → A strengthened host-side migration contract (Migration contract below),
  with negative controls; no Core feature.

## Context

WorkQueue is a Behavior Model for work whose own lifecycle must be observable and changeable as
domain state: what work exists, when it may be claimed, which worker holds it, which attempt that
worker may finish, and how it ends. **WorkQueue defines which changes of work state are valid. It
does not define how, when or by whom work is scheduled or physically executed.** It lowers
completely to ordinary Behavior IR:

```text
WorkQueue  (id)
    ├── <Entity>Status enum  { READY, LEASED, SUCCEEDED, FAILED, CANCELLED }        (generated)
    ├── <Entity> entity      status, payload…, attempts, owner, lease_until, available_at
    │                        + the per-entity invariant ValidWork                     (generated)
    ├── <id>__enqueue        creation (status := READY, attempts := 0)
    ├── reads                ready(now), expired(now)   → identities only
    ├── derived value        terminal(w)
    └── embedded StateMachine over <Entity>.status
          claim    READY          → LEASED     attempts + 1, owner, lease_until
          renew    LEASED         → LEASED     lease_until only
          complete LEASED         → SUCCEEDED
          retry    LEASED         → READY      available_at
          fail     LEASED         → FAILED
          recover  LEASED         → READY      expired lease, attempts < M
          exhaust  LEASED         → FAILED     expired lease, attempts = M
          cancel   READY | LEASED → CANCELLED
                    ↓
             ordinary Behavior IR → behavior-ir-core (unchanged)
```

```text
WQ = (id, A, T, Π, M, G, K)
  id   stable semantic model identifier
  A    worker identity type (Id<A>, never Ref<A>)
  T    an author-declared nominal type with the `order` op (time, with declared units)
  Π    payload schema FieldId → CoreType, immutable after creation
  M    maximum number of claims per item, a model constant, M ≥ 1, representable as Int
  G    authored guards per capability (authorization is mandatory for enqueue, claim,
       cancel, recover, exhaust)
  K    authored command intents per capability

ValidWork(w) =                                     (per-entity invariant, single-state)
      0 ≤ attempts(w) ≤ M
  ∧ (status(w) = LEASED  ⇔  is_some(owner(w)) ∧ is_some(lease_until(w)))
  ∧ (status(w) = READY   ⇒  attempts(w) < M)
  ∧ (status(w) = LEASED  ⇒  attempts(w) ≥ 1)

Live(w, now)        = is_some(lease_until(w)) ∧ now <  lease_until(w)
Expired(w, now)     = status(w) = LEASED ∧ is_some(lease_until(w)) ∧ now ≥ lease_until(w)
Current(w, a, k, now) = status(w) = LEASED ∧ owner(w) = a ∧ attempts(w) = k ∧ Live(w, now)
Ready(S, now)       = { id(w) | status(w) = READY ∧ available_at(w) ≤ now ∧ attempts(w) < M }

token(w) = (id(w), attempts(w))       the claim generation; created only by a committed claim
```

`Live` and `Expired` show the ordering explicitly guarded by `is_some`: when the lease is
absent the conjunction is false regardless of any default value (Q16). They are written into
the generated guards; they are not separate derived values.

| Capability | Parameters (context · input) | Requires (plus authored `G`) | Effect on the subject | Generated ensures |
|---|---|---|---|---|
| enqueue | — · id, payload, available_at | id unused (Core identity lifecycle); authored authorization | create: READY, attempts 0, no owner, no lease | — |
| claim | worker, now, lease_until · expected_generation | READY, available_at ≤ now, attempts < M, attempts = expected_generation, lease_until > now; authored eligibility | LEASED, owner := worker, lease_until, attempts + 1 | attempts = expected_generation + 1 |
| renew | worker, now, lease_until · generation | Current, new lease_until > old lease_until | lease_until | attempts = generation |
| complete | worker, now · generation | Current | SUCCEEDED, owner and lease cleared | attempts = generation |
| retry | worker, now, available_at · generation | Current, attempts < M, available_at ≥ now | READY, owner and lease cleared, available_at | attempts = generation |
| fail | worker, now · generation | Current | FAILED, owner and lease cleared | attempts = generation |
| recover | now, available_at | Expired, attempts < M, available_at ≥ now; authored authorization | READY, owner and lease cleared, available_at | — |
| exhaust | now | Expired, attempts = M; authored authorization | FAILED, owner and lease cleared | — |
| cancel | — | READY or LEASED; authored authorization | CANCELLED, owner and lease cleared | — |

Context parameters come from the host and are recorded for replay; authored guards may add
their own context (for example an `actor`). Input parameters come from the caller and are only
compared, never trusted. Evaluation proposes; only a successful Store commit makes a transition
authoritative, and the commit is each capability's linearization point.

**Guaranteed within Core's documented semantics and the stated assumptions**: at most one
committed owner per item; an earlier generation never again satisfies `Current` after a later
committed claim (through ordinary transitions); at most one terminal outcome per item; a refused
or conflicting commit persists nothing. **Not guaranteed**: that only one physical worker
executes the job, that external I/O happens once, that an expired worker stops, that a lease is
still valid at the physical moment of commit, or that an external resource honors the
generation. External adapters need their own fencing or idempotency where duplicate execution is
harmful.

### Time

Core reads no clock. A lease is valid or expired **relative to the trusted evaluation time
`now`** that the host supplies as context; `now` is recorded with the decision and replayed. At
commit, Core re-derives the candidate with the same recorded context and requires the exact
parent head; it does not compare `now` with the commit time, and the commit time is not required
to be monotonic. The model therefore guarantees deadline semantics only relative to `now`.
Freshness of `now` at commit, clock monotonicity and a bounded evaluation-to-commit delay are
host assumptions, stated in every result that relies on them (VO-012). Recommended host
practices (one clock reading for `now` and the commit time, a lease margin) are recommendations,
not enforced guarantees.

### Genesis and provenance

A genesis state may contain work items when each satisfies `ValidWork`; Core checks this at
store creation (`GENESIS_INVALID` otherwise). **Validity is not provenance.** An item that is
LEASED at genesis is trusted imported state: no claim transition is implied or recorded for it,
and its deadline is not assumed unexpired. Fencing for such an item holds from genesis forward
(its generation k is exceeded by any later committed claim). Any stronger guarantee about
imported leases must be explicit and independently enforced.

### Migration contract

The model authority cannot constrain a Store migration. The fencing theorem, VO-003, VO-005 and
VO-007 are therefore stated for **ordinary transitions**. A migration that touches work items is
under this host contract, which the report lists as unenforced assumptions:

1. `attempts` never decreases and a generation is never reused.
2. A terminal item stays terminal.
3. A migration never manufactures a new active lease or reactivates an old claim generation.
4. READY → LEASED by migration is prohibited in v1.
5. A LEASED item either keeps owner, lease_until and attempts unchanged, or becomes READY with
   attempts unchanged and owner and lease cleared, provided the result satisfies `ValidWork`.
6. Hence LEASED → READY by migration requires attempts < M.
7. A migration across model or schema versions does not silently reinterpret claim
   generations or time semantics.

Core enforces only that the migrated state satisfies the target module's validity (including
`ValidWork`). Rules 1–7 beyond that are host obligations.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Author a work queue and run work to completion (Priority: P1)

A model author declares a WorkQueue for document processing (payload `Id<Document>`, M = 3,
worker type `Worker`, a nominal time type). The build returns an admitted module with a lowering
receipt. A host enqueues work, a worker reads the eligible set, claims an item with the
generation it read, and completes it with the generation it received. All of this goes through
the public binding and ordinary Core evaluation and commit.

**Why this priority**: Without construction, enqueue, claim and complete there is no WorkQueue.

**Independent Test**: Build the model, enqueue one item, claim it, complete it and check each
committed state against hand-stated expectations.

**Acceptance Scenarios**:

1. **Given** a valid WorkQueue definition, **When** it is built, **Then** the module is admitted
   and the receipt names model kind, model semantic version, lowerer version, Core release,
   ModelHash, BehaviorHash, canonical document, generated declarations, embedded StateMachine
   identity and source map. (W01 precondition)
2. **Given** an admitted queue, **When** work is enqueued with a fresh id and authorized,
   **Then** the item is READY with attempts 0, no owner and no lease. (W01)
3. **Given** a READY item with `available_at` later than `now`, **When** `ready(now)` is read or a
   claim is evaluated, **Then** the item is absent from the read and the claim is denied. (W02)
4. **Given** an eligible item, **When** an eligible worker claims it with
   `expected_generation = 0` and a deadline later than `now`, **Then** the committed item is
   LEASED, owned by that worker, with generation 1. (W03)
5. **Given** a LEASED item, **When** its owner completes it with the current generation while
   the lease is live, **Then** it is SUCCEEDED with owner and lease cleared. (W05)
6. **Given** a used id, **When** enqueue is evaluated again with it, **Then** Core refuses it
   (`ENTITY_ID_ALREADY_USED`) and no second item exists. (FR-024)

---

### User Story 2 - Fence stale and unauthorized workers (Priority: P1)

Two workers compete for the same item, leases expire, and an earlier worker returns late with a
result. Only the current, committed claim may finish the work.

**Why this priority**: Fencing is the core safety promise of the model.

**Independent Test**: Run the competing-claim and stale-completion sequences on a Store and check
which commits succeed and which evaluations are denied.

**Acceptance Scenarios**:

1. **Given** a READY item and two claims evaluated against the same head, **When** both are
   committed, **Then** exactly one commits and the other is refused with `STATE_CONFLICT`. (W04)
2. **Given** a LEASED item, **When** a worker other than the owner completes it, **Then** it is
   denied. (W06)
3. **Given** an item reclaimed after expiry (generation k + 1), **When** a completion with
   generation k is submitted, including by the same worker identity, **Then** it is denied.
   (W07)
4. **Given** a LEASED item, **When** its owner completes it with `now ≥ lease_until`, **Then** it
   is denied. (W08)
5. **Given** a claim with an `expected_generation` that differs from the item's attempts,
   **When** it is evaluated, **Then** it is denied.

---

### User Story 3 - Retry, renew, recover, exhaust, fail and cancel (Priority: P2)

Workers extend their leases, give work back for a later retry, or fail it permanently. A trusted
host recovers or exhausts expired leases. An authorized actor cancels work.

**Why this priority**: Needed for real workloads, but builds on the P1 lifecycle.

**Independent Test**: Drive each capability from a constructed state and compare with the
capability table.

**Acceptance Scenarios**:

1. **Given** a current lease, **When** the owner renews with a later deadline, **Then** the
   deadline changes and the generation does not. (W09)
2. **Given** an expired lease, **When** the owner renews, **Then** it is denied. (W10)
3. **Given** a current lease with attempts < M, **When** the owner retries with `available_at ≥
   now`, **Then** the item is READY at that time with attempts unchanged. (W11)
4. **Given** a current lease with attempts = M, **When** the owner retries, **Then** it is denied.
   (W12)
5. **Given** an expired lease with attempts < M, **When** the authorized host recovers it,
   **Then** it is READY at the stated time. (W13)
6. **Given** an expired lease with attempts = M, **When** the authorized host exhausts it,
   **Then** it is FAILED. (W14)
7. **Given** a READY or LEASED item, **When** an authorized actor cancels it, **Then** it is
   CANCELLED; without authority it is denied; a later completion by the former owner is denied.
   (W15)
8. **Given** a SUCCEEDED, FAILED or CANCELLED item, **When** any WorkQueue capability is invoked,
   **Then** it is denied. (FR-013)

---

### User Story 4 - The lifecycle cannot be bypassed (Priority: P2)

A module author composes queues with hand-written actions, a RequestFlow and an Approval. The
build rejects every action that would write WorkQueue-owned fields or status, or create or remove
work items, outside the capabilities that own them.

**Why this priority**: Without full-module ownership, every guarantee above can be bypassed.

**Independent Test**: Build modules that add a forbidden writer for each owned item and check
that each build fails with its typed error.

**Acceptance Scenarios**:

1. **Given** any action (hand-written, from another model, or an authored effect inside a
   WorkQueue capability) that writes `payload`, `attempts`, `owner`, `lease_until`,
   `available_at` or `status` through any WorkItem-typed binding, or creates or removes a
   WorkItem, outside its owning capabilities, **When** the module is built, **Then** the build
   fails with a typed ownership error. (W16)
2. **Given** a WorkQueue capability with an extra WorkItem-typed binding, **When** the module is
   built, **Then** the build fails. (W16)
3. **Given** a missing mandatory authorization guard, a plain `Int` time type, or a generated
   parameter name used by an authored context parameter, **When** the module is built, **Then**
   the build fails with a typed error.
4. **Given** a second model or declaration that claims a WorkQueue-generated name or owner,
   **When** the module is built, **Then** composition fails. (W22)
5. **Given** two queues in one module sharing worker and time types, **When** the module is
   built and exercised, **Then** neither queue's actions can write the other's items, both
   invariants hold, and their ModelHashes are distinct and independently reported. (W25)
6. **Given** a RequestFlow whose transition emits a command intent asking for work, **When** an
   adapter enqueues work referencing the request's id, **Then** both are ordinary, separately
   committed transitions, and WorkQueue imposes no RequestFlow semantics.

---

### User Story 5 - Replay, verify and compare across bindings (Priority: P3)

An auditor replays the reference history, checks each verification obligation with its evidence,
and confirms that the Python and Rust authoring give the same module.

**Why this priority**: Evidence of correctness; it depends on P1–P2.

**Independent Test**: Run the reference history, then data replay, behavior replay, the verifier,
the structural checks, the negative controls and the cross-binding comparison.

**Acceptance Scenarios**:

1. **Given** the committed reference history, **When** it is replayed as data and as behavior,
   **Then** both reproduce the final state and every recorded decision. (W17)
2. **Given** a recorded history with altered lease evidence or historical context, **When** it is
   replayed or verified, **Then** integrity verification fails. (W18)
3. **Given** a change to a referenced declaration or a model policy (M, payload, guards),
   **When** the model is rebuilt, **Then** the ModelHash, and where semantics change the
   BehaviorHash, change. (W19)
4. **Given** the same definition authored in Python and in Rust, **When** both are built,
   **Then** they produce the same admitted module and identities. (W20)
5. **Given** a direct evaluation whose parent snapshot omits a declared type, **When** it is
   evaluated, **Then** Core rejects the incomplete snapshot. (W21)
6. **Given** a commit whose acknowledgement is lost, **When** the host resubmits the original
   bundle, **Then** the result is `already: true` with the original record and no record is
   added. (W23a)
7. **Given** the same lost acknowledgement, **When** the host instead re-evaluates, **Then** this
   is a new attempt with an ordinary outcome: a re-evaluated claim is denied because the item is
   LEASED, and a re-evaluated enqueue is refused with `ENTITY_ID_ALREADY_USED`. (W23b)
8. **Given** a completion carrying effects on other bound entities and command intents, **When**
   it is evaluated and committed, **Then** the candidate is exactly those effects and intents,
   committed atomically, and nothing executes during evaluation. (W24)
9. **Given** the verification obligations, **When** the suite runs, **Then** each VO is reported
   with each of its evidence dependencies and their statuses, and no VO is reported better than
   its weakest dependency.

### Reference history

A document-processing system with `Document` and `Worker` entities, payload `Id<Document>` and
M = 3. It also includes a RequestFlow whose transition emits a command intent; an adapter step
enqueues the work in a separate commit, referencing the request's id.

| Step | Operation | Expected |
|---|---|---|
| 1 | enqueue d1 | READY, attempts 0 |
| 2 | `ready(now)` before available_at, then after | absent, then present |
| 3 | worker A claims (expected_generation 0) | LEASED, owner A, generation 1 |
| 4 | A retries with available_at r | READY at r, attempts 1 |
| 5 | worker B claims (expected_generation 1) | LEASED, owner B, generation 2 |
| 6 | B's lease expires; `expired(now)` lists d1; host recovers | READY, attempts 2 |
| 7 | A claims (expected_generation 2) | LEASED, owner A, generation 3 |
| 8 | B completes with generation 2 | denied |
| 9 | A completes with generation 3, with an effect on another entity and a command intent | SUCCEEDED; effect and intent committed atomically |
| 10 | data replay, behavior replay, verification | final state and decisions reproduced |

Expected outcomes are stated by hand and are never taken from one implementation's output.

### Edge Cases

- **M = 1**: a claim leaves attempts = M, so retry and recover are denied; complete, fail,
  exhaust and cancel can end the lease.
- **Deadline equal to `now`**: a claim with lease_until = now is denied; a lease with
  now = lease_until is expired (complete, renew, retry and fail are denied; recover or exhaust
  applies).
- **Renewal to the same deadline**: denied.
- **Retry or recovery time earlier than `now`**: denied.
- **Stale read before claim**: a claim whose `expected_generation` no longer matches is denied;
  a claim against a moved head is refused with `STATE_CONFLICT`.
- **Cancellation while a worker is still running**: the item is CANCELLED and later completions
  are denied; the external work is not interrupted or undone.
- **Same worker reclaims**: generation k + 1; requests with k are denied.
- **Expired but not yet recovered**: the item stays LEASED; it is listed by `expired(now)`, not by
  `ready(now)`, and cannot be claimed until recovered.
- **Concurrent recover and late complete**: at most one commits; a completion with
  now ≥ lease_until is denied anyway.
- **Lease valid at evaluation, expired at commit**: the commit succeeds if the head is unchanged;
  the guarantee is relative to the recorded `now` (Time), not to the physical commit moment.
- **Reading is not reserving**: two workers may read the same eligible item; only one claim
  commits.
- **Imported lease at genesis**: valid state, no recorded claim, deadline not assumed live.
- **Migration resetting attempts or reactivating a lease**: violates the migration contract and
  breaks fencing; shown by negative controls.

## Requirements *(mandatory)*

### Functional Requirements

**Model and authoring**

- **FR-001**: WorkQueue MUST be a new model kind in the single model authority, lowering to
  ordinary Behavior IR with no WorkQueue-specific evaluator, runtime state, error or side channel.
- **FR-002**: WorkQueue MUST generate the status enum, the WorkItem entity with the `ValidWork`
  per-entity invariant, the enqueue creation, the reads `ready(now)` and `expired(now)`, the
  derived value `terminal(w)` and the lifecycle transitions, and MUST lower the transitions
  through the unchanged StateMachine lowerer. Generated names follow Q4; collisions are build
  errors.
- **FR-003**: WorkQueue MUST expose a typed handle to the generated WorkItem entity for other
  models and hand-written behavior (reads, guards, payload references).
- **FR-004**: The ModelHash MUST cover every semantic configuration choice (M, payload schema,
  guards, commands, the time and worker types) and the exact declaration identities of every
  referenced declaration (feature 505). Display names, descriptions, source locations and
  authoring order MUST NOT affect it.
- **FR-005**: The Python binding MUST only describe what the author wrote. Validation, defaults,
  ordering, generated names and lowering belong to the model authority.
- **FR-006**: The time type MUST be a nominal type declared with the `order` op; any other type
  MUST be a build error. M MUST be an integer constant ≥ 1.
- **FR-007**: A module MAY contain several WorkQueues; each has its own generated declarations,
  ownership and identity.

**Lifecycle**

- **FR-008**: Enqueue MUST create exactly one WorkItem with an explicit id in READY with
  attempts 0, no owner and no lease, and the supplied `available_at` (unconstrained).
- **FR-009**: Claim MUST require READY, `available_at ≤ now`, attempts < M, attempts =
  `expected_generation`, `lease_until > now` and the authored eligibility, and MUST set LEASED,
  owner := worker, lease_until and attempts + 1.
- **FR-010**: Ownership of work MUST become authoritative only through a committed claim; an
  allowed evaluation grants nothing.
- **FR-011**: Renew, complete, retry and fail MUST require `Current(w, worker, generation, now)`.
- **FR-012**: Renew MUST keep the generation and only move the deadline later.
- **FR-013**: SUCCEEDED, FAILED and CANCELLED MUST be terminal: no ordinary action may change
  their status, fields or existence.
- **FR-014**: Retry MUST keep attempts, clear owner and lease, require attempts < M and set
  `available_at ≥ now`.
- **FR-015**: Recover and exhaust MUST require `Expired` and partition it by attempts < M
  (READY, `available_at ≥ now`) and attempts = M (FAILED).
- **FR-016**: Cancel MUST apply to READY and LEASED, require only its authored authorization,
  and clear owner and lease.
- **FR-017**: No item MAY be removed by any action.

**Safety and authority**

- **FR-018**: `ValidWork` MUST be a per-entity invariant contributed by the model, checked by Core
  at genesis and on every resulting state of every action, and part of the ModelHash; authors
  cannot remove it.
- **FR-019**: A WorkQueue ownership check over the final admitted module MUST reserve, for every
  WorkItem-typed binding of every action (including the queue's own): creation to enqueue;
  removal to nobody; `payload` to enqueue; `attempts` to enqueue and claim; `owner` and
  `lease_until` to enqueue (initial `none`), claim, renew (`lease_until` only), complete, retry,
  fail, recover, exhaust and cancel as the capability table states; `available_at` to enqueue,
  retry and recover. A creation's initial values count as enqueue's writes; `status` to enqueue and
  the embedded StateMachine. It MUST NOT rely on another model's exemption. Each violation MUST be
  a typed build error.
- **FR-020**: WorkQueue capabilities MUST NOT bind any WorkItem other than their subject, and
  authored effects MUST NOT target the subject; violations MUST be typed build errors.
- **FR-021**: Enqueue, claim, cancel, recover and exhaust MUST carry an authored authorization
  guard; a missing guard MUST be a build error. No authority is inferred from a role name.
  Claim's authorization guard is the worker eligibility predicate (`eligible`).
- **FR-022**: `worker`, `now` and `lease_until` (claim, renew) and `available_at` (retry,
  recover) MUST be host context with these fixed names. Enqueue's `available_at` is caller
  input, unconstrained (Q10). `generation` and `expected_generation` MUST be caller input and only
  compared.
  An authored context parameter with a generated name MUST be a build error.
- **FR-023**: Every ordering comparison with `lease_until` MUST be guarded by an explicit
  `is_some` conjunction; no sentinel value MAY stand for an absent lease.
- **FR-024**: Repeating an enqueue with a used id MUST be refused by Core, never create a second
  item. Recovery after a lost acknowledgement MUST resubmit the original bundle; a re-evaluation
  MUST be treated as a new attempt.
- **FR-025**: Concurrent capabilities MUST rely only on Core's Store head check and
  evidence-freshness contract; WorkQueue adds no lock or compare-and-swap.
- **FR-026**: The generated postconditions MUST be `attempts == generation` on renew, complete,
  retry and fail, and `attempts == expected_generation + 1` on claim.
- **FR-027**: No documentation, name or API MAY claim exactly-once external execution, automatic
  scheduling, ordering, fairness or physical deadline enforcement.

**Reads, effects and integration**

- **FR-028**: `ready(now)` and `expired(now)` MUST be ordinary declared reads with `now` as
  context, projecting identities only; `terminal(w)` MUST be an ordinary per-entity derived value.
  Time- and worker-dependent conditions MUST stay in guards; no new derived-value semantics or
  binding-side evaluation.
- **FR-029**: Reads MUST NOT reserve or change work.
- **FR-030**: All nine capabilities MAY carry authored guards, effects on other explicitly bound
  entities and command intents through Core's existing contracts; other models' ownership rules
  still apply. WorkQueue MUST NOT add an outbox, delivery or execution mechanism.

**Verification and conformance**

- **FR-031**: Every refusal MUST be Core's ordinary denial, typed error or Store refusal.
- **FR-032**: Each VO MUST be reported with its evidence dependencies (SMT, structural argument,
  conformance test, Core/Store contract) and each dependency's status. A VO is established only
  when all its dependencies are. A conformance test MUST NOT be presented as a universal proof,
  and a local SMT result MUST NOT be labelled as a global theorem.
- **FR-033**: Each structural argument MUST state premises, transition domain, reasoning and
  conclusion, with each premise linked to executable enforcement (a build error, a generated
  guard or postcondition checked in the lowered IR) and a negative control.
- **FR-034**: Data replay and behavior replay MUST reproduce the reference history, including
  leases, renewals, recovery and terminal outcomes.
- **FR-035**: Static verification MUST run through the pinned Core verifier and report each check
  faithfully. A Core limitation MUST be reported as `blocked` with its Core issue; the model MUST
  NOT be changed to make a result green.
- **FR-036**: Negative controls MUST show that removing a guard, weakening the generation
  comparison, allowing an unauthorized field writer, a migration that decreases attempts, and a
  migration that reactivates a lease without decreasing attempts each make the corresponding
  check fail.
- **FR-037**: The WorkQueue suite MUST reuse the 506 conformance infrastructure and conventions
  with its own report, completion status, readiness registry and scenario ids, and MUST run in
  the existing gates. Feature 506's scenarios and completion MUST NOT depend on 507.
- **FR-038**: Model compatibility (pinned ModelHash → BehaviorHash cases) MUST run in the existing
  gate.
- **FR-039**: Any construct the pinned public Core cannot express faithfully MUST be recorded as
  a planning blocker with its owning layer and a Core issue, never worked around.

### Verification obligations

| ID | Obligation | Evidence (all required) |
|---|---|---|
| VO-001 | Every item in genesis and every enqueued item satisfies `ValidWork` (validity only, not provenance) | Core contract (genesis check) · SMT (enqueue's resulting state) |
| VO-002 | Every action preserves `ValidWork` | SMT (per-action preservation) |
| VO-003 | Through ordinary transitions, attempts never decreases; only claim changes it, by exactly 1 | Structural (ownership FR-019, FR-020) · SMT (local postconditions FR-026) · test |
| VO-004 | 0 ≤ attempts ≤ M and the increment is representable | SMT (invariant preservation, guard attempts < M) |
| VO-005 | Through ordinary transitions, after a committed claim to generation k′ > k, no request with k satisfies `Current` | Structural (inductive argument from VO-003 and the `Current` guard) · SMT (local postconditions) · test · negative control |
| VO-006 | LEASED exactly when owner and lease_until are present | SMT (invariant preservation) |
| VO-007 | Through ordinary transitions, a terminal item stays terminal and is never removed | Structural (no transition from a terminal state, status owned, FR-017, FR-019) · test |
| VO-008 | Two competing claims cannot both commit from the same parent head | Store contract (exact head, `STATE_CONFLICT`) · backend conformance · test (W04) |
| VO-009 | Evaluation persists nothing and executes no command | Core contract · test |
| VO-010 | Behavior replay reproduces each recorded decision from its historical module, snapshot, input, context and facts | Core contract · test |
| VO-011 | No action of the final module bypasses WorkQueue ownership | Structural (build errors) · negative controls |
| VO-012 | Every result relying on time, actor identity, Store atomicity, external fencing or migrations lists its assumptions (Time, Migration contract) | Report |

### Key Entities

- **WorkQueue definition**: the immutable model `WQ = (id, A, T, Π, M, G, K)`; its canonical
  document gives the ModelHash.
- **WorkItem**: the generated entity; one unit of work with status, immutable payload, attempts
  (claim generation), optional owner, optional lease deadline and availability time, governed by
  `ValidWork`. Its id is explicit and never reused.
- **WorkStatus**: the generated enum; SUCCEEDED, FAILED and CANCELLED are terminal.
- **Claim generation**: (WorkItem id, attempts) after a committed claim; the fencing token.
- **Worker**: an author-declared type referenced by identity (`Id<A>`) only.
- **Time**: an author-declared ordered nominal type; values come from host context.
- **Lowering receipt**: kind, versions, Core release, ModelHash, BehaviorHash, canonical document,
  generated declarations, embedded StateMachine identity, source map.
- **Evidence record**: per VO, its dependencies, their classes and statuses, and its assumptions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every capability and both reads are exercised through the public interface, and
  all acceptance cases (W01–W22, W23a, W23b, W24, W25) pass against hand-stated expectations.
- **SC-002**: In the competing-claim scenario, exactly 1 of 2 claims commits on every run.
- **SC-003**: 100% of stale-generation, wrong-worker and expired-lease requests in the reference
  history are denied, and 0 forbidden writers pass the build.
- **SC-004**: Data replay and behavior replay reproduce the reference history's final state and
  all recorded decisions exactly.
- **SC-005**: All 12 VOs are reported with every evidence dependency and its status; no VO is
  reported better than its weakest dependency, and no unenforced assumption is reported as a
  guarantee.
- **SC-006**: The Python and Rust authoring of the reference queue give identical admitted
  modules, ModelHash and BehaviorHash; repeated builds give identical receipts.
- **SC-007**: Every negative control turns its check from passing to failing.
- **SC-008**: The 507 checks and compatibility cases run in the existing gates, the gates pass
  without any change to Core, and 506's report is unchanged.

## Assumptions

- The pinned Core 0.13.0 provides what the design uses (confirmed by review on 2026-10-17):
  StateMachine self-transitions and multi-source transitions; transition effects on other subject
  fields; implicit `Some`, `none`, `is_some` and `value_or`; Int arithmetic with checked overflow;
  ordered nominal types; host context recorded for replay; per-entity invariants checked at
  genesis and on every resulting state; postconditions over the resulting state with input
  parameters binding pre-state values; identity-only read projections (each row carries `id`);
  original-event recovery; `ENTITY_ID_ALREADY_USED`; `STATE_CONFLICT`. Planning re-confirms each
  by execution; a missing one is a blocker under FR-039.
- Effects on other entities from the eight transitions need optional state bindings on
  StateMachine transitions, which do not exist today. This is a model-layer facade gap, not a
  Core gap. Decided 2026-10-17 (plan D1(a)):
  - a general, versioned StateMachine extension (language level 3; models without bindings
    keep version 2 and their identities);
  - a binding never grants authority over owned fields.
- Model-contributed invariants and per-field ownership are new mechanisms in the model authority,
  built from existing Core pieces; no Core change.
- Composition with RequestFlow is asynchronous only; one transition cannot atomically invoke
  another capability in v1. Composition with Approval is through ordinary derived values in
  authored guards.

### Planning gates

Planning must show that the pinned public Core and model interfaces faithfully express:
(1) the generated declarations and invariant; (2) creation and the embedded StateMachine
lifecycle; (3) optional owner and deadline with `is_some`-guarded comparisons; (4) the exact
attempt increment and its bound; (5) generation-guarded capabilities and their postconditions;
(6) full-module per-field ownership; (7) the two reads with identity-only projections;
(8) command intents and effects on other bound entities; (9) deterministic ModelHash and
receipts; (10) Store conformance, replay and the verification obligations. A failed gate is
classified as a missing public facade capability or a missing Core semantic capability.

### Out of scope

FIFO, priority or starvation freedom; queue-wide concurrency limits or capacity scheduling;
recurring work; dependencies between items; bulk claims or bulk mutation; automatic expiry
transitions; automatic backoff; purge or retention; worker registration, health checks or
discovery; exactly-once external effects; physical deadline enforcement at commit;
model-enforced migration limits; distributed transactions; new Core expressions, effects, query
operators or Store protocols; a WorkQueue-specific verifier.
