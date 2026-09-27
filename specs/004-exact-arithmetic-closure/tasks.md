---

description: "Task list for feature 004: exact arithmetic closure"
---

# Tasks: Exact Arithmetic Closure

**Input**: Design documents from `/specs/004-exact-arithmetic-closure/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R12), data-model.md,
contracts/numeric-semantics.md, contracts/engine-api.md, quickstart.md

**Tests**: required (constitution III). Test tasks come before the implementation that makes them
pass and are seen failing first. Goldens are regenerated only after the semantic tests are green,
and every regenerated file is reviewed (FR-013, SC-004).

**Organization**: US1 bounded exact domain (P1), US2 exact ratios (P2), US4 general decimals exact
(P2), US3 composition and verification (P3). US4 comes after US2 because it reuses the
dimensionless exact type.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1, US2, US3, US4

---

## Phase 1: Setup

- [X] T001 Record the semantic identity baseline before any change: extend `tests/fixtures/frozen_versions_002.json` (rename to `tests/fixtures/frozen_versions.json`, keeping the 002 entries) with the behavior versions of every current fixture module (`wire/valid/*`, `wire/python/*`, `verify/*`), and add `tests/fixtures/requests/004/` with a README entry in `tests/fixtures/README.md` (feature 004 section)
- [X] T002 [P] Add a small tool `scripts/record-diff.py` that compares two decision records ignoring `record_version` and prints any other difference; used to check that goldens of semantically unchanged modules differ only in `record_version` (research R8)

---

## Phase 2: Foundational (exact type with a unit, wire IR 0.4)

**Purpose**: the type and version changes every story needs. No story work before this phase.

### Tests (write first, must fail)

- [X] T003 [P] Write `crates/behavior-core/tests/ir_0_4.rs`: a `"0.1"`, `"0.2"`, and `"0.3"` document is rejected with `UNSUPPORTED_IR_VERSION` whose message contains "0.4" and "scale"; the same modules re-serialized as `"0.4"` admit with **the same behavior version** as recorded in `tests/fixtures/frozen_versions.json` for every module without decimal arithmetic (`empty`, `dead_and_vacuous`, `overflow`, and each other module whose version must not change per research R11); `to_wire_json` always writes `"0.4"`; `{"t": "exact"}` (no name) and `{"t": "exact", "name": "Plain"}` for an unscaled nominal are accepted as declared derived types
- [X] T004 [P] Extend `python/tests/test_schema.py` for `schema/wire-ir-0.4.schema.json` (every valid wire file validates against the schema of its version; only `"0.4"` files remain under `wire/valid/` and `wire/python/`)

### Implementation

- [X] T005 Generalize the exact type in `crates/behavior-core/src/semantic/types.rs`: `enum Unit { Dimensionless, Nominal(Arc<NominalInfo>) }`, `Type::Exact(Unit)` replacing `Type::Exact(Arc<NominalInfo>)` (display `Exact<Decimal>` / `Exact<Money>`, wire `{"t":"exact"}` / `{"t":"exact","name":…}`), update every match site (admission, hashing with type code `0x24` for dimensionless and `0x23` for any decimal nominal, pretty, value, eval, serialize, builder, PyO3 binding `Type.exact(None | name)`, verifier sort), keeping 003 typing behavior for now
- [X] T006 Implement wire IR 0.4 in `crates/behavior-core/src/wire.rs` (`ir_version "0.4"` only; `0.1`–`0.3` → `UNSUPPORTED_IR_VERSION` with the migration message of contracts/engine-api.md; `{"t":"exact"}` without `name`), `crates/behavior-core/src/serialize.rs` (always `"0.4"`, constraints list always present), `crates/behavior-core/src/admit/mod.rs` (version error message), and `schema/wire-ir-0.4.schema.json`
- [X] T007 Migrate every fixture envelope to 0.4 without changing semantics yet: update `tests/fixtures/wire/build_fixtures.py`, `tests/fixtures/verify/build_verify_fixtures.py`, `tests/fixtures/verify/build_money2_fixtures.py`, and the DSL-emitted files under `tests/fixtures/wire/python/` to write `"0.4"`; regenerate; convert the invalid version fixtures (`ir_version_0_9`, `constraints_in_0_1`, `fixed_scale_in_0_2`) to the new expectations (`UNSUPPORTED_IR_VERSION` for 0.1–0.3); convert `wire/invalid/exact_not_fixed_scale` (an `exact` of an unscaled nominal is valid in 0.4) into `rescale_not_fixed_scale` (a `rescale` target without a scale → `EXACT_NOT_FIXED_SCALE`); make T003 and T004 pass and keep the whole workspace green with 003 semantics

**Checkpoint**: one exact type constructor with a unit; only 0.4 documents; semantic hashes unchanged.

---

## Phase 3: User Story 1 — Exact values never exceed what the runtime can represent (P1) 🎯 MVP

**Goal**: admission-time representation analysis with bit bounds; the runtime's 512-bit check
becomes unreachable (FR-007–FR-009).

**Independent Test**: quickstart §2.

### Tests (write first, must fail)

- [X] T008 [P] [US1] Write `crates/behavior-core/tests/bounds.rs` against the rules of contracts/numeric-semantics.md → Bound analysis: leaf facts (fixed-scale `scale s, cd 28`; general decimal `scale 28, cd 56`; integer `scale 0, cd 19`; literals exact), and for `·`, `±`, `÷` by a literal `m·10^−k` with `|m| = 2^i5^j`, and general `÷` the exact `nb`/`db`/`scale`/`cd` results on hand-built expressions; derived references reuse the body's bound; a chain of divisions by general decimal inputs crosses 511 bits after the computed number of steps
- [X] T009 [P] [US1] Create `tests/fixtures/wire/invalid/exact_bound_exceeded.json` (a derived value dividing a fixed-scale amount by the same general decimal input repeatedly until the bound exceeds 511 bits) with `.expected.json` (`EXACT_BOUND_EXCEEDED` at the expression; the message states required and supported bits) via `tests/fixtures/wire/build_fixtures.py`, and a valid neighbour with one division fewer that still admits
- [X] T010 [P] [US1] Write the SC-001 property test `crates/behavior-core/tests/exact_bound_fuzz.rs`: for every admitted fixture module with exact expressions (`fixed_scale.json` and the 004 fixtures once they exist), ≥ 10,000 randomized evaluations with inputs drawn from the full accepted ranges (extreme magnitudes and 28-digit scales) never produce the internal exact-size error

### Implementation

- [X] T011 [US1] Implement `crates/behavior-core/src/admit/bounds.rs` (research R4): `Facts { nb, db, scale: Option<u8>, cd: Option<u32> }`, leaves, the operation rules, derived-body facts computed once in evaluation order, and `EXACT_BOUND_EXCEEDED` when `nb` or `db` exceeds 511; run it for every exact expression during `admit/typecheck.rs`; make T008 and T009 pass
- [X] T012 [US1] Turn the runtime's 512-bit overflow in `crates/behavior-core/src/exact.rs` / `crates/behavior-core/src/eval.rs` into an internal consistency error (`internal: exact bound exceeded in <expr>`), never an ordinary evaluation error; make T010 pass

**Checkpoint**: no admitted module can reach an exact-size overflow.

---

## Phase 4: User Story 2 — Ratios of amounts stay exact (P2)

**Goal**: same-type division yields `Exact<Decimal>`; ratio algebra (FR-004–FR-006).

**Independent Test**: quickstart §3.

### Tests (write first, must fail)

- [X] T013 [P] [US2] Create `tests/fixtures/wire/valid/exact_closure.json` (two-decimal `Money`; entity `Share{amount, budget, total, part: Money}`; derived `share = amount / budget` declared `{"t":"exact"}`; actions `scaled_share` (`part := rescale((amount / budget) * total, Money, half_even)`), `share_threshold` (precondition `amount / budget <= 0.25`)) and invalid fixtures `ratio_stored` (`part := amount / budget` → `LOSSY_CONVERSION`), `exchange_rate` (`SEK / JPY` → `TYPE_MISMATCH`), `ratio_plus_amount` (`amount / budget + amount` → `TYPE_MISMATCH`), `exact_unwrap_004` (`unwrap(amount * 0.5)` → `LOSSY_CONVERSION`, message says the nominal unit would be erased) via `tests/fixtures/wire/build_fixtures.py`, with requests in `tests/fixtures/requests/004/` and `expectations.json` in the 003 subset format: `share` (`10.00 / 30.00` → derived value `"1/3"`), `scaled_share` (`total 90.00` → `part "30.00"`, one rescale entry with exact `"30"`), `share_threshold_true`/`_false`, `share_zero_budget` (→ `ERROR` `division by zero in share.amount / share.budget`)
- [X] T014 [P] [US2] Extend `crates/behavior-core/tests/fixed_scale_admission.rs` (typing table) with the ratio rows of contracts/numeric-semantics.md: `T ÷ T → Exact<Decimal>` (replacing the 003 row `T ÷ T → Decimal`), `Exact<T> ÷ T`, `T ÷ Exact<T>`, `Exact<T> ÷ Exact<T>` → `Exact<Decimal>`, `Exact<Decimal> × T → Exact<T>`, ratio `± × ÷` ratio/number → `Exact<Decimal>`, comparisons ratio vs number → `Bool`, `T(ratio) → Exact<T>`, `unwrap(Exact<T>)` rejected; and write `crates/behavior-core/tests/evaluate_004.rs` asserting `requests/004/expectations.json` (same subset matcher as `evaluate_003.rs`) plus byte-for-byte replay

### Implementation

- [X] T015 [US2] Implement the ratio rows of research R2 in `crates/behavior-core/src/semantic/types.rs` (`fixed_scale_op` generalized to units), the store rule for fixed-scale targets (`Exact<F>` accepted when the bound analysis proves `scale ≤ scale(F)`, research R3) in `crates/behavior-core/src/admit/typecheck.rs`, ratio evaluation in `crates/behavior-core/src/eval.rs`, and bounds for the new operations in `crates/behavior-core/src/admit/bounds.rs`; make T013 and T014 pass

**Checkpoint**: Money ÷ Money is exact; no implicit rounding remains in fixed-scale arithmetic.

---

## Phase 5: User Story 4 — General decimals compute exactly (P2)

**Goal**: decimal arithmetic is exact; stores into general decimals only when proven
representable; fixtures migrated (FR-012, FR-012a–c, FR-013, FR-016).

**Independent Test**: quickstart §4.

### Tests (write first, must fail)

- [X] T016 [P] [US4] Write `crates/behavior-core/tests/decimal_exact.rs`: typing (`D ± D`, `D × D`, `D ÷ D`, `I ÷ I` → `Exact<Decimal>`; `I ± I` stays `Int`; unscaled `T ± T` → `Exact<T>`); store table of contracts/numeric-semantics.md → Stores (`dec := x - y` → `LOSSY_CONVERSION`; `dec := 10 / 2` admitted and stores `"5"`; `dec := x / 3` → `LOSSY_CONVERSION`; `dec := int_field` admitted; a copy `dec := other_dec` admitted); evaluation of the `project_margin` margin exactly (revenue `100`, cost `33` → `"0.67"`; revenue `3`, cost `2` → `"1/3"`); and a proof that admission does not read entity constraints (adding a constraint that bounds `x` and `y` does not make `dec := x - y` admissible)
- [X] T017 [P] [US4] Create invalid fixtures `decimal_sum_stored` and `decimal_ratio_stored` (unscaled Money `amount - discount` and `x / 3` stored) with `LOSSY_CONVERSION` expectations naming `rescale`, via `tests/fixtures/wire/build_fixtures.py`

### Implementation

- [X] T018 [US4] Implement general-decimal closure: typing rows of research R2 for `D`, `I ÷ I`, unscaled nominals in `crates/behavior-core/src/semantic/types.rs`; the store rules of research R3 (general decimal targets need `scale ≤ 28 ∧ cd ≤ 28` from `bounds.rs`; `LOSSY_CONVERSION` otherwise) in `crates/behavior-core/src/admit/typecheck.rs`, also for `value_or`, `some`, and declared derived types; exact evaluation of all decimal arithmetic in `crates/behavior-core/src/eval.rs` (remove the 28-digit `Dec` arithmetic paths and the 003 "exact region" special case; stores convert `Exact → Dec` exactly); `record_version "0.4"` for all records; make T016 and T017 pass
- [X] T019 [US4] Migrate the semantics of fixtures and examples that store computed money (research R11): declare `scale=2` for `Money` in `tests/fixtures/wire/build_fixtures.py` (`invoice`, `constraints`), `tests/fixtures/verify/build_verify_fixtures.py` (`purchase*`, `unchanged_entity`, `postcondition`, `sum_rounding` rewritten as a representability test), `examples/invoice/behavior.py`, `examples/tryout/model.py` (remove the now-redundant `examples/*/*_money2.py` variants and the `money2` option of `examples/*/dump.py`; keep the `verify/*money2*` fixtures), `examples/project_margin/behavior.py` if needed; update request fixtures whose inputs are no longer on the grid; update `tests/fixtures/requests/003/expectations.json` (`third_via_derived` now stores `"0.01"`: the 003 exact-region rule is gone) and rewrite `crates/behavior-core/tests/evaluate_003.rs::exact_context_differs_from_general_decimals_where_it_matters` to assert that both forms are exact; regenerate `tests/fixtures/hash_vectors.json` (vectors without decimal arithmetic keep their values; changed vectors go into the migration table; update the `tests/fixtures/README.md` sentence about hash vectors to "changes require a reviewed regeneration"); regenerate `tests/fixtures/wire/python/*`, goldens (`BLESS_RECORDS=1`) and `tests/fixtures/versions.json`; run `scripts/record-diff.py` on every regenerated golden and record each file that differs beyond `record_version` in a migration table for the review
- [X] T020 [US4] Make the verifier exact for decimal arithmetic in `crates/behavior-verify/src/encode.rs` (research R9): remove `rounded`, `rounded_sum`, the `exact` hint list tied to them, and the general-decimal `DEC_MAX` overflow obligations; encode decimal `+ − ×` as terms and `÷` as `q·d = n` under `d ≠ 0`; keep integer and fixed-scale range obligations and exact rescale constraints; bump `VERIFIER_VERSION` to `0.4.0` in `crates/behavior-verify/src/lib.rs`
- [X] T021 [US4] Update verification expectations under exact semantics (SC-007): `tests/fixtures/verify/*.expected.json` for `rounding` (tight bound now proven), `purchase_remaining` (proven), `sum_rounding` (representability test), `project_margin` (evaluation errors under exact division), and every fixture whose outcome changes; list each change with the reason in the migration table; all verifier tests green

**Checkpoint**: no implicit rounding anywhere; only rescale rounds; no check inconclusive because of decimal arithmetic.

---

## Phase 6: User Story 3 — Exact arithmetic composes (P3)

**Goal**: a complete, predictable algebra across evaluation, replay, trace, and verification.

**Independent Test**: quickstart §5.

### Tests (write first, must fail)

- [X] T022 [P] [US3] Write `crates/behavior-core/tests/closure_table.rs`: every row of contracts/numeric-semantics.md → Typing checked at admission, then evaluated on example values and replayed, asserting the documented types and exact values (including the user's examples: `amount * 0.25`, `amount / 3`, `amount / budget`, `(amount / budget) * amount`, `amount * 1.25 <= budget`, `money := amount / 3` rejected, `money := rescale(amount / 3, Money, half_even)` valid)
- [X] T023 [P] [US3] Create `tests/fixtures/verify/share_bound.json` via `tests/fixtures/verify/build_money2_fixtures.py` (action `allocate(share: Share, *, amount: Input[Money])` with `requires(amount <= share.budget)`, `requires(share.budget > 0)`, effect `share.part := rescale(share.total * (amount / share.budget), Money, floor)`, invariant/postcondition `share.part <= share.total` under non-negativity constraints; and `allocate_unchecked` without `amount <= budget`) with `.expected.json` (proven / counterexample), and `crates/behavior-verify/tests/closure.rs` asserting it and that counterexamples are confirmed (SC-005)
- [X] T024 [P] [US3] Extend `crates/behavior-cli/tests/cli_fixed_scale.rs` (`behavior admit` of a `"0.3"` document exits 2 with `UNSUPPORTED_IR_VERSION`; `behavior eval` of `wire/valid/exact_closure.json` with `requests/004/share.json` exits 0 with `record_version "0.4"`) and `python/tests/test_fixed_scale.py` (or a new `python/tests/test_exact_closure.py`): `Exact[Decimal]` as a derived annotation; `amount / budget` builds `{"t":"exact"}`; storing a ratio raises `BehaviorTypeError` naming `rescale`; a module with a stored unscaled decimal sum raises `BehaviorTypeError`

### Implementation

- [X] T025 [US3] Close any remaining gaps found by T022–T024 in `crates/behavior-core/src/semantic/types.rs`, `crates/behavior-core/src/admit/bounds.rs`, `crates/behavior-verify/src/encode.rs`, and the DSL (`python/behavior/types.py`: `Exact[Decimal]`, `Exact[T]` for any decimal nominal; `python/behavior/_engine.pyi`); make T022–T024 pass

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T026 [P] Measure admission time of the example modules before and after bound analysis (`crates/behavior-core/tests/perf.rs`, `#[ignore]`): ≤ 10% or 50 ms more (SC-006); keep the verifier perf tests within their limits
- [X] T027 [P] Extend `scripts/determinism-check.sh` to `requests/004/` and the new verify fixtures
- [X] T028 [P] Update `docs/verification.md` (numeric model: exact by default, admission bound, rescale as the only rounding; the "general decimals" and 003 open risks marked resolved), `README.md` (numeric semantics section), and `PRINCIPLES.md` if wording needs to follow the implementation
- [X] T029 Run fmt, clippy (`-D warnings`), `cargo test --workspace`, `pytest python/tests`, `mypy`, `scripts/determinism-check.sh`, and the ignored perf tests; fix all findings
- [X] T030 Write `specs/004-exact-arithmetic-closure/checklists/implementation-review.md`: review against `PRINCIPLES.md` §9–§10 (exact by default, statically proven lossless stores, admission vs verification) and the constitution; the migration table of every fixture whose semantic identity, records, or verification outcome changed (old/new, reason); every deviation from this task list
- [X] T031 Run quickstart.md §1–§5 from a fresh `nix develop` shell and fix any failures

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (1)** → **Foundational (2)** → **US1 (3)** → **US2 (4)** → **US4 (5)** → **US3 (6)** → **Polish (7)**
- US1 needs the unit-carrying exact type (T005); US2 needs the bound analysis for fixed-scale store proofs (T011); US4 needs the dimensionless exact type from US2 and the analysis's `cd` facts; US3 closes over everything.

### Within Each Phase

- Tests first, seen failing
- `types.rs` → `bounds.rs` → `typecheck.rs` → `eval.rs` → verifier → DSL
- Goldens (T019) only after T016–T018 are green; every regenerated golden goes through `scripts/record-diff.py`
- `build_fixtures.py` is edited by T007, T009, T013, T017, T019 in that order; `evaluate_004.rs` by T014; `types.rs` by T005, T015, T018, T025

### Parallel Opportunities

- Setup: T001, T002
- Foundational tests: T003, T004
- US1 tests: T008, T009, T010
- US2 tests: T013, T014
- US4 tests: T016, T017
- US3 tests: T022, T023, T024
- Polish: T026, T027, T028

## Parallel Example: User Story 1

```text
Task: "T008 Write crates/behavior-core/tests/bounds.rs …"
Task: "T009 Create tests/fixtures/wire/invalid/exact_bound_exceeded.json …"
Task: "T010 Write the SC-001 property test crates/behavior-core/tests/exact_bound_fuzz.rs …"
```

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1–2: unit-carrying exact type, IR 0.4, envelopes migrated with unchanged semantic hashes
2. Phase 3 (US1): the soundness gap is closed — "never a wrong proof" holds again for exact arithmetic
3. **Stop and validate** with quickstart §2

### Incremental Delivery

1. + US2: exact ratios
2. + US4: general decimals exact, fixture migration, verifier interval model removed
3. + US3: the full algebra, verified end to end
4. Polish: perf, determinism, docs, review
