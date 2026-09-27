# Implementation Review: Fixed-Scale Decimals

**Purpose**: Review of the implementation against `PRINCIPLES.md` and the constitution (task T031)
**Date**: 2026-09-27
**Scope**: `crates/behavior-core/` (exact domain, types, admission, evaluation, serialization),
`crates/behavior-verify/src/encode.rs`, `crates/behavior-py/`, `python/behavior/`,
`schema/wire-ir-0.3.schema.json`, `tests/fixtures/{numeric,wire,requests/003,verify}`, `examples/`

Each item was checked in the code or by a test. Evidence is noted.

## PRINCIPLES.md

- [x] §9 Lossless operations may be implicit; lossy conversions must be explicit:
  - `T ± T` and `T × Int` type as `T`. `T × Decimal`, `T ÷ n`, and anything with an exact
    operand type as `Exact<T>` (`fixed_scale_admission.rs::typing_table`).
  - `Money(decimal)`, `unwrap(Exact)`, storing an `Exact<T>`, and a derived value declared `T`
    returning `Exact<T>` are admission errors whose message names `rescale`: `LOSSY_CONVERSION`
    and `DECLARED_TYPE_MISMATCH` (the invalid fixtures, `test_fixed_scale.py`).
  - A type has no rounding of its own. `rescale` is the only place a value is rounded, and its
    mode is required (no default in the wire IR, the builder, or the DSL).
- [x] §9 Exactness propagates implicitly:
  - Exact quantities are bounded rationals (`exact.rs`).
  - Comparisons with `T` lift `T` losslessly (`check_margin_*`).
  - Nested rescales round once per rescale the author wrote (`split3` → 13.33,
    `split3_nested` → 13.34).
  - General decimals inside an exact computation are exact (`third_exact` → 0.01), but not
    through a derived reference (`third_via_derived` → 0.00).
- [x] §9 Domain distinctions via nominal types: `SEK + JPY` is rejected, also as exact
  quantities (`mixed_nominals` fixture). There is no currency concept in the core.
- [x] §9 Invalid data cannot enter: off-grid or out-of-range request values are rejected before
  any rule runs (`OFF_GRID`, `OUT_OF_RANGE`). A proptest checks random off-grid values and
  on-grid values (`evaluate_003.rs`). Off-grid literals are admission errors.
- [x] §6 Content addressing:
  - Scale and rounding mode are hashed: a new tag `behavior.nominal.fixed.v1`, a new type code
    for `Exact`, and a new op code for `rescale`.
  - Declared derived types are not hashed.
  - All 001/002 behavior versions are frozen and unchanged (`compat_003.rs` against
    `tests/fixtures/frozen_versions_002.json`, plus the existing hash-vector tests).
- [x] §10 The verifier assumes exactly what the runtime guarantees:
  - A fixed-scale input is a real tied to an integer grid index `k` with `|k| < 10^28`. That is
    exactly the runtime's `on_grid` and `in_fixed_range` checks.
  - Lossless arithmetic is exact, with the runtime's range obligation.
  - Exact quantities are exact reals.
  - Each rescale is the linear constraint set of its mode. Half-even ties use `k mod 2`.
  - `rescale_modes.json` shows that `floor` is proven and `ceiling` is refuted for the same
    postcondition.
  - `purchase_money2_remaining` is proven: it was inconclusive in feature 002 (SC-001).

## Constitution

- [x] I/IV Determinism: exact rationals, fixed rounding definitions, canonical text forms. The
  determinism script now also runs `eval` and `replay` twice on every 002/003 request.
- [x] V Auditability: every rescale is a trace entry with its expression, the exact pre-rescale
  value (`"40/3"`), the mode, and the result.
- [ ] III Test-first: mostly followed; one deviation (below).
- [x] VI Simplicity: one new type constructor, one expression, and the `num-*` crates, all
  justified in the plan.
- [x] Gates (T030): fmt, clippy `-D warnings`, 117 Rust tests, 69 Python tests, mypy,
  determinism, and both perf suites all pass.
  - SC-005: fixed-scale examples verify in 0.86 s against 0.76 s for the general-decimal versions
    (the allowance is +1 s).
  - SC-003/SC-004 hold, and every 002 verification expectation is unchanged under verifier 0.3.0.

## Deviations from tasks.md and the design documents

1. **Test-first**: most of the evaluator (exact evaluation, `rescale`, fixed-scale arithmetic,
   trace entries) was written in Phase 2, before the US1/US2 evaluation tests. Adding
   `ExprKind::Rescale` made the evaluator stop compiling, and a placeholder would have been
   thrown away. The tests (T012/T013, T018/T019) came right after. A deliberately broken
   expectation was checked to fail. Admission, exact-domain, and verifier work followed
   test-first.
2. **Off-grid input is `INVALID_INPUT` in every section**, not by section (spec FR-004 and US1
   scenario 1 corrected). Every other request type error is `INVALID_INPUT`, and
   `INVALID_STATE` / `INVALID_CONTEXT` come only from entity constraints. The earlier analysis
   finding F4 had assumed otherwise.
3. **Wire form of rescale** is `{"op": "rescale", "nominal", "rounding", "args": [expr]}`, like
   `wrap`, instead of `"arg"`. Contracts updated.
4. **Python decisions expose canonical text** (`change.new == "100.50"`), consistent with
   general decimals since feature 001, instead of `Decimal` values. Contract updated.
5. **Extra fixture actions** in `fixed_scale.json`: `compare_fee`, `per_unit`, `scale_up`,
   `third_exact`, `third_via_derived`, and `round_<mode>`. Each makes one required scenario
   testable (derived values in records, division by zero and overflow at a rescale, the
   exact-region rule, the six modes at ties and negatives).
6. **Money2 verification fixtures** come from a separate script,
   `tests/fixtures/verify/build_money2_fixtures.py`. Adding them to the 002 builder shifted
   source lines in 002 fixtures, and function-local classes did not resolve under
   `from __future__ import annotations`.
   - The Money2 purchase models add non-negativity entity constraints, as a realistic money
     model.
   - With them, `amount <= remaining(project)` has no evaluation errors, so the default profile
     exits 0.
   - The invariant-shaped precondition `spent + amount <= budget` has a genuine overflow inside
     the precondition itself (exit 1).
7. **Exact region and ranges**: inside an exact computation, fixed-scale `±` is exact and not
   range-checked (the intermediate is never stored). Only the rescale result is range-checked.
   The verifier mirrors this exactly.
8. **Two invalid fixtures** (`declared_type_mismatch`, `exact_not_fixed_scale`) drop
   `compare_fee`. A derived value that fails admission also produces an `UNKNOWN_DERIVED`
   follow-on error for its users, which is pre-existing behavior not changed here.
9. **Codes not listed in the contract**:
   - a rescale to a nominal without a scale reuses `EXACT_NOT_FIXED_SCALE`;
   - a parameter of type `exact` is `TYPE_MISMATCH`;
   - an `exact` literal is `TYPE_MISMATCH`.
10. **Records of rejected requests** echo the raw input (`"100.505"`); the fixed text form
    applies after successful decoding.
11. **Added** `tests/fixtures/frozen_versions_002.json` (a snapshot for the compatibility test)
    and a `money2` option for `examples/*/dump.py`.

## Open risks and follow-ups

- **The 512-bit bound of exact quantities is not modelled by the verifier.** The runtime reports
  an overflow if a reduced numerator or denominator reaches 2^512; the verifier treats exact
  quantities as unbounded reals. A proof of "no evaluation error" is therefore only valid for
  expressions that cannot reach that size. With inputs below 10^28, that takes a long chain of
  multiplications and divisions. No fixture comes close, but this is a gap in "never a wrong
  proof" and should be closed, for example with a static size bound per expression checked at
  admission.
- **`T ÷ T` stays a lossy implicit step** (general decimal, 28 digits; FR-011, research R13). An
  exact ratio type is the natural follow-up.
- Carried over from feature 002: unsigned attestations and a trusted cache.
