# Implementation Review: First-Class Reads

**Feature**: 010 · **Reviewed**: 2026-10-03 · **Tasks**: T001–T059

## Principles

- [x] **Observing the world is not changing it.**
  - `Store::read` and `Store::read_intent` take `&self`; every backend write needs `&mut`, so a
    read cannot write by construction.
  - 1,000+ reads interleaved with transitions and a migration leave the head, every record and
    every state identity byte-identical to the same history without reads
    (`read_history_prop.rs`, SC-002).
- [x] **One semantics.** A read runs on the transition evaluator (`run_read` in `eval.rs`):
  derived items are `DerivedRef` calls with the member (`Projection::item_expr`, shared with the
  verifier), queries use the existing query-fact path, phase is always S.
- [x] **Capabilities are entry points, not building blocks** (`PRINCIPLES.md` §14).
  `READ_CALL_NOT_ALLOWED` for every module expression site (precondition, effect, rule,
  invariant, derived value, read); `DUPLICATE_CAPABILITY` against actions and derived values.
- [x] **Capabilities expose declared information; records preserve complete evidence.**
  `ReadResponse` is a distinct type with only `result`, `value`/`reasons` and `record_id`; a
  derived item's internal reads (the credit limit behind `standing`) appear only in the record
  (`internal_observations_stay_in_the_record`, the CLI and Python tests).
- [x] **Cardinality belongs to the read contract.** A query projection is a list in identity
  order; an entity projection is one record, and an identity that does not exist is
  `INVALID_BINDING`, never an empty result.

## Constitution

| Principle | Status | Evidence |
|---|---|---|
| I. Deterministic core | ✓ | Reads are pure evaluation over one snapshot; canonical identity order; golden read records and the lab example run twice in the determinism check |
| II. AI output validated | ✓ | Read intents are parsed and fully validated (known read, targets and their existence, input types) before evaluation; every problem is listed (`read_intents/` fixtures) |
| III. Test-first | ✓ with notes | Test tasks preceded their implementations and were seen failing (compile or assertion): T002, T003–T006, T013–T015, T021–T024, T033–T036, T041–T045, the T040 and T051 CLI tests, T044 (verifier). **Notes:** (1) T029–T031 (US3) tested `at` handling that T018 had already implemented, because the `Store::read` signature needed `at`; they passed when written and were mutation-checked instead: ignoring `at`, skipping the `state_at` validation, and binding the schema at the head each turn them red. (2) T023's fetch-once test was mutation-checked: without the entity cache a 20-member projection makes 120 `version_at` calls instead of at most 40. |
| IV. Reproducibility | ✓ | Read records are canonical and content-addressed (`record_id`); replay from facts (`replay_read`) and against the store (`Store::replay_read`), byte for byte; goldens in `tests/fixtures/reads/records/` |
| V. Explicit state and auditability | ✓ | Every read emits a record naming the exact state (`data_version`), the read (hash, definition for ad-hoc reads), parameters, observations and result |
| VI. Simplicity | ✓ | No new crate, backend method or store document. Additions as justified in plan.md: the read item, the read record, the response type, wire IR 0.7 |
| Tech constraints | ✓ | fmt, clippy `-D warnings`, no `unsafe`; no unwrap/expect outside tests |

## Requirements

| Requirement | Status | Where |
|---|---|---|
| FR-001 read operation | ✓ | `behavior_core::read`, `run_read`; `read_eval.rs` |
| FR-002 one state | ✓ | plain facts, `StoreFacts { position }`; `read_store.rs`, `read_store_past.rs` |
| FR-003 no bound entity, parameters | ✓ | parameterless reads; `INVALID_INPUT` lists every problem; `INVALID_BINDING` |
| FR-004 full read semantics, no effects | ✓ | queries, folds, derived values, exact arithmetic; no effect syntax in reads; Python refuses `set_`/`create`/`remove`/`requires`/`ensures` in `@read` |
| FR-005 result kinds | ✓ | `VALUE`, `EVALUATION_ERROR` (`UNKNOWN_FACT`), `INVALID_INPUT` (`INCONSISTENT_FACTS`), `INVALID_BINDING`; schema mismatch is an error with no record |
| FR-006 projections over a query or a bound entity | ✓ | `read_eval.rs` projection tests, `read_store.rs` |
| FR-006a derived items with existing semantics | ✓ | `Projection::item_expr`; dependency facts recorded |
| FR-006b one failure fails the read | ✓ | `one_failing_member_fails_the_whole_read` (message names `T#id.item`) |
| FR-007 every projected field observed, absent kept | ✓ | field facts / `observed`; `null` under its key |
| FR-008 unknown items refused | ✓ | `UNKNOWN_PROJECTION_ITEM`, `DUPLICATE_PROJECTION_ITEM`, `INVALID_PROJECTION` fixtures |
| FR-009 read record | ✓ | `ReadRecord`, `record_id`; `read_record.rs` |
| FR-010 deterministic | ✓ | `reads_are_deterministic`, goldens, determinism check |
| FR-011 replay from facts and against the store | ✓ | `read_replay.rs`, `read_store_replay.rs`, proptest |
| FR-012 never history | ✓ | `reads_never_change_the_store`, SC-002 proptest |
| FR-013 schema binding | ✓ | `read_schema.rs` |
| FR-014 the past | ✓ | `read_store_past.rs`, `test_reads_past.py` |
| FR-015 declared reads | ✓ | wire IR 0.7 `reads`, `Kind::Read`; schema unchanged (`a_declared_read_needs_no_migration`) |
| FR-015a entry points only | ✓ | `READ_CALL_NOT_ALLOWED` (six sites), Python `ReadFn.__call__` |
| FR-016 capability boundary | ✓ | `evaluate_read_intent`, `Store::read_intent` (`UNKNOWN_TARGET`) |
| FR-016a response vs record | ✓ | `ReadResponse`; CLI `read-intent` prints only the response |
| FR-017 verifier | ✓ | `behavior_verify::reads`; `tests/fixtures/verify/reads.expected.json` |
| FR-018 Python | ✓ | `@read`, `project`, `evaluate_read`, `Store.read/read_intent/replay_read`, `read_intent`, `replay_read` |
| FR-019 command line | ✓ | `behavior read`, `read-replay`, `read-intent` (`cli_read.rs`) |
| FR-020 skills | ✓ | sections and runnable examples in all three consumer skills; eval cases |
| FR-020a principles | ✓ | `PRINCIPLES.md` §14 and the formal core |
| FR-021 bytes and identities kept | ✓ | hash vectors (insertions only), frozen versions, all goldens and store fixtures unchanged; action intents byte-identical after the shared validator |
| SC-001 questions without read-only actions | ✓ (stand-in) | `examples/lab_reads`, modeled on the external application's read patterns; `test_every_action_changes_something`. The external application itself is not in this repository; its port is the next pressure round |
| SC-002 1,000 reads leave no trace | ✓ | `a_thousand_reads_leave_no_trace_and_all_replay` (1,100+ reads, one migration) |
| SC-003 every record replays, tampering detected | ✓ | same test and the proptest |
| SC-004 5 items × 10,000 < 2 s | ✓ | 1.48 s in release (`read_perf.rs`), 30,000 field facts |
| SC-005 past reads repeat | ✓ | proptest `ReadPast`, `read_store_past.rs` |
| SC-006 agents through declared reads only | ✓ | intent fixtures, type-level response, Python and CLI tests |
| SC-007 bytes and identities | ✓ | as FR-021 |

## Deviations

- **CLI exit codes** follow the CLI's existing convention: `EVALUATION_ERROR` exits 3 (like a
  decision's `ERROR`), a replay mismatch exits 2 (like `replay`). The contract and T040 were
  amended.
- **Wire `over`** is a wire expression (`{"op": "param", …}` or a query), not a `{"param": …}`
  object; the item decoder accepts any non-empty name so admission can refuse reference paths with
  guidance. The contract was amended.
- **`ReadSource::AdHoc(Box<ReadItem>)`**: boxed for clippy's size check.
- **Read intents in plain mode** take only the host-supplied entities that the read binds; a host
  may hold entities for every capability.
- **Verifier checks of reads** name the read as their action (`read:<name>`), so attestations
  group them like action checks.
- **T044's entity-projection case** uses a division instead of an unwrap of absent: outside
  migrations there is no strict unwrap that can fail.
- **The entity fetch cache** (`EvaluationFacts::entity`) also serves transitions. It changes the
  number of backend calls only; recorded facts are identical (every existing golden passes).

## Found during implementation

- **The verifier found a real overflow** in the fixture's `average_ph`: a sum of `Int` fields can
  exceed the `Int` range. It is kept as a confirmed counterexample, documented in the fixture.
- **Precision debt:** `count(select(T)) + 1` is inconclusive for overflow, because counts are
  not bounded in the encoding. The fixture uses a bound the model states (a constraint) instead.
- **A projection of a derived value observes the whole member:** a derived value over a member is
  called with the member's full value, as in query predicates since feature 007, so every field of
  the member becomes a field fact. This is the existing semantics, not a new rule.
- **Two stores with the same genesis are the same store** (same identity): the foreign-store
  replay test needed a store with another genesis.
- **The disk filled** again during the first workspace build (`target/debug`, 21 GB); it was
  cleared, and builds in this session ran without incremental state or debug info.

## Code review fixes

Each fix started with a test that was seen failing.

- **Read intents and host entities** (`host_entities_named_like_inputs_stay_out_of_the_state`):
  a host entity keyed like one of the read's `input` or `context` parameters was copied into
  `state`, so a valid intent was refused. Only `state` parameters take host entities now.
- **A read's identity binds its output keys** (`a_projection_hash_binds_its_item_names`): a
  derived item was hashed by its value's hash only, and derived hashes exclude names, so two
  reads with differently keyed records could share a hash. The item name is hashed too; the
  `read_project_entity` hash vector and the golden records of reads with derived items were
  re-blessed (their content is unchanged).
- **A derived value named `id`** (`derived_named_id`): only a field named `id` was refused; a
  derived one overwrote each record's identity. Any item named `id` is
  `DUPLICATE_PROJECTION_ITEM` now.
- **Counterexamples with a bound entity of the projected type**
  (`a_projection_with_a_bound_entity_of_its_type_is_confirmed`): confirmation built a universe
  without the bound entity, so the facts check refused it and a real counterexample became
  inconclusive. The universe now holds the bound entities of the member's type as well.

## Follow-ups

- **SC-004 headroom (25 %):** a query projection fetches each candidate during membership and
  again for its items; reusing the membership fetch would halve the version reads.
- **Ordering and pagination** stay a semantic gap: projections list members in identity order.
- **Evidence inspection** (`explain_read(record_id)`) as a separate, policy-controlled capability.
- **Python value mapping:** read values are the record's canonical JSON (exact values and
  decimals as strings), not converted to `Decimal`.
- **The external application's port** (the real SC-001 check) is the next pressure round.
