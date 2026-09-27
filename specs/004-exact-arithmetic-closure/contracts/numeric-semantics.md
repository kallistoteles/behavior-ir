# Contract: Numeric Semantics v2 (exact closure)

Principles: numeric computation is exact by default; bounded representation and rounding are
explicit. Lossless representation changes may be implicit only when statically proven. Admission
proves representational validity; verification proves reachable behavioral safety.

Notation: `D` Decimal, `I` Int, `N` ∈ {D, I}, `T` decimal nominal (any), `F` fixed-scale nominal,
`X<T>` = `Exact<T>`, `R` = `Exact<Decimal>` (dimensionless exact ratio/number).

## Typing

| Expression | Type | Runtime |
|---|---|---|
| `I ± I`, `I × I` | `I` | exact; 64-bit overflow is an evaluation error |
| `I ÷ I`; `±`, `×`, `÷` with `D` or `R` and numbers | `R` | exact rational |
| `F ± F`, `F × I`, `I × F` | `F` | exact; range overflow is an evaluation error |
| `T ± T`, `T × I` (unscaled `T`) | `X<T>` | exact |
| `T × N`, `N × T`, `T ÷ N`, `X<T> ± T/X<T>`, `X<T> × N/R`, `X<T> ÷ N/R` | `X<T>` | exact |
| `R × T`, `T × R`, `R × X<T>` | `X<T>` | exact |
| `T ÷ T`, `X<T> ÷ T`, `T ÷ X<T>`, `X<T> ÷ X<T>` (same `T`) | `R` | exact; division by zero is an evaluation error |
| comparisons in {`T`, `X<T>`} (same `T`) or in {`D`, `I`, `R`} | `Bool` | exact |
| `unwrap(X<T>)` | — | admission error: erasing a nominal unit is not an implicit coercion (deferred) |
| `T(e)`, `e : R` | `X<T>` | exact (attaches the unit explicitly) |
| `rescale(e, F, mode)` | `F` | one rounding (six modes, 003); range overflow is an evaluation error |
| `SEK ÷ JPY`, `R ± T`, `T ± D`, different nominals | — | admission error |

## Stores

| Target | Source | Without rescale when (proven at admission) |
|---|---|---|
| any | same type | always |
| `D` | `I` | always |
| `D`, unscaled `T` | `R`, `X<T>` | finite decimal `c·10^−s` provably with `s ≤ 28` and `|c| < 10^28` |
| `F` | `X<F>` | finite decimal, `scale ≤ scale(F)` |
| otherwise | | `LOSSY_CONVERSION` |

Examples (`amount`, `budget`: two-decimal `Money`; `x`, `y`: `Decimal`):

| Store | Result |
|---|---|
| `money := amount / 3` | `LOSSY_CONVERSION` |
| `money := rescale(amount / 3, Money, half_even)` | valid |
| `money := amount * 2` | valid (`F × I`) |
| `dec := x - y` | `LOSSY_CONVERSION` (two 28-digit inputs: not provably ≤ 28 digits) |
| `dec := 10 / 2` | valid (constant `5`) |
| `dec := x / 3` | `LOSSY_CONVERSION` (non-terminating) |

## Bound analysis

For every exact expression, bottom-up from leaves (fixed-scale inputs: scale `s`, 28 coefficient
digits; general decimal inputs: scale 28 and 56 coefficient digits, since their own scale ≤ 28 is
unknown; integers: scale 0, 19 digits; literals: exact; rescale results: like `F`; derived
references: the derived body's bound):

Decimal's representation: `c · 10^−s`, `|c| < 10^28`, `0 ≤ s ≤ 28` (the input format). The
analysis tracks the coefficient's digit count `cd` at the tracked scale; `s ≤ 28 ∧ cd ≤ 28` proves
representability (conservative only in not crediting trailing zeros).

| Op | numerator bits | denominator bits | scale | coefficient digits |
|---|---|---|---|---|
| `a · b` | `n₁+n₂` | `d₁+d₂` | `s₁+s₂` | `c₁+c₂` |
| `a ± b` (`s = max(s₁,s₂)`) | `max(n₁+d₂, n₂+d₁)+1` | `d₁+d₂` | `s` | `max(c₁+s−s₁, c₂+s−s₂)+1` |
| `a ÷ b` (literal `b = m·10^−k`, `|m| = 2^i5^j`) | `n₁+d₂` | `d₁+n₂` | `s₁+max(i,j)` | `c₁+max(i,j)+k` |
| `a ÷ b` (otherwise) | `n₁+d₂` | `d₁+n₂` | none | unbounded |

Any bound above 511 bits → `EXACT_BOUND_EXCEEDED` at that expression. The runtime keeps its
512-bit check only as an internal consistency error that admitted modules never reach.

## Examples (feature 001 `project_margin`, now exact)

`margin = (revenue - cost) / revenue` with revenue `100`, cost `33` → `67/100` (text `0.67`);
revenue `3`, cost `2` → `1/3`; `margin < 0.05` is exact.
