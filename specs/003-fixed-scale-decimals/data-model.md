# Data Model: Fixed-Scale Decimals

Extends `specs/001-verifiable-behavior-ir/data-model.md` and
`specs/002-smt-verification/data-model.md`. Semantics: [contracts/numeric-semantics.md](contracts/numeric-semantics.md).

## Fixed-scale nominal type

| Field | Content | Rules |
|-------|---------|-------|
| `name` | nominal name, e.g. `SEK` | unique among declarations (as today) |
| `underlying` | `decimal` | a scale requires `decimal` |
| `ops` | subset of `order`, `add`, `scale`, `ratio` | as today |
| `scale` | integer 0–28 | part of the declaration hash (`behavior.nominal.fixed.v1`) |

Range: `|v| < 10^(28 − scale)`. No rounding mode belongs to the type.

## Exact quantity `Exact<T>`

| Aspect | Rule |
|--------|------|
| Exists for | fixed-scale nominals `T` only |
| Value | reduced rational `n/d`, `d > 0`, `|n|, d < 2^512` (else overflow error) |
| Produced by | `T × Decimal`, `T ÷ Int`, `T ÷ Decimal`, and any arithmetic with an `Exact<T>` operand |
| Usable in | arithmetic, comparisons, predicates, derived values (declared or inferred `Exact<T>`) |
| Never | stored in a field, used as an effect value of type `T`, unwrapped, part of a request |
| Becomes `T` | only via `rescale` |
| Text form | normalized finite decimal if `d = 2^a·5^b`, else `"n/d"` |

## Rescale (expression)

| Field | Content |
|-------|---------|
| `arg` | expression of type `Exact<T>`, `T`, `Decimal`, or `Int`; evaluated exactly |
| `nominal` | target fixed-scale type `T` |
| `rounding` | `half_even` \| `half_up` \| `down` \| `up` \| `floor` \| `ceiling` (required) |
| result | `T`; overflow error if outside `T`'s range |
| hash | `arg ‖ nominal decl hash ‖ u8 rounding` (new op code) |

## Rescale trace entry (record 0.3)

| Field | Example |
|-------|---------|
| `rescale` | `"(invoice.amount / 3) * 2"` (expression text of `arg`) |
| `exact` | `"40/3"` |
| `rounding` | `"half_even"` |
| `scale` | `2` |
| `result` | `"13.33"` |

Appended to the trace step (condition, effect, rule check) that evaluated it, in evaluation
order; a skipped step has none.

## Derived value (addition)

| Field | Content |
|-------|---------|
| `type` (optional, wire 0.3) | declared type; must equal the inferred type; not hashed |

## Input validation (addition)

| Code | When | Result |
|------|------|-------------------|
| `OFF_GRID` | a `T` value has a non-zero digit beyond `scale` | INVALID_INPUT (any section, like other request type errors) |
| `OUT_OF_RANGE` | `|v| ≥ 10^(28 − scale)` | INVALID_INPUT |

## Versions

| Artifact | Version | When |
|----------|---------|------|
| wire IR | `0.3` | a module uses `scale`, `exact`, `rescale`, or a derived `type`; otherwise the lowest sufficient version |
| decision record | `0.3` | the module is 0.3; otherwise unchanged `0.2` |
| verifier | `0.3.0` | verification of fixed-scale modules; attestation format unchanged |
