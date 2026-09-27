---

description: "Task list for feature 006: entity lifecycle (universe transitions)"
---

# Tasks: Entity Lifecycle (Universe Transitions)

**Input**: Design documents from `/specs/006-entity-lifecycle/`

**Prerequisites**: plan.md, spec.md (FR-001–FR-019, FR-010a–g, FR-010a2), research.md (R1–R14), data-model.md,
contracts/lifecycle-api.md, quickstart.md

**Tests**: required by the constitution (principle III). Test tasks come before the implementation
that makes them pass and are seen failing first. **Existing identities, goldens and persistence
documents must stay byte-identical** (SC-006); every task that touches formats re-runs the frozen
checks.

**Organization**:
- US1: create (P1);
- US2: remove (P1);
- US3: existence, references and verification (P2);
- US4: history, replay and concurrency (P2).

The Python DSL and CLI work comes in Polish, after the engine semantics are complete.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Add the lifecycle fixture module to `tests/fixtures/wire/build_fixtures.py`, written to
  `tests/fixtures/wire/valid/accounts.json` (wire `0.5`):
  - **Entities:** `Customer{name: String}`, `Account{owner: Ref<Customer>, balance: Money (scale
    2)}`, and `AuditNote{about: Id<Customer>, text: String}` (a plain identity).
  - **Constraint:** `balance >= 0`.
  - **Actions:**
    - `open_account(owner: state Customer; account_id: input Id<Account>, initial: input
      Money)`: `requires(initial >= 0)`, then `create(Account, account_id, {owner: owner.id,
      balance: initial})`;
    - `open_account_unchecked`: the same without the precondition;
    - `register_customer(customer_id: input Id<Customer>, name: input String)`: `create`;
    - `deposit`;
    - `close_account(account)`: `requires(balance == 0)`, then `remove`;
    - `remove_customer(customer)`: `requires(not referenced(customer.id))`, then `remove`;
    - `remove_customer_unchecked`;
    - `switch_and_remove(account, old, new: state)`: `account.owner := new.id; remove(old)`;
    - `check_exists(note: state AuditNote)`: `requires(exists(note.about))`;
    - `create_twice(a, b: input Id<Account>, owner)`: two creations.
  - **Invalid fixtures:**
    - `create_incomplete` (`CREATE_INCOMPLETE`);
    - `create_wrong_id_type` (`TYPE_MISMATCH`);
    - `lifecycle_in_0_4` (`UNSUPPORTED_IR_VERSION`);
    - `remove_non_state` (`TYPE_MISMATCH`);
    - `remove_and_update` (`LIFECYCLE_CONFLICT`);
    - `create_same_input_twice` (`LIFECYCLE_CONFLICT`).

  Regenerate the fixtures.
- [X] T002 [P] Create `tests/fixtures/requests/006/` (plain-evaluation requests with `facts`
  sections, and `expectations.json` in the subset format of feature 004), and add a "Feature 006"
  section to `tests/fixtures/README.md`

---

## Phase 2: Foundational (wire 0.5, semantic forms, evaluation facts)

**Purpose**: the IR forms, hashing, the version boundary and the facts interface every story
needs. No story work before this phase.

### Tests (write first, must fail)

- [X] T003 [P] Write `crates/behavior-core/tests/ir_0_5.rs`:
  - **Frozen identities:** every module in `tests/fixtures/frozen_versions.json` and
    `migration_004.json` keeps its identity, and all goldens and hash vectors are unchanged
    (SC-006).
  - **The version boundary:** a 0.4 document with any 006 form is `UNSUPPORTED_IR_VERSION`, for
    each form: `create`, `remove`, `exists`, `referenced`, `ref`. A module with a 006 form
    serializes as `0.5`, and one without serializes as `0.4`.
  - **Round trip:** `accounts.json` round-trips through serialization with an equal behavior
    version.
  - **Hashing:** the `Ref` flag and the new nodes change the hash; renaming keeps item hashes.
- [X] T004 [P] Write `crates/behavior-core/tests/facts.rs`, covering the facts interface through
  plain evaluation with request `facts` sections (FR-010c, FR-010f, FR-010g):
  - `check_exists` records exactly one existence read;
  - a short-circuited `exists` records nothing;
  - a missing fact gives `UNKNOWN_FACT`;
  - inconsistent supplied facts give `INCONSISTENT_FACTS`: `exists` with `used=false`, an incoming
    reference to a non-existing target, a bound entity marked absent, a duplicated fact;
  - `referenced` is the projection of incoming references;
  - records with facts are `record_version "0.5"`, and records without are byte-identical to
    before.

### Implementation

- [X] T005 Extend `crates/behavior-core/src/wire.rs` and `crates/behavior-core/src/serialize.rs`:
  - `IR_VERSION_LIFECYCLE = "0.5"`;
  - decoding of `{"t":"ref","entity":T}`, `create` and `remove` effects, and the `exists` and
    `referenced` ops;
  - accept 0.4 and 0.5, and reject 006 forms in 0.4 with a message naming the form;
  - serialize the minimal version;
  - add `schema/wire-ir-0.5.schema.json`.
- [X] T006 Extend the semantic IR:
  - **Semantic types** in `crates/behavior-core/src/semantic/{types,expr,module}.rs`: a reference
    flag on `Id` fields, `ExprKind::Exists` and `ExprKind::Referenced`, and `Effect` as an enum
    `SetField | Create{entity, id, fields} | Remove{param}`.
  - **Resolution** in `crates/behavior-core/src/admit/resolve.rs`: `Ref<T>` becomes an `Id<T>`
    field with the flag, plus a synthesized entity constraint `exists(field)` (optional
    references: `is_none or exists`).
  - **Hash codes** in `crates/behavior-core/src/admit/hash.rs`: for the new nodes and the flag.
    Modules without 006 forms must hash exactly as before.

  Make T003 pass.
- [X] T007 Implement `crates/behavior-core/src/facts.rs`:
  - the `EvaluationFacts` trait: `exists` and `incoming` (current state), `used` (history);
  - `RefEdge`;
  - observed-fact collection (existence, identity and reference reads);
  - parsing of the request `facts` section and the snapshot consistency check (FR-010g);
  - the record `facts` section, in canonical order.

  Also in this task:
  - Extend `crates/behavior-core/src/eval.rs` with `evaluate_with(module, request, facts)`. It
    evaluates `exists` and `referenced` on S (preconditions, incoming rules) through the
    interface, lazily, and records only observed facts.
  - `evaluate` and `evaluate_observed` use the request's facts. `ObservedReads` gains existence,
    identity and reference reads.
  - Records carrying `facts` are record `0.5`.

  Make T004 pass.

**Checkpoint**: 006 forms parse, resolve and hash; facts are observed; existing bytes are unchanged.

---

## Phase 3: User Story 1 - Create an entity (Priority: P1) 🎯 MVP

**Goal**: `create` with a complete initial value and a host-supplied typed identity, checked against
the identity registry, committed at revision 1.

**Independent Test**:
- `open_account(id=a42, balance=100.00)` commits, and `Account#a42` exists at revision 1.
- Opening `a42` again gives `ENTITY_ID_ALREADY_USED`.
- A negative initial balance is refused by the constraint.
- An incomplete creation is refused at admission.

### Tests (write first, must fail)

- [X] T008 [P] [US1] Write `crates/behavior-core/tests/lifecycle_create.rs`:
  - **Admission:** the invalid fixtures of T001 produce their codes.
  - **Evaluation with facts:**
    - `used=false` gives `ALLOW`, with a `lifecycle` create entry carrying the complete value;
    - `used=true` gives the result `ENTITY_ID_ALREADY_USED`, naming the identity;
    - `create_twice` with equal runtime identities gives `LIFECYCLE_CONFLICT`;
    - a negative initial balance gives `DENY` (constraint on the created entity, on S');
    - postconditions see the created entity (`exists` on S' is true).
  - **Record:** it is 0.5, and replay matches.
- [X] T009 [P] [US1] Write `crates/behavior-store/tests/lifecycle_create.rs`:
  - `Store::evaluate` answers `used` from the backend;
  - a commit creates version 1 at the new position, and the state identity inserts the new content
    hash (checked against a fresh MuHash);
  - `load` at the new state works, and at the parent it is `ENTITY_NOT_FOUND`;
  - an identity used in the genesis gives `ENTITY_ID_ALREADY_USED`;
  - the record's `created` field is present, and 005-style records without lifecycle keep their
    hash (store hash vectors unchanged);
  - the registry is keyed by type name and id, not the declaration hash;
  - an `exists` read made during a store evaluation appears in the bundle's `read_set.existence`,
    with its value, and is re-checked at commit.

### Implementation

- [X] T010 [US1] Admission in `crates/behavior-core/src/admit/typecheck.rs` (research R2):
  - **Complete initial value:** a creation's fields must be complete, known and well-typed
    (`CREATE_INCOMPLETE`, `UNKNOWN_FIELD`, `TYPE_MISMATCH`).
  - **Typed identity:** the identity expression has type `Id<T>`.
  - **Static lifecycle conflicts** (`LIFECYCLE_CONFLICT`): the same input identity twice, update
    plus remove of a parameter, removing twice.
- [X] T011 [US1] Evaluation of creations in `crates/behavior-core/src/eval.rs` (research R4, R5):
  - read `used` through the facts interface, giving `ENTITY_ID_ALREADY_USED`;
  - allow at most one lifecycle operation per typed identity (`LIFECYCLE_CONFLICT`);
  - build the created entity from the evaluated field expressions;
  - check constraints and invariants on S', with the created entity included;
  - compute `exists'` for postconditions;
  - write the record's `lifecycle` entry.

  Make T008 pass.
- [X] T012 [US1] Persistence of creations:
  - **Backend** in `crates/behavior-store/src/lib.rs`: `used_at(key, position)` (default via
    `version_at`), plus the commit parameters for creations. Also update `InMemoryBackend` in
    `crates/behavior-store/src/memory.rs`.
  - **Store** in `crates/behavior-store/src/store.rs`: an `EvaluationFacts` provider over the
    backend as of the evaluated position. The commit re-derives the facts and checks them
    (`BUNDLE_INVALID` on a mismatch), re-checks `used` at the parent, builds the created versions
    (revision 1, `created_at` = position), and updates the accumulator.
  - **Documents** in `crates/behavior-store/src/documents.rs`: optional fields
    `read_set.existence`, `read_set.identities`, `write_set.lifecycle` and
    `TransitionRecord.created`, omitted when empty. Existence reads observed through the store
    provider go into `read_set.existence` (FR-018).

  Make T009 pass.

**Checkpoint**: entities can be created through actions and stores (MVP).

---

## Phase 4: User Story 2 - Remove an entity (Priority: P1)

**Goal**: `remove(p)` makes the entity absent from S' and keeps its history; removed identities
are never reused.

**Independent Test**:
- Create, then close an account. It is absent now, `load` at the earlier state returns its last
  version, and both replays reconstruct the removal.
- Re-creating it gives `ENTITY_ID_ALREADY_USED`.

### Tests (write first, must fail)

- [X] T013 [P] [US2] Write `crates/behavior-core/tests/lifecycle_remove.rs`:
  - `close_account` gives a `lifecycle` remove entry with the last value;
  - `exists` on S' is false;
  - `remove_and_update` is refused at admission;
  - removing a bound entity is always valid by binding;
  - replay matches.
- [X] T014 [P] [US2] Write `crates/behavior-store/tests/lifecycle_remove.rs`:
  - a commit of a removal: the accumulator removes the last content hash, and `removed_at` is set;
  - `load` of the removed entity at the new state is `ENTITY_NOT_FOUND`, and at earlier states it
    returns the old versions;
  - re-creation after removal gives `ENTITY_ID_ALREADY_USED` (SC-007 unit case);
  - no placeholder version is written;
  - the record's `removed` field is present.

### Implementation

- [X] T015 [US2] Evaluation of removals in `crates/behavior-core/src/eval.rs`:
  - the removal entry carries the removed entity's last value;
  - removed entities are excluded from outgoing single-entity rules on S';
  - `exists'` for the removed identity is false.

  Make T013 pass.
- [X] T016 [US2] Persistence of removals:
  - **Backend:** `removed_at(key)`, and removals in the atomic commit. Implement it in
    `InMemoryBackend`.
  - **Store:** `exists_at` = `version_at` plus not removed. The commit checks that removed
    entities exist at the parent (`ENTITY_NOT_FOUND`) and removes their content hash from the
    accumulator.
  - **Document:** `TransitionRecord.removed` (optional).
  - **`load`:** respects removals.

  Make T014 pass.

**Checkpoint**: the full lifetime (create, update, remove) works in the store.

---

## Phase 5: User Story 3 - Existence, references and verification (Priority: P2)

**Goal**: `Ref<T>` integrity checked on S' via the derived reverse-reference index; `exists` and
`referenced` evaluated on the right state; verifier and runtime share lifecycle semantics.

**Independent Test**:
- `remove_customer` while an account references the customer gives `DANGLING_REFERENCE`.
- `switch_and_remove` is valid.
- `AuditNote` identities block nothing.
- Verification finds the seeded defects in `open_account_unchecked` and
  `remove_customer_unchecked` with runtime-confirmed counterexamples, and proves the guarded
  versions.

### Tests (write first, must fail)

- [X] T017 [P] [US3] Write `crates/behavior-core/tests/references.rs` (plain evaluation with
  facts):
  - **Dangling references:** `remove_customer_unchecked` with a surviving incoming reference gives
    `DENY` with `DANGLING_REFERENCE`, listing the references. With no incoming references it gives
    `ALLOW`.
  - **Retarget and remove:** `switch_and_remove` gives `ALLOW` (integrity on S').
  - **Created references:** creating an account whose `owner` does not exist gives `DENY` (the
    synthesized constraint, via `exists'`). Creating a customer and an account referencing it in
    one transition gives `ALLOW` (a helper action in the test module).
  - **Plain identities:** `AuditNote.about` never blocks a removal.
  - **`referenced`:** in preconditions it reads S, and in postconditions it sees `incoming'`.
- [X] T018 [P] [US3] Write `crates/behavior-store/tests/references.rs`:
  - a genesis whose seed has a `Ref` to a non-seed identity is refused (`GENESIS_INVALID`);
  - `incoming_at` as of a position;
  - index edges added and dropped by creations, retargets and removals;
  - integrity on S' at commit;
  - a reference read is recorded and re-checked at commit.
- [X] T019 [P] [US3] Create the verification fixture `tests/fixtures/verify/lifecycle.json` (built
  from `accounts.json`) and its `.expected.json`:
  - `open_account_unchecked`: a counterexample (constraint);
  - `remove_customer_unchecked`: a counterexample (`referential_integrity`);
  - `open_account` and `remove_customer`: proven;
  - `switch_and_remove`: proven.

  Then write `crates/behavior-verify/tests/lifecycle.rs`: the outcomes match, every counterexample
  is confirmed by plain evaluation with its `facts` section, and nothing is inconclusive (SC-005).

### Implementation

- [X] T020 [US3] Referential integrity in `crates/behavior-core/src/eval.rs` (research R6):
  - read `incoming(x)` for removed identities, as observed reference reads;
  - compute `incoming'` (removed sources, retargeted fields, created references);
  - `DANGLING_REFERENCE` with the surviving references;
  - `exists'` and `referenced'` for postconditions and outgoing rules.

  Make T017 pass.
- [X] T021 [US3] The derived reverse-reference index in the store (research R9):
  - **Backend:** `incoming_at(target, position)`, stored as edge events `{target, source, field,
    added_at, dropped_at?}` (with `dropped_at` set once, atomically), plus `ref_changes` in the
    commit. Implement it in `InMemoryBackend`.
  - **Genesis:** validate that every seed `Ref` field points at a seed entity (`GENESIS_INVALID`),
    then derive the genesis index from the seed.
  - **Store:** the store-side facts provider answers `incoming`. The commit computes the edge
    changes and checks integrity on S' against `incoming_at` at the parent.
  - **Documents:** `TransitionRecord.ref_changes` and `read_set.references` (optional).

  Make T018 pass.
- [X] T022 [US3] Verifier support in `crates/behavior-verify/src/encode.rs` and `checks.rs`
  (research R12):
  - **Uninterpreted functions** per entity type: `ex_T`, `used_T` and `refd_T`, with the relations
    `ex→used`, bound identities existing and used, and `refd→ex`.
  - **Creation:** assumes `¬used_T` and distinctness; checks constraints and invariants on the new
    entity and postconditions on S'.
  - **Removal:** `ex'`. The integrity check (new check kind `referential_integrity`, blocking)
    covers `refd_T`, plus bound and created surviving references.
  - **Predicates:** `exists` and `referenced` on S and S'.
  - **Counterexamples:** extract concrete facts into a request `facts` section so confirmation
    reproduces them.

  Make T019 pass.

**Checkpoint**: lifecycle is verified; integrity is exact and checked on S'.

---

## Phase 6: User Story 4 - History, replay and concurrency (Priority: P2)

**Goal**: every universe change replays; the index never diverges; colliding creations never both
commit; no backend path changes the universe outside a commit.

**Independent Test**:
- Mixed histories replay both ways.
- Tampering with a creation, removal or reference change is found at its position.
- The new conformance cases pass for the reference backend and a Python dict backend, and mutants
  fail their cases.

### Tests (write first, must fail)

- [X] T023 [P] [US4] Write `crates/behavior-store/tests/lifecycle_replay.rs`:
  - **Clean replay:** data replay and behavior replay over a generated history of 1,000 mixed
    transitions (creations, updates, removals, retargets).
  - **Tamper proptest (SC-002):** tampering with a created value, a removal, a `ref_change`, a
    recorded fact, or the registry (a record that re-creates a used identity). Each is found at its
    position with its kind (`lifecycle`, `references`, `decision`, `state`, `chain`).
  - **At scale:** an `#[ignore]` variant with 10,000 transitions.
- [X] T024 [P] [US4] Write `crates/behavior-store/tests/lifecycle_props.rs`:
  - **SC-003:** proptest interleavings with colliding creations from several hosts; no identity
    is ever created twice, and `evaluated_against == committed_on`.
  - **SC-007:** removals followed by re-creation attempts are always refused.

  Run 10,000 cases in an `#[ignore]` release variant.
- [X] T025 [P] [US4] Extend `crates/behavior-store/tests/conformance.rs` with the 7 new cases of
  contracts/lifecycle-api.md:
  - `create_and_remove_entity`, which also asserts FR-014: the universe changes only through
    `commit`. After creations and removals, every entity key a backend returns is justified by a
    genesis seed or a recorded creation, and every absence by a recorded removal. No other backend
    method writes;
  - `identity_never_reused`;
  - `removed_entity_history`;
  - `referential_integrity_on_resulting_state`;
  - `reference_index_consistency`;
  - `concurrent_creation_same_identity`;
  - `existence_snapshot`.

  Add mutants that fail their designated case:
  - `ForgetsRemovedIdentities` (`used_at` false after removal) → `identity_never_reused`;
  - `DeletesVersionsOnRemove` → `removed_entity_history`;
  - `StaleIndex` (the index is not updated on retarget) → `reference_index_consistency`;
  - `CurrentOnlyIndex` (`incoming_at` ignores the position) → `existence_snapshot`.

### Implementation

- [X] T026 [US4] Replay (research R11) in `crates/behavior-store/src/replay.rs`:
  - **Data replay** applies `created`, `removed` and `ref_changes` with the write sets. It checks
    the registry (no identity created twice), that removed entities existed, the rebuilt index
    against `incoming_at` at every position with reference changes, and the state identities.
  - **Behavior replay** uses the recorded facts as the facts provider, and checks them against
    the store at the parent.
  - **New divergence kinds:** `lifecycle` and `references`.

  Make T023 pass.
- [X] T027 [US4] Conformance cases in `crates/behavior-store/src/conformance.rs`. Implement the 7
  cases, generic over any backend, using the built-in `accounts` module (`include_str!`), and keep
  the existing 17. Make T024 and T025 pass.

**Checkpoint**: lifecycle history is replayable, and backends are conformance-checked.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T028 [P] Python DSL and binding:
  - **DSL** in `python/behavior/`: `create(T, id=..., **fields)`, `remove(p)`, `exists(e)`,
    `referenced(e)`, `Ref[T]`; `evaluate(..., facts={...})`; the exceptions and results for
    `ENTITY_ID_ALREADY_USED`, `DANGLING_REFERENCE`, `UNKNOWN_FACT` and `INCONSISTENT_FACTS`.
  - **Binding** in `crates/behavior-py/src/lib.rs` (builder nodes for the new forms, Python
    backends with `removed_at` / `incoming_at` / optional `used_at`, and commit arguments) and in
    `python/behavior/_engine.pyi`.
  - **Tests:** `python/tests/test_lifecycle.py`, and the new cases in
    `python/tests/test_store_conformance.py` for `DictBackend`.
- [X] T029 [P] CLI: `behavior eval` and `behavior verify` accept 0.5 modules and requests with
  `facts`. Add tests in `crates/behavior-cli/tests/cli_lifecycle.rs`
- [X] T030 [P] Add an ignored perf test, `crates/behavior-store/tests/lifecycle_perf.rs` (SC-004):
  creating and removing one entity in a store with 100,000 entities takes under 50 ms in release
  mode, and within 2× of a store with 1,000 entities
- [X] T031 [P] Add the example `examples/accounts/` (`behavior.py`, `run.py`), following
  quickstart §2, with a README section
- [X] T032 [P] Extend `scripts/determinism-check.sh`:
  - a fixed lifecycle history built twice (the `behavior-store` example `lifecycle_history`), byte
    for byte;
  - the `requests/006` evaluations and replays.
- [X] T033 [P] Documentation:
  - `docs/persistence.md`: universe transitions, registry, derived index, the evaluation
    snapshot, and the trust boundary of existence facts;
  - `docs/verification.md`: lifecycle checks;
  - the README language section.
- [X] T034 Run all gates (fmt, clippy `-D warnings`, `cargo test --workspace`, `pytest`, `mypy`,
  the determinism check, the ignored release tests) and fix all findings
- [X] T035 Write `specs/006-entity-lifecycle/checklists/implementation-review.md`: a review
  against FR/SC, the constitution and the five principles, with deviations and follow-ups. Include
  an explicit FR-014 item: the `Backend` trait offers no write besides `create` (genesis) and
  `commit`, and no host path adds or removes entities outside a recorded transition
- [X] T036 Run quickstart.md §1–§4 and fix any failures

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T007)** → stories.
- **US1 (T008–T012)** needs the foundational phase. It is the MVP.
- **US2 (T013–T016)** needs US1 (to create before removing).
- **US3 (T017–T022)** needs US1 and US2.
- **US4 (T023–T027)** needs US1–US3.
- **Polish** comes after the stories. T028 (Python DSL) should start as soon as US3 is done, in
  parallel with US4, so the main authoring surface (SC-001) is not last.
- **Shared files:**
  - `eval.rs`: T007 → T011 → T015 → T020;
  - `store.rs`: T012 → T016 → T021;
  - `lib.rs` / `memory.rs` of `behavior-store`: T012 → T016 → T021;
  - `encode.rs`: T022 only.

## Parallel Opportunities

- Foundational tests T003 and T004.
- Test pairs: T008/T009, T013/T014, T017/T018/T019, T023/T024/T025.
- Polish: T028–T033.

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 (creation through actions and stores).
2. **Add US2**: the full lifetime, with removal that keeps history.
3. **Add US3**: explicit referential integrity and verification.
4. **Add US4**: replay and conformance for lifecycle history.
5. **Polish**: Python, CLI, performance, example, determinism, docs, review.
