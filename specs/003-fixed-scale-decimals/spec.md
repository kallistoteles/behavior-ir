# Feature Specification: Fixed-Scale Decimals

**Feature Branch**: `003-fixed-scale-decimals`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "fixed-scale decimals"

## Context

Behavior authors model money and similar quantities as decimal values. Today every decimal is a
general decimal of up to 28 significant digits, and the engine rounds silently when a result
needs more digits. The verifier (feature 002) must therefore treat decimal arithmetic as "exact
value plus a rounding bound", which makes many ordinary money properties undecidable: for
example "an approval never exceeds the remaining budget" written via
`remaining = budget − spent` is reported as inconclusive, and a waiver is needed to commit
(see `docs/verification.md`, "Known limitations").

Real money has a fixed number of decimal places (for example two). This feature lets authors
declare that a decimal-based type has a fixed scale, so that values of that type are always
exact multiples of 10^-scale: nothing outside that grid can enter, lossless operations (addition,
subtraction, multiplication by a whole number) never round, and every value that must be narrowed
back to the grid passes through an explicit rescale that names its rounding. The verifier can
then reason about these values exactly and prove such properties.

## Clarifications

### Session 2026-09-27

- Q: Where is the rounding for multiplication, division, and conversion to a fixed-scale type
  stated? → A: Explicit at every scale-reduction boundary. Fixed-scale addition/subtraction that
  remains exactly representable needs no rounding declaration. Any operation or conversion whose
  result must be narrowed/rescaled to a fixed-scale type must use an explicit rescale operation
  (target scale and rounding mode). The rounding mode is part of the Behavior IR, hash, trace,
  and SMT semantics; types do not silently supply a default. Principle: *lossless operations may
  be implicit; lossy conversions must be explicit.*
- Q: When a rescale wraps a longer expression, is the whole expression computed exactly and
  rounded once at the end? → A: Yes. Arithmetic inside a rescale is evaluated exactly, using
  rational arithmetic where necessary, and the result is rounded exactly once at the rescale
  boundary. Nested explicit rescale operations remain separate lossy boundaries and therefore may
  intentionally introduce multiple rounding steps. The trace records the exact pre-rescale value
  and the explicit rounding operation.
- Q: Which rounding modes can a rescale use? → A: Six: `half_even`, `half_up`, `down`, `up`,
  `floor`, `ceiling`. They are distinct deterministic semantics, particularly for negative
  values. No mode is implicit; every lossy rescale states its mode, and evaluation, tracing,
  replay, hashing, and SMT verification use the same definition.
- Q: Where may an off-grid intermediate value (e.g. `amount * 0.25`) be used without a rescale?
  → A: Anywhere exactness is kept, as an exact quantity rather than as Money itself. Operations
  that leave a fixed-scale grid produce `Exact<Money>` (or equivalent): it may participate in
  exact arithmetic, comparisons, predicates, and exact derived values. A conversion into a
  fixed-scale Money field, effect, or Money-typed derived value requires an explicit rescale.
  Fixed-scale values may be losslessly lifted to the exact domain for mixed comparisons and
  arithmetic. Principle: *exactness propagates implicitly; loss of information requires an
  explicit operation.*
- Q: Should currency be part of this feature? → A: No. Keep currency out of the core feature.
  Represent each currency, where needed, as a distinct nominal fixed-scale decimal type, e.g.
  `SEK = nominal(Decimal, scale=2)` and `JPY = nominal(Decimal, scale=0)`. Different nominal
  types cannot be mixed. Currency conversion and exchange-rate semantics belong to a later
  domain layer and can be introduced explicitly without changing the fixed-scale or
  exact-arithmetic model.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Declare money with a fixed number of decimals (Priority: P1)

A behavior author declares a money type with two decimal places and uses it for amounts,
budgets, and limits. Requests that carry more decimal places than declared are rejected before
any rule runs; sums and differences of such amounts are always exact; decision records show
amounts with exactly the declared number of decimals.

**Why this priority**: It is the foundation: without a type that guarantees the grid, neither
exact evaluation nor sharper verification is possible, and it removes a class of silent-rounding
surprises at runtime on its own.

**Independent Test**: Declare a two-decimal money type in a module, evaluate requests with
`100.5`, `100.50`, and `100.505`, and add and subtract amounts near the largest allowed value;
no verifier is needed.

**Acceptance Scenarios**:

1. **Given** a money type with two decimals, **When** a request carries the amount `100.505`,
   **Then** it is rejected before any rule is evaluated as INVALID_INPUT (like every other type
   error in a request, whichever section the field is in), with reason code `OFF_GRID` naming the
   field and the declared scale.
2. **Given** the same type, **When** a request carries `100.5`, **Then** it is accepted and the
   decision record shows the value as `100.50`.
3. **Given** two amounts of the type, **When** a rule adds or subtracts them, **Then** the result
   is exact and has the same scale; a result outside the type's range is an evaluation error
   (overflow), never a rounded value.
4. **Given** a module without any fixed-scale type, **When** it is admitted and evaluated,
   **Then** its behavior version, hashes, and decision records are unchanged from feature 002.

---

### User Story 2 - Explicit rescale at every scale-reduction boundary (Priority: P2)

An author computes a 25% discount on an amount, or splits an amount between participants.
Because the exact result can leave the money type's grid, turning it back into money requires an
explicit rescale that names the target scale and the rounding mode. The rescale is a step of the
behavior: it is part of the behavior version, visible in the trace, and modelled by the verifier.

**Why this priority**: Money arithmetic beyond addition is common (discounts, taxes, shares), and
it is exactly where silent rounding happens today. Making the rounding explicit keeps
evaluation exact and auditable, but it is only needed once fixed-scale types exist (US1).

**Independent Test**: `fee = rescale(amount × 0.25, scale 2, half-even)` on a two-decimal type,
evaluated for `10.01`, gives `2.50`; the same assignment without the rescale is rejected at
admission.

**Acceptance Scenarios**:

1. **Given** a two-decimal amount `10.01` and `fee = rescale(amount × 0.25, scale 2, half-even)`,
   **When** it is evaluated, **Then** the fee is `2.50`, identical on every run and in replay, and
   the trace shows the rescale step with its exact input `2.5025`, the rounding mode, and the
   result.
2. **Given** an amount multiplied by a decimal, divided by a number, or a general decimal, used
   where a fixed-scale value is required without a rescale, **When** the module is admitted,
   **Then** admission fails at that expression, naming the narrowing and asking for an explicit
   rescale.
3. **Given** `amount = 20.00` and `rescale((amount / 3) * 2, Money, half-even)`, **When** it is
   evaluated, **Then** the result is `13.33`, computed from the exact value `40/3` with a single
   rounding, and the trace shows `40/3`; with an inner rescale around `amount / 3` written by the
   author, the result is `13.34` and the trace shows both rounding steps.
4. **Given** `subtotal = a + b` or `total = price × quantity` (a whole number) on the same
   two-decimal type, **When** it is admitted and evaluated, **Then** no rescale is required and
   the result is exact (lossless operations may be implicit).
5. **Given** the precondition `amount * 1.25 <= budget` on two-decimal amounts, **When** it is
   admitted and evaluated, **Then** no rescale is required: `amount * 1.25` is an exact quantity,
   `budget` is lifted losslessly, and the comparison is exact (for `amount = 20.01` the trace shows
   `25.0125 <= budget`).
6. **Given** `set_(invoice.fee, amount * 0.25)` or a derived value declared as Money returning
   `amount * 0.25`, **When** the module is admitted, **Then** admission fails and asks for a
   rescale; a derived value declared as `Exact<Money>` returning the same expression is admitted.
7. **Given** the ratio of two amounts (amount ÷ amount), **When** it is evaluated, **Then** the
   result is a general decimal (not money), as today.
8. **Given** a fixed-scale type declaration, **When** an author tries to give it a default
   rounding mode, **Then** there is no such option: rounding exists only on rescale operations.

---

### User Story 3 - Money properties become provable (Priority: P3)

A verifier user verifies the purchase-approval model with a two-decimal money type. Properties
that are inconclusive with general decimals — such as preserving the budget invariant when the
precondition is written via `remaining(project)` — are now proven, and counterexamples use
values with the declared number of decimals.

**Why this priority**: This is the payoff for verification, but it depends on US1 (and on US2 for
models that multiply or divide money).

**Independent Test**: Verify `purchase_remaining` with Money declared at two decimals: the
`within_budget` preservation check is proven and the module is verified without waivers.

**Acceptance Scenarios**:

1. **Given** the purchase model with a two-decimal money type and the precondition
   `amount <= remaining(project)`, **When** it is verified, **Then** `within_budget` is proven
   for `approve` and the attestation has no inconclusive finding.
2. **Given** the same model without the budget precondition, **When** it is verified, **Then**
   the counterexample's amounts have exactly two decimals and the evaluator reproduces the
   violation.
3. **Given** a module mixing general decimals and fixed-scale money, **When** it is verified,
   **Then** only the general-decimal parts keep the rounding interval; fixed-scale arithmetic is
   reasoned about exactly.

---

### Edge Cases

- A request value with trailing zeros beyond the scale (`100.500` for a two-decimal type) is
  accepted: it lies on the grid. A value with a non-zero digit beyond the scale is rejected.
- The largest value of a fixed-scale type: the number of integer digits is limited so that every
  value fits the engine's 28 significant digits; a larger request value is INVALID_INPUT; a
  computed result beyond the range is an overflow evaluation error.
- Scale 0 (whole units, for example öre or cents stored as integers) is allowed; a scale larger
  than 28 is rejected at admission.
- Two different nominal types (for example `SEK` and `JPY`, whatever their scales), or a
  fixed-scale type and a general decimal, cannot be added, subtracted, or compared without an
  explicit conversion, neither as fixed-scale values nor as exact quantities; admission reports
  the mismatch.
- Converting a fixed-scale value to a general decimal is always exact; converting a general
  decimal to a fixed-scale type requires an explicit rescale, as for multiplication.
- A rescale whose input is already on the target grid returns it unchanged; the trace still
  records the step.
- Converting between two fixed-scale types (for example to a four-decimal type) is written
  `rescale(unwrap(x), Target, mode)`: `unwrap` is exact and the rescale is the explicit conversion
  (it never rounds when the target grid is finer); nominal types never convert implicitly.
- Literals in the behavior must lie on the grid of their type (`Money(0.005)` for a two-decimal
  type is an admission error).
- Changing a type's scale is a semantic change: the behavior version changes, and existing
  attestations, waivers, and decision records keep referring to the old version.
- Division by zero in a fixed-scale expression (also inside a rescale) remains an evaluation
  error.
- An exact value inside a rescale that grows beyond the engine's limits for exact arithmetic is an
  overflow evaluation error, never an approximation; a rescaled result outside the target type's
  range is an overflow error as well.
- General-decimal sub-expressions that feed an exact computation (an operand of an `Exact<T>`
  operation or of a rescale) are evaluated exactly as part of it, through arithmetic operators
  only: a referenced derived general decimal keeps the value it has on its own (computed with
  general-decimal rules and reported once in the record). Standalone general-decimal expressions
  keep their current semantics.
- `Exact<T>` can never be stored: an effect assigning an `Exact<T>` to a field of type `T` without
  a rescale is an admission error.

## Requirements *(mandatory)*

### Functional Requirements

**Declaring scale**

- **FR-001**: Authors MUST be able to declare a fixed scale (a whole number of decimal places,
  0–28) for a decimal-based nominal type, in the Python DSL and in the wire IR.
- **FR-002**: The scale MUST be part of the type's semantic identity: it is included in the
  content hash, so changing it changes the behavior version; modules that declare no scale MUST
  keep their feature-002 behavior versions and hashes.
- **FR-003**: The range of a fixed-scale type MUST be defined by its scale (at most 28 − scale
  integer digits) and documented.

**Values and requests**

- **FR-004**: A request value for a fixed-scale field MUST be accepted only if it lies exactly on
  the type's grid and within its range; otherwise the request is INVALID_INPUT, as for every other
  type error in a request (whichever section), with reason code `OFF_GRID` or `OUT_OF_RANGE`
  naming the field and the scale. Values are never rounded on input.
- **FR-005**: Fixed-scale values MUST appear in decision records, traces, and changes with
  exactly `scale` fractional digits (for example `100.50`); replay MUST reproduce them byte for
  byte.
- **FR-006**: Literals of a fixed-scale type MUST lie on its grid; otherwise admission fails.

**Arithmetic**

- **FR-007**: Lossless operations MUST need no rounding: addition and subtraction of two values of
  the same fixed-scale type, and multiplication of such a value by a whole number, are exact and
  yield that type; a result outside the range MUST be an overflow evaluation error.
- **FR-008**: Comparison of two values of the same fixed-scale type MUST be exact.
- **FR-009**: Every scale-reduction boundary MUST be explicit (the principle; FR-009a gives the
  typing rule): wherever a value whose exact result can leave the grid (multiplication by a
  `Decimal`, division, a general decimal) is used as a fixed-scale value, the behavior MUST
  contain a rescale operation naming the target fixed-scale type (and so its scale) and a
  rounding mode; otherwise admission fails at that
  expression. There MUST be no type-level default rounding and no implicit narrowing anywhere.
- **FR-009a**: Operations whose result can leave a fixed-scale grid (multiplication by a
  `Decimal` operand, decided by type, even for `2.0`; division by a number) MUST yield an
  **exact quantity** `Exact<T>` of the same nominal type `T`: a mathematically exact value
  (rational where necessary, e.g. `20.00 / 3 = 20/3`) that is not necessarily on `T`'s grid. `Exact<T>` values MUST be usable in exact arithmetic,
  comparisons, predicates, and derived values declared as `Exact<T>`; fixed-scale values of `T`
  MUST be lifted to `Exact<T>` implicitly (lossless) when mixed with them, so
  `amount * 1.25 <= budget` is valid and exact. An `Exact<T>` MUST become a `T` (a field value,
  an effect, a derived value declared as `T`) only through a rescale; otherwise admission fails.
  Within an exact computation nothing is rounded; the only rounding happens at a rescale, unless
  the author writes another rescale inside.
  `rescale((20.00 / 3) * 2, Money, half-even)` is `40/3 → 13.33` (one rounding);
  `rescale(rescale(20.00 / 3, Money, half-even) * 2, Money, half-even)` is
  `20/3 → 6.67`, `13.34 → 13.34` (two roundings, both written by the author).
- **FR-010**: The rescale operation MUST be part of the Behavior IR: its target scale and rounding
  mode are included in the behavior hash, it appears in the trace with the expression, its exact
  pre-rescale value (as a fraction when it has no finite decimal form, e.g. `40/3`), the rounding
  mode, and the rounded result, and the verifier models it exactly (exact rational value, then
  rounding to the grid).
- **FR-010a**: Exactly six rounding modes MUST be available, each defined identically for
  evaluation, tracing, replay, hashing, and verification:

  | Mode | Rule | `2.5` → | `-2.5` → | `-1.21` → (scale 0) |
  |------|------|--------|---------|-------------------|
  | `half_even` | nearest; ties to the even neighbour | 2 | -2 | -1 |
  | `half_up` | nearest; ties away from zero | 3 | -3 | -1 |
  | `down` | toward zero | 2 | -2 | -1 |
  | `up` | away from zero | 3 | -3 | -2 |
  | `floor` | toward −∞ | 2 | -3 | -2 |
  | `ceiling` | toward +∞ | 3 | -2 | -1 |

  No mode is a default; a rescale without a mode is an admission error.
- **FR-011**: The ratio of two values of the same fixed-scale type MUST yield a general decimal,
  as today; converting a fixed-scale value to a general decimal MUST be exact. Known tension:
  the ratio is computed with general-decimal rules (up to 28 digits), so it can round without an
  explicit rescale; this is kept in scope deliberately and recorded as a follow-up (an exact
  ratio type) rather than changed here.
- **FR-011a**: Exact quantities MUST be shown in traces and records as a finite decimal when one
  exists (`2001/400` as `5.0025`) and otherwise as a reduced fraction (`40/3`). Exact values never
  appear in state: only fixed-scale values are stored in fields.
- **FR-012**: Adding, subtracting, or comparing values of different nominal types (for example
  different scales), or a fixed-scale value with a general decimal, MUST be an admission error
  unless an explicit conversion is written. Multiplying or dividing a fixed-scale value by a
  number is allowed and yields an exact quantity (FR-009a).

**Verification**

- **FR-013**: The verifier MUST reason about fixed-scale values and exact quantities exactly:
  `Exact<T>` as mathematical rationals, fixed-scale values as rationals constrained to their grid
  and range (exactly what the runtime enforces), and each rescale as its stated rounding function.
  No rounding interval applies to them.
- **FR-014**: Counterexamples MUST use values on the declared grid; every counterexample is still
  confirmed by evaluation (feature 002).
- **FR-015**: General decimals MUST keep their current semantics and verification model; this
  feature changes nothing for modules that do not use fixed-scale types.

**Interfaces**

- **FR-016**: Fixed-scale types MUST be available through the Python DSL, the wire IR (with its
  JSON Schema), the CLI, and the Python binding; values cross the Python boundary as `Decimal`.
- **FR-017**: The examples and verification fixtures MUST include models using a two-decimal money
  type, added as new variants so the existing feature 001/002 fixtures stay unchanged (SC-004),
  and `docs/verification.md` MUST be updated.

### Key Entities

- **Fixed-scale nominal type**: a named decimal-based type with a scale (0–28) and its permitted
  operations (as today: order, add, scale, ratio). Its scale is part of its semantic identity. It
  has no rounding of its own.
- **Rescale operation**: a behavior step that narrows an exact value to a fixed-scale type: its
  input (an exact quantity or general decimal, evaluated exactly), target type (scale), and
  rounding mode. The only lossy
  boundary in an expression. Part of the behavior hash, the trace (exact value and rounded
  result), and the verifier's model.
- **Exact quantity (`Exact<T>`)**: a mathematically exact value of the same nominal type `T` that
  need not lie on `T`'s grid (a rational where necessary). Produced by lossy-looking operations,
  used freely in exact arithmetic, comparisons, predicates, and exact derived values; never
  stored; becomes a `T` only through a rescale.
- **Rounding mode**: one of six named, deterministic rules for bringing an exact value onto a grid
  (`half_even`, `half_up`, `down`, `up`, `floor`, `ceiling`; FR-010a), used only by rescale
  operations and always stated explicitly.
- **Fixed-scale value**: an exact multiple of 10^-scale within the type's range, written with
  exactly `scale` fractional digits in all artifacts.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With Money declared at two decimals, the purchase-approval model whose precondition
  uses `remaining(project)` is verified with zero inconclusive findings (it is inconclusive
  today).
- **SC-002**: With Money at two decimals, the example modules (purchase approval, invoice) produce
  no inconclusive finding caused by money arithmetic; every remaining finding is a confirmed
  counterexample or a warning.
- **SC-003**: 100% of request values off the grid of their fixed-scale type are rejected before
  any rule runs, and no evaluation ever changes a fixed-scale value by rounding except at an
  explicit rescale step, which appears in the trace.
- **SC-004**: All feature 001 and 002 fixtures (behavior versions, hash vectors, decision records,
  attestation outcomes) are unchanged. Attestation hashes and check keys change only because the
  verifier version is part of them.
- **SC-005**: Verifying the example modules with fixed-scale money takes no longer than with
  general decimals (within 10% or 1 second, whichever is larger).
- **SC-006**: Two runs of evaluation, replay, and verification on fixed-scale models produce
  byte-identical artifacts.

## Assumptions

- Scale is declared on nominal types (the way Money is modelled today); plain `Decimal` fields
  stay general decimals. Authors who want fixed scale for a plain field declare a nominal type.
- Currency is not a core concept: each currency is its own nominal fixed-scale type
  (`SEK` scale 2, `JPY` scale 0), and mixing nominal types is an admission error, also as exact
  quantities (`Exact<SEK> + Exact<JPY>`). Currency conversion, exchange rates (source, timestamp,
  direction), and triangulation are a later domain layer, e.g. a conversion yielding
  `Exact<EUR>` followed by an explicit rescale.
- Input is strict: off-grid values are rejected, never rounded, in line with "invalid behavior
  and data cannot enter the semantic model".
- The canonical text form of a fixed-scale value keeps exactly `scale` fractional digits; general
  decimals keep their normalized form.
- A new wire IR version is introduced only for modules that use fixed-scale types; older
  documents remain valid unchanged (as with 0.1 → 0.2).
- Division by zero and overflow keep their current error semantics.
- "Multiplication by a whole number" (lossless, FR-007) is decided by type: the other operand is
  an `Int` expression. A decimal operand, even a literal like `2.0`, yields an exact quantity.
- Exact arithmetic has a fixed, documented size limit (chosen in planning) so evaluation stays
  bounded; exceeding it is an overflow evaluation error, never an approximation.
- The persistence contract (`docs/proposals/persistence-contract.md`) is a separate, later
  feature.
