# Implementation Plan: Fixed-Scale Decimals

**Branch**: `003-fixed-scale-decimals` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-fixed-scale-decimals/spec.md`; builds on
features 001 and 002, `PRINCIPLES.md` §9 ("lossless operations may be implicit; lossy
conversions must be explicit"), and `docs/verification.md`.

## Summary

Decimal-based nominal types can declare a fixed scale (`Money = nominal(Decimal, scale=2)`).
Values of such a type always lie on their grid: off-grid input is rejected, `T ± T` and
`T × Int` are exact, and operations that can leave the grid yield an exact quantity
`Exact<T>` (a bounded rational) that may be compared and computed with exactly but becomes a `T`
only through an explicit `rescale(e, T, mode)` with one of six rounding modes. Scale, `Exact<T>`,
and rescale are part of the typed semantic IR, its hashes (new tags only; old modules keep their
hashes), wire IR 0.3, records (0.3 only when used), traces, and the verifier, which models
fixed-scale values as integers on their grid, exact quantities as exact rationals, and each
rescale as its rounding function.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13 (binding and DSL).

**Primary Dependencies**: existing, plus `num-bigint`, `num-rational`, `num-integer`,
`num-traits` in `behavior-core` for the exact domain (research R4, R14). Z3 4.16.0 unchanged.

**Storage**: N/A (values in requests/records as today).

**Testing**: `cargo test` (typing table, rounding-mode table at ties and negatives, grid/range
input checks, exact evaluation, hash stability of all 001/002 vectors, verification fixtures),
property tests for rounding (`proptest`: each mode against a reference over random rationals);
`pytest` for the DSL; determinism script extended to fixed-scale fixtures.

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + CLI + Python binding (existing workspace).

**Performance Goals**: verification of the example modules with fixed-scale Money no slower
than with general decimals (within 10% or 1 s, SC-005); exact arithmetic bounded (512-bit
numerator/denominator).

**Constraints**: all feature 001/002 fixtures, hashes, records, and attestation outcomes
unchanged (SC-004); byte-identical artifacts across runs (SC-006); no implicit rounding of
fixed-scale values anywhere (SC-003).

**Scale/Scope**: 3 user stories; one new type constructor, one new expression, six rounding
modes, one wire/record version step.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Pure arithmetic; rounding modes are total, deterministic functions. No AI. |
| II. AI output validated | Pass (n/a) | No AI. Off-grid input is rejected with typed errors, never repaired (FR-004). |
| III. Test-first | Pass (process) | Typing table, rounding table, grid checks, and fixtures come before implementation; rounding also gets property tests against a reference. |
| IV. Reproducibility | Pass | Exact rationals and fixed rounding definitions; canonical text forms (R6); record version bump only when new features are used (R10). |
| V. Explicit state and auditability | Pass | Every rounding is an explicit rescale recorded in the trace with its exact input (R7). Illegal states unrepresentable: `Exact<T>` cannot be stored (R2). |
| VI. Simplicity | Pass with justification | One new type constructor and one expression; the `num-*` crates are justified below. |
| Tech constraints | Pass | Rust, fmt/clippy, no `unsafe`, typed errors, exact numerics. |

Checked against `PRINCIPLES.md`: §6 (scale and rounding in hashes, new tags only), §9 (lossless
implicit / lossy explicit, exactness propagates, nominal types for domain distinctions), §10
(the verifier assumes exactly the grid and range the runtime enforces, R11).

**Post-design re-check (after Phase 1)**: all rows pass; no further dependencies.

## Project Structure

### Documentation (this feature)

```text
specs/003-fixed-scale-decimals/
├── plan.md
├── research.md                 # R1–R14
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── numeric-semantics.md    # typing table, ranges, rounding modes, text forms, exact domain
│   └── engine-api.md           # wire IR 0.3, hashing, records 0.3, Rust/CLI/Python, verifier
└── tasks.md                    # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/
│   └── src/
│       ├── exact.rs              # new: bounded rationals, rounding modes, text form
│       ├── decimal.rs            # grid/range checks, fixed-scale formatting
│       ├── semantic/types.rs     # NominalInfo.scale, Type::Exact, typing table (R2)
│       ├── semantic/expr.rs      # ExprKind::Rescale
│       ├── semantic/value.rs     # Value::Exact (evaluation only), fixed-scale decode/encode
│       ├── wire.rs               # ir_version 0.3: scale, exact type, rescale, derived type
│       ├── admit/                # typecheck (R2, R3), hash tags (R9), literal grid checks
│       ├── eval.rs               # exact context, rescale trace entries, OFF_GRID/OUT_OF_RANGE
│       ├── builder.rs            # rescale node, declared derived types
│       ├── pretty.rs             # rescale(...) text
│       └── serialize.rs          # lowest sufficient ir_version
├── behavior-verify/
│   └── src/encode.rs             # fixed-scale Int encoding, exact terms, rescale constraints (R11)
├── behavior-cli/                 # no new commands
└── behavior-py/                  # Type.exact, Builder.rescale, nominal scale, derived type
python/behavior/                  # nominal(scale=), Exact[...], rescale(), Rounding
schema/wire-ir-0.3.schema.json
tests/fixtures/
├── wire/valid/fixed_scale.json   # + invalid cases (off-grid literal, lossy wrap, Exact stored, ...)
├── requests/003/                 # grid/range inputs, rescale evaluations, expectations
├── verify/                       # purchase_money2*, discount_rescale, ... (+ .expected.json)
└── numeric/rounding.json         # the FR-010a table as data, shared by Rust and Python tests
examples/tryout/                  # Money with scale 2 variant
docs/verification.md              # updated limitations
```

**Structure Decision**: the exact domain and rounding live in `behavior-core` (a new `exact.rs`)
because evaluation needs them; the verifier only changes its encoder. No new crate.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| `num-bigint` / `num-rational` (+ `num-integer`, `num-traits`) | Exact quantities need rationals larger than 128 bits (Money × 28-digit decimal ≈ 190 bits; divisions add denominators) | `i128` rationals overflow on ordinary money arithmetic; `rust_decimal` rounds at 28 digits, which is exactly what this feature removes; a hand-written big integer is more code and risk |
| New type constructor `Exact<T>` | The clarification requires exact intermediates that are distinct from storable values | Treating intermediates as general decimals reintroduces implicit rounding; allowing them only inside `rescale` forbids exact comparisons like `amount * 1.25 <= budget` |
