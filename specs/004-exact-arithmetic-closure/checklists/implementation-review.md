# Implementation Review: Exact Arithmetic Closure

**Feature**: 004 · **Reviewed**: 2026-09-27 · **Tasks**: T001–T031

## Principles (PRINCIPLES.md §9–§10)

- [x] **Numeric computation is exact by default.** Every decimal operation is typed exact
  (`Exact<Decimal>`, `Exact<T>`) or stays on a fixed-scale grid (`F ± F`, `F × I`). The runtime
  evaluates all decimal arithmetic on bounded rationals, and the 28-digit `Dec` arithmetic paths
  in `eval.rs` are removed. `project_margin` gives `0.67` and `1/3` exactly
  (`tests/decimal_exact.rs`), and the 29 rows of the typing table are checked in
  `tests/closure_table.rs`.
- [x] **Bounded representation and rounding are explicit.** The only rounding is `rescale`,
  in six modes with no default. Stored values leave the exact domain only through `rescale` or
  through a store that admission proves representable.
- [x] **Lossless changes are implicit only when statically proven.** Stores of `Exact<T>` into
  `T`, or `Exact<Decimal>` into `Decimal`, are accepted by typing and then decided by the bound
  analysis:
  - fixed-scale targets need `scale ≤ scale(F)`;
  - decimal targets need `scale ≤ 28 ∧ cd ≤ 28`.

  Only types and literals are read. Provenance does not matter: `dec := other_dec` is admitted
  because it has the same type, and `dec := x - y` is rejected even when entity constraints bound
  `x` and `y` (`admission_does_not_read_entity_constraints`).
- [x] **Admission proves representational validity; verification proves reachable behavioral
  safety.** Admission rejects exact intermediates over 511 bits (`EXACT_BOUND_EXCEEDED`). The
  fuzz test covers 10,000 cases, and no admitted module reaches the runtime's internal bound
  error. Fixed-scale range overflow stays a runtime error, which the verifier finds
  (`sum_rounding`, `share_bound`).
- [x] **Verification comes from semantics.** The verifier encodes decimal `+ − ×` as exact terms
  and `÷` as `q·d = n` under `d ≠ 0`. The rounding-interval model, `DEC_MAX` obligations and
  "no rounding" model levels are removed. `VERIFIER_VERSION` is `0.4.0`.

## Constitution

- [x] **I Deterministic core:** `scripts/determinism-check.sh` is OK, extended to `requests/004/`
  and `share_bound`.
- [x] **III Test-first:** the US1, US2 and US4 tests were written and failing before their
  implementation (T008–T010 → T011–T012, T013–T014 → T015, T016–T017 → T018).
- [x] **IV Replay:** every 004 record replays (`evaluate_004.rs`, `closure_table.rs`). Records carry
  `record_version "0.4"`.
- [x] **VI Simplicity:** one exact domain replaces the "exact region" of 003 and the verifier's
  interval model.
- [x] **Quality gates:** all green.
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
  - `cargo test --workspace` passes.
  - `pytest` passes 77 tests, and mypy is clean.
  - The ignored perf tests pass: admission, SC-006 bound-analysis overhead (at most 0.1% of
    admission), and the verifier perf tests.

## Migration table

Machine-readable form: `tests/fixtures/migration_004.json`, checked by
`crates/behavior-core/tests/compat_003.rs`. Every other module in `frozen_versions.json` keeps
its identity. The UNCHANGED list is checked in `ir_0_4.rs`: empty, dead_and_vacuous, overflow,
purchase_money2 and its two variants, invoice_money2, rescale_modes.

### Semantic identity (behavior version)

| Module | Reason |
|---|---|
| `wire/valid/invoice.json`, `wire/python/invoice.json`, `wire/valid/constraints.json` | Money declared `scale=2` (it stores computed amounts) |
| `verify/purchase*.json` (three), `verify/unchanged_entity.json`, `verify/postcondition.json` | Money declared `scale=2` |
| `wire/valid/project_margin.json`, `wire/python/project_margin.json` | `margin` is an exact ratio (typed `Exact<Decimal>`); Money stays unscaled (nothing stored) |
| `wire/valid/fixed_scale.json` | `1 / 3` is an exact ratio everywhere; `third_via_derived` stores `0.01` (was `0.00`) |
| `verify/rounding.json` | `a / b` is an exact ratio |
| `verify/discount_rescale.json` | `1 - rate` is typed `Exact<Decimal>` (was `Decimal`, already evaluated exactly inside the 003 exact region). The meaning is unchanged, but the identity changes because the node hash includes the result type |
| `verify/sum_rounding.json` | Rewritten as a representability test: a fixed-scale sum whose range overflow must be found |
| `hash_vectors.json`: `expr_sub`, `expr_mul`, `expr_div` | Decimal arithmetic is typed `Exact<Decimal>` (reviewed regeneration) |

### Records (goldens, `scripts/record-diff.py`)

No decision changed.
- `invoice_*` and `discount_breaks_invariant`: action, behavior and trace hashes changed; money
  values print with two decimals (`"43200.00"`); the input error message says "at most 2 decimal
  places".
- `margin_*`: only hashes changed (derived values `0.02` / `false` are unchanged).
- `requests/002` (`transfer_ok`, `constraint_post_violation`): money prints with two decimals.
- `requests/003/third_via_derived`: stores `0.01` (was `0.00`).

### Verification outcomes (SC-007)

| Fixture | Change |
|---|---|
| `rounding` | `third_tight` is proven (was inconclusive) |
| `sum_rounding` | The postcondition is proven (was a counterexample caused by rounding). New: an `evaluation_error` counterexample, because the Money sum can leave its range |
| `project_margin` | The two numeric-overflow checks on the margin are gone (an exact value that is never stored cannot overflow); division by zero is unchanged |
| `postcondition` | Subjects print `Money(0.00)`; outcomes unchanged |
| `purchase_remaining` | No longer inconclusive |
| `purchase_fixed` and `unchanged_entity` (full profile) | Genuine range-overflow `evaluation_error` for `spent + amount` (26 integer digits at scale 2) |
| `share_bound` (new) | `allocate` is proven. `allocate_unchecked` has a postcondition counterexample and a range-overflow counterexample, both confirmed by the runtime |

## Deviations from the task list

1. **Integer lifting.** Outside fixed-scale contexts, integer operands keep the explicit
   `ToDecimal` conversion of 001–003. This way `x + 1` ≡ `x + 1.0` and `r < 1` ≡ `r < 1.0`
   keep one identity (`hash_properties.rs`), and hash churn stays minimal. Fixed-scale contexts
   keep 003's typing, so their frozen identities hold.
2. **Conservative limit (not final semantics).** `value_or`, `some` and declared derived types
   with an exact value always require an explicit `rescale` (`LOSSY_CONVERSION` /
   `DECLARED_TYPE_MISMATCH`), even when admission could prove the value representable.
   - This is safe, but narrower than the rule "lossless representation changes may be implicit
     when statically proven".
   - In effect, some syntactic forms do not yet allow proven lossless narrowing.
   - It is a deliberate, temporary imprecision of the admission analysis, not a special rule to
     keep. It is tracked as follow-up 2 below.
3. **Where unscaled stores are rejected (T024).** A stored unscaled decimal sum is typed by the
   DSL builder and rejected at admission (`BehaviorInvalid`, `LOSSY_CONVERSION` naming
   `rescale`), not with `BehaviorTypeError` at build time. The proof needs the bound analysis.
   Storing a ratio into Money is still a build-time `BehaviorTypeError`.
4. **`exact_unwrap_004` fixture (T013) not created.** The `unwrap(Exact)` message ("erase its
   nominal unit") is asserted in `fixed_scale_admission.rs` instead.
5. **Fixture choices.**
   - `dead_and_vacuous` keeps an unscaled `PlainMoney` so its identity is unchanged.
   - `project_margin` keeps unscaled Money (it stores nothing computed).
   - `sum_rounding` uses fixed-scale Money.
6. **Governance and CLI.** No fixture is inconclusive because of rounding any more.
   - The waiver tests and the determinism script use **forced inconclusive** findings from
     `rlimit: 1` (`governance/forced_inconclusive_profile.json`, `forced_inconclusive()` in
     `governance.rs`).
   - The tests assert every such check has reason `resource_limit`, and that the same module is
     decided under the default budget (`forced_inconclusive_module_is_decided_under_the_default_budget`).
     Nobody should read these findings as the model being hard for the solver.
   - The CLI's "verified" checks use `governance/preservation_profile.json`, because the purchase
     fixtures have the genuine range finding under the full profile. These tests assert
     preservation only. They never claim the whole module is verified.
   - The README example points at `purchase_money2_remaining` (verified under the full profile).
7. **`compat_003.rs`.** It was converted from "all frozen" (003's SC-004) to "frozen or listed in
   the migration table". Listed modules must actually have changed.
8. **SC-006 measurement (T026).** Measured as the cost of the bound analysis against the whole
   admission time, rather than a build without the analysis.

## Expression rendering (fixed in 004)

Traces, check subjects and counterexamples are evidence: they are used for human review, AI
explanation and audit. Invariant: **a rendered expression preserves the structure of the
semantic AST unambiguously; a trace or check subject never implies a different expression
tree.**

- **What was wrong:** `pretty.rs` dropped the parentheses of a right operand with equal
  precedence. `a * (b / c)` printed as `a * b / c`, and `a + (b - c)` as `a + b - c`. It also
  flattened a nested `and`/`or`, and printed `Money(ratio)` as `Exact<Money>(…)`. These faults
  predate 004.
- **The fix:** binary operators are rendered left-associative. A right operand of equal
  precedence keeps its parentheses, and a nested `and`/`or` keeps its grouping. `wrap` prints the
  nominal's name.
- **The test:** `crates/behavior-core/tests/pretty_roundtrip.rs` reads every rendered expression
  back with a small reader for the rendered syntax. It compares the fully parenthesized tree:
  `parse(pretty(e))` must equal `e` structurally. This covers every sub-expression of every
  fixture module, 1,000 generated expressions and fixed shapes.
- **Impact:** the only changed golden text is the `share_bound` check subject, now
  `rescale(share.total * (amount / share.budget), Money, floor)`.

## Code review fixes

1. **Soundness: fixed-scale range inside exact arithmetic.** The runtime range-checked a
   fixed-scale sum under `underlying(...)`, but the verifier looked through it without an
   obligation. Without the wrapper, neither side checked. Fixed strictly: an `F`-typed node
   (`F ± F`, `F × I`) is a value of `F` wherever it occurs.
   - `eval_exact` checks its range.
   - `encode_exact` adds the same obligation.

   Regression tests: `closure_table.rs::fixed_scale_nodes_inside_exact_arithmetic_are_range_checked`
   and `closure.rs::fixed_scale_sums_inside_exact_arithmetic_have_range_obligations`, which
   checks for a confirmed counterexample in both forms. No fixture outcome changed.
2. **Error code.** Wrapping a non-numeric value into a fixed-scale nominal is now `TYPE_MISMATCH`.
   `LOSSY_CONVERSION` with its `rescale` hint is kept for `Decimal` and `Exact` operands
   (`fixed_scale_admission.rs`).
3. **Integer nominals in the bound analysis.** Integer-nominal arithmetic is bounded by the
   64-bit check, not as exact values. The analysis now uses the evaluator's and verifier's
   integer test (`bounds.rs::integer_nominal_arithmetic_is_not_bounded_as_exact`).

## Follow-ups

1. **Expression rendering correctness, extended.** The round-trip reader lives in a test today.
   If rendered text becomes an input anywhere (review tooling, AI explanations), promote the
   reader, or generate the formatter and reader from one precedence table. Keep
   `parse(pretty(e)) == e` as a gate.
2. **Generalize representability analysis through wrappers (precision of admission analysis).**
   `value_or(…)`, `some(…)` and declared derived types should not force a `rescale` by
   themselves. Representability should propagate through the expression tree, independent of the
   syntactic wrapper:
   - `value_or(o, d)`: the join of the facts of `o` and `d`;
   - `some(e)`: the facts of `e`;
   - a declared derived type: the store rule applied to the body's facts.

   Then remove deviation 2.
3. **Currency and exchange rates.** Out of scope: `SEK ÷ JPY` stays a type error, and a rate
   needs a later feature.
