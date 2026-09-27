# Data Model: Exact Arithmetic Closure

Extends `specs/003-fixed-scale-decimals/data-model.md`. Semantics:
[contracts/numeric-semantics.md](contracts/numeric-semantics.md).

## Exact value `Exact<U>`

| Aspect | Rule |
|--------|------|
| Unit `U` | a decimal nominal (with or without scale), or dimensionless (`Exact<Decimal>`, the exact ratio / exact number) |
| Value | reduced rational; admission guarantees numerator and denominator stay below 2^512 on every reachable path |
| Produced by | every non-integer arithmetic that is not a lossless fixed-scale operation (R2) |
| Never | an action input, an entity field type, or implicitly a general decimal |
| Leaves the exact domain | through `rescale(e, F, mode)`, or a store proven representable (R3) |
| Text form | normalized finite decimal if one exists, else `n/d` (unchanged from 003) |

## Representation facts (admission, per expression)

| Fact | Meaning | Used for |
|------|---------|----------|
| `nb`, `db` | upper bounds on bits of reduced numerator / denominator | reject `> 511` (`EXACT_BOUND_EXCEEDED`) |
| `scale` | `Some(s)`: always a finite decimal with ≤ `s` fractional digits; `None`: possibly non-terminating | stores into general decimals (digits) and fixed-scale fields (grid) |
| `cd` | upper bound on the coefficient's digits at the tracked scale (`v = c·10^−scale`), or unbounded | stores into general decimals (`scale ≤ 28 ∧ cd ≤ 28`; conservative only in not crediting trailing zeros) |

Computed from types, literals, and type-level bounds only (FR-012c).

## Store rules (R3)

| Target | Source | Allowed without rescale when |
|--------|--------|------------------------------|
| any | same type | always |
| `Decimal` | `Int` | always |
| `Decimal` / unscaled nominal | `Exact<Decimal>` / `Exact<T>` | `scale = Some(s)`, `s ≤ 28`, `cd ≤ 28` |
| fixed-scale `F` | `Exact<F>` | `scale = Some(s)`, `s ≤ scale(F)` (range: runtime check) |
| otherwise | | `LOSSY_CONVERSION` (names `rescale`) |

## Admission errors (new or changed)

| Code | When |
|------|------|
| `EXACT_BOUND_EXCEEDED` | an exact expression's bound exceeds 511 bits (message: required vs supported) |
| `LOSSY_CONVERSION` | a store the rules cannot prove (also non-terminating ratios, possible digit overflow) |
| `TYPE_MISMATCH` | `SEK ÷ JPY`, `ratio + amount`, mixed units |
| `UNSUPPORTED_IR_VERSION` | wire IR `0.1`–`0.3` (message: migrate to 0.4) |

## Versions

| Artifact | Version |
|----------|---------|
| wire IR | `0.4` only |
| decision record | `0.4` (unchanged modules differ only in this field) |
| verifier | `0.4.0` |
