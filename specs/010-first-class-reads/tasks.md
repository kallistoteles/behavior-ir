---
description: "Task list for feature 010: first-class reads"
---

# Tasks: First-Class Reads

**Input**: Design documents from `/specs/010-first-class-reads/`:
- plan.md;
- spec.md, with clarifications Q1–Q4 of 2026-10-03;
- research.md (R1–R14);
- data-model.md;
- contracts/ (read-wire, read-api);
- quickstart.md.

**Tests**: test-first (constitution III). Every test task comes before its implementation and
must be seen failing.

**Organization**: tasks are grouped by user story; paths are relative to the repository root.

## Format: `[ID] [P?] [Story] Description`

## Standing rules

These apply to every task:
- **Reads never write.** No read path takes `&mut` of a store or backend, or calls `commit`.
- **One semantics.** Read evaluation reuses the `Evaluator`: field reads, the `DerivedRef` path,
  query facts and field facts. Never add a second implementation of an expression form for reads.
- **Byte stability (FR-021).** Existing modules, records, store documents, hash vectors, frozen
  versions and goldens keep their bytes. A module without reads keeps its behavior version.
- **Disclosure.** `ReadResponse` never gets an evidence field. Nothing outside `read.rs` builds a
  response from a record.

---

## Phase 1: Setup

- [X] T001 Bump the release to 0.10.0:
  - set `[workspace.package] version = "0.10.0"` in `Cargo.toml`;
  - set `release: "0.10.0"` in `api/public-api.json` and in each `skills/*/SKILL.md`
    frontmatter;
  - create `tests/fixtures/reads/{valid,invalid,requests,records}/`,
    `tests/fixtures/read_intents/` and `examples/lab_reads/`;
  - confirm that `cargo test --workspace` and `pytest python/tests` still pass with no golden
    changed.
- [X] T002 [P] Add `schema/wire-ir-0.7.schema.json`. It is the 0.6 schema plus:
  - the `reads` section: `name`, `params[{name, role ∈ {state,input,context}, type}]`,
    `body: {value} | {project: {over, param, items[{field}|{derived}]}}` and `loc`;
  - the read document `{ir_version: "0.7", read}`.

  Extend `python/tests/test_schema.py` so the valid read fixtures conform.

---

## Phase 2: Foundational (read definitions: wire, semantic IR, hashing, records)

**Purpose**: a read definition can be decoded, admitted, hashed and turned into a canonical
record (FR-001, FR-005, FR-009, FR-010, FR-015). All stories depend on this phase.

### Tests first

- [X] T003 [P] Write `crates/behavior-core/tests/read_wire.rs`. Test that:
  - a module with a `reads` section and `ir_version` 0.6 is refused with `NeedsReadVersion`;
  - at 0.7 it decodes;
  - a read document decodes;
  - strict decoding gives a `DECODE_ERROR` with its JSON path for: an unknown key, a body with
    both `value` and `project`, an item with both `field` and `derived`, and a role `read`.
- [X] T004 [P] Write `crates/behavior-core/tests/read_admission.rs` for value reads. Test that:
  - a value read with `state`, `input` and `context` parameters admits;
  - an entity-typed body is `TYPE_MISMATCH`;
  - a query-typed body is `TYPE_MISMATCH` ("a query is not a value");
  - an unknown parameter type is `UNKNOWN_TYPE`;
  - `Module::reads()` lists declared reads in name order;
  - `name_table` has a `(Kind::Read, name)` entry only for declared reads;
  - adding a read changes `behavior_version` but not `schema(module).hash`.
- [X] T005 [P] Extend `tests/fixtures/hash_vectors.json` (via `tests/fixtures/build_hash_vectors.py`)
  and `crates/behavior-core/tests/hash_vectors.rs`:
  - vectors for one value read and one module with a read;
  - every existing vector, frozen version (`tests/fixtures/frozen_versions*.json`) and module
    hash is unchanged.

- [X] T006 [P] Write `crates/behavior-core/tests/read_record.rs`. Test that:
  - a `ReadResponse` serializes to exactly the keys `result`, `value|reasons` and `record_id`;
  - `ReadRecord::from_json(…)` recomputes `record_id`, and the identity changes when any other
    record field changes.

### Implementation

- [X] T007 In `crates/behavior-core/src/wire.rs`:
  - add `IR_VERSION_READS = "0.7"`;
  - add `WRead`, `WReadBody::{Value(WExpr), Project{over: WOver, param, items: Vec<WItem>}}`,
    `WOver::{Param(String), Query(WExpr)}` and `WItem::{Field(String), Derived(String)}`;
  - decode `WModule.reads` and the read document (`decode_read_document`);
  - add `DecodeError::NeedsReadVersion`.

  Make T003 pass.
- [X] T008 In `crates/behavior-core/src/semantic/module.rs`:
  - add `Kind::Read = 8` (`as_str` "read");
  - add `ReadItem { name, params, body: ReadBody, hash, declared, loc }`;
  - add `ReadBody::{Value(Expr), Project(Projection)}`;
  - add `Projection { over: Over::{Query(QueryNode), Param(String)}, entity, member, items:
    Vec<Item::{Field(String), Derived{name, target: Hash}}> }`;
  - add `Module.reads: BTreeMap<String, ReadItem>`, with `reads()` and `read(name)`.
- [X] T009 In `crates/behavior-core/src/admit/hash.rs`:
  - add `TAG_READ = "behavior.read.v1"` and `TAG_PROJECTION = "behavior.projection.v1"`;
  - add `read_hash(params, body)`, which hashes parameter roles and types and the body
    expression, or for a projection `H(TAG_PROJECTION, over, member, items as field names or
    derived hashes)`.
  - Name-table entries for reads only exist when reads exist, so the module hash of read-less
    modules is unchanged.
- [X] T010 In `crates/behavior-core/src/admit/typecheck.rs`, admit `reads`:
  - parameters via `params(..)` with a new `ParamSite::Read`, which allows `state`, `input` and
    `context`, and rejects others with the existing `DECODE_ERROR` at `params[i].role`;
  - a value body is typed in the derived-value context (queries allowed), and an entity-typed
    result is refused;
  - projection bodies are only decoded and stored here; their checks are T025.

  Add `pub fn admit_read(module: &Module, doc: &str) -> Result<ReadItem, AdmissionResult>` with
  `declared = false`. Make T004 and T005 pass.
- [X] T011 Generalize `decode_sections`, `check_snapshot` (through `bound_entities`) and
  `check_query_snapshot` in `crates/behavior-core/src/eval.rs` from `&ActionItem` to
  `&[Param]`. Action call sites keep identical behavior, and every existing record golden and
  eval test passes unchanged.
- [X] T012 Create `crates/behavior-core/src/read.rs` with:
  - `ReadSource::{Declared(String), AdHoc(ReadItem)}`;
  - `ReadRecord`, which wraps canonical JSON with the fields of data-model.md in this order:
    `format`, `behavior_version`, `read`, `data_version`, `state`, `input`, `context`,
    `result`, `value`/`reasons`, `derived`, `observed`, `facts`, `record_id`;
  - `ReadResponse { result, value, reasons, record_id }`;
  - `ReadExecution { response, record }`;
  - `record_id = "read:" + hash_display(H("behavior.read_record.v1", canonical(record without
    record_id))))`.

  `ReadResponse` is only constructible inside `read.rs`. Export the module from
  `crates/behavior-core/src/lib.rs`, and add `IR_VERSION_READS` and the read record format to
  `format_versions()`. Add `ReadRecord::from_json`, which parses a record and checks its
  `record_id`. Make T006 pass.

**Checkpoint**: read definitions admit and hash. Existing identities are unchanged.

---

## Phase 3: User Story 1 - Answer a question about the current state (Priority: P1) 🎯 MVP

**Goal**: evaluate a value read in plain mode and against a store's head, with no bound entity
and no write (FR-001–FR-005, FR-012).

**Independent Test**: commit transitions, then read a count and a derived value. The values are
correct, and the store head, records and state identity are byte-identical before and after.

### Tests for User Story 1

- [X] T013 [P] [US1] Write `crates/behavior-core/tests/read_eval.rs` (plain mode). Test:
  - a parameterless count over supplied universes gives `VALUE`, with the query fact recorded;
  - a read with an `input` parameter and one with a bound `state` entity: the bound entity's
    read fields appear in `observed`;
  - a missing input, a wrong type or an extra key gives `INVALID_INPUT` listing every problem,
    with nothing evaluated (empty `derived` and `facts`);
  - a division by zero gives `EVALUATION_ERROR` with `loc`;
  - a missing supplied fact gives result `EVALUATION_ERROR` with reason `UNKNOWN_FACT`;
  - inconsistent facts give result `INVALID_INPUT` with reason `INCONSISTENT_FACTS`;
  - the same request twice gives byte-identical records and `record_id`;
  - an empty-set aggregate gives the count 0 and an absent minimum (edge case).
- [X] T014 [P] [US1] Write `crates/behavior-store/tests/read_store.rs`. Test that:
  - `Store::read` at the head returns the value;
  - the head, every `record(p)` and `state_at(p)` are byte-identical before and after 100
    reads;
  - an unknown binding identity gives a record with result `INVALID_BINDING`, its existence fact
    (`false`) recorded and empty `derived`; nothing else is evaluated;
  - a binding that names a non-parameter is refused;
  - `data_version` equals `data_version(store_id, head)`;
  - `Store::read` compiles with `&Store` (a `fn(&Store<B>)` helper calls it).
- [X] T015 [P] [US1] Write `python/tests/test_reads.py` (value reads). Test that:
  - `@read` functions with no parameters, an `Input[...]` and an entity parameter trace into
    the module's `reads`;
  - `behavior.evaluate_read(model, r, state=…, facts=…, data_version=…)` gives a `ReadResult` with
    `.value`, `.record_id` and `.record`;
  - an undeclared `@read` passed directly reads ad hoc, and its record carries
    `read.definition`;
  - `set_`, `create`, `remove`, `requires` or `ensures` inside `@read` raise
    `BehaviorDefinitionError`;
  - `store.read(...)` leaves `store.current()` and `list(store.history())` unchanged.

### Implementation for User Story 1

- [X] T016 [US1] Add `run_read` to `crates/behavior-core/src/eval.rs`. It:
  - decodes the parameters (T011) and checks supplied facts;
  - evaluates a `ReadBody::Value` with `Phase::S`;
  - maps errors to `EVALUATION_ERROR` or `UNKNOWN_FACT` via `error_reason`;
  - returns the value, the derived entries (phase `S`), the observed field reads and the
    observed facts.

  It runs no rules or invariants and has no S′. Expose `pub(crate) fn evaluate_read_inner(module,
  read: &ReadItem, request, provider)`.
- [X] T017 [US1] In `crates/behavior-core/src/read.rs`:
  - implement `evaluate_read(module, &ReadSource, request)` (plain mode) and
    `evaluate_read_with(module, &ReadSource, request, &dyn EvaluationFacts)`;
  - parse the request
    `{read, data_version, state, input, context, facts}`, where `read` is a name or
    `{definition}`;
  - an unknown declared read gives `INVALID_INPUT` (`UNKNOWN_READ`);
  - build the record and response (T012).

  Make T013 pass.
- [X] T018 [US1] Add `Store::read(&self, module, &ReadSource, bindings, input, context, at:
  Option<&StateRef>) -> R<ReadExecution>` to `crates/behavior-store/src/store.rs`. It:
  - reads the head once when `at` is `None`;
  - applies `bind_schema` at the position;
  - loads bound `state` parameters with `exists_at` and `version_at`; an identity that does not
    exist at the position gives a record with `INVALID_BINDING` (existence fact recorded);
  - `SCHEMA_MISMATCH` and a foreign or future `StateRef` stay `Err`, with no record;
  - evaluates with `StoreFacts { position }` and `data_version(store, at)`.

  Make T014 pass for the head.
- [X] T019 [US1] Bind reads in `crates/behavior-py/src/lib.rs`:
  - the module gets reads (`Module.reads`);
  - add `evaluate_read` (plain), `Store.read`, `admit_read`, and the `ReadRecord` and
    `ReadResponse` pyclasses;
  - add stubs in `python/behavior/_engine.pyi`.
- [X] T020 [US1] Add the Python DSL for reads:
  - in `python/behavior/decl.py`, add `@read` / `ReadFn`, and make effect and condition
    statements inside a read raise `BehaviorDefinitionError`;
  - in `python/behavior/module.py`, add `BehaviorModule(reads=[...])` and `_trace_read`, which
    emits `ir_version` 0.7 only when reads exist;
  - in `python/behavior/reads.py`, add `evaluate_read()` (not `read`, which is only the decorator),
    `ReadResult`, `ReadResponse` and `ReadRecord`;
  - in `python/behavior/store.py`, add `Store.read`;
  - export from `python/behavior/__init__.py`.

  Make T015 pass.

**Checkpoint**: US1 is fully usable. Reads answer questions with no placeholder entity.

---

## Phase 4: User Story 2 - Read whole records with an explicit projection (Priority: P1)

**Goal**: query projections (a list) and entity projections (one record) of stored fields and
derived values, where every projected field is observed, absent values stay `null`, and one
failure fails the whole read (FR-006–FR-008).

**Independent Test**: a filtered query projection gives exactly the matching members in id
order, with exactly the projected items. An entity projection gives one record. The observations
cover every projected stored field.

### Tests for User Story 2

- [X] T021 [P] [US2] Add invalid fixtures to `tests/fixtures/reads/invalid/` with
  `.expected.json` files, and a runner in `crates/behavior-core/tests/read_admission.rs`. The
  expected codes are:

  | Code | Cases |
  |---|---|
  | `UNKNOWN_PROJECTION_ITEM` | an unknown field; `customer.name` (a reference path); a derived value with two parameters; a derived value over another entity type |
  | `DUPLICATE_PROJECTION_ITEM` | a repeated field; a field and a derived item with the same name |
  | `INVALID_PROJECTION` | `over` naming an `input` parameter; `over` as a non-query expression |
  | `NON_LOCAL_PREDICATE` | a `where` that reads another entity |
- [X] T022 [P] [US2] Extend `crates/behavior-core/tests/read_eval.rs` with projections:
  - a query projection of `[name, stage, age(derived)]` over universes gives a list sorted by
    `id`, each record `{"id", "name", "stage", "age"}` and nothing more;
  - an absent `stage` is `null`, present under its key;
  - `facts.fields` holds every projected stored field of every member, even when a derived item
    did not read it;
  - a derived item's dependency facts (its own query and field facts) are recorded;
  - an entity projection gives one object, and its stored fields are in `observed` as
    `[param, field]`;
  - a derived item failing for one member gives `EVALUATION_ERROR` naming `T#id` and the item,
    with no `value`;
  - an empty query gives `[]`.
- [X] T023 [P] [US2] Extend `crates/behavior-store/tests/read_store.rs`:
  - a store query projection equals the plain projection over the same state (with universes
    built from the store);
  - an entity projection of an unknown id gives `INVALID_BINDING` before evaluation;
  - a `CountingBackend` wrapper counts `version_at` calls: at most one per member, plus the
    query's own reads (R8).
- [X] T024 [P] [US2] Extend `python/tests/test_reads.py`:
  - `project(select(C).where(...), lambda c: [c.name, c.stage, age(c)])` gives a `list[dict]`;
  - `project(order, lambda o: [o.status, total(o)])` gives a `dict`;
  - the item `o.customer` traversal, `o.amount + 1` or `total(other)` raises
    `BehaviorDefinitionError` (`UNKNOWN_PROJECTION_ITEM`);
  - an absent value maps to `None` under its key.

### Implementation for User Story 2

- [X] T025 [US2] Add projection admission in `crates/behavior-core/src/admit/typecheck.rs`. It
  resolves:
  - `over` to a query node of `T` (reusing query typing, so `NON_LOCAL_PREDICATE` applies) or
    to a `state` parameter of type `Entity<T>`, otherwise `INVALID_PROJECTION`;
  - items to `Field` (a declared field of `T`) or `Derived` (exactly one parameter, of type
    `Entity<T>`), otherwise `UNKNOWN_PROJECTION_ITEM` naming the item;
  - duplicates and field/derived name clashes to `DUPLICATE_PROJECTION_ITEM`.

  `id` is implicit. Make T021 pass.
- [X] T026 [US2] Add a defaulted `fn entity(&self, entity, id) -> Result<Json, FactError>` to
  `EvaluationFacts` in `crates/behavior-core/src/facts.rs`, answered through `field`-by-field
  defaults (`Facts` answers it from its universe or field facts). Override it in `StoreFacts`
  in `crates/behavior-store/src/store.rs` with one `version_at`.
- [X] T027 [US2] Extend `run_read` in `crates/behavior-core/src/eval.rs` with `ReadBody::Project`:
  - for a query, get the members through the existing query-fact path and sort them by id;
  - for each member, fetch the entity once (`entity`) and record a field fact for every
    projected stored field, explicitly and never through short-circuiting;
  - evaluate derived items through `DerivedRef` with the member as the candidate;
  - for an entity projection, read the bound value and record `(param, field)` observations;
  - stop at the first failure with `EVALUATION_ERROR`, with the message
    `"<T>#<id>.<item>: <cause>"`.

  The output is `[{"id", items…}]` or `{"id", items…}`. Make T022 and T023 pass.
- [X] T028 [US2] Add `project(over, items)` to `python/behavior/query.py`:
  - `over` is a `Query` or an entity parameter;
  - `items` is a lambda over the member returning a list;
  - each element is resolved to a field or a derived item over the member, otherwise
    `BehaviorDefinitionError` is raised;
  - emit the `project` wire body;
  - map the results to `list[dict]` or `dict` in `python/behavior/reads.py`;
  - export from `python/behavior/__init__.py`.

  Make T024 pass.

**Checkpoint**: US1 and US2 together remove TCUP's need for read-only actions.

---

## Phase 5: User Story 3 - Read the past exactly (Priority: P2)

**Goal**: reads at a past position give that state's result under that position's schema
(FR-002, FR-013, FR-014).

**Independent Test**: read at p, commit more, read at p again; the records are identical. Across
a migration, the old module reads old positions and the new module is refused there.

### Tests for User Story 3

- [X] T029 [P] [US3] Write `crates/behavior-store/tests/read_store_past.rs`:
  - a read at `state_at(4)` with the store at 10 equals a read recorded when the store was at 4,
    byte for byte, including `data_version` and `record_id`;
  - a `StateRef` that is not a state of this store is refused (as `load` refuses it);
  - a position beyond the head is refused.
- [X] T030 [P] [US3] Write `crates/behavior-store/tests/read_schema.rs` using the
  `tests/fixtures/migration/` modules:
  - after migrating at position 6, a read at 4 with v1 succeeds;
  - a read at 4 with v2 gives `SCHEMA_MISMATCH` before evaluation;
  - a read at the head with v2 succeeds;
  - a read at the head with v1 gives `SCHEMA_MISMATCH`.
- [X] T031 [P] [US3] Write `python/tests/test_reads_past.py`: `store.read(model, r,
  at=store.state_at(p))` matches an earlier record, and a schema mismatch raises
  `CommitRefused` with code `SCHEMA_MISMATCH`.

### Implementation for User Story 3

- [X] T032 [US3] In `Store::read` (`crates/behavior-store/src/store.rs`):
  - validate `at` (`state_at(at.position) == *at`);
  - bind the schema with `bind_schema(module, at.position)`;
  - load bindings and facts at `at.position`.

  Pass `at` through `crates/behavior-py/src/lib.rs` and `python/behavior/store.py`. Make
  T029–T031 pass.

**Checkpoint**: history is queryable with present semantics.

---

## Phase 6: User Story 4 - Reads are reproducible evidence (Priority: P2)

**Goal**: read records replay from their facts and against the store, and tampering is reported
(FR-009–FR-012).

**Independent Test**: a record replays byte for byte both ways. An altered result, fact, state
reference or `record_id` is reported at its first differing path.

### Tests for User Story 4

- [X] T033 [P] [US4] Write `crates/behavior-core/tests/read_replay.rs`. Test that:
  - plain replay of declared, ad-hoc, value, projection, error and `INVALID_BINDING` records
    matches;
  - a mutated `value`, a fact (a query member, a field fact), `input`, `read.hash`,
    `behavior_version` or `record_id` gives `matches: false` with the first differing path;
  - an ad-hoc record replays from its `read.definition` alone.
- [X] T034 [P] [US4] Write `crates/behavior-store/tests/read_store_replay.rs`:
  - `Store::replay_read` matches for records taken at the head and at past positions;
  - a record whose `data_version` names another store or another state at that position is
    reported;
  - a record taken before a commit still replays against the store at its own position after
    the commit.
- [X] T035 [P] [US4] Write a proptest, `crates/behavior-store/tests/read_history_prop.rs`, over
  generated histories of up to 1,000 operations interleaving reads, transitions and one
  migration (generator reused from `crates/behavior-store/tests/migration_replay.rs`). It checks:
  - the head, the records and the versions equal those of the same history without reads
    (SC-002);
  - every read record replays plain and against the store (SC-003);
  - a random mutation of any record field is detected;
  - reads at sampled past positions repeated at the end are identical (SC-005).
- [X] T036 [P] [US4] Add golden read records:
  - `tests/fixtures/reads/requests/*.json` (value, query projection, entity projection,
    evaluation error, invalid input);
  - `tests/fixtures/reads/records/*.expected.json`;
  - a runner in `crates/behavior-core/tests/read_goldens.rs`;
  - register them in `scripts/determinism-check.sh`.

### Implementation for User Story 4

- [X] T037 [US4] Implement `replay_read(module, record) -> ReplayResult` in
  `crates/behavior-core/src/read.rs`. It:
  - checks `behavior_version`;
  - resolves the read (a declared read by name with a matching hash, or an ad-hoc read
    re-admitted from `definition`);
  - re-evaluates with the record's `state`, `input`, `context`, `data_version` and `facts`;
  - compares with `record::first_difference` (made `pub(crate)`).

  Make T033 and T036 pass.
- [X] T038 [US4] Implement `Store::replay_read(&self, module, record) -> R<ReplayResult>` in
  `crates/behavior-store/src/store.rs`. It:
  - parses `data_version` into the store identity and `StateRef`;
  - refuses a foreign store or state;
  - recovers the bindings from `state` ids, and the read source;
  - re-reads at that position and compares.

  Make T034 and T035 pass.
- [X] T039 [US4] Bind `replay_read` (plain and store) in `crates/behavior-py/src/lib.rs`,
  `python/behavior/reads.py` and `python/behavior/store.py`, with tests in a new
  `python/tests/test_reads_replay.py`.
- [X] T040 [US4] Add the CLI commands `behavior read <module> <request>` (exit 0, 3 or 2, the
  CLI's convention) and `behavior read-replay <module> <record>` (exit 0 or 2, as `replay`) in `crates/behavior-cli/src/lib.rs`.
  Test them in a new `crates/behavior-cli/tests/cli_read.rs` with the T036 fixtures, including a tampered
  record (exit 1).

**Checkpoint**: every read is auditable.

---

## Phase 7: User Story 5 - Declared reads as capabilities (Priority: P3)

**Goal**: agents call declared reads by name through a validated boundary and get only the
result and the record identity. Declared reads are entry points only, and the verifier checks
them (FR-015a, FR-016, FR-016a, FR-017).

**Independent Test**:
- valid read intents return a `ReadResponse` without evidence;
- malformed intents are rejected with every problem listed;
- a module calling a declared read is refused with `READ_CALL_NOT_ALLOWED`;
- adding a read needs no migration.

### Tests for User Story 5

- [X] T041 [P] [US5] Add fixtures to `tests/fixtures/reads/invalid/`, with expected outputs:
  - `READ_CALL_NOT_ALLOWED`, separately for an action precondition, an effect value, a rule, an
    invariant, a derived value and another read that call a declared read; the message says
    "move the shared computation into a derived value";
  - `DUPLICATE_CAPABILITY`, for a read and an action with the same name, and for a read and a
    derived value with the same name.
- [X] T042 [P] [US5] Add read-intent fixtures in `tests/fixtures/read_intents/*.json`, with
  `.expected.json`, and a runner in `crates/behavior-core/tests/read_intents.rs`. Fixtures:
  - an unknown read, and an action name (`UNKNOWN_CAPABILITY`);
  - a missing target, an extra target, a missing input, an extra input and a wrong type;
  - an intent carrying `definition` or `context` (`EXTRA_ARGUMENT`);
  - every problem listed at once.

  Also test that:
  - a valid intent's `response` serializes to exactly the keys `result`, `value` and
    `record_id`;
  - the `record` of a projection whose derived item reads an internal field holds that field
    fact, and the response does not;
  - an action intent naming a read is `UNKNOWN_CAPABILITY`.
- [X] T043 [P] [US5] Write `crates/behavior-store/tests/read_store_intent.rs`:
  - `Store::read_intent` at the head and at a past position;
  - adding a declared read to the store's module evaluates and commits with no migration (the
    `schema_hash` is unchanged);
  - a nonexistent target together with a wrong input type is rejected with both problems
    (`UNKNOWN_TARGET`, `WRONG_TYPE`), and nothing is evaluated.
- [X] T044 [P] [US5] Add verifier fixtures in `tests/fixtures/verify/reads.expected.json`, plus
  a module in `tests/fixtures/verify/`, and a runner in a new
  `crates/behavior-verify/tests/reads.rs` (following `tests/migration.rs`). Cases:
  - `read:average_ph` (division by `count`) gives a confirmed counterexample with an empty
    state;
  - the guarded version is proven;
  - a query projection whose derived item divides by a field that the `where` predicate
    guarantees non-zero is proven, and the same item without the filter gives a
    counterexample;
  - an entity projection with an unwrap of absent gives a counterexample.

  Every existing verify fixture is unchanged except for `verifier_version`.
- [X] T045 [P] [US5] Write `python/tests/test_reads_capabilities.py`:
  - calling a `@read` inside an action, a derived value or another read raises
    `BehaviorDefinitionError` (`READ_CALL_NOT_ALLOWED`);
  - `store.read_intent` returns `ReadExecution`, whose `response` has no `record` or `facts`
    attribute;
  - `IntentRejected` lists every problem.

### Implementation for User Story 5

- [X] T046 [US5] Add to `crates/behavior-core/src/admit/typecheck.rs`:
  - a `Derived` call whose name is a declared read (in any module expression, read bodies
    included) gives `READ_CALL_NOT_ALLOWED` with the guidance message, checked before
    `UNKNOWN_DERIVED`;
  - a read and an action or a derived value with the same name give `DUPLICATE_CAPABILITY`.

  Make T041 pass.
- [X] T047 [US5] Extract the parameter validation of `crates/behavior-core/src/intent.rs` into a
  shared `fn check_intent(params: &[Param], obj, host_state) -> Vec<InputProblem>`, keeping
  action-intent outputs byte-identical (the existing `tests/fixtures/intents` must pass).
  Implement `evaluate_read_intent(module, intent, host) -> Result<ReadExecution,
  IntentRejection>` in `crates/behavior-core/src/read.rs`. It:
  - allows only `capability`, `targets` and `input`;
  - resolves declared reads only;
  - builds the read request from host state and context.

  Make T042 pass.
- [X] T048 [US5] Add `Store::read_intent(&self, module, intent, context, at) ->
  R<Result<ReadExecution, IntentRejection>>` to `crates/behavior-store/src/store.rs`. It
  validates the intent shape and every target's existence at the position into one
  `IntentRejection` (`UNKNOWN_TARGET` for a missing one), then calls `Store::read` with the
  targets as bindings. Make T043 pass.
- [X] T049 [US5] Add to `crates/behavior-verify/src/checks.rs` and
  `crates/behavior-verify/src/encode.rs` / `encode/relational.rs`:
  - `evaluation_error` for every declared read, with subject `read:<name>`;
  - a value body is encoded like an action's expressions over the bound entities and the state;
  - a projection's derived items are encoded over a symbolic member that satisfies the query's
    candidate-local predicate (`where` gives ∧, `union` ∨, `intersection` ∧, `difference`
    a ∧ ¬b);
  - an entity projection is encoded over the bound entity;
  - confirmation evaluates the read on the concrete counterexample state (`confirm.rs`).

  Set `VERIFIER_VERSION = "0.6.0"` in `crates/behavior-verify/src/lib.rs`. Make T044 pass.
- [X] T050 [US5] Add `READ_CALL_NOT_ALLOWED` detection to `python/behavior/decl.py`: calling a
  `ReadFn` inside a trace scope raises. Add `read_intent` (plain and store) and `ReadExecution`
  to `python/behavior/reads.py` and `python/behavior/store.py`, and the bindings to
  `crates/behavior-py/src/lib.rs`. Make T045 pass.
- [X] T051 [US5] Add the CLI command `behavior read-intent <module> <intent> <host> [--record
  <path>]` in `crates/behavior-cli/src/lib.rs`. It prints only the response and writes the
  record to `--record`. Test it in `crates/behavior-cli/tests/cli_read.rs`.

**Checkpoint**: the read/transition split is a capability boundary for agents.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T052 [P] Add `examples/lab_reads/{model,run}.py` (SC-001), modeled on the external
  application's read patterns:
  - a whole-record view (entity projection), a filtered list (query projection), a count, an
    existence check and a past-position audit;
  - a declared-read capability set used through `read_intent`;
  - no placeholder entity, and no action without effects.

  Add `python/tests/test_lab_reads.py`, which runs the example and asserts that every action
  has an effect, and register the example in `scripts/determinism-check.sh`.
- [X] T053 [P] Add an ignored release test in `crates/behavior-store/tests/read_perf.rs`
  (SC-004): a 5-item projection (3 fields and 2 derived values) over 10,000 entities on
  `InMemoryBackend` completes in under 2 s, with exactly 30,000 projected field facts recorded.
- [X] T054 [P] Amend `PRINCIPLES.md` (FR-020a): "Capabilities are entry points, not building
  blocks. Derived values compose semantics; declared reads and actions expose capabilities."
  Add "Capabilities expose declared information; records preserve complete evidence."
- [X] T055 [P] Update the skills (FR-020), with runnable examples checked by the skill gates:
  - `skills/behavior-application/SKILL.md`: when to read instead of transition, store reads,
    past positions, read intents, and replaying read records. Example:
    `skills/behavior-application/examples/reading_state.py`.
  - `skills/behavior-authoring/SKILL.md`: declaring reads, projections, the entry-point rule.
    Example: `skills/behavior-authoring/examples/declaring_reads.py`.
  - `skills/behavior-verification/SKILL.md`: evaluation errors in declared reads.
  - `skills/evals/`: add read cases.
- [X] T056 [P] Update the docs:
  - `docs/versioning.md`: 0.10.0 is a minor bump for wire IR 0.7, the read record format and
    verifier 0.6.0;
  - `docs/persistence.md`: reads take `&self`, and read records are never stored;
  - `docs/verification.md`: read subjects;
  - `README.md`: the runtime model table (read / transition / migration).
- [X] T057 [P] Update `api/public-api.json` with every new Python name, Rust entry and CLI
  command from contracts/read-api.md, and make the public-API check pass.
- [X] T058 Run the full gates from quickstart.md:
  - fmt, clippy `-D warnings`, `cargo test --workspace`, `pytest`, `mypy`;
  - `scripts/determinism-check.sh`;
  - `cargo test --release -- --ignored`;
  - `scripts/release-check.sh --skip-gates`.

  Walk quickstart sections 2–7 by hand.
- [X] T059 Write `specs/010-first-class-reads/checklists/implementation-review.md`. Map each
  FR and SC to its test, and record any test that was written after its implementation, with
  its mutation check.

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T012)** → user stories.
- **US1 (T013–T020)** is the MVP. The other stories build on its `run_read`, `Store::read` and
  Python `evaluate_read`.
- **US2 (T021–T028)** depends on US1.
- **US3 (T029–T032)** depends on US1 only.
- **US4 (T033–T040)** depends on US1. Its goldens (T036) gain projection cases once US2 is done.
- **US5 (T041–T051)** depends on US1. Its verifier part (T049) needs US2 for projections.
- US3, US4 and US5 can proceed in parallel after US1 (and after US2 for their projection
  cases).
- **Polish (T052–T059)** comes after all stories. T058 and T059 come last.

Within each story, the test tasks come first and must fail before the implementation starts.
Tasks in the same file are sequential.

## Parallel Examples

- **Foundational tests**: T003, T004, T005 and T006 together (different files).
- **US1 tests**: T013 (core), T014 (store) and T015 (Python) together.
- **US2 tests**: T021, T022, T023 and T024 together, then T025, then T026 and T027, then T028.
- **After US2**: T029–T031 (US3), T033–T036 (US4) and T041–T045 (US5) test-writing can run
  side by side.
- **Polish**: T052–T057 in parallel.

## Implementation Strategy

1. **MVP**: Setup, Foundational and US1. Validate quickstart §2: value reads with no placeholder
   entity and no writes.
2. **Add US2**: projections. Together with US1 this replaces TCUP's read-only actions (SC-001
   can be checked early).
3. **Add US3 and US4**: history and evidence.
4. **Add US5**: capabilities, the entry-point rule and the verifier.
5. **Polish**: the example, performance, principles, skills, docs, the API manifest and the
   gates.
