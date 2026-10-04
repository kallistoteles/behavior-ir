# Implementation Plan: First-Class Reads

**Branch**: `010-first-class-reads` | **Date**: 2026-10-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/010-first-class-reads/spec.md`, with the
clarifications of 2026-10-03:
- projections hold stored fields and declared derived values, and one failure fails the whole
  read;
- declared reads are entry points only;
- a projection covers a query (a list) or a bound entity (one record);
- the capability response is separate from the evidence.

## Summary

Add **read** as a third engine operation next to transition and migration. A read evaluates one
typed, pure definition against one exact state and returns a value, with a content-addressed
**read record** as evidence. It never writes anything.

- **One definition shape (`WRead`)**, used in two places:
  - a module's new `reads` section (wire IR 0.7) holds **declared reads**, which are capabilities
    and part of the behavior version but not of the schema;
  - a standalone read document is an **ad-hoc read**, for trusted hosts only.
- **Bodies.** A body is either a value expression or a **projection**: named stored fields and
  derived values over a query (a list of records) or over one bound entity (one record).
- **Evaluation** is `run_read` on the existing evaluator, in phase S only. Derived items use the
  existing `DerivedRef` path, so their semantics and dependency tracking are exactly those of
  transitions.
- **Records.** Read records (`behavior.read_record.v1`) replay from their own facts or against
  the store at their `data_version`.
- **Capability responses.** A read intent returns a `ReadResponse` (the result and the record
  identity only) as a type distinct from the `ReadRecord`.
- **Store reads** take `&self`, at the head or at a past `StateRef`, under exact schema binding.
- **Verifier.** `evaluation_error` covers declared reads.
- **Python** gets `@read` and `project`, and the CLI gets `read`, `read-intent` and `read-replay`.
- **Version.** Release 0.10.0. Every existing document keeps its bytes.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python ≥ 3.13.

**Primary Dependencies**: the existing crates and Z3 4.16.0; no new dependencies.

**Storage**: no change to the backend contract or to any store document. Reads use the existing
as-of reads (`version_at`, `keys_at`, `incoming_at`, …) and never call `commit`.

**Testing**:
- `cargo test`:
  - read admission, with valid and invalid fixtures;
  - plain evaluation of value reads, query projections and entity projections;
  - refusals, record determinism, replay and tampering;
  - store reads at the head and at past positions, schema binding across a migration;
  - read intents.
- Hash vectors and frozen versions for SC-007.
- Verifier fixtures for read evaluation errors: proven, counterexample, inconclusive.
- `pytest`: the DSL (`@read`, `project`, the DSL errors), store reads, intents, replay, skills.
- Proptest: generated histories of up to 1,000 operations interleaving reads, transitions and a
  migration (SC-002, SC-003, SC-005).
- An ignored performance test (SC-004).
- The determinism check, extended with the read fixtures.

**Target Platform**: Linux x86-64, as for 0.9.

**Project Type**: library + CLI + Python binding + skills.

**Performance Goals**: a 5-item projection over 10,000 entities in under 2 s (SC-004), with one
`version_at` per member (R8).

**Constraints**:
- a read never writes, enforced by type (`&self`) and by test;
- no second implementation of evaluation semantics;
- disclosure is decided by the engine (distinct response and record types);
- existing bytes and identities are unchanged (FR-021);
- no new crates.

**Scale/Scope**:
- 5 user stories;
- 1 wire IR version (0.7), 1 new record kind, 1 new name-table kind;
- 5 new admission codes and 3 CLI commands;
- 1 verifier version bump;
- skills updates and the `PRINCIPLES.md` amendment (FR-020a).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Reads are pure evaluation over one snapshot. Projection order is canonical identity order. Agents get a validated capability boundary for reads, as for actions, and never send expressions. No AI involvement in the engine |
| II. AI output validated | Pass | Agent read intents are parsed and fully validated (known read, targets, input types) before evaluation, and rejected with typed errors (R9) |
| III. Test-first | Pass (process) | Admission fixtures, evaluation and refusal tests, replay and tamper proptests, intent fixtures, verifier fixtures and the DSL tests are written and seen failing before the implementation |
| IV. Reproducibility | Pass | Read records are canonical and content-addressed, replayable from facts and against the store. The determinism check runs the read fixtures twice |
| V. Explicit state and auditability | Pass | Every read emits a record with its inputs, exact state, observations and result. Reads never alter state, by type |
| VI. Simplicity | Pass with justification | No new crate, backend method or store document. Additions: a read item, a record kind and a response type (see Complexity Tracking) |
| Tech constraints | Pass | Rust gates; no `unsafe`; typed errors; no `unwrap` outside tests |

**Post-design re-check (after Phase 1)**: all rows pass. The design reuses the evaluator, the
facts provider, the intent validator and the replay comparison. Every new type in the
contracts is justified below.

## Project Structure

### Documentation (this feature)

```text
specs/010-first-class-reads/
├── plan.md
├── research.md            # R1–R14
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── read-wire.md       # wire IR 0.7 `reads`, read documents, admission codes, hashing
│   └── read-api.md        # Python, Rust, CLI
├── checklists/requirements.md
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/src/
│   ├── wire.rs              # IR 0.7, WRead/WReadBody/WItem, `reads` decode, NeedsReadVersion
│   ├── admit/typecheck.rs   # read admission, projection checks, READ_CALL_NOT_ALLOWED, DUPLICATE_CAPABILITY
│   ├── admit/hash.rs        # TAG_READ, TAG_PROJECTION, Kind::Read entries
│   ├── semantic/module.rs   # Kind::Read, ReadItem, Projection, Module::reads()
│   ├── eval.rs              # run_read; decode_sections/check_*_snapshot over a parameter slice
│   ├── facts.rs             # EvaluationFacts::entity (defaulted)
│   ├── read.rs              # new: ReadSource, ReadRecord, ReadResponse, ReadExecution,
│   │                        #      evaluate_read(_with), evaluate_read_intent, replay_read, admit_read
│   ├── intent.rs            # shared parameter-list validator for action and read intents
│   └── lib.rs               # exports, format_versions (IR 0.7, read record format)
├── behavior-store/src/
│   ├── store.rs             # Store::read, read_intent, replay_read; StoreFacts::entity
│   └── (no document changes)
├── behavior-verify/src/
│   ├── checks.rs            # evaluation_error for read subjects
│   ├── encode*.rs           # symbolic member under a local query predicate
│   └── lib.rs               # VERIFIER_VERSION 0.6.0
├── behavior-cli/src/lib.rs  # read, read-intent, read-replay
└── behavior-py/src/lib.rs   # read bindings, ReadResponse/ReadRecord classes
python/behavior/
├── decl.py                  # @read, ReadFn; READ_CALL_NOT_ALLOWED; effect statements refused in reads
├── query.py                 # project(...)
├── module.py                # BehaviorModule(reads=[...]), tracing reads
├── reads.py                 # new: evaluate_read(), read_intent(), replay_read(), ReadResult/Response/Record/Execution
└── store.py                 # Store.read, read_intent, replay_read
schema/wire-ir-0.7.schema.json
tests/fixtures/reads/        # modules, valid/invalid read documents, requests, golden records
tests/fixtures/read_intents/ # intent rejection fixtures
tests/fixtures/verify/       # read evaluation_error cases
examples/lab_reads/          # SC-001: the external application's read patterns, no placeholder entity
skills/                      # reading section in application, declaring reads in authoring, checks in verification
PRINCIPLES.md                # "capabilities are entry points, not building blocks"
docs/                        # versioning.md (0.10.0, IR 0.7, read records), persistence.md (reads never stored)
api/public-api.json          # new names and commands
```

**Structure Decision**: read semantics go in a new `behavior-core/src/read.rs`, which owns the
operation, its record and its response, on top of `eval.rs` (`run_read`) and admission. This
mirrors how `intent.rs` and `record.rs` sit on top of evaluation for transitions. The store,
verifier, CLI and binding each gain entry points. There is no new crate.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A new record kind (`behavior.read_record.v1`) | A read has no action, changes or ALLOW/DENY; its record must never pass for a decision or a transition | Reusing `DecisionRecord` would mix reads into the decision record versions and fields |
| Distinct `ReadResponse` and `ReadRecord` types | Disclosure must be fixed by the engine, not by host routing (clarification Q4) | One object with an optional trace would make exposure configurable |
| Wire IR 0.7 and `Kind::Read` | Declared reads are capabilities and so part of the behavior version (FR-015) | Keeping reads outside the module would make capabilities ungoverned and unversioned |
| A projection body form (not an expression) | Projections must expose only stored fields and declared derived values, with no traversal (clarification Q1) | A record type in the expression language would amount to struct types, which are out of scope |
