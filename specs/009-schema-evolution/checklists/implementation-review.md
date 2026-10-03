# Implementation Review: Schema Evolution and Migration

**Feature**: 009 · **Reviewed**: 2026-10-02 · **Tasks**: T001–T040 (with T008a, T017a, T018a,
T028a, T032a)

## Principles

- [x] **State is meaningful only under an exact schema; schema changes never reinterpret history.**
  - Every store position has exactly one schema: `schema_at` and `schema_history` walk back
    through migration records only. An entity version is content-hashed under the declaration
    of the position that created it.
  - Loading a pre-migration state returns the stored bytes (`after_the_migration_the_store_speaks_only_the_new_schema`).
- [x] **Migrations change representation; Behavior changes information.**
  - Transforms are entity-local. Admission refuses a transform that reads other entities
    (`NON_LOCAL_TRANSFORM`); queries and `exists`/`referenced` are not available to it.
  - Cross-entity population takes the staged path: broaden, backfill with an ordinary action,
    narrow under a requirement (`the_staged_path_broadens_backfills_and_then_narrows`, the
    example).
- [x] **Broaden freely; narrow only after proving the data already fits.**
  - `strict_unwrap` and `strict_enum_map` are narrowing sites. Each is a verifier obligation
    (proven `under` the named requirements, or a confirmed counterexample) and a runtime check
    (`MIGRATION_TRANSFORM_ERROR`).
  - A requirement that does not hold refuses with `MIGRATION_REQUIREMENT_FAILED`, naming it and
    counting the violators.
- [x] **Implicit in authoring, explicit in the resolved Migration IR.**
  - Admission resolves every automatic copy into `{"copy": f}`, also for changed types the author
    did not mention.
  - The resolved document re-admits to the same migration. The identity is the semantic hash of
    the resolved form; name and locations are excluded.
- [x] **Runtime establishes validity; verification establishes what can be proven; governance
  decides which evidence suffices.**
  - Application always runs source validation, requirements, the transform and target
    validation; no policy skips them.
  - The evidence policy's optional `migration` section decides the evidence. A migration
    authorization binds the migration hash and the store state.

## Constitution

| Principle | Status | Evidence |
|---|---|---|
| I. Deterministic core | ✓ | Transforms are pure and applied in canonical order; the result is independent of the universe's order (`the_result_does_not_depend_on_the_order_of_the_universe`); the migration history example runs twice in the determinism check |
| II. AI output validated | ✓ (n/a) | Agent-written migrations go through the same admission, verification and application |
| III. Test-first | ✓ with one note | Test tasks preceded their implementations and were seen failing (compile or assertion): T002 (fixture check), T003/T004, T008a, T009–T011, T015, T017a, T018a, T021, T022, T026, T028a, T029. **Note:** T019 (apply refusals), T020 (store refusals) and T031/T032a (binding properties, message) tested behavior that T014/T016 and T006/T007 had already implemented, so they passed when written. Each was checked by disabling the guarded code instead: source validation, target validation, requirement checking and the schema binding each turn their tests red |
| IV. Reproducibility | ✓ | Migration records carry everything data replay needs; behavior replay re-runs each migration; replay walks the schema chain itself, never trusting the head |
| V. Explicit state and auditability | ✓ | Explicit schema at every position; records state the migration, schemas, requirement outcomes, the report, source and target validation, the policy, the authorization and the cited verification |
| VI. Simplicity | ✓ | No new crate or backend method. Additions as justified in plan.md: the migration document, the record kind, two-sided typing |
| Tech constraints | ✓ | fmt, clippy `-D warnings`, no `unsafe`; no unwrap/expect outside tests |

## Requirements

| Requirement | Status | Where |
|---|---|---|
| FR-001 store schema, SchemaHash | ✓ | `behavior_core::schema`, `tests/fixtures/schema_hashes.json` |
| FR-002 schema per position | ✓ | `Store::schema_at`, `schema_history`, `schema_history_consistency` |
| FR-003 no reinterpretation | ✓ | binding at every evaluation and commit; replay hashes under the creating position's schema |
| FR-004 behavior-only changes free | ✓ | `behavior_only_edits_always_evaluate` |
| FR-005 exact binding, `SCHEMA_MISMATCH` | ✓ | `schema_binding.rs`, `schema_binding_props.rs` |
| FR-006 migration document | ✓ | migration IR 0.1, `schema/migration-ir-0.1.schema.json` |
| FR-007 entity-local transforms | ✓ | `NON_LOCAL_TRANSFORM`; order independence |
| FR-007a resolution | ✓ | automatic copies, `MISSING_MIGRATION_FIELD` |
| FR-007b explicit drop | ✓ | `UNACKNOWLEDGED_FIELD_DROP` |
| FR-007c summary | ✓ | `Migration::summary`, `migration.summary()` |
| FR-007d source requirements | ✓ | part of the identity; checked first on application |
| FR-007e proven narrowing | ✓ | narrowing sites, `migration_narrowing` checks |
| FR-008 identity preserving | ✓ | `identities_are_preserved_one_to_one`; `id` cannot be assigned |
| FR-009 admission refusals | ✓ | `tests/fixtures/migration/invalid/` (9 cases) |
| FR-010 content-addressed | ✓ | `the_identity_ignores_name_and_locations_but_not_content` |
| FR-011 atomic application, order of checks | ✓ | `Store::migrate`; refusals leave the store unchanged; fault injection |
| FR-012 new types empty, retirement | ✓ | `RETIRED_TYPE_NOT_EMPTY`, `MISSING_RETIREMENT` |
| FR-013 target validation | ✓ | constraints, invariants, references, module invariants (`MIGRATION_INVALID_RESULT`) |
| FR-014 identity registry continues | ✓ | `the_identity_registry_continues_across_a_migration` |
| FR-015 migration record | ✓ | `MigrationBundle`, `kind: "migration"` |
| FR-016 data replay | ✓ | `data_migration` |
| FR-017 behavior replay | ✓ | `replay_behavior_with` |
| FR-018 past states as written | ✓ | load of pre-migration states |
| FR-019 concurrency | ✓ | `an_action_evaluated_before_a_migration_conflicts_after_it` |
| FR-019a mandatory runtime checks | ✓ | not skippable by any input of `Store::migrate` |
| FR-019b policy per transition kind | ✓ | `EvidencePolicy.migration`, `migration_evidence.rs` |
| FR-019c what the record states | ✓ | `source_validated`, `target_validated`, `verification`, policy, authorization |
| FR-020 local and whole-state verification | ✓ | `verify_migration`, `tests/fixtures/verify/migration.expected.json` |
| FR-021 undecidable whole-state properties inconclusive | ✓ | `module_invariant` and non-structural references inconclusive |
| FR-022 Python authoring | ✓ | `Migration`, `enum_map`, `strict_enum_map`, `strict_unwrap`, `Store.migrate` |
| FR-023 command line | ✓ | `behavior schema-hash`, `behavior migration admit|verify|apply` |
| FR-024 skills | ✓ | three skill sections with runnable examples |
| FR-025 bytes and identities kept | ✓ | frozen identity tests, store hash vectors, action records keep their bytes |
| SC-001 TCUP's changes in place | ✓ | enum value, rename, optional field, type change (`int` → `Decimal`, `Money` scale) |
| SC-002 ≥ 1,000 transitions, 3 migrations, tampering | ✓ | `migration_replay.rs` (and an ignored 10,000-transition variant) |
| SC-003 seeded invalid migrations refused | ✓ | admission and application refusal tests |
| SC-004 three outcomes | ✓ | the verifier fixture |
| SC-005 100,000 entities < 60 s | ✓ | 16.7 s in release (`migration_perf.rs`) |
| SC-006 / SC-006a | ✓ | binding properties; the staged path |
| SC-007 bytes and identities | ✓ | all goldens and frozen identities unchanged |

## Deviations

- **Counterexample confirmation** runs the migration's transform and the target entity rules
  on the one reported entity (`transform_one`), not `apply_migration` on a one-entity universe.
  The difference matters for references: a one-entity universe cannot satisfy a source reference,
  while the transform of the entity is exactly what application runs for it.
- **Errors beyond the plan:** `MISSING_RETIREMENT`, `INVALID_RETIREMENT`, `INVALID_TRANSFORM`,
  `NON_LOCAL_TRANSFORM`; `MIGRATION_SOURCE_INVALID` came from the analysis remediation (U1).
- **The `migration` CLI** is one command with subcommands (`admit`, `verify`, `apply`); it has no
  store, so schema history is a store API (FR-023 as amended).
- **Explicit assignments** may use the same lossless implicit conversions as effects
  (`T → Option<T>`, `Int → Decimal`). An explicit assignment is the stated conversion; only
  automatic copies require identical types.

## Found during implementation

- **The verifier found a real defect in our own fixture:** widening `Money` from two to four
  decimals shrinks its range, so prices of 10²⁴ and more overflow on migration. The finding is
  kept as a confirmed `evaluation_error` in the fixture, as documentation of the case.
- **Quadratic field index:** the in-memory backend found an entity's open field-index edge by
  scanning every entity that shares the value. A migration rewrites many such entities in one
  commit, so it took 989 s for 100,000 entities. Edges are now found directly (16.7 s).
- **A transition's cost must not grow with the store:** the schema binding first recomputed the
  genesis schema on every evaluation and commit. That re-hashed the whole genesis, seed included,
  and the ignored test `one_creation_or_removal_does_not_depend_on_the_store_size` caught it. The
  genesis schema is now computed once per `Store`, like the store identity.
- **Replay must not trust later records about earlier schemas:** a tampered `previous_schema`
  was first reported at position 0, because replay took the schema history from the head. Replay
  now walks the chain from the genesis.

## Code review fixes

- **New target rules on unchanged types:** declaration hashes do not cover rules, so a target
  can add a constraint or invariant to a type the migration carries over unchanged. The verifier
  checked only transformed types and could attest `verified` for a migration that application then
  refuses. It now checks every target rule the source does not state identically on unchanged types
  too, with the value copied (`new_rule_on_an_unchanged_type`). `transform_one` checks the target
  rules of unchanged types so that counterexamples are confirmed.
- **Reference replay across a migration:** `replay_index` used one module for the whole history,
  which made a renamed `Ref` field a false divergence. `replay_index_with` takes the module of each
  schema and reads a migration's new values under its target. With one module, a history under
  another schema is reported as `schema`, not as a reference divergence.

## Follow-ups

- **Provable lossless compatibility** without rewriting data (appending an optional field read as
  absent).
- **Relational migrations** reading the immutable source state, if real applications need more
  than backfills between migrations.
- **Precision:** module invariants over migrated values and references that are not carried over
  are `inconclusive` (set-to-set reasoning over transformed predicates).
- **TCUP:** run the external application's schema changes as its next acceptance round.
