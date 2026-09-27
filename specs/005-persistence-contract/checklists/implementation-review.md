# Implementation Review: Persistence Contract

**Feature**: 005 · **Reviewed**: 2026-09-27 · **Tasks**: T001–T032

## Guiding invariant

> Every committed transition is reproducible from an immutable semantic state, an exact history
> position, an exact behavior version, exact input and exact context, and no storage
> implementation may alter those semantics.

- [x] **Immutable semantic state:**
  - Evaluation reads the head once, and every bound entity `version_at` that position (FR-018).
    Tested by `commit.rs::evaluation_reads_one_consistent_snapshot` and the
    `snapshot_consistency` conformance case, which the `LatestReads` mutant fails.
- [x] **Exact position and behavior version:**
  - Records carry `evaluated_against == committed_on` and the behavior version.
  - The record's `data_version` binds `store:<genesis>;state:<id>;position:<n>`.
- [x] **Exact input and context:**
  - The decision record in the bundle carries the full canonical context, and replay re-evaluates
    it byte for byte.
- [x] **No storage implementation may alter the semantics:**
  - All rules live in `Store<B>`; a backend only stores documents.
  - The 17 conformance cases and 6 broken backends make this checkable for any backend, from Rust
    and Python.

## Requirements

| Requirement | Status | Evidence |
|---|---|---|
| FR-001 revisions | ✓ | `commit.rs` (revisions + 1 only on value change; `touch` keeps them) |
| FR-002 content identity | ✓ | `muhash.rs`, `documents.rs::content_excludes_history`; returning content repeats the identity at a later position (`commit.rs`, `identity_is_content`) |
| FR-003 canonical forms | ✓ | `tests/fixtures/store/hash_vectors.json` (frozen) |
| FR-004 bundle | ✓ | `Store::evaluate`; bundles only for `ALLOW` |
| FR-005 operations over backend primitives | ✓ | `Backend` (7 methods), `Store` |
| FR-006 crash-safe atomicity | ✓ | `crash_retry` via `FaultInjector` (before and after the durable write); `no_partial_application` at the compare-and-set primitive |
| FR-007 whole-state conflict | ✓ | `STATE_CONFLICT` with changed keys; the race between the head check and the compare-and-set is covered (`LandOn::Commit`) |
| FR-007a future entity level | ✓ | Records carry observed read sets, write sets, and `evaluated_against` / `committed_on` |
| FR-008 record contents | ✓ | `TransitionRecord` |
| FR-009 idempotency incl. late retry | ✓ | Check of `record(p+1)`; `commit.rs::resubmission_and_late_retries_are_idempotent` |
| FR-010 refusals | ✓ | `commit.rs::invalid_bundles_are_refused`, `another_declaration_is_refused` |
| FR-011 replays | ✓ | `replay.rs` (1,000 clean; tamper proptest; 10,000 in release) |
| FR-012 snapshots | ✓ | `verify_snapshot` |
| FR-013/013a evidence policy | ✓ | `evidence.rs` (real attestation and authorization through Z3) |
| FR-014 reference backend | ✓ | `InMemoryBackend` |
| FR-015 conformance | ✓ | `conformance::run`, 17 cases, 6 Rust mutants |
| FR-016 Python | ✓ | `Store`, `InMemoryBackend`, Python backends, replay, `run_conformance`; `DictBackend` passes all 17 cases; 3 Python mutants fail theirs |
| FR-017 determinism | ✓ | `scripts/determinism-check.sh` (200-transition history twice, byte for byte) |
| FR-018 snapshot reads | ✓ | see the invariant |
| FR-019 store binding | ✓ | `store_binding` case; `evidence.rs::evidence_from_another_store_is_refused` |
| FR-020 engine-derived sets | ✓ | `behavior_core::evaluate_observed`; the store re-derives at commit |
| FR-021 fixed universe | ✓ | `ENTITY_UNIVERSE_CHANGED`; data replay flags writes outside the universe |
| FR-022 context snapshot | ✓ | The record's context; `commit.rs` asserts it |
| FR-023 structural trust | ✓ | `evidence_trust: "structural"`; `docs/persistence.md` → Trust boundary |
| SC-001 | ✓ | 10,000 interleavings (release, 79 s) |
| SC-002 | ✓ | 1,000 by default; 10,000 in release (104 s) |
| SC-003 | ✓ | 100% for the reference and `DictBackend`; 6 Rust and 3 Python mutants |
| SC-004 | ✓ | 4.9 ms per transfer at 1,000 and at 100,000 accounts |
| SC-005 | ✓ | Determinism check |
| SC-006 | ✓ | `examples/ledger/run.py`: 11 lines of glue plus a 3-line helper |

## Constitution

- [x] **I Deterministic core:** no clock or I/O in the engine; `BTreeMap` ordering; the
  determinism check covers store histories.
- [x] **III Test-first:** mostly followed; see deviation 5.
- [x] **IV Replay:** both replays, plus the hash-chained history.
- [x] **V Auditability:** every commit has a record that cites its parent, its evidence policy and
  its authorization.
- [x] **VI Simplicity:** one new crate and no new dependencies. MuHash uses the existing `sha2`
  and `num-bigint`.
- [x] **Gates:**
  - `cargo fmt --check` and clippy (`-D warnings`) pass.
  - `cargo test --workspace`: 181 tests pass.
  - `pytest`: 85 tests pass, and mypy is clean.
  - The determinism check passes.
  - All ignored release tests pass: store perf, replay, concurrency; core and verifier perf.

## Deviations from the task list and contracts

1. **`Store::create` and `Store::commit` take the module.** Validating the seed needs field types
   and constraints, and re-deriving the read set at commit (FR-020) re-evaluates the record. The
   contract's `create(backend, genesis)` / `commit(expected_parent, bundle)` became
   `create(backend, module, genesis)` / `commit(module, expected_parent, bundle)`. In Python,
   `store.commit(model, bundle, expected_parent=None)`, where the parent defaults to the bundle's
   evaluated state.
2. **New core API.** `behavior_core::canonical_entity` (seed validation) and
   `evaluate_observed` (observed reads) are public. Decision records are byte-identical, and all
   goldens are unchanged.
3. **`conflict_on_outdated_parent` also covers the race.** `Store::commit` checks the head
   before the compare-and-set, so a backend that ignores the expected head (`IgnoresHead`) is
   only visible if the head moves in between. `Interleave` gained `LandOn::Commit` to test exactly
   that window.
4. **The `NonAtomicHead` mutant** is modelled as always losing the record of a commit, i.e. a crash
   between moving the head and writing the record. A deferred-record variant healed itself at the
   next commit.
5. **Test-first order.**
   - `documents.rs`, `muhash.rs` and `store.rs` were written just before their tests. Those tests
     were seen failing only as non-compiling (the API did not exist yet).
   - `observed_reads.rs` and the conformance, replay and evidence tests did precede their
     implementations.
6. **Fault injection is a wrapper.** It is a wrapper (`FaultInjector`) instead of hooks on
   `InMemoryBackend` (analysis finding U1), so it works around any backend.
7. **Evidence tests are split by concern.**
   - Backend conformance tests the storage consequences of evidence:
     - `evidence_policy`: `EVIDENCE_REQUIRED` and policy citation;
     - `evidence_atomicity`: a synthetic, well-formed authorization's reference is committed
       atomically with the record, versions and head, across a crash and retry.
   - The engine's acceptance and rejection of real evidence (a Z3-verified attestation and a
     governance authorization) is tested in `evidence.rs`.
8. **Performance fix found by SC-004.** The genesis (with its seed) was re-read and re-hashed on
   every evaluate and commit (1.29 s at 100,000 accounts). `Store` now caches the genesis and store
   identity; a transfer takes 4.9 ms at any size.
9. **Tamper proptest window.** It replays a window starting just before the tamper, for speed. The
   full clean history is replayed both ways in its own test, and 10,000 transitions in release
   mode.
10. **New error code `GENESIS_INVALID`** for seed and genesis validation (not in data-model.md's
    table).
11. **Quickstart §5 (evidence with an authorization) runs in Rust** (`evidence.rs`). The Python
    tests cover `EVIDENCE_REQUIRED`.

## Plan review after implementation (2026-09-27)

- **Behavior identity is checked first.** `hash(module) == bundle.behavior_version ==
  record.behavior_version` is now the first check in `Store::commit`, before the module is used
  for anything (idempotency, re-evaluation). Test:
  `commit.rs::the_module_must_be_the_behavior_that_made_the_bundle`.
- **The new conformance case `evidence_atomicity`** is described under deviation 7 (now 17 cases).

## Code review fixes

1. **Data replay recomputes stored content hashes at its starting position.** Before, it trusted
   them, so an altered stored value of an entity never written later went unnoticed. Test:
   `replay.rs::stored_values_are_checked_against_their_content_hashes`.
2. **Commit derives the touched entities' declarations from the action's parameters.** Before, it
   took them from the bundle's own list, so a hand-made bundle with an empty list skipped the
   check. Every touched entity must be declared identically by the module and the genesis, and the
   bundle's list must equal that set. Test:
   `commit.rs::declarations_are_derived_from_the_action_not_the_bundle`.
3. **Python `Store.current()` and `store_id` translate engine errors.** A backend outage now
   surfaces as `CommitRefused(BACKEND_ERROR)`. Test:
   `test_store_conformance.py::test_backend_errors_surface_as_commit_refused`.

## Follow-ups

- **Resolve behavior by content hash.** `store.commit(bundle)` with a `BehaviorResolver`
  (`get(behavior_hash) → Module`), instead of the host supplying the module at commit. The bundle
  already names the behavior hash it needs, and this removes the chance of a wrong module.
- **Evidence trust from structural to cryptographic,** when signed commit authorizations arrive:
  `evidence_trust` gains a `"cryptographic"` level, with trusted keys in the evidence policy.

- **Entity-level concurrency:** the records are ready (observed read and write sets,
  `evaluated_against` / `committed_on`).
- **Universe transitions:** recorded import, create and delete, instead of a fixed entity
  universe.
- **Signed commit authorizations** with trusted keys in the evidence policy (structural →
  cryptographic); ties into the feature-002 open risks.
- **Changing a store's evidence policy:** a new store, a configuration transition, or a versioned
  policy.
- **A `behavior-evidence` crate** if the verifier gains heavy dependencies.
- **A reusable read-set conformance module in Python,** and database adapters built by hosts
  against the contract.
