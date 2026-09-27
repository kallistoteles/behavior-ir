# Feature Specification: Persistence Contract

**Feature Branch**: `005-persistence-contract`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "please read the data model document we created earlier as input for
this specification". The document is `docs/proposals/persistence-contract.md`, the persistence
contract proposal recorded on 2026-09-27.

## Context

Today the engine decides transitions but keeps nothing between them.
- A host passes in the state, input and context.
- The engine returns a decision record: the trace, the change set ΔS addressed by
  `{entity, id, field}`, and the decision.
- `replay` re-evaluates a record.
- A commit authorization cites the transition, the execution policy, the verification
  attestation and any waivers used.

What is missing is the part that makes a *history* of transitions reproducible:
- which exact version of each entity a decision read;
- which exact system state a transition started from and produced;
- how a host commits a transition atomically, without applying a decision that was made against
  outdated state.

Guiding principle (from the proposal):

> **The engine defines the canonical state-transition and persistence contract; hosts choose how
> that contract is stored.** The engine defines what must be preserved for behavior to be
> reproducible. It does not dictate how it is stored.

This feature defines that contract: a minimal State / Transition / Commit data model, a small
set of store operations implemented once by the engine over backend primitives that any host
storage can implement, and an in-memory reference backend with a conformance suite. No concrete database adapter is built. Snapshots are only a cache; the
transition history is the semantic truth.

## Clarifications

### Session 2026-09-27

- Q: What counts as a conflict (conflict granularity)?
  → A: Whole-state optimistic concurrency. A commit is accepted only when its expected parent is
  still the store's current state; otherwise it is `STATE_CONFLICT`, and the host re-reads,
  re-evaluates and retries. The model is kept open for a later entity-level rule:
  - every evaluation carries its evaluated state and its read and write dependencies;
  - records distinguish the state a transition was evaluated against from the state it was
    committed on.

  In this feature the two are always identical (an invariant). A future rule may separate them
  without changing the meaning of historical records. Entity-level conflict detection is not
  introduced now: it changes transition and replay semantics, not just concurrency.
- Q: Must every commit carry governance evidence?
  → A: Every store has an explicit, content-addressed evidence policy, and every commit must
  satisfy it. A policy may require no governance evidence, or require a matching commit
  authorization. Each transition record cites the exact evidence-policy hash and any
  authorization used. Authorization is never simply "optional": the policy decides whether its
  absence is valid. No commit has an implicit policy.

  Whether and how a store's policy can change (a new store, an explicit configuration
  transition, or a versioned policy) is deliberately not decided by this data model. In this
  feature the policy is fixed when the store is created.

Two persistence principles follow:

> **A transition is committed against an explicit state version.**
>
> **A commit is valid only relative to an explicit evidence policy.**

No important assumption is hidden in the host implementation. The strongest invariant of the
contract:

> **Every committed transition is reproducible from an immutable semantic state, an exact history
> position, an exact behavior version, exact input and exact context, and no storage
> implementation may alter those semantics.**

Three concepts are kept apart:

| Concept | Question | Represented by |
|---|---|---|
| State identity | What information exists? | A pure content hash |
| History position | When and where in this store's history? | A monotonic position + the record hash chain |
| Entity version | Which historical incarnation supplied this value? | Revision + the position that created it |

## User Scenarios & Testing *(mandatory)*

The actors:
- a **host developer**, who embeds the engine in an application and owns its storage;
- an **auditor**, who must answer "why is the system in this state?";
- a **behavior author**, who deploys new behavior versions over existing data.

### User Story 1 - Commit a decided transition against a known state (Priority: P1)

A host developer loads the current state from a store and evaluates an action against it. They
receive a commit bundle: everything the store must preserve. They ask the store to commit it on
top of the state it was evaluated against. The store applies the change atomically and returns
the identity of the new state. If another transition was committed in the meantime and the
decision is no longer valid, the commit is refused with a state conflict, and nothing is applied.

**Why this priority**: this is the core of the contract. Without state identity and a guarded
commit, a host can silently apply a decision made against outdated state. That breaks the
guarantee that every committed change was decided by the behavior on the state it changed.

**Independent Test**: use the reference store (the store over the in-memory reference backend)
with an account-transfer module.
- Evaluate `transfer` against the current state and commit the bundle. Check the new state and
  its identity.
- Evaluate two transfers against the same state and commit both. The second commit is refused
  with a state conflict, and the store is unchanged by the refusal.

**Acceptance Scenarios**:

1. **Given** a store at state S42 and a transfer evaluated against S42, **When** the host
   commits the bundle with expected parent S42, **Then** the store reaches a new state S43. S43's
   identity is derived from its content. The balances reflect exactly the change in the bundle,
   and each changed entity's revision is incremented by one.
2. **Given** two transfers evaluated against S42 (whether or not they touch the same accounts),
   **When** both are committed with expected parent S42, **Then** the first succeeds and the second is refused
   with `STATE_CONFLICT`. The refusal names the conflicting state and the entities that changed,
   and the store's state after the refusal equals the state after the first commit.
3. **Given** a decision that was denied or ended in an evaluation error, **When** the host tries
   to commit it, **Then** the store refuses it, because only allowed transitions produce a
   commit bundle.
4. **Given** a commit that fails partway in the store (simulated failure), **When** the store
   is queried afterwards, **Then** it shows either the complete new state or the unchanged old
   state, never a mix.

---

### User Story 2 - Reproduce and audit the history (Priority: P1)

An auditor takes a store's transition history from its initial state to its current state. They
verify it without knowing how the store keeps its data. Two checks are available:
- **Data replay:** apply each recorded change to the previous state and check each resulting
  state identity.
- **Behavior replay:** re-evaluate each transition with its exact behavior version, recorded
  state, input and context, and check that the same change and decision result.

**Why this priority**: history is the semantic truth, so it must be checkable independently of
the storage. Behavior replay checks not just the data but the determinism of the engine itself.
This is what makes the contract more than a database schema.

**Independent Test**: build a history of 1,000 committed transitions in the reference store, then:
- run data replay and behavior replay over it;
- tamper with one recorded change, one recorded input and one state identity, and check that
  each tamper is detected at the right transition.

**Acceptance Scenarios**:

1. **Given** a history S0 → … → Sn, **When** data replay runs, **Then** applying each change to
   the previous state reproduces every recorded state identity, and the final identity equals
   the store's current state.
2. **Given** the same history, **When** behavior replay runs, **Then** every transition
   re-evaluates to the recorded change and decision under its recorded behavior version.
3. **Given** a history where one transition's recorded change was altered, **When** either
   replay runs, **Then** it reports the first diverging transition and the kind of divergence.
4. **Given** a snapshot of state Sk kept as a cache, **When** replay from S0 to Sk runs,
   **Then** the snapshot is confirmed or reported as inconsistent. A snapshot never overrides
   the history.

---

### User Story 3 - Implement the contract for any storage (Priority: P2)

A host developer writes their own storage (for example over a relational database or an event
log) and implements the backend primitives; the engine supplies all store semantics on top. They
run the engine's conformance suite against it. If it passes, their store behaves like the store
over the reference backend in every observable way:
- the same state identities for the same histories;
- the same conflicts;
- atomic commits;
- the same history queries.

**Why this priority**: the contract only has value if other implementations can prove they
follow it. The reference store alone proves the model, but the conformance suite makes the
contract portable.

**Independent Test**: run the conformance suite against the reference store (it passes) and
against deliberately broken stores. Each broken store must fail a named conformance case:
- one that ignores the expected parent;
- one that applies commits partially;
- one that reorders history.

**Acceptance Scenarios**:

1. **Given** a store implementation, **When** the conformance suite runs, **Then** it reports
   pass or fail per named case, covering at least:
   - commit and new state identity;
   - conflict on an outdated parent;
   - no partial application;
   - loading an entity at a past state;
   - history between two states;
   - idempotent re-submission of the same bundle.
2. **Given** a store that ignores the expected parent, **When** the suite runs, **Then** the
   conflict case fails with a message naming the violated rule.
3. **Given** a host that implements the store in the Python layer, **When** the suite runs
   through the Python binding, **Then** the same cases apply with the same outcomes.

---

### User Story 4 - Evidence travels with the commit (Priority: P3)

When a transition is committed, its governance evidence is bound into the committed record: the
verification attestation, the execution policy, the waivers and the commit authorization. An
auditor looking at any past transition can see under which behavior version, verification
result and policy it was allowed, and check that the evidence refers to exactly this transition.

**Why this priority**: features 002–004 produce this evidence, but today it lives outside any
history, and no commit has an explicit rule about whether evidence was needed. Binding it to commits closes the loop from "the behavior was verified" to "this change
was made under that verification". It builds on stories 1–2 and is not needed for a working
store.

**Independent Test**: create one store whose policy requires a commit authorization and one whose
policy requires none. In the first, commit a transition together with a commit authorization for
it, then:
- read the committed record back and check that the authorization's transition hash matches the
  committed transition;
- try to commit with an authorization issued for a different transition, and check that it is
  refused;
- try to commit without an authorization, and check that it is refused.

In the second, commit without evidence and check that the record cites the "none required"
policy.

**Acceptance Scenarios**:

1. **Given** a commit authorization issued for a transition, **When** that transition is
   committed with it, **Then** the committed record references the authorization, the policy,
   the attestation and the waivers by their content hashes.
2. **Given** an authorization whose transition hash does not match the bundle, **When** the host
   commits, **Then** the commit is refused as inconsistent evidence.
3. **Given** a store whose evidence policy requires a matching commit authorization, **When** a
   bundle without one is committed, **Then** the commit is refused with a named error citing the
   policy.
4. **Given** a store whose evidence policy requires no governance evidence, **When** a bundle
   without evidence is committed, **Then** it is accepted, and the transition record cites that
   policy's hash. An auditor years later can see that the commit was allowed without evidence
   under this exact rule, without guessing.

---

### Edge Cases

- **The first commit.** The store starts from a defined initial state (empty, or seeded with
  entities). Seeding has its own recorded origin, so history replay has a defined starting
  point.
- **A transition that changes nothing.** An allowed transition with an empty change set
  (all effects keep their values). It still produces a committed transition record. The state
  content, and therefore its identity, is unchanged. The record stays in the history, because
  "it was decided and allowed" is itself history.
- **Entities created or removed.** Today actions only change fields of existing entities. Entity
  creation and deletion are out of scope. The model must not prevent adding them later: an
  entity's first version (revision 1) marks "no previous revision".
- **Behavior version changes over existing data.** Transitions of different behavior versions
  can follow each other in one history if the entity declarations they use are identical. Data
  migration between different entity declarations is out of scope. A commit whose behavior
  version declares an entity differently from the store's state is refused with a named error.
- **Re-submitting the same transition** (e.g. after a timeout whose commit actually succeeded).
  It must not apply twice. Re-submitting the same semantic transition on the same parent, even
  with a new commit time and even after other commits followed, returns the original commit's
  result instead of a conflict or a duplicate (FR-009).
- **Clock values.** The commit time is supplied by the host and recorded. It never takes part in
  state identity or behavior evaluation, so replay is independent of when it runs.
- **Concurrent commits that touch disjoint entities.** They conflict under the whole-state rule
  (FR-007). The host re-evaluates and retries. A later entity-level rule could accept them, and
  the recorded dependencies are designed to make that possible.
- **Changing the evidence policy of an existing store.** Out of scope. The policy is fixed at
  store creation, and records cite it, so a later feature can choose among a new store, a
  configuration transition or a versioned policy without reinterpreting history.
- **Large states.** State identity must not require the host to re-read the whole state on every
  commit (see SC-004).

## Requirements *(mandatory)*

### Functional Requirements

**State identity and entity versions**

- **FR-001**: Every entity instance in a store MUST have an identity (entity type and id) and a
  revision. The revision starts at 1 and increases by exactly one with each committed change to
  that entity. Each entity version MUST carry the content hash of its canonical value.
- **FR-002**: Every state MUST have an identity derived only from its semantic content. Each entity
  contributes its entity type, its declaration identity, its id and its canonical value.
  Revisions and positions are history, not content, and MUST NOT take part. Values swapped between
  ids or types give a different identity. The same raw values under a different entity
  declaration are a different state. Two states with equal content MUST have equal identities, whatever history
  produced them and whichever store holds them. A store that returns to exactly the content of an
  earlier state (e.g. position 42 and position 57) has the same state identity at both positions;
  the position tells the histories apart.
- **FR-003**: The engine MUST define the canonical form of states, entity versions, changes,
  transition records and commit bundles. Every conforming store MUST reproduce the same
  identities from the same content.

**Transitions and commits**

- **FR-004**: An allowed evaluation MUST produce a commit bundle containing:
  - the expected parent state identity;
  - the behavior version;
  - the action;
  - the input and the context;
  - the evaluated state identity, and the entities and fields read (read set) and written
    (write set), with the revisions read;
  - the change set, each change addressed by entity, id and field, with old and new values;
  - the trace;
  - the host-supplied commit time;
  - optionally, governance evidence (FR-013).

  Denied or failed evaluations MUST NOT produce a commit bundle.
- **FR-005**: The engine MUST provide the store operations below, implemented once by the engine
  over host-provided backend primitives (a small set of storage functions with one atomic
  compare-and-set). Hosts implement only the backend primitives; the semantics are never
  re-implemented per host. The operations are, conceptually:
  - current state;
  - load an entity at a given state;
  - commit a bundle with an expected parent;
  - the transition history between two states.

  The engine MUST NOT prescribe how a store keeps data: snapshots, change logs, versioned rows
  or content-addressed trees are all acceptable.
- **FR-006**: A commit MUST be atomic, including across crashes. It covers all of the following
  as one unit: the new entity versions, the state accumulator and head, the transition record,
  and the idempotency information. After a crash at any point, the store shows either the
  complete commit or nothing, and a retry of the same transition is recognized as already
  committed or applied once.
- **FR-007**: A commit MUST be accepted only when its expected parent is the store's current
  state (whole-state optimistic concurrency). The expected parent MUST equal the state the bundle
  was evaluated against; a mismatch is refused as an invalid bundle. Otherwise the result MUST be `STATE_CONFLICT`,
  naming the store's current state and the entities that changed since the parent. Nothing is
  applied, and the host re-reads, re-evaluates and retries.
- **FR-007a**: The model MUST allow entity-level conflict detection to be introduced later:
  - a transition record MUST distinguish the state it was evaluated against from the state it
    was committed on;
  - it MUST carry its read and write sets.

  In this feature, "evaluated against" equals "committed on" for every record, and the store and
  both replays MUST check this invariant. The read and write sets are recorded and reported
  (diagnostically, e.g. in a conflict) but do not decide conflicts.
- **FR-008**: A committed transition record MUST contain:
  - the commit bundle's content;
  - the state it was committed on;
  - the resulting state identity;
  - the hash of the evidence policy in force;
  - the hash of any commit authorization used. It is the system's commit object. It MUST have its own content
  identity, and the history MUST link each record to its parent state.
- **FR-009**: Re-submitting the same transition (the same semantic transition on the same parent,
  even with a different commit time) after it was committed MUST return that commit's result
  without applying it again. This holds even if further transitions were committed in between: a
  late retry must never be reported as a conflict, because a conflict leads the host to
  re-evaluate and apply the transition twice. Under whole-state concurrency, a transition evaluated
  at position p can only have been committed as record p+1, so that one record decides. The semantic identity of a transition MUST exclude audit metadata
  such as commit time and evidence. The audit record's identity includes them.
- **FR-010**: A commit MUST be refused, with a named error, when:
  - the bundle's behavior version declares an entity differently from the state's entities;
  - its entity revisions do not match the parent state;
  - its content does not match its declared identity.

**History, replay and snapshots**

- **FR-011**: The engine MUST provide data replay: apply each recorded change set, in order, to
  the previous state, and check each resulting state identity against the record. It MUST also
  provide behavior replay: re-evaluate each transition with its recorded behavior version,
  state, input and context, and check that the decision and change set equal the recorded ones.
  Both MUST report the first diverging transition and the kind of divergence.
- **FR-012**: Snapshots MAY be kept by any store as a cache. A snapshot MUST be verifiable by
  replay, and MUST NOT be accepted as truth when it disagrees with the history.

**Evidence**

- **FR-013**: Every store MUST have an explicit, content-addressed evidence policy, fixed when the
  store is created and recorded at its origin. Every commit MUST satisfy it. A policy either
  requires no governance evidence, or requires a commit authorization matching the transition.
  The policy format MUST allow further requirements to be added later.
- **FR-013a**: A commit bundle MAY carry a commit authorization and the documents it cites:
  execution policy, verification attestation and waivers. When present, the store MUST check that
  the authorization's transition hash matches the bundle, and MUST refuse mismatched evidence.
  - If the evidence policy requires an authorization and none is present, the store MUST refuse
    the commit with a named error citing the policy.
  - Accepted evidence MUST be referenced by content hash in the committed record.

**Reference store, conformance and hosts**

- **FR-014**: The engine MUST ship an in-memory reference backend with which the store implements
  the full contract. It is for tests and as an executable definition. It is not production storage.
- **FR-015**: The engine MUST ship a conformance suite of named cases, runnable against any
  backend implementation, including from the Python binding. The suite MUST include broken reference
  variants that each fail their named case.
- **FR-016**: The Python binding MUST let a host evaluate against a store's state, obtain the
  commit bundle, commit it, and run replay. A host MUST be able to implement a store in Python
  and run the conformance suite against it.

**Snapshot consistency, store identity and trust**

- **FR-018**: Evaluation MUST read every entity from one consistent snapshot: a single state that
  existed in the store. It MUST NOT read entities one by one from a store that may move between
  reads. The evaluated state identity and every entity read MUST belong to that same state, and
  the commit's expected parent MUST be that state.
- **FR-019**: A commit bundle, and every decision record evaluated against a store, MUST bind the
  store's identity (its genesis) as well as the evaluated state. An authorization for a
  transition in one store MUST NOT be usable in another store, even one with identical content
  and behavior.
- **FR-020**: The read set and the write set MUST be derived by the engine from the evaluation.
  A store MUST re-derive or check them at commit and MUST NOT accept host-supplied dependency
  sets.
  - The **read set** is the set of semantic dependencies actually observed by this evaluation:
    every entity field whose value was read. This includes reads inside derived values, rules,
    invariants and constraints, and excludes operands skipped by short-circuiting. For example,
    `account.active and account.balance > amount` with `active = false` does not read `balance`.
  - A static superset of possible reads, if one is ever recorded, is a different concept named
    `dependency_set`.
- **FR-021**: The set of entity identities is fixed after genesis in this feature. No committed
  transition may create or remove an entity. The engine MUST enforce this (a write to an entity
  absent from the parent state is refused), and replay MUST check it.
- **FR-022**: The context an evaluation read MUST be preserved in full (the canonical context
  snapshot, not only identifiers), so that behavior replay does not depend on today's version of
  context data.
- **FR-023**: `require: commit_authorization` MUST be documented and reported as a structural
  guarantee within the current trust boundary. It says that a matching, well-formed
  authorization is bound to the commit. It is not yet cryptographic end-to-end proof of who
  issued it. That remains the open feature-002 risk of unsigned attestations and authorizations,
  to be closed by signed authorizations with trusted keys in the evidence policy.

**Determinism**

- **FR-017**: State identities, record identities, conflicts and replay results MUST be
  deterministic. The same history MUST give byte-identical canonical documents on every run and
  platform. The commit time MUST NOT influence any identity other than the transition record's
  own.

### Key Entities

- **Entity version**: one historical incarnation of an entity. It carries the entity's content
  hash (type, declaration, id, canonical value), its canonical field values, its revision, and the
  position that created it.
- **State**: the set of current entity values, with an identity derived only from their content
  hashes (not from revisions). It
  is a logical concept: a store may keep it as snapshots, a change log or a tree.
- **Change set (ΔS)**: the field changes of one transition, each addressed by entity type, id
  and field, with old and new values. It is a first-class part of every record.
- **Commit bundle**: what an allowed evaluation hands to a store:
  - parent state;
  - behavior version and action;
  - input, context and entity versions read;
  - change set and trace;
  - evaluated state, read set and write set;
  - commit time;
  - evidence, when supplied.
- **Transition record**: a committed bundle plus:
  - the state it was committed on (in this feature always the evaluated state);
  - the resulting state identity;
  - the evidence-policy hash;
  - the hash of any authorization used.

  It has its own content identity. It is the commit object, and the history is the ordered chain of these
  records.
- **Store**: the engine's store operations (current state, load an entity at a state, guarded
  commit, history between states) over a backend.
- **Backend**: the host's storage primitives (reads of the genesis, the head, entity versions and
  records, plus one atomic compare-and-set commit). Hosts implement only this.
- **Evidence policy**: a content-addressed rule stating what governance evidence a commit needs
  (none, or a matching commit authorization; extensible). It is fixed per store at creation and
  cited by every transition record.
- **Read set / write set**: the entity fields a transition actually observed (not a static
  superset), with the revisions of their entities, and
  the fields it wrote. They are recorded for diagnostics and future entity-level conflicts.
- **State conflict**: a refused commit whose expected parent is no longer the current state. It names the current state and the entities that changed.
- **Snapshot**: a cached materialization of a state. It can be verified against the history and
  is never authoritative.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In 10,000 generated concurrent-commit scenarios against the reference store, no
  decision is ever committed on top of a state other than the one it was evaluated against
  (every record satisfies "evaluated against = committed on"). A partial commit is never observable.
- **SC-002**: For generated histories of up to 10,000 transitions, both data replay and behavior
  replay reproduce every recorded state identity and change. Every single-field tamper (change,
  input, context, state identity, behavior version) is detected at the first affected transition.
- **SC-003**: The reference store passes 100% of the conformance cases, and each of at least
  three deliberately broken stores fails its designated case, from Rust and from Python.
- **SC-004**: Committing one transition to a store holding 100,000 entities takes time
  proportional to the entities the transition touches, not to the store size. It completes in
  under 50 ms on developer hardware in the reference store.
- **SC-005**: The same sequence of commits produces byte-identical state identities, transition
  records and replay reports on two runs, across all supported platforms.
- **SC-006**: A host developer can connect evaluation, commit and replay in the Python binding
  with under 20 lines of glue code, as demonstrated by an example in the repository.

## Assumptions

- **Scope:** the contract, the in-memory reference backend and the conformance suite. Database
  adapters (relational, embedded, event store) are out of scope; hosts build them against the
  contract.
- **State is host-owned.** The engine never opens files or connections for state. Stores are
  supplied by the host.
- **Actions change fields of existing entities**, as today. Entity creation and deletion, and
  data migration between different entity declarations, are out of scope. The model leaves room
  for them.
- **Determinism is kept.** The commit time is host-supplied data, like context, and never read
  from a clock inside the engine.
- **Replay reuses existing machinery.** Behavior replay uses the existing evaluator and record
  replay. The change-set addressing (`{entity, id, field}`) from feature 002 is kept as-is.
- **Evidence is not re-checked.** Commit authorizations, attestations, policies and waivers keep
  their existing formats from feature 002. This feature binds them to commits; it does not change
  how they are produced. Open risks from feature 002 (unsigned attestations, trusted verification
  cache) remain open and are not solved here. A store that holds evidence records who supplied
  it but does not re-verify it.
- **Identities are content hashes.** They are computed with the engine's existing domain-tagged
  scheme.
