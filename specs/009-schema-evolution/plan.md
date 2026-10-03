# Implementation Plan: Schema Evolution and Migration

**Branch**: `009-schema-evolution` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/009-schema-evolution/spec.md`, with the
clarifications of 2026-10-02:
- exact store-schema binding;
- automatic copies resolved into an explicit Migration IR;
- entity-local transforms with global validity;
- source requirements with proven narrowing;
- evidence decided by the evidence policy.

## Summary

Make every store **schema-aware** and give it an explicit, verifiable way to **change schema**.

- **SchemaHash** is the hash of the store's entity declaration hashes, which already include the
  enums and nominals they use. Every store has a well-defined schema at every position: the
  genesis schema, plus one change per migration.
- **Binding.** Evaluation and commit require `module.schema_hash == store.schema_at(position)`.
  This replaces the 0.8 touched-types comparison.
- **Migrations** are a separate document (migration IR 0.1) admitted against a source and a target
  module. Admission resolves automatic copies into explicit `Copy` nodes, requires explicit drops,
  and types expressions across both schemas, with enum maps and strict narrowings.
- **Applying** a migration:
  1. validate the source state under the source module's rules, then check the source requirements;
  2. transform entity-locally;
  3. validate the target state in full;
  4. commit everything as one compare-and-set, with a new `migration` record kind and a `SchemaRef`
     in the head.
- **Replay** crosses generations, and **verification** gives local proofs per type plus
  whole-state checks.
- **Evidence** for migrations comes from the store's evidence policy, per transition kind.
- **Version.** Release 0.9.0. Every existing document keeps its bytes.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python ≥ 3.13.

**Primary Dependencies**: the existing crates and Z3 4.16.0; no new dependencies.

**Storage**: the backend contract is unchanged: migrations use `commit`. Optional new fields:
`Head.schema`, the record `kind`/`migration`, and `EvidencePolicy.migration`.

**Testing**:
- `cargo test`: schema hashing and binding, migration admission (valid and invalid fixtures),
  `apply_migration`, store migrate and refusals, schema history, replay across generations,
  tampering, conformance (`migration_atomicity`, `schema_history_consistency`).
- Verifier fixtures with proven, counterexample and inconclusive outcomes.
- `pytest`: the DSL, store.migrate, the skills.
- Proptest: generated histories crossing up to 3 migrations (SC-002).
- An ignored performance test (SC-005).
- The determinism check.

**Target Platform**: Linux x86-64, as for 0.8.

**Project Type**: library + CLI + Python binding + skills.

**Performance Goals**: migrating 100,000 entities in under 60 s (SC-005).

**Constraints**:
- no reinterpretation of history;
- runtime validity is never optional;
- transforms are entity-local, so the order of transformation is irrelevant;
- existing bytes and identities are unchanged (FR-025);
- no new crates.

**Scale/Scope**:
- 5 user stories and 1 new document kind;
- 1 new record kind and 3 optional document fields;
- 11 new error codes and 4 CLI commands;
- skills updates.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Transforms are pure and entity-local and applied in canonical id order; the result is order-independent; requirements are evaluated on the immutable source state; no AI involvement |
| II. AI output validated | Pass (n/a) | Agent-written migrations are admitted, verified and applied like any author's |
| III. Test-first | Pass (process) | Admission fixtures, apply/refusal tests, replay and tampering proptests, conformance cases with mutants, and verifier fixtures are written and seen failing first |
| IV. Reproducibility | Pass | Migration records carry everything for data replay; behavior replay re-runs the migration; the schema at each position is derivable |
| V. Explicit state and auditability | Pass | The schema is explicit at every position; records state requirements, the resolved summary, evidence and verification outcome |
| VI. Simplicity | Pass with justification | No new crate or backend method. Additions: a migration document, a record kind and a two-sided typing context (see Complexity Tracking) |
| Tech constraints | Pass | Rust gates; no `unsafe`; typed errors |

**Post-design re-check (after Phase 1)**: all rows pass.

## Project Structure

### Documentation (this feature)

```text
specs/009-schema-evolution/
├── plan.md
├── research.md            # R1–R12
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── migration-api.md   # Python, Rust, CLI, evidence policy
│   └── migration-wire.md  # migration IR 0.1
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/src/
│   ├── schema.rs            # new: StoreSchema, SchemaHash
│   ├── migration/           # new: wire decode, two-sided typing, resolution, hashing, apply_migration
│   └── lib.rs               # exports
├── behavior-store/src/
│   ├── documents.rs         # Head.schema, TransitionRecord kind/migration, EvidencePolicy.migration
│   ├── store.rs             # schema binding, schema_at/history, migrate, content hashing by position
│   ├── replay.rs            # data and behavior replay across generations
│   └── conformance.rs       # migration_atomicity, schema_history_consistency
├── behavior-verify/src/     # verify_migration (local and whole-state), governance::authorize_migration
├── behavior-cli/src/lib.rs  # schema-hash, migration admit|verify|apply
└── behavior-py/src/lib.rs   # Migration builder, apply/verify/migrate bindings
python/behavior/migration.py # Migration, enum_map, strict_enum_map, strict_unwrap
schema/migration-ir-0.1.schema.json
tests/fixtures/migration/    # valid/invalid migration documents, history fixtures
examples/schema_evolution/   # V1 → V2 → V3 staged path
skills/                      # authoring, verification and application sections with examples
api/public-api.json          # new names and commands
```

**Structure Decision**: migration semantics live in a new `behavior-core/src/migration/` module
next to admission and evaluation, and the schema identity in `schema.rs`. The store, verifier,
CLI and binding each gain one entry point. There is no new crate.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A separate migration document | A migration relates two schemas; a module describes one | A section inside the target module would change the module's identity with every migration |
| Two-sided typing (source and target types) | A changed enum keeps its name but is a different type (Q2) | Name-based typing would silently equate different types |
| A second record kind | A migration has no action bundle; audit must state requirements, report and evidence | Faking a migration as an action would mix the two transition kinds that Q5 keeps apart |
| `Head.schema` | Finds the current and historical schema in O(number of migrations) | Scanning every record on open is O(history) |
