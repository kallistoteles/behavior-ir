# Feature Specification: Entity Lifecycle (Universe Transitions)

**Feature Branch**: `006-entity-lifecycle`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "Feature 006 — Entity Lifecycle / Universe Transitions: Behavior IR
can describe deterministic, verifiable changes to *which* entities exist, not only to their
fields" (full description in the conversation; summarized below).

## Context

After features 001–005 the engine covers a complete chain for changing existing state:
- behavior definition;
- formal verification;
- evaluation against an exact snapshot;
- governance;
- atomic persistence;
- content-addressed history;
- deterministic replay.

What it cannot express is a change to the **set** of entities: today the entity universe is fixed
when a store is created, and a transition can only change fields of existing entities
(`Account#123.balance: A → B`). Opening an account, placing an order or closing a case is
impossible.

This feature makes the entity universe part of state semantics:

```text
S  = { entity universe U, values of the entities in U }
ΔS = field updates + entity creations (∅ → E#id) + entity removals (E#id → ∅)
```

Lifecycle changes are new kinds of state change, not a new execution system: an ordinary action
evaluates its conditions and proposes ΔS, which may now contain creations and removals. All
effects of a transition are validated together against the complete resulting state.

**Primary invariant**:

> The complete state consists of both entity existence and entity values; every change to either
> is an explicit, deterministic, replayable transition.

## Clarifications

### Session 2026-09-27

- Q: May an identity that has been removed ever be created again?
  → A: No. An entity identity names exactly one lifetime across the entire store history, and a
  removed identity may never be created again. Creation therefore checks against every identity the
  store has ever used, not merely the current entity universe. Reuse fails as
  `ENTITY_ID_ALREADY_USED`.
- Q: Does an `Id<Customer>` field imply that the customer must exist?
  → A: No. `Id<T>` remains typed identity only and does not imply current existence. There is one
  core semantic predicate, `exists(Id<T>)`, for current-state existence. `Ref<T>` is declarative
  sugar for `Id<T>` plus an existence constraint, not a second, independent mechanism. Only
  explicit existence constraints take part in referential integrity and may block removal.
  Historical and external identities remain plain `Id<T>`.
- Q: When an action removes a customer, how must the engine find the orders whose `Ref<Customer>`
  still points to it, given that those orders are not parameters of the action?
  → A: With an engine-maintained reverse-reference index, kept as a **derived** store index for
  every `Ref<T>` field.
  - The index is not canonical state: it does not contribute to the state identity, and it can be
    reconstructed from entity content.
  - Referential integrity is checked against the proposed resulting state S'. Incoming references
    that the same transition removes or retargets therefore do not block the removal.
  - Index updates are part of the same atomic commit as entity, state and history changes.
  - Queries of incoming references are observed state reads, from the same snapshot position,
    recorded for replay.
  - Verification models the semantic invariant that every surviving `Ref<T>` in S' points to an
    existing entity, independent of the index.

Four principles follow:

> **Identity is permanent; existence is state.**
>
> **Referential integrity is explicit, not inferred from identity typing.**
>
> **State is observed through facts; every fact that influences a decision is part of that
> decision's reproducible snapshot.**
>
> **Indexes accelerate semantics; they do not define semantics.**
>
> **State identity describes current content; history position may contribute additional facts
> that affect valid future transitions.**

The last principle is new with this feature. Two positions with the same state identity can allow
different creations, because an identity used and removed in between is no longer available. The
deterministic input of a transition is therefore an **evaluation snapshot**:
- the exact state;
- the current-state facts it observed (existence, incoming references);
- the history facts it observed (whether an identity was ever used).

The store's three-part `data_version` (store, state, position) already names exactly that.

The third generalizes the observed field reads of feature 005: a field read and an existence read
are both state facts.

## User Scenarios & Testing *(mandatory)*

The actors:
- a **behavior author**, who describes what the application may do;
- a **host developer**, who runs the engine and its store;
- an **auditor**, who must reconstruct the history.

### User Story 1 - Create an entity through an action (Priority: P1)

A behavior author writes an action that opens a new account. The account's identity is supplied
by the host as a typed input, and the action gives the complete initial value (owner, balance,
status). Evaluating the action against the current state produces a transition that creates
exactly that account. Committing it adds the account to the store, at its first revision. If the
store has ever used that identity, the transition is refused with a dedicated result, whether or
not the entity still exists.

**Why this priority**: creation is the missing capability that blocks modeling real
applications, such as opening accounts, placing orders or registering users.

**Independent Test**: define an `open_account` action whose input is the account's identity and
its initial balance.
- Evaluate and commit it against a store without that account, and check the new state. The
  account exists at revision 1, with exactly the given values, and the state identity changed.
- Evaluate it again with the same identity, and check that it is refused with
  `ENTITY_ID_ALREADY_USED`.

**Acceptance Scenarios**:

1. **Given** a store that does not contain `Account#42`, **When** `open_account(id=42,
   balance=100.00)` is evaluated and committed, **Then** `Account#42` exists with balance
   100.00 at revision 1. The transition record shows the creation with its complete initial
   value, and the new state identity differs from the parent's.
2. **Given** a store that contains `Account#42`, **When** `open_account(id=42, …)` is
   evaluated, **Then** the decision is refused with `ENTITY_ID_ALREADY_USED`, and nothing can be
   committed.
3. **Given** an entity constraint `balance >= 0`, **When** `open_account(id=43,
   balance=-5.00)` is evaluated, **Then** the decision is refused because the created entity
   violates its constraint.
4. **Given** an action that creates an entity without giving a value for every field, **When**
   the behavior is admitted, **Then** admission refuses it, naming the missing fields.

---

### User Story 2 - Remove an entity through an action (Priority: P1)

A behavior author writes an action that closes an account whose balance is zero. Committing it
removes the account from the current state. The account's history stays intact: loading it at an
earlier state, and replaying the history, still show it as it was.

**Why this priority**: without removal, the universe only grows. Many domains need entities to
end: closed accounts, cancelled orders, resolved cases.

**Independent Test**:
- Create an account, then remove it with `close_account`. Check that it is absent from the new
  state, that loading it at the state before the removal returns its last version, and that data
  replay and behavior replay reconstruct the removal.
- Try to remove an account that does not exist, and check that it is refused.

**Acceptance Scenarios**:

1. **Given** `Account#42` with balance 0.00, **When** `close_account(Account#42)` is evaluated
   and committed, **Then** `Account#42` is absent from the resulting state, and the state identity
   changes accordingly.
2. **Given** the history after that removal, **When** `Account#42` is loaded at the state before
   the removal, **Then** its last version is returned unchanged.
3. **Given** a removal whose target does not exist in the evaluated state, **When** it is
   committed, **Then** it is refused with a named error.
4. **Given** a removed identity, **When** a later action tries to create an entity with the same
   identity, **Then** it is refused with `ENTITY_ID_ALREADY_USED`: an identity names one lifetime.

---

### User Story 3 - Lifecycle changes are verified and atomic (Priority: P2)

A behavior author relies on verification to prove that creations and removals keep the state
valid:
- a created entity satisfies its constraints;
- removals do not break rules that other entities depend on;
- postconditions hold on the resulting state.

A transition that removes two related entities together is judged by its final state only. There
is no observable intermediate state.

**Why this priority**: lifecycle without verification would reopen the gap that features 002–004
closed. Runtime and verifier must share the same lifecycle semantics.

**Independent Test**:
- Verify a module with a creating action whose initial values can violate an entity constraint.
  Check that a counterexample is found and reproduced by the runtime.
- Add the missing precondition, and check that the property is proven.
- Verify a removing action against a rule that depends on the removed entity, and check that the
  violation is found.

**Acceptance Scenarios**:

1. **Given** `open_account` whose balance input is not required to be non-negative, **When** the
   module is verified, **Then** constraint preservation for the created account has a confirmed
   counterexample. With `requires(balance >= 0)` it is proven.
2. **Given** a transition that creates one entity and updates another, **When** it is evaluated,
   **Then** all constraints, invariants and postconditions are checked against the complete
   resulting state, and never against a partially applied one.
3. **Given** verification of a creating action, **When** the verifier reasons about it, **Then**
   it assumes, exactly as the runtime enforces, that the new identity is not in the evaluated
   state.
4. **Given** `Order.customer: Ref<Customer>`, **When** an action removes a customer that an
   existing order still references, **Then** the resulting state violates the order's existence
   constraint, so the transition is refused. Verification finds this with a confirmed
   counterexample.
5. **Given** `AuditRecord.historical_customer: Id<Customer>`, a plain identity, **When** that
   customer is removed, **Then** nothing is blocked: plain identities imply no existence.
6. **Given** a transition that removes an order and its customer together, **When** it is
   evaluated, **Then** it is valid if the resulting state satisfies every existence constraint,
   even though removing either one alone would not be.

---

### User Story 4 - Every universe change is recorded, and history replays (Priority: P2)

An auditor reconstructs a store's history, including every creation and removal, from its
transition records alone, independently of how the store keeps its data. The store never adds or
deletes entities outside a recorded transition.

**Why this priority**: feature 005 made history the semantic truth. Lifecycle changes must not
become a backdoor around it.

**Independent Test**:
- Build a history mixing creations, updates and removals, and run data replay and behavior replay
  over it.
- Tamper with one recorded creation or removal, and check that the replays find it at its
  position.
- Check that the storage contract offers no way to add or remove an entity except through a
  commit.

**Acceptance Scenarios**:

1. **Given** a history with creations, updates and removals, **When** data replay runs, **Then**
   every recorded state identity is reproduced, including the membership of the universe at each
   position.
2. **Given** a recorded creation whose initial value was altered, **When** replay runs, **Then**
   the divergence is reported at that position.
3. **Given** two concurrent creations of the same identity evaluated against the same state,
   **When** both are committed, **Then** at most one succeeds.
4. **Given** the storage contract, **When** a host wants to add an entity after genesis, **Then**
   the only way is a recorded transition.

---

### Edge Cases

- **Create and update in one transition.** A transition may not create an entity and also update
  or remove that same new entity. The complete initial value is given by the creation itself.
- **Several creations in one transition.** They are allowed if their identities differ from each
  other and have never been used by the store. Two creations of the same identity in one transition are
  invalid.
- **Remove and update in one transition.** Updating an entity and removing it in the same
  transition is invalid. Removing two related entities together is valid if the final state is
  valid.
- **The created identity is referenced by another effect.** An effect in the same transition may
  store the new entity's identity in another entity's field (e.g. a new order's `customer:
  Ref<Customer>` pointing at a customer created in the same transition). This is valid, because
  existence constraints are checked on the resulting state.
- **References removed or retargeted together with their target.** Removing `Order#7`, retargeting
  `Order#19.customer` to `Customer#84` and removing `Customer#42` in one transition is valid: the
  check runs on the resulting reference graph.
- **Where `exists` is evaluated.** It is evaluated on the state its condition is checked against:
  the evaluated state for preconditions and incoming rules, and the resulting state for
  postconditions and outgoing rules. An identity created in the transition exists in the
  resulting state; a removed one does not.
- **An identity that was never used.** For it, `exists` is false, and a `Ref<T>` field holding it
  violates the existence constraint.
- **Removed entities in replay.** Behavior replay of an earlier transition that read an entity
  later removed still uses that entity's version at the transition's parent state.
- **Genesis.** The genesis seed remains the initial universe. After genesis, every universe change
  is a recorded transition. Imports are ordinary creating actions.
- **Identity type safety.** Creating an `Account` with an identity typed `Id<Customer>` is a type
  error at admission.
- **A domain status is not removal.** A field `status = CLOSED` is a domain value, and removal
  means the entity no longer exists. The two are distinct, and nothing converts one into the
  other implicitly.

## Requirements *(mandatory)*

### Functional Requirements

**Language**

- **FR-001**: An action MAY contain, besides field updates, **creation** effects and **removal**
  effects. There is no separate action kind for lifecycle, and conditions, evaluation order and
  decisions work as for every action.
- **FR-002**: A creation MUST name the entity type, the new identity, and a value for **every**
  field of that entity. Admission MUST refuse a creation with missing, unknown or ill-typed fields.
- **FR-003**: The new identity MUST be given by the behavior's inputs, as a value of the typed
  identity `Id<T>` for the created type `T`. The engine MUST NOT generate identities (no random,
  time-based or counter-based identities).
- **FR-004**: A removal MUST name an entity bound as state for the action. Its value is visible to
  the action's conditions before removal.
- **FR-005**: A transition MUST contain **at most one lifecycle operation per typed identity**:
  - create + create, remove + remove, remove + create, and create + remove of the same identity are
    all invalid;
  - updating a created or removed entity in the same transition is invalid too. Admission MUST refuse this where it is
  statically visible, and evaluation otherwise.

**Semantics**

- **FR-006**: All effects of a transition (updates, creations, removals) MUST be applied together
  to produce one proposed resulting state. Entity constraints, state invariants and postconditions
  MUST be checked against that complete state; there is no observable intermediate state.
- **FR-007**: A creation whose identity has ever been used by the store (whether it exists in the
  evaluated state or was removed earlier) MUST make the decision `ENTITY_ID_ALREADY_USED`, a
  distinct result, not a generic invalid state.
- **FR-008**: A removal of an entity that does not exist in the evaluated state MUST be refused
  with `ENTITY_NOT_FOUND`. Evaluation cannot produce this case, because removal targets are bound
  state parameters. The store refuses such a bundle at commit.
- **FR-009**: An entity identity names exactly one lifetime across the whole store history. The
  store MUST keep a registry of every identity it has ever used (genesis seed and creations). The
  registry is part of store semantics, but not of the state identity. Evaluation and commit MUST
  check creations against it.
- **FR-010**: Created entities MUST satisfy their entity constraints.
- **FR-010a**: There MUST be one core predicate, `exists(Id<T>) → Bool`, meaning "the identity
  exists in the state this condition is checked against". It also accepts `Option<Id<T>>`, where
  an absent value gives false. It MAY be used in any condition, rule or
  constraint. It is the only existence semantics shared by evaluator, verifier and replay.
- **FR-010a2**: A predicate `referenced(Id<T>) → Bool` MUST be available in conditions, rules and
  constraints. It is true if some surviving `Ref<T>` field points at the identity. It is a
  projection of the incoming-reference fact (FR-010f), never a separate fact source. Like `exists`,
  it depends on the state: the evaluated state for preconditions and incoming rules, the resulting
  state for postconditions and outgoing rules. Its purpose is to let authors state, and the
  verifier prove, that a removal leaves no dangling reference.
- **FR-010b**: `Ref<T>` MUST be sugar for `Id<T>` plus the entity constraint `exists(field)`
  (for optional references, when present). It is not an independent mechanism. `Id<T>` alone MUST
  NOT imply existence. Only explicit existence constraints take part in referential integrity, and
  a removal succeeds only if the resulting state satisfies all of them.
- **FR-010c — observed existence reads**: `exists(Id<T>)` is a state read over entity-universe
  membership, not a context read. State reads come in two kinds:
  - a **field read** (type, id, field, value);
  - an **existence read** (type, id, exists: bool).

  Every existence fact actually evaluated MUST be captured, with its boolean value, in the
  evaluation's observed state snapshot and in its read set. It MUST originate from the same state
  position as all other state reads (with a store: `exists_at` the evaluated position, as
  `version_at` is for values). Replay uses the recorded fact, never a live store. Unreached
  existence checks, including those skipped by short-circuit evaluation, are not recorded. Only
  observed facts are recorded; the snapshot never holds the whole universe.
- **FR-010d — trust boundary of existence facts**: a recorded existence fact is reproducible
  through history (data replay reconstructs membership at every position). It is not a standalone
  cryptographic membership or non-membership proof for a state identity: the multiset state hash
  of feature 005 cannot prove that `Customer#42` is absent from state `ABC` on its own. This MUST
  be documented. Standalone proofs, e.g. a Merkle set, are a possible later extension, not part of
  this feature.
- **FR-010e — referential integrity against S', via a derived index**: for every `Ref<T>` field,
  the engine MUST maintain a reverse-reference index (which entities' `Ref` fields point at each
  identity).
  - **Derived, not canonical:** the index is derived from entity content, MUST NOT contribute to
    the state identity, and MUST be reconstructible from the state alone.
  - **Checked against S':** referential integrity MUST be checked against the proposed resulting
    state S' (the current reference graph plus ΔS), and never only against the index at S. A
    violation is refused with `DANGLING_REFERENCE`, listing the surviving references. A
    transition that removes `Order#7`, retargets `Order#19.customer` to `Customer#84`, and removes
    `Customer#42` is valid.
  - **Updated atomically:** index updates (added, retargeted and removed references) MUST be part
    of the same atomic commit as the entity versions, state accumulator, transition record, head
    and idempotency information.
  - **Observed reads:** a query of the incoming references of an identity is an observed state read
    (a **reference read**: target plus the incoming `(entity, id, field)` references). It MUST come
    from the same snapshot position as all other reads, and be recorded for replay.
  - **Conformance:** the conformance suite MUST check that a store never lets the index diverge from
    the canonical state.
- **FR-010f — current-state facts versus history facts**:
  - **Current-state facts** are properties of the evaluated state: existence and incoming
    references.
  - **History facts** are properties of the store's history: whether an identity was ever used.
  - `referenced(id)` is a projection of the incoming-reference fact (`incoming ≠ ∅`), not an
    independent fact source.
  - Facts about the resulting state are derived from the evaluation snapshot plus ΔS, never read
    from storage.
- **FR-010g — supplied facts form a valid snapshot**: facts supplied to a plain evaluation (without
  a store) MUST be checked for internal consistency before use. A violation is the evaluation
  error `INCONSISTENT_FACTS`. Consistency means:
  - an existing identity is used;
  - every incoming reference's target exists;
  - a supplied field value is compatible with supplied existence;
  - each fact is given at most once.

  Unsoundness must not move from the store into the request.

**State, persistence and history**

- **FR-011**: Entity existence MUST be part of state content and therefore of state identity. A
  state with entities {A, B} differs from one with {A, B, C}, even if A and B are equal. Creation
  and removal MUST update the state identity at a cost proportional to the entities changed.
- **FR-012**: A created entity's first version MUST have revision 1 and be recorded at the
  creating position. A removal MUST NOT create a placeholder version (no "deleted" flag).
  Instead, the transition record states the removal with the removed entity's last content, and
  the entity is absent from later states.
- **FR-013**: Removal MUST NOT erase history. Loading an entity at any state where it existed, the
  history queries, and both replays MUST keep working after removal.
- **FR-014**: After genesis, the entity universe MUST change only through recorded transitions. The
  storage contract MUST NOT offer any way to add or remove entities outside a commit.
- **FR-015**: Data replay and behavior replay MUST reconstruct every creation and removal and
  report the first divergence, as for field updates.

**Verification**

- **FR-016**: Verification MUST check, for creating and removing actions:
  - constraint preservation for created entities;
  - invariant and constraint preservation on the resulting state;
  - postconditions;
  - evaluation errors in initial values.

  It MUST assume exactly what the runtime enforces: a created identity has never been used, and a
  removed entity exists in the evaluated state. It MUST model `exists` identically to the runtime,
  on the states where the runtime evaluates it, and check existence constraints (including those
  from `Ref<T>`) on the resulting state. Referential integrity is modelled as the semantic invariant
  "every surviving `Ref<T>` in S' points to an existing entity", independent of how the runtime
  indexes references.
- **FR-017**: Counterexamples involving creation or removal MUST be reproducible by the runtime,
  as for all counterexamples.

**Dependencies and concurrency**

- **FR-018**: Read sets and write sets MUST represent entity existence as well as fields. A creation
  writes the existence and all fields of the new entity; a removal writes the existence of the
  removed entity. Evaluating `exists(id)` reads the existence of `id`. A future entity-level conflict rule can then detect concurrent creations of the
  same identity, and concurrent removals and updates.
- **FR-019**: Under the whole-state concurrency rule of feature 005, two transitions creating the
  same identity against the same state MUST NOT both be committed.

### Key Entities

- **Entity universe**: the set of entity identities that exist in a state. It is part of state
  content.
- **Creation**: a state change `∅ → T#id` with a complete initial value; the identity comes from
  the behavior's inputs.
- **Removal**: a state change `T#id → ∅`. The entity is absent from the resulting state, and its
  history remains.
- **Entity lifetime**: the span of positions during which an identity exists, from its creation
  (or genesis) to its removal (if any). One identity has at most one lifetime.
- **Identity registry**: every identity a store has ever used. It is part of store semantics, but
  not of the state identity, and creations are checked against it.
- **Existence predicate**: `exists(Id<T>)`, true if the identity exists in the state a condition is
  checked against.
- **State read (observed fact)**: a field read (type, id, field, value), an existence read (type,
  id, exists) or a reference read (target, incoming references). Every fact that influenced a decision is recorded in its snapshot and read set.
- **Reference (`Ref<T>`)**: a typed identity field with an existence constraint (sugar over
  `Id<T>` and `exists`).
- **Reverse-reference index**: a derived store index of which `Ref` fields point at each identity.
  It accelerates the referential-integrity check, is reconstructible from state, and is not part
  of the state identity.
- **Existence dependency**: a read or write of whether an identity exists, recorded alongside
  field dependencies.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A behavior author can model open, update and close of an account (create, update,
  remove) in one behavior module and commit a full lifetime in the store. Every step is replayable
  and verified, with no host code beyond supplying identities.
- **SC-002**: In 10,000 generated histories mixing creations, updates and removals, data replay and
  behavior replay reproduce every state identity. Every single tamper of a creation or removal is
  found at its position.
- **SC-003**: In 10,000 generated concurrent-commit scenarios including creations of colliding
  identities, no identity is ever created twice, and no transition is committed on a state other
  than the one it was evaluated against.
- **SC-004**: Creating or removing one entity in a store with 100,000 entities takes time
  proportional to the entities changed, and completes in under 50 ms on the reference machine.
- **SC-005**: Verification of the example lifecycle actions finds every seeded defect (a negative
  initial balance, a removal breaking a `Ref<T>` existence constraint) with a runtime-confirmed
  counterexample, and proves the fixed versions.
- **SC-006**: Every existing module and history (features 001–005) keeps its behavior: modules
  without lifecycle effects keep their identities, and all existing tests pass unchanged, except
  where this feature deliberately extends a document format.
- **SC-007**: No identity is ever created twice in a store's history. In generated histories
  containing removals followed by attempted re-creations, every re-creation is refused with
  `ENTITY_ID_ALREADY_USED`.

## Assumptions

- **Identities come from the host.** It uses UUIDs, ULIDs or domain keys, obtained before
  evaluation and recorded as canonical input. Replay never generates identities.
- **Removal is logical.** Physical deletion of history or audit data, and garbage collection of
  historical versions, are out of scope.
- **Bulk import beyond recorded creating actions is out of scope.** Imports are ordinary
  creating actions.
- **Concurrency stays whole-state (feature 005).** Entity-level concurrency remains a follow-up;
  this feature only records the existence dependencies it will need.
- **Actions bind existing entities as state parameters, as today.** The new identity of a creation
  is an input, not a state binding.
- **Formats change deliberately.** The evidence-policy and governance formats from features 002
  and 005 are unchanged. **Any use of a semantic form introduced by this feature requires wire IR
  0.5**, even if no action ever creates or removes anything. The forms are `create`, `remove`,
  `exists`, `referenced` and `Ref<T>`, plus lifecycle or evaluation facts. Modules that use none of
  them stay valid as 0.4 and keep their semantic identity.
