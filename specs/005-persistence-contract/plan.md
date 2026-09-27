# Implementation Plan: Persistence Contract

**Branch**: `005-persistence-contract` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-persistence-contract/spec.md`. It is based on
`docs/proposals/persistence-contract.md`, with the clarifications of 2026-09-27: whole-state
optimistic concurrency, and an explicit evidence policy per store.

## Summary

Define the engine's persistence contract without building a database adapter.

A new crate, `behavior-store`, owns the semantics:
- **Content-addressed documents:** entity versions, store genesis, evidence policy, commit bundle
  and transition record.
- **State identity:** an incremental multiset hash (MuHash3072) over the current entity versions.
  It depends only on content, and a commit costs time proportional to the entities it touches.
- **Store logic:** `Store<B>` evaluates an action against one consistent snapshot of the store (as-of
  reads over immutable, position-tagged versions) and returns
  the decision record plus a commit bundle for allowed decisions. It commits bundles with
  whole-state optimistic concurrency (`STATE_CONFLICT`), idempotency, bundle validation and the
  evidence policy. It answers history queries and runs data and behavior replay.

Hosts implement only a small `Backend`: read the head, as-of versions and records, plus one atomic,
crash-safe compare-and-set commit (versions, record with idempotency information, and head as one
unit).

Records and bundles bind the store identity (the genesis) as well as the state, so evidence cannot
be replayed into another store with identical content.
- **Engine-derived dependencies:** read and write sets are always derived by the engine.
- **Fixed entity universe:** no entity is created or removed after genesis.
- **Full context:** the full context snapshot is part of every record.
- **Evidence trust:** `require: commit_authorization` is documented as a structural guarantee within
  the current trust boundary. Signed authorizations are a follow-up.

Guiding invariant: every committed transition is reproducible from an immutable semantic state, an
exact history position, an exact behavior version, exact input and exact context, and no storage
implementation may alter those semantics.

Three concepts are kept apart:
- **State identity**, a pure content hash that includes no revision: what information exists.
- **History position**, a monotonic position plus the record chain: where in this store's history.
- **Entity version**, revision plus created position: which incarnation supplied a value.

The read set is the set of fields actually observed by the evaluation.

The crate ships an in-memory reference backend and a conformance suite, with broken backends that
must fail their cases. The Python binding exposes the store, Python backends, replay and the
conformance suite.

The model is ready for a later entity-level concurrency rule: records carry read and write sets,
and `evaluated_against` / `committed_on`, which are equal in this feature.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13.

**Primary Dependencies**: no new external dependency.
- `sha2` and `num-bigint` for MuHash3072;
- the existing canonical JSON and document hashing;
- `behavior-core` for evaluation and replay;
- `behavior-verify` for the transition hash and authorization decoding;
- pyo3 for the binding.

**Storage**: host-provided through `Backend`. The crate itself has only the in-memory reference
backend.

**Testing**:
- `cargo test`: document hashing vectors, MuHash algebra (order independence, add/remove),
  commit/conflict/idempotency, and the conformance suite with 6 broken backends.
- `proptest` for SC-001: 10,000 generated concurrent-commit interleavings.
- `proptest` for SC-002: histories up to 10,000 transitions, with single-field tampering.
- An ignored perf test for SC-004: 100,000 entities.
- `pytest`, including a Python dict backend through the conformance suite.
- The determinism script, extended with store histories.

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + Python binding (no CLI change).

**Performance Goals**: a commit on a store with 100,000 entities in under 50 ms, proportional to
the touched entities (SC-004). History replay is linear in the number of transitions.

**Constraints**:
- no I/O or clock inside the engine;
- byte-identical documents across runs and platforms;
- existing record, authorization and wire formats unchanged;
- `evaluated_against == committed_on` for every record.

**Scale/Scope**: 4 user stories, 1 new crate, 7 document kinds (plus the replay report), a 7-method backend trait, and a
conformance suite with 17 cases and 6 broken backends.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Commit validation, state identity, conflicts and replay are deterministic engine code. There is no AI. Hosts only store bytes. |
| II. AI output validated | Pass (n/a) | No AI. |
| III. Test-first | Pass (process) | The conformance cases, the broken backends, and the SC-001/SC-002 properties are written and seen failing before `Store` is implemented. |
| IV. Reproducibility | Pass | State identity depends only on content. Records are hash-chained, and behavior replay re-evaluates recorded transitions. The commit time is recorded input, never read inside the engine. `BTreeMap` ordering throughout. |
| V. Explicit state and auditability | Pass | This feature makes state explicit: every state has an identity, every change a record, and every commit a cited policy and evidence. |
| VI. Simplicity | Pass with justification | One new crate (justified below). A multiset hash instead of a Merkle tree, so hosts store one accumulator. Whole-state concurrency only. |
| Tech constraints | Pass | Rust gates, no new dependencies, no `unsafe`, typed errors (`thiserror`). |

**Post-design re-check (after Phase 1)**: all rows pass. The split of the store into `Store`
(engine) and `Backend` (host) keeps every semantic rule in one implementation (R2).

**Plan review (2026-09-27), correctness items resolved**:
- consistent snapshot evaluation (FR-018, R2);
- crash-safe atomic commit (FR-006, `crash_retry`);
- entity versions bound to type, declaration and id (R3, R4);
- store identity bound separately from content identity (FR-019, R6);
- engine-derived dependency sets (FR-020);
- a fixed entity universe (FR-021);
- the full context snapshot (FR-022);
- evidence as a structural, not yet cryptographic, guarantee (FR-023, R9);
- semantic versus audit identities (R8);
- whether verify links a solver: it does not, so `behavior-evidence` is a later split (R1).

**Second review (2026-09-27)**:
- revisions are removed from the state identity: entity content hash = type + declaration + id +
  value, and revision and created position live only in the entity version (FR-002, R3, R4);
- the read set is defined as fields actually observed, and a static superset would be named
  `dependency_set` (FR-020, R7);
- this needs an observed-read collector in `behavior-core` that covers derived bodies and memo
  hits, with decision records unchanged (R7).

## Project Structure

### Documentation (this feature)

```text
specs/005-persistence-contract/
├── plan.md
├── research.md            # R1–R12
├── data-model.md          # documents, identities, results, backend primitives
├── quickstart.md
├── contracts/
│   └── store-api.md       # Rust + Python API, conformance cases
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-store/                 # new
│   ├── src/lib.rs                  # public API
│   ├── src/documents.rs            # canonical documents, tags, validation
│   ├── src/muhash.rs               # MuHash3072 accumulator (R3)
│   ├── src/store.rs                # Store<B>: evaluate, commit, conflicts, evidence, history
│   ├── src/replay.rs               # data/behavior replay, snapshot check (R10)
│   ├── src/memory.rs               # InMemoryBackend
│   ├── src/conformance.rs          # named cases (R12)
│   └── tests/                      # conformance (incl. broken backends), properties, perf
├── behavior-core/src/eval.rs       # observed-read collector (incl. derived bodies, memo hits); records unchanged
├── behavior-verify/                # unchanged (transition hash, authorization decoding reused)
└── behavior-py/src/lib.rs          # Store, backends, replay, conformance bindings
python/behavior/store.py            # Python wrappers, StateConflict / CommitRefused
python/tests/test_store*.py
examples/ledger/                    # host example (SC-006)
tests/fixtures/store/               # hashing vectors, genesis and history fixtures
scripts/determinism-check.sh        # + store histories
```

**Structure Decision**: a new workspace crate for the persistence layer. It depends on core and
verify, and nothing depends on it except the binding.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New crate `behavior-store` | Persistence needs evaluation (core) and governance hashing (verify), and it is a separate layer in the architecture | In core, it would invert the dependency on governance; in verify, it would mix storage with the SMT verifier |
| MuHash3072 (bigint accumulator) | Content-only state identity (FR-002) with commit cost independent of store size (SC-004) | A full rehash is O(state); a hash chain depends on history; a Merkle tree forces every host to store tree nodes |
