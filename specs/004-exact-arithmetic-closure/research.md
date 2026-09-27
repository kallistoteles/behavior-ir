# Research: Exact Arithmetic Closure

Decisions for `spec.md` (clarified 2026-09-27: general decimals compute exactly (B); implicit
stores only when statically proven representable; admission proves representational validity,
verification proves reachable behavioral safety).

## R1 — One exact type constructor with a unit

**Decision**: generalize feature 003's `Exact<T>` to `Exact<U>` where the unit `U` is a decimal
nominal (with or without scale) or **dimensionless** (written `Exact<Decimal>`; this is the
"exact ratio / exact number" of the spec). No separate `ExactDecimal` / `ExactRatio` types:
whether a value is a finite decimal (and with how many digits) is a *static fact* tracked by the
representation analysis (R4), not a type distinction.

**Rationale**: the type only has to carry what decides validity of operations — the unit
(Money vs dimensionless vs SEK). Finite-vs-rational matters only for storing, which the analysis
decides from bounds. One constructor keeps the algebra small (closure = "arithmetic on `Exact<U>`
stays `Exact<…>`").

**Alternatives**: `ExactDecimal` + `ExactRatio` as types (division would change the type even when
the divisor is `2`; the store rule would still need an analysis); a single unitless exact type
(loses `SEK / JPY` rejection and the Money-ness of `amount * 0.25`).

## R2 — Typing table (closure)

`D` = `Decimal`, `I` = `Int`, `N` ∈ {D, I}, `T` = decimal nominal (any), `F` = fixed-scale nominal,
`X<U>` = `Exact<U>`, `R` = `Exact<Decimal>` (dimensionless).

| Expression | Result |
|---|---|
| `I ± I`, `I × I` | `I` (unchanged; 64-bit overflow is a runtime error) |
| `I ÷ I`, and any `±`, `×`, `÷` involving `D` or `R` with numbers | `R` |
| `F ± F`, `F × I`, `I × F` | `F` (unchanged from 003: exact, range-checked) |
| `T ± T` for unscaled `T`, `T × I` for unscaled `T` | `X<T>` |
| `T × N`, `N × T`, `T ÷ N`, `X<T>` with `T`/`X<T>` in `±`, `X<T>` with `N`/`R` in `×` and `X<T> ÷ N`/`R` | `X<T>` |
| `R × T`, `T × R`, `R × X<T>` | `X<T>` |
| `T ÷ T`, `X<T> ÷ T`, `T ÷ X<T>`, `X<T> ÷ X<T>` (same `T`) | `R` |
| comparisons within {`T`, `X<T>`} (same `T`) and within {`D`, `I`, `R`} | `Bool` (exact) |
| `unwrap(X<T>)` | admission error (unchanged from 003): numerically lossless but it erases the nominal unit and could bypass nominal type safety; deferred until a concrete use case, then as an explicit semantic operation |
| `T(e)`, `e : R` | `X<T>` (attaches the unit explicitly, like `T(decimal)` for unscaled nominals) |
| `rescale(e, F, mode)`, `e` any of the above numeric types | `F` |
| `SEK ÷ JPY`, `R + T`, `T ± D`, different nominals | admission error (unchanged / new) |

`I ± I` stays `Int`: integer arithmetic is already exact.

## R3 — Stores (FR-012a): implicit only when statically proven

A value of type `S` flows into a slot (effect target field, `value_or` default, `some`, declared
derived type, action input never) of type `S'`:

| Slot type | Accepted without rescale when |
|---|---|
| same type | always (copies, literals, parameters) |
| `D` from `I` | always (`to_decimal`, ≤ 19 digits) |
| `D` (or unscaled `T`) from `R` / `X<T>` | the analysis proves a finite decimal whose coefficient has **at most 28 digits** at a scale ≤ 28 (R4) |
| `F` from `X<F>` / `R` via `T(e)` | the analysis proves **scale ≤ scale(F)** (grid membership); magnitude is a runtime range check |
| anything else | admission error `LOSSY_CONVERSION` naming `rescale` |

Decimal's representation is `c · 10^−s` with `|c| < 10^28` and `0 ≤ s ≤ 28` (at most 28
significant digits, at most 28 fractional digits — exactly the input format). An unscaled input can
already use all 28 coefficient digits, so sums and products of unconstrained inputs are never
provable (spec edge case). For fixed-scale targets only the grid
(precision) is proven; the range stays a runtime error found by verification (spec Q3).

## R4 — Representation analysis (admission)

**Decision**: a single bottom-up pass over every expression computes, from types, literals, and
type-level bounds only (never constraints, invariants, preconditions — FR-012c):

- `nb`, `db`: upper bounds on the bit length of the reduced numerator and denominator
  (for the 512-bit runtime limit);
- `scale`: `Some(s)` if the value is always a finite decimal with at most `s` fractional digits,
  `None` otherwise;
- `cd`: an upper bound on the number of digits of the value's coefficient `c` at the tracked scale
  (`v = c · 10^−scale`), or unbounded. The normalized coefficient (trailing zeros removed) has at
  most `cd` digits, so `scale ≤ 28 ∧ cd ≤ 28` implies the value is a Decimal; the check is exact
  except that trailing zeros are not credited (a value like `100.00` computed at scale 2 counts 5
  digits), which only rejects, never admits wrongly.

Leaves: fixed-scale `F` (`scale = s`, `cd = 28`); general `D` or unscaled `T` input
(`scale = 28`, `cd = 56`: its own scale `s ≤ 28` is unknown, and `c · 10^−s = (c · 10^(28−s)) ·
10^−28` has at most `56 − s` coefficient digits at scale 28); `I` (scale 0, `cd = 19`); literals
(their exact `scale` and `cd`); rescale results (like `F`); derived references (the bound of the
derived body, computed once). Copies of a general Decimal into a Decimal slot never need the
analysis (same type).

Rules (bits: `bits(10^k) ≈ 3.33k`, rounded up):

| Op | `nb` | `db` | `scale` | `cd` |
|---|---|---|---|---|
| `a · b` | `nb₁+nb₂` | `db₁+db₂` | `s₁+s₂` | `cd₁+cd₂` |
| `a ± b` (`s = max(s₁,s₂)`) | `max(nb₁+db₂, nb₂+db₁)+1` | `db₁+db₂` | `s` | `max(cd₁+s−s₁, cd₂+s−s₂)+1` |
| `a ÷ b`, `b` a non-zero literal `m·10^−k`, `|m| = 2^i·5^j` | `nb₁+db₂` | `db₁+nb₂` | `s₁+max(i,j)` | `cd₁+max(i,j)+k` |
| `a ÷ b`, otherwise | `nb₁+db₂` | `db₁+nb₂` | `None` | unbounded |

Admission fails with `EXACT_BOUND_EXCEEDED` at an expression where `nb` or `db` exceeds 511
(message: required vs supported bits), and with `LOSSY_CONVERSION` at a store the rules of R3
cannot prove. Integer-only arithmetic is not in the exact domain and is not bounded here
(64-bit overflow stays a runtime error).

**Rationale**: the bit formulas are exact upper bounds for rationals (reduction only lowers them);
the scale/magnitude facts decide representability; everything is local and structural, as the
admission/verification principle requires.

**Alternatives**: interval arithmetic on values (needs magnitudes of divisors; more complex, no
gain for the questions asked); asking the SMT solver at admission (mixes admission and
verification, violates FR-012c).

## R5 — Runtime

- Every `Exact`-typed expression is evaluated with the rational domain of feature 003
  (`exact.rs`); general decimal arithmetic no longer uses 28-digit `rust_decimal` operations.
  Integer arithmetic is unchanged.
- The rational domain's 512-bit check stays as a defensive **internal** error (by R4 it is
  unreachable for admitted modules); SC-001 tests that it never fires.
- Stores proven by R3 convert the exact value to its stored form exactly (`Exact → Dec`); a
  fixed-scale store checks the range (runtime overflow, as in 003).
- The "exact region" rule of feature 003 disappears: every decimal operation is exact, including
  inside derived values (`third_via_derived` now gives `0.01`, like `third_exact`).

## R6 — Wire IR 0.4 and old documents

**Decision**: the same wire text must not silently change meaning. `ir_version "0.4"` denotes
exact-closure semantics; documents with `"0.1"`–`"0.3"` are rejected with `UNSUPPORTED_VERSION`
and a message pointing to the migration (re-serialize from the DSL, declare scales where computed
values are stored). The serializer always writes `"0.4"`. The type form `{"t": "exact"}` (no
`name`) is the dimensionless exact type; `{"t": "exact", "name": "Money"}` is allowed for any
decimal nominal. Schema: `schema/wire-ir-0.4.schema.json` (0.1–0.3 schemas stay for history).

**Rationale**: features 001–003 are not an external compatibility promise (spec Assumptions); an
explicit version step makes the break visible instead of reinterpreting stored documents.

**Alternatives**: keep accepting 0.1–0.3 with the new semantics (silent reinterpretation of old
documents — rejected); keep two semantics side by side (permanent complexity — rejected).

## R7 — Hashing

Expression hashes already include the result type, so every expression whose type changes
(decimal arithmetic becomes `Exact<…>`) gets a new hash automatically. New type codes: `0x24`
for dimensionless `Exact`; `0x23` (`Exact<T>`) now allowed for unscaled nominals. No new tags are
needed. The frozen `hash_vectors.json` and `frozen_versions_002.json` are regenerated, each
change reviewed: vectors without decimal arithmetic must keep their values (SC-004 check).

## R8 — Records

`record_version "0.4"` for all records. Identical record bytes are not a goal: for modules without
decimal arithmetic the regenerated goldens must differ only in `record_version` (checked
mechanically), and their behavior versions and outcomes stay the same. The review lists every file
that differs beyond that, with the reason.

## R9 — Verifier

- All decimal arithmetic is encoded exactly (`+ − ×` as terms, `÷` as `q·d = n` under `d ≠ 0`);
  the rounding-interval terms (`rounded`, `rounded_sum`) and the general-decimal `DEC_MAX`
  overflow obligations are removed. Remaining overflow obligations: integer (64-bit) and
  fixed-scale range (including stores into fixed-scale fields).
- Only rescale introduces rounding, modelled exactly as in 003.
- Inputs: general decimals keep the `|v| ≤ 10^28 − 1` domain; fixed-scale inputs keep their grid.
- Stores into general decimals are proven representable at admission, so no extra obligation.
- Verifier version `0.4.0`. Fixture expectations of 002/003 are updated where exact semantics
  decides a formerly inconclusive or rounding-dependent check (SC-007).

## R10 — Python DSL

`Exact[Decimal]` is the dimensionless exact type (derived return annotations); `Exact[T]` accepts
any decimal nominal. No other API change; `rescale` and `Rounding` as in 003.

## R11 — Migration of fixtures and examples

Wire format and semantic identity are separate: every fixture's wire file is re-serialized as 0.4
(new envelope), while semantically unchanged modules keep their behavior versions (checked against
`frozen_versions_002.json`, which stores semantic hashes, not bytes). Fixtures that store computed
values in unscaled decimals become inadmissible and are migrated by
declaring `scale=2` on their `Money` (or equivalent) in the fixture builders and examples:
`wire/valid/invoice.json`, `wire/python/invoice.json`, `wire/valid/constraints.json`,
`verify/purchase*.json`, `verify/unchanged_entity.json`, `verify/postcondition.json`,
`verify/sum_rounding.json` (the latter's point — silent rounding of sums — no longer exists; it
becomes a representability test), the invoice and try-out examples. Modules with decimal
arithmetic only in conditions (`project_margin`, `verify/rounding.json`) stay admissible with new
hashes. Every change goes into the implementation review.

## R12 — Dependencies

None new.
