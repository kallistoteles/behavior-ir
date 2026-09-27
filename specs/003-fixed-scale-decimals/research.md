# Research: Fixed-Scale Decimals

Decisions for `spec.md` (clarified 2026-09-27). Each entry: decision, rationale, alternatives.

## R1 — Where scale lives in the type system

**Decision**: `NominalInfo` gains `scale: Option<u8>` (0–28, only for `underlying = Decimal`).
A new type constructor `Type::Exact(Arc<NominalInfo>)` exists only for fixed-scale nominals.
Plain `Decimal` and nominals without scale are unchanged.

**Rationale**: the spec puts scale on nominal types (Assumptions) and the clarification makes
`Exact<T>` a distinct type with the same nominal identity. Keeping both on `NominalInfo` means
`Exact<SEK> + Exact<JPY>` is rejected by the existing nominal-identity check.

**Alternatives**: a built-in `Fixed<s>` primitive (rejected: currencies and domains are nominal
types, PRINCIPLES §9); scale on fields (rejected: the same unit would get different grids).

## R2 — Typing rules (lossless implicit, lossy explicit)

**Decision** for a fixed-scale nominal `T` (ops as declared; `Exact<T>` inherits `T`'s ops):

| Expression | Result | Why |
|---|---|---|
| `T ± T` | `T` | lossless; range-checked (overflow error) |
| `T × Int`, `Int × T` | `T` | lossless (whole-number multiplication is decided by type) |
| `T × Decimal`, `T ÷ Int`, `T ÷ Decimal` | `Exact<T>` | may leave the grid |
| `Exact<T>` with `T` or `Exact<T>` in `±`, and with numbers in `×`, `÷` | `Exact<T>` | exactness propagates; `T` is lifted implicitly |
| comparisons among `T` / `Exact<T>` | `Bool` | exact; `T` lifted |
| `T ÷ T` (ratio) | `Decimal` | FR-011, unchanged (see R13) |
| `rescale(e, T, mode)`, `e : Exact<T> \| T \| Decimal \| Int` | `T` | the only lossy boundary |
| `T(literal)` | `T` | literal must lie on the grid (admission) |
| `T(e)`, `e : Int` | `T` | lossless (range-checked at runtime) |
| `T(e)`, `e : Decimal` | admission error | lossy; write `rescale(e, T, mode)` |
| `unwrap(t)`, `t : T` | `Decimal` | exact |
| `unwrap(x)`, `x : Exact<T>` | admission error | rescale first |

Effects into a field of type `T`, and entity fields in general, accept only `T` (never
`Exact<T>`): an `Exact<T>` where a `T` is required is the admission error `LOSSY_CONVERSION`,
whose message names `rescale`. Entity fields cannot be declared `Exact<T>`.

**Rationale**: implements FR-007–FR-012 and the clarifications literally, as a small table in the
existing `type_of_op`.

**Alternatives**: `T × Decimal → T` with implicit rounding (today's rule; rejected, lossy
implicit).

## R3 — Declared types for derived values

**Decision**: derived values keep inferred types; wire IR 0.3 allows an optional declared
`type` on a derived value, checked at admission (inferred must equal declared, otherwise
`DECLARED_TYPE_MISMATCH`, whose message names `rescale` when the inferred type is `Exact<T>` and
the declared one is `T`). The declaration is not hashed: it adds no semantics once it equals the
inferred type.
The DSL passes a Python return annotation (`-> Money`, `-> Exact[Money]`) through.

**Rationale**: spec US2 scenario 6 needs "a derived value declared as Money"; without
declarations it would be vacuous.

**Alternatives**: mandatory declared types (breaks every existing module).

## R4 — The exact numeric domain

**Decision**: exact values are rationals over arbitrary-precision integers (`num-bigint` +
`num-rational`), bounded: numerator and denominator each at most 512 bits after reduction;
larger values are an overflow evaluation error (spec Assumptions: fixed, documented limit).

**Rationale**: a two-decimal amount (up to 10^28) times a 28-digit decimal already needs ~190
bits; divisions produce arbitrary denominators. 512 bits comfortably covers chains of a few
operations and keeps evaluation bounded.

**Alternatives**: `i128` rationals (overflow for `Money × Decimal`); unbounded rationals
(violates "bounded evaluation"); `rust_decimal` (rounds at 28 digits — the problem being solved).

## R5 — Runtime values and ranges

**Decision**:
- A fixed-scale value is a `Dec` guaranteed on the grid of scale `s` with
  `|v| < 10^(28−s)` (at most `28 − s` integer digits). `T ± T` and `T × Int` use exact `Dec`
  arithmetic followed by a range check; nothing rounds (both operands have ≤ 28 digits, so
  `rust_decimal` stays exact or reports overflow).
- `Value::Exact(BigRational)` exists only during expression evaluation; it never enters state,
  requests, or changes.
- Input decoding (`state`, `input`, `context`) of a `T` field rejects values with a non-zero
  digit beyond scale `s` or out of range: `INVALID_INPUT` (as every request type error, in any
  section), code `OFF_GRID` or `OUT_OF_RANGE`, message naming field and scale. Trailing zeros
  are fine (`100.500` for scale 2).
- Exact context: arithmetic nodes of type `Decimal`/`Int` that are direct operands of an
  `Exact<T>` operation or of a `rescale` are evaluated in the exact domain, recursively through
  arithmetic nodes. The exact context does not extend through derived references: a derived
  general decimal is a value with its own (general) semantics, reported once in the record.

**Rationale**: SC-003 (nothing off grid enters; no silent rounding); the derived-reference rule
keeps each derived value's reported value unique.

**Alternatives**: store fixed-scale values as scaled integers in `Value` (a second numeric
representation in records and requests; unnecessary since `Dec` holds grid values exactly).

## R6 — Canonical text form

**Decision**: fixed-scale values are written with exactly `s` fractional digits (`"100.50"`,
scale 0 → `"100"`, `-0.50`, zero as `"0.00"`). Exact quantities in traces are written as a finite
decimal in normalized form when the reduced denominator is `2^a·5^b` (`2001/400` → `"5.0025"`),
otherwise as `"n/d"` in lowest terms (`"40/3"`). General decimals keep the normalized form.

**Rationale**: FR-005, FR-011a; replay compares bytes.

## R7 — Rescale semantics and rounding modes

**Decision**: `rescale(e, T, mode)` evaluates `e` exactly, rounds to the grid `10^-s` with
`mode`, then range-checks (overflow error if outside). Modes, for exact value `x` and
`y = x·10^s`: `floor` = ⌊y⌋, `ceiling` = ⌈y⌉, `down` = trunc(y), `up` = away from zero,
`half_up` = nearest with ties away from zero, `half_even` = nearest with ties to even. The
result is `k / 10^s` for the integer `k` chosen. Definitions and the tie table of FR-010a are
tested at ±0.5 ties, exact values, and negative numbers.

Trace: every evaluated rescale appends to its step an entry
`{"rescale": expr_text, "exact": "40/3", "rounding": "half_even", "scale": 2, "result": "13.33"}`
in evaluation order (inner rescales first).

**Alternatives**: rescale results in a separate record section (harder to relate to the step
that used them).

## R8 — Wire IR 0.3

**Decision**: `ir_version "0.3"` adds, and only 0.3 documents may use:
- `nominals[].scale` (integer 0–28, only with `underlying: decimal`);
- the type `{"t": "exact", "name": "<nominal>"}` (only for fixed-scale nominals; derived
  declarations only);
- the expression `{"op": "rescale", "nominal": "<name>", "rounding": "<mode>", "args": [<expr>]}` (like `wrap`);
- optional `derived[].type`.

0.1 and 0.2 documents stay valid unchanged; a 0.3 document that uses none of this is valid too
but is serialized with the lowest version that suffices (as 0.2 is today), so feature 001/002
modules keep their bytes and hashes. JSON Schema `schema/wire-ir-0.3.schema.json`.

## R9 — Hashing

**Decision**: nominals without scale keep `behavior.nominal.v1`. A fixed-scale nominal is hashed
under a new tag `behavior.nominal.fixed.v1` = `str name ‖ type underlying ‖ u8 ops ‖ u8 scale`.
The `Exact<T>` type gets a new type code in the type encoding (`exact ‖ nominal decl hash`).
The rescale expression gets a new op code: `arg hash ‖ nominal decl hash ‖ u8 rounding`
(`half_even=0, half_up=1, down=2, up=3, floor=4, ceiling=5`). Derived declared types are not
hashed (R3).

**Rationale**: FR-002 (scale and rounding are semantic identity); SC-004 (no existing hash
changes); domain separation for the new declaration form.

## R10 — Records

**Decision**: records of modules that use any fixed-scale feature get `record_version "0.3"`
(fixed-scale text form, rescale trace entries, new input codes); all other records stay
`"0.2"` byte for byte (SC-004). Replay reproduces both.

## R11 — Verification encoding

**Decision** (in `behavior-verify/src/encode.rs`):
- A fixed-scale value is an `Int` symbol `k` with value `k / 10^s` and axiom
  `|k| < 10^28` (i.e. at most `28 − s` integer digits); counterexamples print `k` with `s`
  fractional digits, so they are on the grid by construction (FR-014).
- `T ± T`, comparisons: exact linear integer arithmetic on the `k`s; `T × Int`: exact
  (nonlinear if both are variables, still exact); range check → overflow obligation.
- `Exact<T>` and exact-context general decimals: exact `Real` terms, no rounding variables;
  division as `q` with `q·d = n` under `d ≠ 0` (as today, without the rounding bound).
- `rescale(e, T, mode)`: a fresh `Int` `k` with the mode's linear constraints over
  `y = e·10^s`: `floor`: `k ≤ y < k+1`; `ceiling`: `k−1 < y ≤ k`; `down`/`up`: by the sign of
  `y`; `half_up`/`half_even`: `|y − k| ≤ 1/2`, and at a tie (`|y − k| = 1/2`) `k` is the one away
  from zero / the even one (`k mod 2 = 0`). Plus the range obligation.
- General decimals outside exact contexts keep the feature-002 interval model (FR-015).

**Rationale**: FR-013; everything stays in linear (or mildly nonlinear) mixed integer/real
arithmetic, where Z3 is complete for the linear fragment, so `purchase_remaining` with a
two-decimal Money becomes a pure integer problem (SC-001).

**Alternatives**: keep reals with grid axioms `x·10^s ∈ ℤ` (equivalent but slower `to_int`
reasoning).

## R12 — Python DSL

**Decision**: `nominal("Money", Decimal, scale=2, ops={...})`; `Exact[Money]` as a type
descriptor (derived return annotations); `rescale(expr, Money, Rounding.HALF_EVEN)` with
`Rounding` an enum of the six modes (no default argument); values cross as `Decimal` both ways
(fixed-scale values quantized to `s` digits on the way out); exact quantities appear only in
records/traces (as text).

## R13 — Known tension kept from the spec

`T ÷ T` yields a general `Decimal` computed with today's 28-digit rules (FR-011): a lossy
implicit step. It is kept to stay within scope; an `Exact<Decimal>` (exact ratios) is a natural
follow-up and would not change anything decided here. Recorded in the implementation review.

## R14 — Dependencies

`num-bigint`, `num-rational`, `num-integer`, `num-traits` (pure Rust, widely used) in
`behavior-core`. No other new dependencies; the verifier needs none.
