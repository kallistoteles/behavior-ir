---
description: "Task list for feature 009: schema evolution and migration"
---

# Tasks: Schema Evolution and Migration

**Input**: Design documents from `/specs/009-schema-evolution/`: plan.md, spec.md (with
clarifications Q1–Q5), research.md (R1–R12), data-model.md, contracts/ (migration-api,
migration-wire), quickstart.md.

**Tests**: test-first (constitution III). Every test task comes before its implementation and
must be seen failing.

**Organization**: tasks are grouped by user story; paths are repository-relative.

## Format: `[ID] [P?] [Story] Description`

## Standing rules

These apply to every task:
- **No reinterpretation.** A stored value is only ever read under the schema at its position.
- **Byte stability.** Existing modules, records, geneses, heads of stores without migrations,
  evidence policies and goldens keep their bytes (FR-025). The frozen identity tests must pass
  unchanged.
- **Runtime checks are never skippable** (FR-019a).

---

## Phase 1: Setup

- [X] T001 Set `[workspace.package] version = "0.9.0"` in `Cargo.toml`, `release: "0.9.0"` in `api/public-api.json` and in each consumer `skills/*/SKILL.md` frontmatter. Create `tests/fixtures/migration/{valid,invalid}/` and `examples/schema_evolution/`. Confirm `cargo test --workspace` and `pytest` still pass, with no golden changed.
- [X] T002 [P] Add `schema/migration-ir-0.1.schema.json` (JSON schema of the document in contracts/migration-wire.md: `migration_ir`, `name`, `source`, `target`, `constants`, `requirements`, `transforms[{entity, fields, drops}]`, `retire`) and a check in `python/tests/test_schema.py` that the valid fixtures conform.

---

## Phase 2: Foundational (schema identity and exact binding)

**Purpose**: every store knows its schema at every position, and every evaluation and commit is
bound to it (FR-001–FR-005). All stories depend on this.

### Tests first

- [X] T003 [P] Write `crates/behavior-core/tests/schema.rs`:
  - `schema(module).hash` is equal for two modules that differ only in actions, rules or derived values, and differs when an enum gains a value, a field is added or renamed, or a field type changes;
  - `declarations` equals the module's entity name → declaration hash map;
  - a snapshot of the SchemaHash of every `tests/fixtures/wire/valid/*.json`.
- [X] T004 [P] Write `crates/behavior-store/tests/schema_binding.rs`:
  - a module identical in schema but with a new action evaluates and commits against an existing store;
  - a module differing only in an entity type the action does not touch gets `SCHEMA_MISMATCH` from `evaluate` before any decision; the error names the store and module SchemaHashes and the differing types;
  - a bundle evaluated under a matching module but committed with a mismatching module is refused;
  - `schema_at(state_at(0))` equals the genesis schema;
  - `schema_history()` of a store without migrations is `[(0, genesis schema)]`;
  - the head of a store without migrations serializes without a `schema` field.

### Implementation

- [X] T005 Add `crates/behavior-core/src/schema.rs` with `StoreSchema { hash, declarations }` and `pub fn schema(&Module) -> StoreSchema`, hashed with the new tag `behavior.store_schema.v1` over the sorted `(entity name, declaration hash)` pairs (research R1). Add `StoreSchema::of(declarations)` for stored declaration maps, and export it from `crates/behavior-core/src/lib.rs`. Make T003 pass.
- [X] T006 In `crates/behavior-store/src/documents.rs`:
  - add `SchemaRef { hash, declarations, since, migration_record }`;
  - add `Head.schema: Option<SchemaRef>`, with serde default and `skip_serializing_if = "Option::is_none"`;
  - add `StoreError::SchemaMismatch { store, module, differing }` with code `SCHEMA_MISMATCH`.
- [X] T007 In `crates/behavior-store/src/store.rs`:
  - add `schema_at(&StateRef)` and `schema_history()`, which walk back from `head.schema` through migration records, reaching the genesis schema at 0;
  - content-hash entity versions with the declaration in force at the version's `created_at`;
  - replace both touched-types comparisons (evaluation and commit) with `schema(module).hash == schema_at(position).hash`, checked first;
  - keep the bundle's `entity_declarations` and check it against the position's schema.

  Make T004 pass, and keep every existing store test green.
- [X] T008a [P] Write `python/tests/test_schema_binding.py` and `crates/behavior-cli/tests/cli_schema.rs`: `schema_hash` equality and inequality, `schema_at`/`schema_history`, `behavior schema-hash`, and `SCHEMA_MISMATCH` raised from `store.evaluate`. Run them and see them fail.
- [X] T008 Expose the schema:
  - `_engine.Module.schema_hash` and `Store.schema_at` / `schema_history` in `crates/behavior-py/src/lib.rs`;
  - `BehaviorModule.schema_hash`, `Store.schema_at` and `Store.schema_history` in `python/behavior/module.py` and `python/behavior/store.py`;
  - `behavior schema-hash <wire>` in `crates/behavior-cli/src/lib.rs`;
  - the new names and command in `api/public-api.json`.

  Make T008a pass.

**Checkpoint**: exact store-schema binding is in force; existing stores are untouched.

---

## Phase 3: User Story 1 — Evolve a store's schema with an explicit migration (P1) 🎯 MVP

**Goal**: a V1 store with history becomes a V2 store in one atomic migration transition. V2
behavior works on it, V1 behavior is refused, and the history stays readable under V1.

**Independent Test**: the store-level test of T011 passes: create V1, commit, migrate, commit V2,
refuse V1, read the old state.

### Tests first

- [X] T009 [P] [US1] Write `tests/fixtures/migration/build_migrations.py` (independent JSON, as for the wire fixtures) and `crates/behavior-core/tests/migration_admission.rs`.
  - **Valid:** add an enum value with `enum_map`; rename `medium`→`medium_type` (no drop needed, because the assignment reads the old field); add `notes: Option<String>` = `None`; drop `legacy_code`; a retired type; a field changed from `Int` to `Decimal(1)` by an explicit conversion; `Money2 → Money4` via `rescale(old.price, V2.Money4, rounding)`; a type whose only change is its field order, with no `transforms` entry, resolves to all copies and the summary shows it as migrated with every field copied.
  - **Invalid, with codes:**
    - a missing target field: `MISSING_MIGRATION_FIELD`;
    - an unread removed field: `UNACKNOWLEDGED_FIELD_DROP`;
    - a partial `enum_map`: `UNMAPPED_ENUM_VALUE`;
    - an assignment of source-type `Status` to a changed target `Status`: `MIGRATION_TYPE_MISMATCH`;
    - a `Money4 → Money2` assignment without explicit rounding: `MIGRATION_TYPE_MISMATCH`;
    - modules whose schema is not `source`/`target`, or `source == target`: `MIGRATION_SCHEMA_MISMATCH`.
  - **Resolution:** automatic copies appear as explicit `{"copy": f}` in the resolved document, and only for an equal type hash.
  - **Identity:** the migration hash is independent of `name` and `loc`; the summary classifies each field as copied, transformed, new or dropped.
- [X] T010 [P] [US1] Write `crates/behavior-core/tests/migration_apply.rs`. `apply_migration` over a supplied source universe:
  - produces the expected V2 values;
  - preserves identities;
  - gives the same result when the universe is supplied in reversed order;
  - leaves unchanged entity types as they are.
- [X] T011 [P] [US1] Write `crates/behavior-store/tests/migration_store.rs`:
  - **The migration commit:** V1 store, 3 commits, then `migrate`. The result is one new position, every entity of a changed type at `revision + 1` with `created_at` = that position, the state identity recomputed, and `head.schema.since` = that position.
  - **After the migration:** a V2 action commits; a V1 action gets `SCHEMA_MISMATCH`; `load` at a pre-migration state returns the V1 value byte-identical to what was written; `schema_history` = `[(0, V1), (p, V2)]`.
  - **Identities and references:** the identity registry continues (a removed V1 identity stays used); renaming a `Ref` field moves the incoming-reference edges.
  - **Retired types:** a retired type with entities gives `RETIRED_TYPE_NOT_EMPTY`.

### Implementation

- [X] T012 [US1] Add `crates/behavior-core/src/migration/wire.rs`: decode migration IR 0.1, with operators `strict_unwrap`, `enum_map` and `strict_enum_map`, the `side` qualifier on named types, and `{"copy": f}`. Decode errors give `DECODE_ERROR`.
- [X] T013 [US1] Add `crates/behavior-core/src/migration/admit.rs`. `admit_migration(source, target, wire)` does the following:
  - checks the schema hashes;
  - builds the two-sided declaration table (research R5);
  - type-checks requirements as closed source expressions (as module invariants) and transforms with the scope `old` plus constants;
  - applies the assignment rule (equal type hash, or an explicit conversion);
  - resolves fields (FR-007a, FR-007b);
  - records narrowing sites;
  - hashes the canonical resolved document with the tag `behavior.migration.v1`;
  - builds the summary (FR-007c).

  Export it from `lib.rs`. Make T009 pass.
- [X] T014 [US1] Add `crates/behavior-core/src/migration/apply.rs`, with `apply_migration(migration, source, target, facts) -> MigrationOutcome`. This task covers the transform phase only: entity-local, canonical id order, unchanged types copied as they are. Make T010 pass.
- [X] T015 [US1] In `crates/behavior-store/src/documents.rs`:
  - add `TransitionRecord.kind: Option<String>` (omitted for actions);
  - add `migration: Option<MigrationBundle>`, with `MigrationBundle { migration_hash, source, target, previous_schema, requirements, report, verification }`;
  - make `bundle` optional only for `kind == "migration"`, so action records keep their exact bytes. Add a test that a golden action record round-trips byte-identically.
- [X] T016 [US1] Add `Store::migrate(migration, source, target, commit_time, evidence)` to `crates/behavior-store/src/store.rs`, as in research R6:
  - check the current schema, then read all entities at the head through `keys_at`/`version_at`;
  - call `apply_migration`, then build the new versions under the target declarations, the reference changes, the migration record and the head with the new `SchemaRef`;
  - commit with one `Backend::commit`; refusals go through `StoreError`.

  Make T011 pass.
- [X] T017a [US1] Write `python/tests/test_migration.py`: `Migration(...).admit()` (valid, and each admission error code), `.summary()`, `enum_map`/`strict_enum_map`/`strict_unwrap`, `store.migrate` and `apply_migration`. Run it and see it fail.
- [X] T017 [US1] Add the Python authoring layer in `python/behavior/migration.py`:
  - `Migration(source, target, requires, transforms, drops, retire, constants)`, with `.admit()` and `.summary()`;
  - `enum_map`, `strict_enum_map`, `strict_unwrap`; target-side types are resolved from the target module's classes;
  - `Store.migrate` and `behavior.apply_migration`.

  It needs the matching builder and binding support in `crates/behavior-py/src/lib.rs` and the `behavior-core` builder, and new stubs in `_engine.pyi`. Add the exports to `__all__` and `api/public-api.json`. Make T017a pass.
- [X] T018a [US1] Write `crates/behavior-cli/tests/cli_migration.rs` for `behavior migration admit|apply`: output and exit codes 0, 1 and 2. Run it and see it fail.
- [X] T018 [US1] Add `behavior migration admit|apply` to `crates/behavior-cli/src/lib.rs` (contracts/migration-api.md). Make T018a pass.

**Checkpoint**: in-place schema change on a living store works end to end.

---

## Phase 4: User Story 2 — A migration can never produce an invalid state (P1)

**Goal**: requirements, narrowing and full target validation. Every refusal names its rule and the
entities and leaves the store unchanged.

**Independent Test**: the seeded invalid migrations of T019/T020 are all refused with the right
code, and the store is unchanged.

### Tests first

- [X] T019 [P] [US2] Extend `crates/behavior-core/tests/migration_apply.rs`:
  - data written under V1a (no constraint `x > 0`) and migrated with source V1b (same schema, with the constraint) gives `MIGRATION_SOURCE_INVALID`, naming the rule and the entity ids;
  - a requirement `all_(select(Order), o.region.is_some())` with 17 orders lacking a region gives `MIGRATION_REQUIREMENT_FAILED`, naming the requirement, the count 17 and the first ids;
  - `strict_unwrap` of `None` (when no requirement covers it) gives `MIGRATION_TRANSFORM_ERROR`;
  - violated target constraints, entity invariants, module invariants (e.g. `unique`) and a dangling reference each give `MIGRATION_INVALID_RESULT`, naming the rule and the entity ids.
- [X] T020 [P] [US2] Add to `crates/behavior-store/tests/migration_store.rs`:
  - for each refusal, the position, state identity, head bytes and `schema_history` are unchanged;
  - with `FaultInjector` (BeforeWrite/AfterWrite) a migration is all-or-nothing after reopening;
  - an action evaluated before a migration and committed after it is a `StateConflict`;
  - the staged path works: broaden (optional region), backfill action, then narrow with the requirement — refused before the backfill, applied after it (SC-006a).
- [X] T021 [P] [US2] Extend `crates/behavior-store/tests/conformance.rs` with the cases `migration_atomicity` and `schema_history_consistency`, the mutant `DropsHeadSchema` (the head is written without `schema`) failing `schema_history_consistency`, and the existing mutants still failing their own cases.
- [X] T022 [P] [US2] Write `crates/behavior-store/tests/migration_evidence.rs`:
  - a policy without a `migration` section treats migrations like actions;
  - a policy with `migration: {require: commit_authorization}` refuses an unauthorized migration (`EVIDENCE_REQUIRED`) and accepts one authorized by `authorize_migration`;
  - an existing policy's bytes and hash are unchanged;
  - the record states the policy, the authorization, `source_validated` and that target validation passed (FR-019c).

### Implementation

- [X] T023 [US2] Complete `apply.rs`:
  - validate the source universe against the source module first (constraints, entity invariants, module invariants), refusing with `MIGRATION_SOURCE_INVALID`;
  - check requirements against the full source universe first, with counts and examples for `all_`/`not any_`-shaped requirements;
  - run narrowing checks at runtime;
  - validate the target universe in full (constraints, entity invariants, referential integrity, module invariants);
  - return refusals with their rule and entities.

  Make T019 and the store part of T020 pass.
- [X] T024 [US2] Add the conformance cases to `crates/behavior-store/src/conformance.rs` (built-in V1/V2 modules via `include_str!` from `tests/fixtures/migration/`). Make T021 pass.
- [X] T025 [US2] Evidence:
  - `EvidencePolicy.migration: Option<MigrationEvidence>`, omitted when absent, in `documents.rs`;
  - its enforcement in `Store::migrate`;
  - `governance::authorize_migration` in `crates/behavior-verify/src/governance.rs`, plus a Python wrapper.

  Make T022 pass.

---

## Phase 5: User Story 3 — Verify a migration before applying it (P2)

**Goal**: local proofs per type under the named requirements, confirmed counterexamples, and
honest inconclusive results for whole-state properties.

**Independent Test**: `crates/behavior-verify/tests/migration.rs` matches
`tests/fixtures/verify/migration.expected.json`.

- [X] T026 [P] [US3] Create `tests/fixtures/verify/migration.expected.json` and `crates/behavior-verify/tests/migration.rs`:
  - **proven:** an enum widening (`enum_map` total); a strict unwrap under the requirement "every order has a region" (reported with `under: ["every_order_has_region"]`); referential integrity for copied references;
  - **counterexample:** a transform mapping into a value that violates a target constraint, confirmed by `apply_migration` on the reported one-entity universe;
  - **counterexample:** a strict unwrap without a covering requirement;
  - **inconclusive:** a target module invariant over a migrated type that is not an identical copy.
- [X] T027 [US3] Add `crates/behavior-verify/src/migration.rs`, with `verify_migration(migration, source, target, profile, cache, solver) -> Attestation` (research R9):
  - local checks per changed type: narrowing sites, target constraints and invariants, evaluation errors, with source constraints, invariants and requirements assumed through the 007 relational encoding (`e` as a known candidate);
  - whole-state checks for referential integrity and target module invariants;
  - confirmation through `apply_migration`.

  Set `VERIFIER_VERSION` to 0.5.0. Make T026 pass.
- [X] T028a [US3] Extend `cli_migration.rs` and `test_migration.py` with `behavior migration verify` and `behavior.verify_migration`: exit codes 0, 1 and 3, and the attestation content. Run them and see them fail.
- [X] T028 [US3] Add `behavior migration verify` (CLI) and `behavior.verify_migration` (Python). Make T028a pass.

---

## Phase 6: User Story 4 — History and replay span schema generations (P2)

**Goal**: data and behavior replay across any number of migrations; tampering is found at its
position.

**Independent Test**: `crates/behavior-store/tests/migration_replay.rs` passes, including the
tampering proptest.

- [X] T029 [P] [US4] Write `crates/behavior-store/tests/migration_replay.rs`:
  - a generated history of at least 1,000 transitions crossing 3 migrations (V1→V2→V3→V4) replays with `replay_data` and `replay_behavior`, given all modules and migrations;
  - a proptest tampers with a migrated value, a migration record's `migration_hash`, its requirement outcomes and its `previous_schema`, and each is detected at its position (SC-002);
  - an `#[ignore]` 10,000-transition variant;
  - in `python/tests/test_migration.py`, `replay_behavior(store, models, migrations=[...])` over a history crossing one migration reports no divergence.
- [X] T030 [US4] In `crates/behavior-store/src/replay.rs`:
  - **data replay at migration records:** recompute content hashes under the target declarations and the state identity, check `previous_schema`, then continue under the new schema;
  - **behavior replay:** take `migrations: BTreeMap<hash, (Migration, Module, Module)>`, rebuild the source universe at the parent, re-run `apply_migration`, and compare the versions, reference changes and requirement outcomes;
  - keep a compatibility wrapper for the old signature.

  Expose it in Python (`replay_behavior(store, models, migrations=[...])`). Make T029 pass.

---

## Phase 7: User Story 5 — Only real schema changes need migrations (P3)

**Goal**: behavior-only changes are free; any declaration change needs a migration, without
exception.

**Independent Test**: `crates/behavior-store/tests/schema_binding_props.rs` passes.

- [X] T031 [P] [US5] Write `crates/behavior-store/tests/schema_binding_props.rs`, a proptest over random perturbations of a module:
  - behavior-only edits (actions, rules, derived values, queries) always evaluate against the store;
  - every declaration edit (enum value, field added, removed or renamed, field type, reference target) is refused with `SCHEMA_MISMATCH` for every action, whether it touches the edited type or not (SC-006);
  - the same holds after a migration, relative to the new schema.
- [X] T032a [US5] Add assertions to `crates/behavior-store/tests/schema_binding.rs` (the message lists the differing types, sorted, with both declaration hashes) and to `python/tests/test_schema_binding.py` (`CommitRefused.code == "SCHEMA_MISMATCH"` and its details). Run them and see them fail.
- [X] T032 [US5] Make the `SCHEMA_MISMATCH` message list the differing entity types, sorted, and each one's store and module declaration hash. Surface it in Python as `CommitRefused` with `.code == "SCHEMA_MISMATCH"` and the details. Make T031 and T032a pass.

---

## Phase 8: Polish & cross-cutting

- [X] T033 [P] Add the example `examples/schema_evolution/` (`v1.py`, `v2.py`, `v3.py`, `migrations.py`, `run.py`), following quickstart §3: broaden, the refused early narrowing, backfill, narrow, schema history and replay. Add a README section.
- [X] T034 [P] Update the skills (FR-024):
  - `skills/behavior-authoring/SKILL.md` gets "Changing a schema": what needs a migration, automatic copy versus explicit, drops, `enum_map`/strict narrowing, requirements, the staged path, and "migrations change representation; Behavior changes information";
  - `skills/behavior-verification/SKILL.md` gets migration outcomes and "proven under requirements";
  - `skills/behavior-application/SKILL.md` gets `store.migrate`, `SCHEMA_MISMATCH` handling, backfills between migrations, and evidence for migrations.

  Add runnable examples under each skill's `examples/`. `test_skills.py` must pass.
- [X] T035 [P] Add the ignored performance test `crates/behavior-store/tests/migration_perf.rs`: migrating 100,000 entities of a changed type finishes in under 60 s in release, and an injected failure leaves the store as before (SC-005).
- [X] T036 [P] Add the example `crates/behavior-store/examples/migration_history.rs` (a fixed history crossing two migrations, with its records and replay reports) and run it twice in `scripts/determinism-check.sh`. Extend `release/smoke.py` with one migration step, checked in the clean environment.
- [X] T037 [P] Update the documentation:
  - `docs/persistence.md`: a schema identity and migrations section, the record kind and `Head.schema`;
  - `docs/verification.md`: migration verification and precision debt for whole-state properties;
  - `docs/versioning.md`: the migration IR, the 0.9 binding change and the `VERIFIER_VERSION` bump;
  - `README.md`.
- [X] T038 Run all gates and fix all findings: fmt, clippy `-D warnings`, `cargo test --workspace`, pytest, mypy, `scripts/determinism-check.sh`, `cargo test --release --workspace -- --ignored` and `scripts/release-check.sh`. Confirm that the frozen identity tests and every golden are unchanged (SC-007).
- [X] T039 Write `specs/009-schema-evolution/checklists/implementation-review.md`: a review against every FR and SC, the constitution and the principles, with deviations and follow-ups (provable lossless compatibility, relational migration, schema-evolution pressure from TCUP).
- [X] T040 Run quickstart.md §1–§4 and fix any failures.

---

## Dependencies & Execution Order

- **Setup (T001–T002)** first.
- **Foundational (T003–T008)** blocks every story.
- **US1 (T009–T018)** is the MVP. US2–US4 build on its migration machinery.
- **US2 (T019–T025)** needs US1.
- **US3 (T026–T028)** needs US1 (admission and `apply_migration` for confirmation). It can run in parallel with US2 once T014 exists.
- **US4 (T029–T030)** needs US1 and the record format (T015).
- **US5 (T031–T032)** needs only the Foundational phase. It can run in parallel with US1–US4.
- **Polish (T033–T040)** last.

## Parallel Opportunities

- T003 ∥ T004.
- T009 ∥ T010 ∥ T011.
- T019 ∥ T020 ∥ T021 ∥ T022.
- T026 ∥ T029 ∥ T031, once their prerequisites exist.
- T008a with T003/T004; T017a ∥ T018a; T028a with T026.
- T033 ∥ T034 ∥ T035 ∥ T036 ∥ T037.

## Implementation Strategy

1. **MVP:** Setup, then Foundational (exact binding), then US1 (in-place migration). Demonstrate a living store moving from V1 to V2.
2. **Safety:** US2 (requirements, narrowing, full validation, evidence) makes migrations safe for real data.
3. **Proof and audit:** US3 (verification) and US4 (replay across generations).
4. **Hardening:** US5's property tests harden the binding rule.
5. **Last:** Polish, the 0.9.0 release check, and an update to the external application's acceptance run.
