---

description: "Task list for feature 005: persistence contract"
---

# Tasks: Persistence Contract

**Input**: Design documents from `/specs/005-persistence-contract/`

**Prerequisites**: plan.md, spec.md (FR-001–FR-023), research.md (R1–R12), data-model.md,
contracts/store-api.md, quickstart.md

**Tests**: required by the constitution (principle III). Test tasks come before the implementation
that makes them pass and are seen failing first. The conformance cases and the broken backends are
tests too: each broken backend must fail its designated case.

**Organization** (US1 and US2 share P1; US1 first, because replay needs committed history):
- US1: guarded commit (P1);
- US2: replay and audit (P1);
- US3: conformance for any storage, including Python (P2);
- US4: evidence bound to commits (P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1, US2, US3, US4

---

## Phase 1: Setup

- [X] T001 Create the crate `crates/behavior-store` (`Cargo.toml`: `behavior-core`, `behavior-verify`, `serde`, `serde_json`, `sha2`, `num-bigint`, `num-traits`, `thiserror`; dev: `proptest`, `pretty_assertions`; `[lints] workspace = true`; `#![forbid(unsafe_code)]`). Add it to the workspace `members` and `[workspace.dependencies]` in `Cargo.toml`, with empty modules `documents`, `muhash`, `store`, `replay`, `memory`, `conformance` in `crates/behavior-store/src/lib.rs`
- [X] T002 [P] Create `tests/fixtures/store/` with a README section "Feature 005" in `tests/fixtures/README.md`. The section lists the hashing vectors, the ledger module, the genesis documents and the recorded histories added by later tasks
- [X] T003 [P] Add the ledger fixture module to `tests/fixtures/wire/build_fixtures.py`, written to `tests/fixtures/wire/valid/ledger.json`:
  - `Money` (scale 2) and `Account{active: bool, balance: Money}`;
  - entity constraint `balance >= Money(0)`;
  - derived `available(account) = balance`;
  - action `transfer(from_, to: state; amount: input Money)` with `requires(amount > 0)`,
    `requires(from_.active and available(from_) >= amount)`, and effects moving `amount`;
  - action `touch(account: state)` with effect `account.active := account.active` (a no-op
    transition);
  - action `freeze(account: state)` setting `active := false`.

  Regenerate the fixture

---

## Phase 2: Foundational (documents, identity, backend, observed reads)

**Purpose**: the canonical documents, the state identity, the backend trait and the engine's
observed-read collector. Every story needs them. No story work before this phase.

### Tests (write first, must fail)

- [X] T004 [P] Write `crates/behavior-store/tests/documents.rs` covering:
  - **Canonical round-trips and hashing vectors** for every document of data-model.md, using the
    tags `behavior.entity_content.v1`, `behavior.entity_version.v1`, `behavior.state.v1`,
    `behavior.evidence_policy.v1`, `behavior.store_genesis.v1`, `behavior.commit_bundle.v1` and
    `behavior.transition_record.v1`. The expected hashes are frozen in
    `tests/fixtures/store/hash_vectors.json` (filled once with `BLESS_STORE_VECTORS=1`).
  - **Validation**: unknown fields are refused; the evidence policy's `require` is `"none"` or
    `"commit_authorization"`; `commit_time` is RFC 3339 UTC.
  - **The content hash** binds `{entity, declaration, id, value}` and **not** the revision or
    created position: two versions with equal content and different revisions have the same
    content hash.
- [X] T005 [P] Write `crates/behavior-store/tests/muhash.rs` covering:
  - the empty set is accumulator 1;
  - order independence (proptest over permutations);
  - add-then-remove restores the identity;
  - a numerator/denominator head normalizes to the same `StateId` as a fresh computation;
  - swapping values between two ids changes the identity;
  - equal content at different revisions gives an equal identity (FR-002).

  Use `p = 2^3072 − 1103717`, with SHA-256 counter-mode expansion to 384 bytes (research R3)
- [X] T006 [P] Write `crates/behavior-core/tests/observed_reads.rs` for the new observed-read API
  (research R7). It must record every `param.field` actually evaluated, including inside derived
  bodies, and again on memo hits. Cases:
  - `from_.active and available(from_) >= amount` with `active = false` observes `from_.active`
    and not `from_.balance` (short-circuit);
  - with `active = true` it observes both, `balance` through the derived value;
  - reads in invariants and constraints are observed;
  - the decision record bytes are identical with and without the collector (all goldens
    unchanged).

### Implementation

- [X] T007 Implement the observed-read collector in `crates/behavior-core/src/eval.rs`:
  - a whole-evaluation set of `(param, field)` with a separate collector for derived bodies;
  - memo entries keep the reads of their first evaluation and re-attribute them on each hit;
  - short-circuited operands are not evaluated, so they are not read.

  Expose it as `behavior_core::evaluate_observed(module, request) -> (DecisionRecord,
  BTreeSet<(String, String)>)` in `crates/behavior-core/src/lib.rs`, with `evaluate` unchanged.
  Make T006 pass and keep every existing golden byte-identical
- [X] T008 Implement `crates/behavior-store/src/documents.rs`:
  - typed structs for `EntityKey`, `EntityContent`, `EntityVersion` (`content_hash`, `entity`,
    `id`, `revision ≥ 1`, `created_at`, `value`), `StateRef` (`state`, `position`), `Head`
    (`state_ref`, `acc_num`, `acc_den`, `last_record`), `EvidencePolicy`, `Genesis`,
    `CommitBundle`, `TransitionRecord` and `ReplayReport`;
  - canonical serialization through `behavior_core::canonical`, and hashing with
    `behavior_verify::hashing::document_hash` and the tags above;
  - strict decoding that refuses unknown fields;
  - the `data_version` string `store:<genesis hash>;state:<state id>;position:<n>` (FR-019);
  - typed `StoreError` codes from data-model.md → Store results and errors: `STATE_CONFLICT`,
    `NOTHING_TO_COMMIT`, `ENTITY_NOT_FOUND`, `ENTITY_DECLARATION_MISMATCH`, `BUNDLE_INVALID`,
    `EVIDENCE_REQUIRED`, `EVIDENCE_MISMATCH`, `ENTITY_UNIVERSE_CHANGED` and `BACKEND_ERROR`.

  Make T004 pass
- [X] T009 Implement `crates/behavior-store/src/muhash.rs`: `Accumulator { num, den }` with
  `insert(content_hash)`, `remove(content_hash)`, and `state_id()`, which normalizes with one
  modular inverse (`num-bigint` `modinv`; fall back to the extended GCD from `num-integer`).
  `state_id` = tag `behavior.state.v1` over 384 big-endian bytes. Make T005 pass
- [X] T010 Define `Backend` and `CasOutcome` in `crates/behavior-store/src/lib.rs`, exactly as in
  contracts/store-api.md: `genesis`, `head`, `create`, `version_at(key, position)` (newest version
  with `created_at ≤ position`), `version(key, revision)`, `record(position)`, and
  `commit(expected_last_record, versions, record, new_head)`, which is atomic and crash-safe.
  Implement `InMemoryBackend` in `crates/behavior-store/src/memory.rs`: `BTreeMap` storage, a
  single commit that writes everything or nothing, and fault injection hooks for tests (fail
  before the write; fail after the write but before the acknowledgement)

**Checkpoint**: documents, identity, backend and observed reads are in place; stories can start.

---

## Phase 3: User Story 1 - Guarded commit (Priority: P1) 🎯 MVP

**Goal**: evaluate against one consistent snapshot, get a bundle for allowed decisions, and commit
it atomically with whole-state optimistic concurrency and idempotency.

**Independent Test**: with the ledger module in the in-memory store, one transfer commits (new
state, revisions + 1). Of two transfers evaluated against the same state, the second commit is
`STATE_CONFLICT`, and the store is unchanged by the refusal.

### Tests (write first, must fail)

- [X] T011 [P] [US1] Write `crates/behavior-store/tests/commit.rs`:
  - **Genesis**: `Store::create` with a seed of 3 accounts gives S0 at position 0 with the
    MuHash identity. The seed is validated (unknown fields, off-grid Money, a duplicate key and a
    constraint violation are refused).
  - **Commit**: `evaluate("transfer", bindings {from_: a1, to: a2})` returns a record with
    `data_version` = `store:…;state:…;position:0` and a bundle. Committing it gives position 1:
    revisions a1:2 and a2:2, `result_state` equal to a fresh MuHash of the new content, the record
    chained to the genesis, and `evaluated_against == committed_on`.
  - **Conflict**: two transfers evaluated at position 1 (disjoint accounts too). The second
    commit is `STATE_CONFLICT` with the current head and the changed keys, and the head, versions
    and records are unchanged.
  - **Denied decisions**: a denied transfer has `bundle = None`, and committing a hand-made bundle
    with `result ≠ ALLOW` is `NOTHING_TO_COMMIT`.
  - **No-op transition**: `touch` keeps the `StateId` and advances the position.
  - **Returning content**: returning to earlier content gives the same `StateId` at a new position
    (FR-002).
  - **Idempotency**: resubmitting the last transition, even with a new `commit_time`, returns
    `already = true` without a new record (FR-009).
  - **Late retry**: commit T1 at position p, let another host commit T2, then resubmit T1. The
    result is `already = true` with T1's record, not `STATE_CONFLICT`, and nothing is applied
    twice.
  - **Parent mismatch**: `expected_parent ≠ bundle.evaluated_state` is `BUNDLE_INVALID`.
  - **Context snapshot**: the record's `context` equals the supplied canonical context, all
    fields included (FR-022).
  - **Crash retry**: injected crash before the write means the retry applies once; injected crash
    after the durable write but before the acknowledgement means the retry returns
    `already = true`, also when another commit landed before the retry (FR-006).
  - **Snapshot consistency**: a commit landing between two `version_at` reads of an evaluation
    does not mix states (FR-018).
  - **Invalid bundles**: `BUNDLE_INVALID` for an altered read set, write set, transition hash or
    old value; `ENTITY_UNIVERSE_CHANGED` for a write to an unknown id;
    `ENTITY_DECLARATION_MISMATCH` for a module declaring `Account` differently.
- [X] T012 [P] [US1] Write `crates/behavior-store/tests/concurrency_props.rs` (SC-001). A proptest
  with 10,000 generated interleavings of evaluate/commit from up to 4 simulated hosts over the
  ledger store asserts that:
  - every committed record satisfies `evaluated_against == committed_on == head before the
    commit`;
  - no partial commit is ever observable;
  - the final state equals the data replay of the committed records.

### Implementation

- [X] T013 [US1] Implement `Store::create`, `open`, `current` and `load` in
  `crates/behavior-store/src/store.rs`:
  - genesis validation: seed values decoded through the module's field types, entity constraints
    checked, and declarations taken from the module's name table;
  - the S0 accumulator;
  - `load(key, at)` via `version_at(key, at.position)`, checking `at` against the history.
- [X] T014 [US1] Implement `Store::evaluate` in `crates/behavior-store/src/store.rs`:
  1. Read the head once (position n).
  2. Load every bound state entity with `version_at(key, n)` (`ENTITY_NOT_FOUND` if missing).
  3. Build the standard request: state from the store; input and context host-supplied;
     `data_version` per FR-019.
  4. Run `evaluate_observed`.
  5. For `ALLOW`, build the bundle: `evaluated_state`, `store`, `behavior_version`, `record`,
     `transition_hash` (`behavior.transition.v1`), the touched `entity_declarations`, a `read_set`
     of observed fields grouped per entity with its revision at n, a `write_set` from `changes`,
     `commit_time`, and optional `evidence`.
- [X] T015 [US1] Implement `Store::commit` in `crates/behavior-store/src/store.rs`, in the order of
  research R8:
  1. **Idempotency**: for a bundle evaluated at position p, if `record(p+1)` exists with the same
     transition hash, return `already` with that record's result, whatever the head is now. This
     runs before the conflict check (FR-009).
  2. **Consistency and conflict**: `expected_parent ≠ bundle.evaluated_state` is
     `BUNDLE_INVALID`. Otherwise, a parent that is not the head is `STATE_CONFLICT`, with changed
     keys from the records after the parent.
  3. **Validation**:
     - re-evaluate the record with `evaluate_observed` and compare the read set and write set;
     - check revisions and old values against the parent;
     - check declarations;
     - check that the entity universe is unchanged (FR-021).
  4. **Build**: the new versions (revision + 1 only where the canonical value changes;
     `created_at` = new position), the accumulator update, and the transition record
     (`previous_record`, `bundle_hash`, `evaluated_against = committed_on`, `result_state`,
     `evidence_policy`, `authorization`).
  5. **Commit**: the backend compare-and-set. `HeadMoved` becomes `STATE_CONFLICT`.

  Make T011 and T012 pass

**Checkpoint**: a working store with guarded, atomic, idempotent commits (MVP).

---

## Phase 4: User Story 2 - Reproduce and audit the history (Priority: P1)

**Goal**: history queries, data replay and behavior replay, independent of storage, that report
the first divergence.

**Independent Test**: build 1,000 committed transitions, and replay both ways (ok). Tampering with
one change, one input and one state identity is each reported at the right position with its kind.

### Tests (write first, must fail)

- [X] T016 [P] [US2] Write `crates/behavior-store/tests/replay.rs`:
  - **Clean history**: data and behavior replay over a generated 1,000-transition ledger history
    report `ok` with `checked = 1000`.
  - **Tamper proptest (SC-002)**: 1,000-transition histories in the default run, plus an
    `#[ignore]` test at 10,000 transitions, run in release mode by T030, with one tamper per case: a write-set value, an input, a context field, a
    `result_state`, a `behavior_version`, a swapped record order, a dropped record, or a record
    with `committed_on ≠ evaluated_against`. Each is reported at the first affected position with
    kind `changes`, `record`, `decision`, `state`, `chain` or `invariant`. Tampering is done
    through a test backend wrapper that alters what it returns.
  - **Snapshots**: `verify_snapshot` confirms a correct snapshot at position k and reports a wrong
    one without changing the store (FR-012).
  - **History queries**: `transitions(from, to)` returns exactly the records in order.

### Implementation

- [X] T017 [US2] Implement `Store::transitions` and `replay_data` in
  `crates/behavior-store/src/replay.rs` (research R10). Data replay walks from the genesis, checks
  `committed_on` against the running reference and the `previous_record` chain, applies each write
  set, recomputes the versions and the accumulator, and compares `result_state`. It ends at the
  head's `last_record`
- [X] T018 [US2] Implement `replay_behavior` and `verify_snapshot` in
  `crates/behavior-store/src/replay.rs`. For each record, behavior replay:
  - looks up the module by `behavior_version` (a missing module is reported, not skipped);
  - checks that the record's state section equals the versions at the parent;
  - runs `behavior_core::replay`;
  - compares the changes with the write set and the observed reads with the read set.

  Make T016 pass

**Checkpoint**: history is verifiable end-to-end by both replays.

---

## Phase 5: User Story 3 - Conformance for any storage (Priority: P2)

**Goal**: a conformance suite runnable against any backend, from Rust and Python, with broken
backends that fail their designated cases.

**Independent Test**: the reference backend passes every case. Each broken backend fails exactly
its designated case. A Python dict backend passes through the binding.

### Tests (write first, must fail)

- [X] T019 [P] [US3] Write `crates/behavior-store/tests/conformance.rs`:
  - run `conformance::run(InMemoryBackend::new)` and expect all 17 cases of
    contracts/store-api.md to pass;
  - define the 6 broken backends as wrappers around `InMemoryBackend`, each expected to fail its
    designated case with a message naming the violated rule:
    - `IgnoresHead` → `conflict_on_outdated_parent`;
    - `PartialWrites` → `no_partial_application`;
    - `ReordersRecords` → `history_between_states`;
    - `DropsVersions` → `load_at_past_state`;
    - `LatestReads` → `snapshot_consistency`;
    - `NonAtomicHead` → `crash_retry`.
- [X] T020 [P] [US3] Write `python/tests/test_store.py`:
  - `Store.create(InMemoryBackend(), genesis)`, `evaluate`, `commit`, `StateConflict` (with
    `current` and `changed`), `CommitRefused` (with `code`), `transitions`, `replay_data` and
    `replay_behavior` on the ledger model;
  - `python/tests/test_store_conformance.py`: `run_conformance(lambda: DictBackend())` passes all
    17 cases, where `DictBackend` is a plain Python class implementing the seven backend methods.
    Three Python mutants each fail their designated case (SC-003):
    - `IgnoresHead` → `conflict_on_outdated_parent`;
    - `LatestReads` → `snapshot_consistency`;
    - `ReordersRecords` → `history_between_states`.

### Implementation

- [X] T021 [US3] Implement `crates/behavior-store/src/conformance.rs`: the 17 named cases of
  contracts/store-api.md, run against a backend factory, returning a `ConformanceReport` with
  `(name, ok, message)` per case. Fault cases inject at the wrapper boundary (an error before the
  inner `commit`, or an error after it returned), so every case runs against every backend and
  none is skipped. Internal commit atomicity is documented as the host's responsibility and tested
  for the reference backend by the `PartialWrites` mutant. Make T019 pass
- [X] T022 [US3] Bind the store in `crates/behavior-py/src/lib.rs`: `Store` over `InMemoryBackend`
  or a Python backend object (the seven methods, dicts in and out, `commit` returning `"applied"`
  or `"head_moved"`), `evaluate`, `commit`, `current`, `load`, `transitions`, `replay_data`,
  `replay_behavior` and `run_conformance`
- [X] T023 [US3] Add `python/behavior/store.py` (the `Store`, `InMemoryBackend`, `Evaluation`,
  `CommitResult` and `ReplayReport` wrappers; the `StateConflict` and `CommitRefused` exceptions
  in `python/behavior/errors.py`), export them from `python/behavior/__init__.py`, and add the
  stubs to `python/behavior/_engine.pyi`. Make T020 pass

**Checkpoint**: the contract is portable and checkable from Rust and Python.

---

## Phase 6: User Story 4 - Evidence bound to commits (Priority: P3)

**Goal**: every commit satisfies the store's evidence policy, and records cite the policy and any
authorization. The guarantee is reported as structural.

**Independent Test**: in a store requiring an authorization:
- a commit without one is `EVIDENCE_REQUIRED`;
- a commit with a matching authorization is accepted, and the record cites both hashes;
- an authorization for another record is `EVIDENCE_MISMATCH`.

In a store requiring none, a commit without evidence is accepted, and the record cites the "none"
policy.

### Tests (write first, must fail)

- [X] T024 [P] [US4] Write `crates/behavior-store/tests/evidence.rs`, using
  `behavior_verify::governance::authorize` with the test keys in `tests/fixtures/governance/`.
  Cases:
  - **Policy `none`**: a commit without evidence is accepted, the record's `evidence_policy` is
    the policy hash, and `authorization` is `null`.
  - **Policy `commit_authorization`**:
    - a commit without an authorization is `EVIDENCE_REQUIRED`, citing the policy;
    - a matching authorization is accepted, with `authorization` set to its hash and
      `evidence_trust: "structural"` in the result;
    - an authorization for another transition is `EVIDENCE_MISMATCH`;
    - an authorization with `decision ≠ allow` is `EVIDENCE_MISMATCH`;
    - cited documents whose hashes differ are `EVIDENCE_MISMATCH`;
    - an execution policy outside `trusted_execution_policies` is `EVIDENCE_MISMATCH`.
  - **Store binding**: a bundle and authorization from a second store with identical genesis
    content except a different evidence policy (so a different store identity) are refused
    (FR-019).
  - **Evidence and idempotency**: evidence attached after evaluation
    (`CommitBundle::with_evidence`) keeps the transition hash, and idempotency still recognizes
    the transition.

### Implementation

- [X] T025 [US4] Implement evidence checking in `crates/behavior-store/src/store.rs`, following
  research R9 and FR-013, FR-013a and FR-023:
  - decode the authorization and documents with `behavior_verify::governance`;
  - check the decision, the transition hash, the cited hashes and the allowlist;
  - enforce the policy's `require`;
  - cite the policy and authorization hashes in the record;
  - report `evidence_trust: "structural"`.

  Also add `CommitBundle::with_evidence` in `crates/behavior-store/src/documents.rs`. Make T024
  pass

**Checkpoint**: commits are valid only relative to an explicit evidence policy.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T026 [P] Add an ignored perf test, `crates/behavior-store/tests/perf.rs` (SC-004): a store
  with 100,000 seeded accounts, where one transfer (evaluate + commit) takes under 50 ms in release
  mode (the reference machine is the NixOS dev machine used for the gates) and does not grow with
  store size (compare with 1,000 accounts: within 2×)
- [X] T027 [P] Add the host example `examples/ledger/` (`behavior.py` mirroring the ledger fixture,
  `run.py` doing genesis, a transfer, a conflict and retry, a denied transfer, and replay). The
  glue code must be under 20 lines (SC-006). Also add the Python example to README
- [X] T028 [P] Extend `scripts/determinism-check.sh`: build a fixed 200-transition ledger history
  twice (via a small `behavior-store` example binary or test helper that writes the records) and
  compare state identities, records and replay reports byte for byte (SC-005)
- [X] T029 [P] Documentation:
  - `docs/persistence.md`: the contract, the three concepts (state identity, history position,
    entity version), the backend primitives, conflicts, evidence trust ("structural"), replay and
    the follow-ups (entity-level concurrency, entity import/create/delete as universe transitions,
    signed authorizations, `behavior-evidence` split);
  - a README section;
  - mark `docs/proposals/persistence-contract.md` as implemented by 005, with deviations.
- [X] T030 Run all gates: fmt, clippy (`-D warnings`), `cargo test --workspace`, `pytest`, `mypy`,
  `scripts/determinism-check.sh`, and the ignored release tests: perf (SC-004) and the
  10,000-transition replay (SC-002). Fix all findings
- [X] T031 Write `specs/005-persistence-contract/checklists/implementation-review.md`: a review
  against the spec (FR-001–FR-023, SC-001–SC-006), the constitution, the guiding invariant, the
  two review rounds of the plan, every deviation from this task list, and the open follow-ups
- [ ] T032 Run quickstart.md §1–§5 from a fresh `nix develop` shell and fix any failures

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T010)** → stories.
- **US1 (T011–T015)** needs the foundational phase. It is the MVP.
- **US2 (T016–T018)** needs US1 (committed history).
- **US3 (T019–T023)** needs US1 and US2: the conformance cases cover commit and replay.
- **US4 (T024–T025)** needs US1. It can run in parallel with US2 and US3 after US1, but it edits
  `store.rs`, so coordinate with T015.
- **Polish (T026–T032)** comes after all stories.
- **Within a phase**, tests are written first and seen failing, then the implementation follows.
- **Shared files**:
  - `store.rs` is edited by T013, T014, T015 and T025, in that order;
  - `documents.rs` by T008 and T025;
  - `crates/behavior-py/src/lib.rs` only by T022.

## Parallel Opportunities

- Setup: T002 and T003 in parallel after T001.
- Foundational tests: T004, T005 and T006 in parallel. Then T007 (core) in parallel with T008 and
  T009 (store). T010 comes after T008.
- US1: T011 and T012 in parallel, then T013 → T014 → T015.
- US3: T019 and T020 in parallel.
- US4: T024 in parallel with US2 work.
- Polish: T026, T027, T028 and T029 in parallel.

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 gives a working store with guarded, atomic, idempotent
   commits against consistent snapshots, validated with the ledger module.
2. **Add US2**: history becomes verifiable by both replays. This completes the P1 contract.
3. **Add US3**: the contract becomes portable (conformance suite, broken backends, Python backends).
4. **Add US4**: evidence policies bind governance to commits.
5. **Polish**: performance, example, determinism, docs, review.
