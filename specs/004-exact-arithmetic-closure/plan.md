# Implementation Plan: Exact Arithmetic Closure

**Branch**: `004-exact-arithmetic-closure` | **Date**: 2026-09-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-exact-arithmetic-closure/spec.md`; builds on
feature 003 and `PRINCIPLES.md` (§9 "numeric computation is exact by default", "lossless
representation changes may be implicit only when statically proven"; §10 "admission proves
representational validity; verification proves reachable behavioral safety").

## Summary

Make the exact numeric domain closed under arithmetic. `Exact<U>` (unit = a decimal nominal or
dimensionless) becomes the result of every non-integer arithmetic operation that is not a
lossless fixed-scale operation: general decimal arithmetic is exact (no 28-digit rounding),
`T ÷ T` is a dimensionless exact ratio, and exact values leave the domain only through `rescale`
or through a store that admission proves representable. A new admission-time representation
analysis computes, from types and literals only, bit bounds (so no exact value can exceed the
runtime's 512-bit representation), finite-decimal scale and digit facts (for stores into general
decimals), and grid membership (for stores into fixed-scale fields). Runtime and verifier encode
all decimal arithmetic exactly; the feature-002 rounding intervals disappear except at explicit
rescale. Wire IR 0.4 marks the semantic break; 0.1–0.3 documents are rejected with a migration
message; fixtures and examples that store computed money declare a scale.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python 3.13.

**Primary Dependencies**: unchanged (rational domain from feature 003; Z3 4.16.0).

**Storage**: N/A.

**Testing**: `cargo test` (typing table incl. ratio algebra, representation analysis unit tests
with bounds at and beyond 511 bits, store-provability table, exact evaluation of former
rounded-decimal cases, migrated goldens), `proptest` for SC-001 (≥10,000 randomized evaluations
of admitted fixtures with extreme inputs: no exact-size overflow), verifier fixtures updated under
exact semantics, `pytest`, determinism script.

**Target Platform**: Linux (NixOS dev shell).

**Project Type**: library + CLI + Python binding.

**Performance Goals**: bound analysis adds ≤ 10% or 50 ms to admission of the examples (SC-006);
verification of the examples no slower than today (the interval model is removed).

**Constraints**: no implicit rounding anywhere except explicit rescale; representability proven
statically; admission never reads entity constraints (FR-012c); deterministic artifacts.

**Scale/Scope**: 4 user stories; one generalized type, one analysis, one IR version step, a
fixture migration across features 001–003.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | Exact rationals and static analysis are deterministic; no AI. |
| II. AI output validated | Pass (n/a) | No AI. |
| III. Test-first | Pass (process) | Typing/analysis tables, SC-001 proptest, and migrated goldens precede implementation; goldens are regenerated only after the semantics tests are green and reviewed. |
| IV. Reproducibility | Pass with justification | The IR version step (0.4) makes the semantic break explicit instead of reinterpreting stored documents; old versions are rejected, not silently re-evaluated. |
| V. Explicit state and auditability | Pass | Exact values appear in traces (finite decimal or fraction); every rounding is an explicit rescale step. |
| VI. Simplicity | Pass | One type constructor generalized instead of two new types; the rounding-interval model is removed from the verifier. |
| Tech constraints | Pass | Rust gates, no new dependencies, no `unsafe`. |

**Post-design re-check (after Phase 1)**: all rows pass.

## Project Structure

### Documentation (this feature)

```text
specs/004-exact-arithmetic-closure/
├── plan.md
├── research.md                 # R1–R12
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── numeric-semantics.md    # typing table v2, store rules, analysis rules
│   └── engine-api.md           # wire IR 0.4, hashing, records, verifier, Python
└── tasks.md                    # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── behavior-core/src/
│   ├── semantic/types.rs       # Exact<U> with a dimensionless unit; typing table v2 (R2)
│   ├── admit/bounds.rs         # new: representation analysis (R4)
│   ├── admit/typecheck.rs      # store rules (R3), EXACT_BOUND_EXCEEDED, LOSSY_CONVERSION
│   ├── admit/hash.rs           # type code 0x24
│   ├── wire.rs                 # ir_version 0.4 only; exact without name
│   ├── eval.rs                 # all decimal arithmetic exact; stores Exact→Dec; version 0.4
│   └── serialize.rs            # always 0.4
├── behavior-verify/src/encode.rs  # exact decimal arithmetic; interval model removed (R9)
└── behavior-py/                # Type.exact(None)
python/behavior/                # Exact[Decimal]
schema/wire-ir-0.4.schema.json
tests/fixtures/                 # migrated builders and goldens (R11), new 004 fixtures
examples/                       # Money with scale where computed values are stored
```

**Structure Decision**: the analysis is a new admission module next to the type checker it serves;
no new crate.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Rejecting wire IR 0.1–0.3 | The same document would otherwise silently change meaning (exact instead of rounded arithmetic) | Accepting old versions with new semantics reinterprets stored behavior; keeping both semantics doubles the numeric core permanently |
