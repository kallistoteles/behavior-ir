# Contract: Numeric Semantics of Fixed-Scale Decimals

Principle: lossless operations may be implicit; lossy conversions must be explicit. Exactness
propagates implicitly; loss of information requires an explicit `rescale`.

`T` is a fixed-scale nominal with scale `s` (grid `10^-s`, range `|v| < 10^(28−s)`). `N` is
`Int` or `Decimal`. `D` is `Decimal`. Operations still require the ops `T` declares (`add` for
`±`, `scale` for `×`/`÷` by a number, `order` for `<`…, `ratio` for `T ÷ T`).

## Typing

| Expression | Type | Runtime |
|---|---|---|
| `T ± T` | `T` | exact; overflow error outside range |
| `T × Int`, `Int × T` | `T` | exact; overflow error outside range |
| `T × D`, `D × T`, `T ÷ N` | `Exact<T>` | exact rational |
| `Exact<T> ± (T \| Exact<T>)` (either order) | `Exact<T>` | exact; `T` lifted |
| `Exact<T> × N`, `N × Exact<T>`, `Exact<T> ÷ N` | `Exact<T>` | exact; division by zero is an error |
| `(T \| Exact<T>) cmp (T \| Exact<T>)` | `Bool` | exact |
| `T ÷ T` | `Decimal` | as today (28-digit rules) |
| `rescale(e, T, mode)`, `e : Exact<T> \| T \| D \| Int` | `T` | round once (modes below); overflow error outside range |
| `T("12.30")` literal | `T` | admission error if off grid or out of range |
| `T(e)`, `e : Int` | `T` | exact; overflow error outside range |
| `T(e)`, `e : D` | — | admission error: use `rescale` |
| `unwrap(e)`, `e : T` | `Decimal` | exact |
| `unwrap(e)`, `e : Exact<T>` | — | admission error: rescale first |
| mixing different nominals (`SEK` with `JPY`, `T` with `D` in `±`/cmp) | — | admission error (as today) |
| `rescale(unwrap(x), T2, mode)`, `x : T1` | `T2` | conversion between fixed-scale types; never rounds when `T2`'s grid is finer |

Where a `T` is required (effect into a `T` field, derived declared `T`, `value_or` default,
`Option<T>` wrapping) an `Exact<T>` is a `LOSSY_CONVERSION` admission error whose message names
`rescale`.

**Exact context**: `Decimal`/`Int` arithmetic sub-expressions that are operands of an `Exact<T>`
operation or of `rescale` are evaluated exactly, recursively through arithmetic nodes, not
through derived references. `Int` sub-expressions keep 64-bit overflow checks.

## Rounding modes (`y = x · 10^s`, result `k / 10^s`)

| Mode | `k` | `2.5` | `-2.5` | `-1.21` (s = 0) | `0.125` (s = 2) |
|------|-----|-------|--------|-----------------|-----------------|
| `half_even` | nearest; tie → even `k` | 2 | -2 | -1 | 0.12 |
| `half_up` | nearest; tie → away from zero | 3 | -3 | -1 | 0.13 |
| `down` | toward zero | 2 | -2 | -1 | 0.12 |
| `up` | away from zero | 3 | -3 | -2 | 0.13 |
| `floor` | toward −∞ | 2 | -3 | -2 | 0.12 |
| `ceiling` | toward +∞ | 3 | -2 | -1 | 0.13 |

If `y` is already an integer, every mode returns it unchanged (the trace still records the
step). The table is also data: `tests/fixtures/numeric/rounding.json`.

## Examples

| Expression (`amount = 20.00`, `Money` s = 2) | Exact | Result |
|---|---|---|
| `rescale((amount / 3) * 2, Money, half_even)` | `40/3` | `13.33` |
| `rescale(rescale(amount / 3, Money, half_even) * 2, Money, half_even)` | `20/3`, then `1334/100` | `6.67`, `13.34` |
| `rescale(10.01 * 0.25, Money, half_even)` | `2.5025` | `2.50` |
| `amount * 1.25 <= budget` (`budget = 25.00`) | `25` vs `25.00` | `true`, no rounding |

## Input

A `T` value in a request: JSON string or integer, plain notation; accepted iff every digit
beyond position `s` is zero and `|v| < 10^(28−s)`. Otherwise `INVALID_INPUT` with `OFF_GRID` /
`OUT_OF_RANGE` (as for other request type errors, in any section). Never rounded.

## Text forms

| Kind | Form | Examples |
|------|------|----------|
| fixed-scale value | exactly `s` fractional digits | `100.50`, `0.00`, `-3.10`, `42` (s = 0) |
| exact quantity | normalized finite decimal if the reduced denominator is `2^a·5^b`, else `n/d` | `5.0025`, `40/3`, `-7/6` |
| general decimal | normalized, as today | `12.5` |

## Limits

Exact quantities: reduced numerator and denominator each below `2^512`; beyond that, the
evaluation error `numeric overflow in <expr>`.
