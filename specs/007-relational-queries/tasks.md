---

description: "Task list for feature 007: relational queries and set semantics"
---

# Tasks: Relational Queries and Set Semantics

**Input**: Design documents from `/specs/007-relational-queries/`

**Prerequisites**: plan.md, spec.md (FR-001–FR-025, FR-009a, FR-013a, FR-020a), research.md
(R1–R14), data-model.md, contracts/query-api.md, quickstart.md

**Tests**: required by the constitution (principle III). Test tasks come before the implementation
that makes them pass and are seen failing first. **Existing identities, goldens and persistence
documents must stay byte-identical** (SC-006); every task that touches formats re-runs the frozen
checks.

**Organization**:
- US1: decide over a set of entities (P1);
- US2: quantifiers, counts, aggregates, set algebra, uniqueness and module invariants (P1);
- US3: queries on the resulting state (P2);
- US4: reproducible, verifiable and auditable queries (P2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Snapshot the identities of every admitted fixture module before 007 into
  `tests/fixtures/frozen_versions_007.json` (behavior versions and item hashes of `wire/valid`,
  `wire/python` and `verify` modules, including `accounts.json`), with the same shape as
  `frozen_versions_006.json`
- [X] T002 Add the query fixture module to `tests/fixtures/wire/build_fixtures.py`, written to
  `tests/fixtures/wire/valid/orders.json` (wire `0.6`):
  - **Entities:** `Customer{name: String, credit_limit: Money (scale 2), region: String}`,
    `Order{customer: Id<Customer>, amount: Money, status: OrderStatus, region: String}` (a plain identity, so a customer whose orders are all closed can be removed, spec US1) with
    `enum OrderStatus {open, closed, blocked}`, `Employee{personnel_number: String}`.
  - **Constraint:** `Order.amount >= 0`.
  - **Module invariant:** `personnel_numbers_unique = unique(select(Employee), e → e.personnel_number)`.
  - **Derived value:** `open_order_count(customer) = count(where(select(Order), o → o.customer ==
    customer.id and o.status == open))`.
  - **Actions** ("orders of customer" is `where(select(Order), o → o.customer == customer.id)`):
    - `close_customer(customer)`: `requires(open_order_count(customer) == 0)`, `remove(customer)`;
    - `place_order(customer; order_id: input Id<Order>, amount: input Money)`:
      `requires(sum(orders of customer, o → o.amount) + amount <= customer.credit_limit)`, create an
      open order, `ensures(sum(orders of customer, o → o.amount) <= customer.credit_limit)`;
    - `place_order_unchecked`: the same without the precondition (a seeded defect for SC-005: the
      postcondition has a counterexample);
    - `check_orders(customer)`: preconditions using `any` (no blocked order), `all` (amounts
      positive), `min`/`max` (optional results), and a union/difference of two queries;
    - `raise_limit(customer; limit: input Money)`: `requires(limit >= customer.credit_limit)`,
      `requires(count(where(select(Order), o → o.amount > customer.credit_limit)) == 0)`,
      `customer.credit_limit := limit`, `ensures(count(where(select(Order), o → o.amount >
      customer.credit_limit)) == 0)`. True, but the S' query is a new instance (changed capture)
      with a fresh summary; the candidate counterexample fails runtime confirmation
      (`INCONSISTENT_FACTS` against the S instance's empty result), so the outcome is inconclusive;
    - `hire(employee_id: input Id<Employee>, number: input String)`:
      `requires(not any(select(Employee), e → e.personnel_number == number))`, creates an employee
      (module invariant preserved: proven);
    - `hire_unchecked`: the same without the precondition (counterexample: a duplicate number);
    - `renumber(employee; number: input String)`: `employee.personnel_number := number`;
    - `add_counted_order(customer; order_id, amount)`: `requires(count(orders of customer) == 3)`,
      creates an order for `customer`, `ensures(count(orders of customer) == 4)` (proven);
    - `add_other_customer_order(customer, other; order_id, amount)`:
      `requires(count(orders of customer) == 0)`, creates an order for `other`,
      `ensures(count(orders of customer) == 0)` (proven);
    - `add_counted_sum(customer; order_id, amount)`:
      `requires(sum(orders of customer, o → o.amount) == Money(100))`, creates an order of `amount`,
      `ensures(sum(orders of customer, o → o.amount) == Money(100) + amount)` (proven);
    - `remove_cheapest(order, customer)`: `requires(order.customer == customer.id)`,
      `requires(min(orders of customer, o → o.amount) == some(order.amount))`, `remove(order)`,
      `ensures(min(orders of customer, o → o.amount).value_or(order.amount) >= order.amount)`
      (inconclusive: true, since every remaining order is at least the removed minimum, but `min`
      after a removal is a fresh value; a candidate counterexample needs a smaller member, which
      makes the precondition false at runtime, so confirmation fails).
  - **Invalid fixtures:**
    - `query_in_constraint` (`QUERY_NOT_ALLOWED`);
    - `query_in_entity_invariant` (`QUERY_NOT_ALLOWED`);
    - `nested_query` (`QUERY_NOT_ALLOWED`: a query inside a `where` body);
    - `exists_in_filter` (`NON_LOCAL_PREDICATE`);
    - `mixed_set_types` (`TYPE_MISMATCH`: union of `Order` and `Customer` queries);
    - `sum_of_string` (`TYPE_MISMATCH`);
    - `min_of_unordered` (`TYPE_MISMATCH`: `min` over an enum);
    - `module_invariant_reads_input` (`UNKNOWN_PARAM`: a module invariant has no parameters, so any
      parameter reference is unknown; no new rule);
    - `query_as_effect` (`DECODE_ERROR`: an effect target is a `{param, field}` object, so a query
      node there does not decode);
    - `query_in_0_5` (`UNSUPPORTED_IR_VERSION`).

  Regenerate the fixtures.
- [X] T003 [P] Create `tests/fixtures/requests/007/` (plain-evaluation requests with `queries` and
  `fields` facts, and `expectations.json` in the subset format of feature 004), and add a "Feature
  007" section to `tests/fixtures/README.md`

---

## Phase 2: Foundational (wire 0.6, semantic forms, identity, facts)

**Purpose**: the IR forms, typing, hashing, query identity and the facts interface every story
needs. No story work before this phase.

### Tests (write first, must fail)

- [X] T004 [P] Write `crates/behavior-core/tests/ir_0_6.rs`:
  - **Frozen identities:** every module in `frozen_versions_006.json` and `frozen_versions_007.json`
    keeps its behavior version and item hashes, and serializes with its previous version (SC-006).
  - **Version boundary:** each 007 form in a 0.5 (and 0.4) document is `UNSUPPORTED_IR_VERSION`,
    with a message naming the form; a module with a 007 form serializes as `0.6`.
  - **Round trip:** `orders.json` round-trips with an equal behavior version.
  - **Admission:** every invalid fixture of T002 produces its code.
  - **Identity:** renaming a lambda parameter or a Python-level alias keeps the definition hash;
    changing the predicate changes it; two occurrences of one query in different actions share a
    definition hash.
- [X] T005 [P] Write `crates/behavior-core/tests/queries.rs` (plain evaluation with facts):
  - `count` over a supplied instance; empty-set values of `any`, `all`, `count`, `sum`, `min`, `max`;
  - exact `sum` (10.00 + 20.00 + 0.01 = 30.01, `Money`); `min`/`max` optional results;
  - union, intersection, difference;
  - `any`/`all` visit members in canonical identity order and record only the field facts they read
    (FR-020a); permuted `members`/`fields` input gives byte-identical records (SC-008 unit case);
  - `UNKNOWN_FACT` for a missing query or field fact;
  - `INCONSISTENT_FACTS`: a listed member marked absent, a bound entity listed but failing the
    predicate (or satisfying it but unlisted), a member whose supplied fields fail the predicate, a
    duplicated instance;
  - records with query facts are `record_version "0.6"`; records without are byte-identical to
    before; replay matches.

### Implementation

- [X] T006 Extend `crates/behavior-core/src/wire.rs` and `serialize.rs`: `IR_VERSION_QUERIES =
  "0.6"`; decoding of the 007 ops (`select`, `where`, `union`, `intersection`, `difference`,
  `count`, `any`, `all`, `sum`, `min`, `max`, `unique`, with `param`/`body` where applicable) and of
  module invariants (no `entity`/`param`); accept 0.4–0.6 and reject 007 forms below 0.6 naming
  the form; serialize the minimal version; add `schema/wire-ir-0.6.schema.json` and extend
  `python/tests/test_schema.py`
- [X] T007 Extend the semantic IR:
  - **Types and nodes** in `crates/behavior-core/src/semantic/{types,expr,module}.rs`:
    `Type::Query(entity)`; `ExprKind::{Select, Where, SetOp, Count, Quantifier, Sum, MinMax,
    Unique, Candidate}`; `InvariantItem` for module invariants (no entity/param).
  - **Typing** in `crates/behavior-core/src/admit/typecheck.rs`: lambda scopes with the candidate;
    candidate-local rules (`QUERY_NOT_ALLOWED`, `NON_LOCAL_PREDICATE`); `sum` only over additive
    numeric types with a canonical zero, result type of repeated exact addition; `min`/`max` over
    ordered types → `Option`; `Query<T>` accepted only by relational forms; module invariants are
    closed state expressions; queries refused in entity constraints and per-entity invariants.
  - **Hashing** in `crates/behavior-core/src/admit/hash.rs`: codes for the new nodes and type, the
    candidate marker (lambda parameter names not hashed), a tag for module invariants. Modules
    without 007 forms hash exactly as before.
  - **Pretty-printing** in `pretty.rs` and the round-trip test `tests/pretty_roundtrip.rs`.

  Make T004 pass.
- [X] T008 Implement `crates/behavior-core/src/query.rs` and extend `facts.rs`:
  - query identity: definition hash, capture reads (sorted by `(param, field)`), canonical capture
    values, instance id `sha256("behavior.query_instance.v1" ‖ definition ‖ captures)`;
  - `QueryPredicate` (membership of a candidate value given captures);
  - `EvaluationFacts::{query, field}`; `Facts` gains `queries` and `fields` (parse, canonical JSON,
    snapshot checks of research R5).

  Also in this task: extend `crates/behavior-core/src/eval.rs` with relational evaluation on S
  (canonical order, short-circuiting, bound members from bound values, other members' fields as
  observed field facts), and record 0.6 for records with query or field facts. Make T005 pass.

**Checkpoint**: 007 forms parse, type, hash and evaluate on S with supplied facts; existing bytes
are unchanged.

---

## Phase 3: User Story 1 - Decide over a set of entities (Priority: P1) 🎯 MVP

**Goal**: a store answers queries as of the evaluated position; decisions record the instance and
its members.

**Independent Test**: `close_customer` is allowed with only closed orders (empty result recorded)
and denied with one open order (exactly that order recorded); results do not depend on backend
iteration order.

### Tests (write first, must fail)

- [X] T009 [P] [US1] Write `crates/behavior-store/tests/queries.rs`:
  - `close_customer` allowed/denied with the recorded members; removed orders are never members;
  - `read_facts` carries `queries` (with `result_hash`) and `fields`; a forged `read_facts` is
    `BUNDLE_INVALID` at commit;
  - results with and without a field index are equal, and a backend returning `keys_at` in
    reversed or shuffled order gives byte-identical records (SC-002 unit case);
  - `keys_at` / `keys_by_field_at` answer as of a position;
  - the per-instance `result_hash` changes when a matching entity is created, even though every
    previous member is unchanged (FR-015).

### Implementation

- [X] T010 [US1] Backend primitives in `crates/behavior-store/src/lib.rs`: `keys_at(entity_type,
  position)` and `keys_by_field_at(entity_type, field, value, position)` (default `None`);
  implement both as derived indexes in `crates/behavior-store/src/memory.rs` (updated from each
  commit's versions and removals and from the genesis seed); forward them in the conformance
  wrappers of `crates/behavior-store/src/conformance.rs` and the test backends
- [X] T011 [US1] Store query answering in `crates/behavior-store/src/store.rs`: `StoreFacts::query`
  (candidates from `keys_by_field_at` for an indexable equality conjunct, else `keys_at`; predicate
  re-evaluated on every candidate's version at the position) and `StoreFacts::field`; bundle
  `read_facts` with `queries`, `fields` and per-instance `result_hash`; commit re-derivation. Make
  T009 pass.

**Checkpoint**: decisions over sets work in stores (MVP).

---

## Phase 4: User Story 2 - Quantifiers, aggregates, set algebra, uniqueness (Priority: P1)

**Goal**: `any`, `all`, `count`, `sum`, `min`, `max`, set algebra and `unique` in stores; module
invariants at genesis and on resulting states.

**Independent Test**: each operator over 0, 1 and many members; creating a second employee with an
existing number is refused; a genesis seed with duplicate numbers is refused.

### Tests (write first, must fail)

- [X] T012 [P] [US2] Write `crates/behavior-core/tests/module_invariants.rs`: module invariant
  admission (closed expressions only), evaluation on S' with supplied facts (`INVARIANT_VIOLATED`,
  phase `invariant_global`), the dependency skip (an unrelated field change does not evaluate the
  invariant; a creation, removal or key change does), and the `unique` delta rule (a derived
  instance `key == k` is recorded instead of the full membership)
- [X] T013 [P] [US2] Write `crates/behavior-store/tests/aggregates.rs`: `check_orders` with every
  operator in a store; `hire` with a duplicate number refused; `renumber` to a free number allowed;
  a genesis with duplicate numbers is `GENESIS_INVALID`

### Implementation

- [X] T014 [US2] Module invariants in `crates/behavior-core/src/eval.rs` and `query.rs`: checked on
  S' after outgoing rules (phase `invariant_global`), skipped only by the sound dependency analysis
  of research R7 (queried types and read fields versus the transition's creations, removals and
  field effects), `unique` by the delta rule; make T012 pass
- [X] T015 [US2] Genesis checks of module invariants in `crates/behavior-store/src/store.rs`
  (answered from the seed, extending `SeedFacts` with `query` and `field`); make T013 pass

**Checkpoint**: every relational operator and module invariant works in stores.

---

## Phase 5: User Story 3 - Queries on the resulting state (Priority: P2)

**Goal**: `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS` for every query evaluated
after the effects.

**Independent Test**: `place_order`'s postcondition sees the created order; `raise_limit` records a
second instance for the changed capture; derived S' results equal full re-evaluation.

### Tests (write first, must fail)

- [X] T016 [P] [US3] Write `crates/behavior-core/tests/result_state_queries.rs`: created members
  join, removed leave, bound entities re-classified on their S' values; a changed capture records
  the baseline instance with S' captures as an observed S fact; nothing else is re-read
- [X] T017 [P] [US3] Write `crates/behavior-store/tests/result_state_props.rs`: proptest over
  generated stores and transitions (creations, removals, bound field changes, capture changes)
  comparing the engine's derived S' result with a full re-evaluation on the committed S' (SC-004;
  10,000 cases in an `#[ignore]` release variant)

### Implementation

- [X] T018 [US3] S' derivation in `crates/behavior-core/src/query.rs` and `eval.rs`: captures in
  S' (FR-013a); baseline query fact for the S' instance against S; removed, bound and created
  candidates re-classified; unbound members keep membership and their S field facts. Make T016 and
  T017 pass.

**Checkpoint**: postconditions and invariants over sets see the proposed state exactly.

---

## Phase 6: User Story 4 - Reproducible, verifiable and auditable queries (Priority: P2)

**Goal**: replay without the store, tamper detection, conformance, and verification with the proof
boundary of FR-017.

**Independent Test**: records with query facts replay from the record alone; tampering is found;
verification fixtures give the expected proven, counterexample and inconclusive outcomes.

### Tests (write first, must fail)

- [X] T019 [P] [US4] Write `crates/behavior-store/tests/query_replay.rs`: a generated history of
  1,000 transitions using queries replays both ways; a proptest tampers with a recorded member or
  field value (`decision`) and with the query section's result hash; an `#[ignore]` 10,000-transition
  variant (SC-003)
- [X] T020 [P] [US4] Extend `crates/behavior-store/tests/conformance.rs` with the 4 cases of
  contracts/query-api.md (`query_snapshot`, `query_index_consistency`, `query_order_independence`,
  `module_invariant_preserved`) and mutants: `StaleFieldIndex` → `query_index_consistency`,
  `CurrentOnlyKeys` (`keys_at` ignores the position) → `query_snapshot`; plus the positive control
  `ReversedKeys` (reverses `keys_at` and `keys_by_field_at`), asserted in a separate test to pass
  every case, including `query_order_independence` (backend order is never semantic)
- [X] T021 [P] [US4] Create `tests/fixtures/verify/queries.expected.json` (over `orders.json`) and
  `crates/behavior-verify/tests/queries.rs`:
  - **proven:** `add_counted_order` (count exactly `n + 1`), `add_other_customer_order` (a
    non-matching creation keeps `count == 0`), `hire` (`unique` preserved), `add_counted_sum` (a sum
    grows by exactly the created amount), `place_order` (credit limit);
  - **counterexample** (runtime-confirmed with its facts): `place_order_unchecked` (credit-limit
    sum), `hire_unchecked` (duplicate number);
  - **inconclusive:** `raise_limit` (changed capture) and `remove_cheapest` (`min` after a removal).
    Nothing proven outside the model (SC-005).

### Implementation

- [X] T022 [US4] Replay in `crates/behavior-store/src/replay.rs`: behavior replay compares
  re-derived query and field facts (and result hashes) with `read_facts`; plain replay uses the
  recorded facts. Make T019 pass.
- [X] T023 [US4] Conformance cases in `crates/behavior-store/src/conformance.rs` (built-in `orders`
  module via `include_str!`); keep the existing 24. Make T020 pass.
- [X] T024 [US4] Verifier support in `crates/behavior-verify/src/encode.rs`, `checks.rs` and
  `confirm.rs` (research R10): per-instance summaries over S constrained by known candidates;
  exact deltas for a fixed instance under the path condition that captures are equal; fresh
  summaries for a changed instance; operator-specific precision (fresh `min`/`max` after
  removals/changes); module invariants assumed on S and checked on S' (subject kind
  `invariant_global`); quantifiers normalized onto counts of the corresponding narrowed instance
  (`any(q, p) ≡ count(where(q, p)) > 0`, `all(q, p) ≡ count(where(q, not p)) == 0`), so a
  precondition such as `not any(select(Employee), e → e.personnel_number == number)` and the
  `unique` delta rule share one summary; counterexample concretization with explicit unknown-member slots
  producing `queries` and `fields` facts, never used for proofs. Make T021 pass.

**Checkpoint**: relational decisions are replayable, conformance-checked and verified.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T025 [P] Python DSL and binding:
  - **DSL** in `python/behavior/`: `select(T)` returning `Query` with `.where`, `.union`,
    `.intersection`, `.difference`; `count`, `any_`, `all_`, `sum_`, `min_`, `max_`,
    `unique(q, by=...)`; `@invariant` without parameters as a module invariant; `Query.__iter__`,
    `__len__`, `__bool__` raise `BehaviorDefinitionError`; `evaluate(..., facts=...)` accepts
    `queries` and `fields`.
  - **Binding** in `crates/behavior-py/src/lib.rs` (builder nodes for the new forms and module
    invariants; Python backends with `keys_at` and optional `keys_by_field_at`) and
    `python/behavior/_engine.pyi`.
  - **Tests:** `python/tests/test_queries.py`, and the new conformance cases for `DictBackend` in
    `python/tests/test_store_conformance.py`.
- [X] T026 [P] CLI: `behavior eval` and `behavior verify` accept 0.6 modules and requests with
  query facts; tests in `crates/behavior-cli/tests/cli_queries.rs`
- [X] T027 [P] Add the ignored perf test `crates/behavior-store/tests/query_perf.rs` (SC-007): a
  selective query (≤ 10 matches) over 100,000 entities with a field index under 50 ms in release,
  and within 2× of a store with 1,000 entities; the same query without the index returns the same
  result
- [X] T028 [P] Add the ignored order-permutation test `crates/behavior-store/tests/query_order_props.rs`
  (SC-002, SC-008): 1,000 permutations of backend iteration order and rebuilt indexes give
  identical results, traces and records
- [X] T029 [P] Add the example `examples/orders/` (`behavior.py`, `run.py`) following quickstart §2,
  with a README section
- [X] T030 [P] Extend `scripts/determinism-check.sh`: a fixed query history built twice (example
  `query_history` in `crates/behavior-store/examples/`), the `requests/007` evaluations and
  replays, and `examples.orders.run`
- [X] T031 [P] Documentation: `docs/persistence.md` (query facts, indexes, the evaluation snapshot,
  record size), `docs/verification.md` (symbolic S, exact change, the proof boundary), and the
  README language section
- [X] T032 Run all gates (fmt, clippy `-D warnings`, `cargo test --workspace`, `pytest`, `mypy`, the
  determinism check, the ignored release tests) and fix all findings
- [X] T033 Write `specs/007-relational-queries/checklists/implementation-review.md`: a review
  against FR/SC, the constitution and the principles, with deviations and follow-ups. Include an
  explicit FR-022 item: evaluation reaches entity sets only through `EvaluationFacts::query` /
  `field`, answered by the store from versions or by supplied facts; no other host callback
  exists in the engine API
- [X] T034 Run quickstart.md §1–§4 and fix any failures

---

## Dependencies & Execution Order

- **Setup (T001–T003)** → **Foundational (T004–T008)** → stories.
- **US1 (T009–T011)** needs the foundational phase. It is the MVP.
- **US2 (T012–T015)** needs US1 (store answering).
- **US3 (T016–T018)** needs the foundational phase; its store proptest (T017) needs US1.
- **US4 (T019–T024)** needs US1–US3.
- **Polish** after the stories; T025 (Python) may start once US2 is done.
- **Shared files:**
  - `eval.rs`: T008 → T014 → T018;
  - `query.rs`: T008 → T014 → T018;
  - `store.rs`: T011 → T015;
  - `lib.rs` / `memory.rs` of `behavior-store`: T010;
  - `encode.rs`: T024 only.

## Parallel Opportunities

- Setup T003 with T002; foundational tests T004 and T005.
- Test groups: T012/T013, T016/T017, T019/T020/T021.
- Polish: T025–T031.

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 (decisions over sets in stores).
2. **Add US2**: every operator and module invariants.
3. **Add US3**: exact resulting-state queries.
4. **Add US4**: replay, conformance, verification.
5. **Polish**: Python, CLI, performance, example, determinism, docs, review.
