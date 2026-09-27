# Feature Specification: Exact Arithmetic Closure

**Feature Branch**: `004-exact-arithmetic-closure`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "Exact arithmetic closure: once an expression is in the exact
numeric domain, further arithmetic stays exact until the author explicitly chooses an operation
that loses information" (full description in the conversation; summarized below).

## Context

Feature 003 introduced fixed-scale values (e.g. two-decimal money), exact quantities
(`Exact<T>`), and an explicit `rescale` as the only place where a fixed-scale value is rounded.
Its review left two gaps in the numeric semantics:

1. **Soundness**: the runtime bounds exact quantities to a fixed size (512 bits for numerator and
   denominator) and reports an overflow beyond it, while the verifier reasons about exact
   quantities as unbounded. A proof of "no evaluation error" can therefore be wrong for an
   expression whose intermediate values grow past the bound.
2. **Semantics**: dividing two amounts of the same type (Money ÷ Money) yields a general decimal
   rounded to 28 digits, and general decimal arithmetic in general rounds after every operation —
   implicit losses of information, contradicting the principle that exactness propagates
   implicitly and loss of information requires an explicit operation.

This feature states the property that closes both gaps:

> **The exact numeric domain is closed under arithmetic until an explicit lossy boundary is
> crossed.** Operations on exact values produce exact values; they never fall back to rounded
> general-decimal semantics. Every exact value the verifier admits is representable by the
> evaluator on every reachable path.

An exact ratio type is one consequence of this rule, not the goal itself. So is the end of
28-digit rounded arithmetic for general decimals: a `Decimal` is an exact finite decimal value,
not a calculator that rounds after each operation. This is a deliberate, versioned break with the
semantics of features 001–003 for modules that do decimal arithmetic.

## Clarifications

### Session 2026-09-27

- Q: What role should general Decimal have after exact arithmetic closure? → A: B. General
  Decimal values remain valid exact finite decimal values, but arithmetic must participate in the
  exact numeric domain rather than silently applying 28-digit rounding. Operations that cannot be
  represented as a finite decimal, particularly division, produce an exact rational/ratio
  representation. Any conversion back to a bounded or fixed-scale decimal representation requires
  an explicit rescale with a rounding mode. Accept the behavior/hash version break now rather
  than making general Decimal a permanent exception to the exactness principle. Principle:
  *numeric computation is exact by default; bounded representation and rounding are explicit.*
- Q: How does a computed value end up in a general decimal field? → A: Computed exact values may
  be stored in a general Decimal only when admission can prove the conversion is lossless and the
  value is always exactly representable within Decimal's bounded representation. No rounding is
  implicit. If representability cannot be proven — including non-terminating ratios or possible
  digit overflow — admission fails and the author must use an explicit rescale to a
  bounded/fixed-scale target. The rule depends on representability, not on whether the value came
  from a literal, copy, or computation. Principle: *lossless representation changes may be
  implicit only when statically proven; lossy representation changes are always explicit.*
- Q: Are the two consequences accepted (the broader break for unscaled decimal stores; magnitude
  overflow of fixed-scale values staying a runtime error)? → A: Confirmed on both points. (1) The
  broader admission break is intentional: a computed exact value may only be stored in an
  unscaled general Decimal/nominal Decimal when lossless representability can be proven
  statically; existing modules that store computed monetary values declare an appropriate fixed
  scale or use an explicit conversion. (2) Fixed-scale magnitude overflow stays a runtime
  semantic error detected by verification, not a general admission failure. Admission
  establishes type/representation correctness and absence of implicit precision loss;
  verification establishes value-dependent safety over the valid reachable domain, including
  overflow and division by zero. Admission does not depend on entity constraints (they belong to
  verification); type-level static bounds may be used where needed to guarantee the runtime's
  exact representation limits. Principle: *admission proves representational validity;
  verification proves reachable behavioral safety.*
- Q: Plan review refinements? → A: (1) `unwrap(Exact<Nominal>) → Exact<Decimal>` is not an
  ordinary lossless coercion: it loses nominal semantics and could bypass nominal type safety; it
  is omitted for now (nominal erasure of exact values would be an explicit semantic operation,
  added only for a concrete use case). (2) Migrated modules need not keep identical wire bytes:
  wire format and semantic identity are separate; a 0.4 migration may change serialization and
  version metadata while semantically unchanged nodes keep their canonical semantic hashes.
  (3) The Decimal representability metric must match the specified 28-digit representation
  exactly, or be documented as a conservative approximation.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Exact values never exceed what the runtime can represent (Priority: P1)

A behavior author writes money arithmetic with exact intermediates. When the module is admitted,
the engine determines, for every exact expression, the largest value its intermediate results
can reach given the runtime's guarantees on inputs. An expression that could exceed the runtime's
exact representation is rejected at admission with a message naming the expression and the
bound, so no admitted module can ever hit an exact-size overflow at runtime, and the verifier's
unbounded reasoning is valid for everything it is asked about.

**Why this priority**: It closes a soundness gap: until it is closed, "the verifier never
produces a wrong proof" is not true for exact arithmetic. Everything else builds on a shared,
finite exact domain.

**Independent Test**: Admit a module with ordinary money arithmetic (accepted) and one with a
long chain of divisions by decimal inputs that could exceed the bound (rejected with the
expression and bound named); evaluate the accepted module on extreme inputs without any
exact-size overflow.

**Acceptance Scenarios**:

1. **Given** a module whose exact expressions are ordinary money arithmetic (a handful of
   operations on fixed-scale values, integers, and decimal literals), **When** it is admitted,
   **Then** admission succeeds and records nothing extra for the author to do.
2. **Given** an exact expression whose intermediate values could exceed the runtime's exact
   representation (for example a chain of many divisions by decimal inputs), **When** the module
   is admitted, **Then** admission fails with an error at that expression stating the required
   and the supported size.
3. **Given** any admitted module, **When** it is evaluated with the most extreme inputs the
   runtime accepts, **Then** no exact-size overflow occurs; overflow errors remain possible only
   where they are value-dependent and found by verification (results out of a fixed-scale type's
   range, integer overflow). Decimal arithmetic itself never overflows, and stores into general
   decimals are proven representable at admission.
4. **Given** a module admitted under the bound, **When** it is verified, **Then** the verifier's
   exact reasoning is sound for it: no proof depends on values the runtime could not represent.

---

### User Story 2 - Ratios of amounts stay exact (Priority: P2)

An author computes the share of a budget an amount represents (`amount / budget`), uses it to
scale another amount (`(amount / budget) * total`), and compares it with a threshold
(`amount / budget <= 0.25`). The ratio is an exact, dimensionless value: comparisons and further
arithmetic are exact, and turning it back into a stored value requires an explicit rescale.

**Why this priority**: It removes the last implicit rounding in fixed-scale arithmetic, making
the numeric semantics of fixed-scale modules fully explicit; it depends on the shared bounded
domain (US1).

**Independent Test**: Evaluate `amount / budget` for `10.00 / 30.00` (exactly `1/3` in the
trace), `(amount / budget) * total` rescaled to money, and a threshold comparison, and check
that storing a ratio without rescale is rejected at admission.

**Acceptance Scenarios**:

1. **Given** two amounts of the same fixed-scale type, **When** one is divided by the other,
   **Then** the result is an exact ratio (e.g. `10.00 / 30.00` is exactly `1/3`), not a rounded
   decimal.
2. **Given** an exact ratio and an amount of type `T`, **When** they are multiplied, **Then** the
   result is an exact quantity of `T`; `rescale((amount / budget) * total, Money, half_even)`
   rounds exactly once.
3. **Given** `amount / budget <= 0.25`, **When** it is evaluated, **Then** the comparison is exact
   and needs no rescale.
4. **Given** an effect or a declared result that would store an exact ratio in a field,
   **When** the module is admitted, **Then** admission fails and names `rescale`.
5. **Given** amounts of two different nominal types (e.g. SEK and JPY), **When** one is divided
   by the other, **Then** admission fails: that is an exchange rate, which belongs to a later
   domain layer, not a dimensionless ratio.

---

### User Story 3 - Exact arithmetic composes everywhere it is meaningful (Priority: P3)

An author mixes exact quantities, ratios, fixed-scale values, integers, and decimal literals in
larger expressions and derived values. Every combination that is meaningful yields an exact
result of a predictable type; nothing is rounded until an explicit rescale; evaluation, replay,
trace, and verification all agree on the value.

**Why this priority**: It turns the rules of US1 and US2 into a complete, predictable algebra and
proves that runtime and verifier share identical semantics across it.

**Independent Test**: A table of expression shapes with their expected result types and example
values, checked at admission, at evaluation, in replay, and by the verifier.

**Acceptance Scenarios** (Money is a two-decimal fixed-scale type; `amount`, `budget`, `total`
are Money):

1. `amount * 0.25` → exact Money; `amount / 3` → exact Money; `amount / budget` → exact ratio;
   `(amount / budget) * amount` → exact Money; `amount * 1.25 <= budget` → Bool, exact.
2. `money_field := amount / 3` → admission error; `money_field := rescale(amount / 3, Money,
   half_even)` → valid.
3. **Given** any expression in the exact domain, **When** it is evaluated, replayed, and
   verified, **Then** the three agree on its value, and the trace shows exact values as finite
   decimals or, when none exists, as fractions (e.g. `1/3`).
4. **Given** a verification of a property over exact ratios (e.g. "a share computed as
   `rescale(total * (amount / budget), Money, floor)` never exceeds `total` when
   `amount <= budget`"), **When** it is verified, **Then** it is proven, and a variant without
   the premise has a confirmed counterexample.

---

### User Story 4 - General decimals compute exactly (Priority: P2)

An author of a module without fixed-scale types (for example the project-margin example) computes
`margin = (revenue - cost) / revenue` with plain decimals. The margin is exact (`67/100` for
revenue 100 and cost 33), a threshold comparison `margin < 0.05` is exact, and a decimal field
receives a computed value only through the storage rule of FR-012a; nothing is rounded silently.

**Why this priority**: Without it, general decimals remain a permanent exception to the exactness
principle; it shares the closed exact domain and bound of US1 and the ratio semantics of US2.

**Independent Test**: Evaluate the project-margin example: its trace shows `margin` as an exact
value and its decisions are unchanged in outcome for the existing requests; verify it, and check
that feature 002's rounding-interval inconclusive cases for decimal arithmetic are now decided.

**Acceptance Scenarios**:

1. **Given** revenue `100` and cost `33`, **When** `(revenue - cost) / revenue` is evaluated,
   **Then** the value is exactly `67/100` (shown as `0.67`), and for revenue `3`, cost `2` it is
   exactly `1/3`.
2. **Given** `margin < 0.05`, **When** it is evaluated, **Then** the comparison is exact.
3. **Given** a decimal field assigned from decimal arithmetic, **When** the module is admitted and
   evaluated, **Then** the storage rule of FR-012a applies and no value is rounded without an
   explicit rescale in the trace.
4. **Given** the verification fixtures of feature 002 that were inconclusive only because of
   decimal rounding intervals (e.g. the tight `a / b < c` bound, sums near 10^28), **When** they
   are verified, **Then** they are decided (proven or refuted with a confirmed counterexample)
   according to exact semantics; the fixture expectations are updated and the changes recorded.

---

### Edge Cases

- A ratio with a zero divisor (`amount / budget` with `budget = 0`) is a division-by-zero
  evaluation error, as for every other division.
- A ratio of a value by itself is exactly `1`; ratios may be negative when amounts are.
- Ratios and plain numbers: a ratio can be compared with and combined with integers and decimal
  literals by lifting them exactly; a ratio is never implicitly converted to a general decimal.
- Ratio of exact quantities: `Exact<T> ÷ T`, `T ÷ Exact<T>`, and `Exact<T> ÷ Exact<T>` are exact
  ratios; `ratio ± ratio`, `ratio × ratio`, `ratio ÷ ratio` are exact ratios.
- Adding a ratio to an amount (`ratio + amount`) is meaningless (dimensionless plus money) and is
  an admission error.
- The size bound is determined from the runtime's guarantees on inputs (the range of each
  fixed-scale type, the digit limit of general decimals, the 64-bit range of integers) and from
  literals; an expression that is admissible for a type with scale 2 may be inadmissible for a
  type with scale 28.
- Derived exact values used inside other exact expressions contribute their own bound; bounds
  compose through derived references.
- Changing a module so that an expression's bound grows past the limit turns an admissible module
  into an inadmissible one; the error names the expression.
- Modules that do decimal arithmetic change meaning (exact instead of 28-digit rounded) and
  therefore change behavior version, whether or not they use fixed-scale types; modules whose
  decimals are only compared and stored keep theirs where the typing is unchanged.
- A decimal input or field value keeps the existing input format (plain notation, at most 28
  significant digits); only arithmetic changes.
- Representability is proven from types and literals only (not from entity constraints or
  preconditions). A general `Decimal` input can carry up to 28 significant digits at any scale,
  so the sum or product of two unconstrained `Decimal` inputs is **not** provably representable
  (for example `10^27 + 10^-28` needs 56 digits) and needs an explicit rescale to be stored;
  copies, literals, constant expressions (`10 / 2`), and arithmetic whose digits are bounded by
  the involved types (fixed-scale operands) are provable.
- A fixed-scale value's range check (`T ± T` exceeding the type's range) is a magnitude overflow,
  not a representation loss: it stays an evaluation error found by the verifier's
  `evaluation_error` check, as in feature 003.

## Requirements *(mandatory)*

### Functional Requirements

**Closure**

- **FR-001**: Every arithmetic operation whose operands are in the exact domain (exact
  quantities, exact ratios, fixed-scale values lifted losslessly, integers, decimal literals and
  decimal values lifted losslessly) MUST produce a value in the exact domain whenever the
  operation is meaningful; it MUST NOT produce a rounded general decimal.
- **FR-002**: Leaving the exact domain MUST happen only through an explicit, rounding-mode-bearing
  operation in the behavior (`rescale` into a fixed-scale type, as defined in feature 003), or
  through a store that admission proves lossless (FR-012a for general decimals; grid membership
  for fixed-scale targets). Nothing else may leave it.
- **FR-003**: Operations that are not meaningful (adding a ratio to an amount; dividing amounts of
  different nominal types; mixing different nominal types) MUST be admission errors.

**Exact ratio**

- **FR-004**: Dividing two values of the same numeric type — plain decimals, a nominal type with or
  without scale, or exact quantities of it, in any combination — MUST yield an exact,
  dimensionless ratio.
- **FR-005**: An exact ratio MUST support exact addition, subtraction, multiplication, and
  division with other ratios and with integers and decimals (lifted exactly), multiplication with
  values and exact quantities of any decimal nominal type (yielding an exact quantity of that
  type), and exact comparison with ratios and numbers.
- **FR-006**: An exact ratio MUST NOT be passed as an input or implicitly converted in a way that
  could lose information; it becomes a stored value only through `rescale` into a fixed-scale
  type, or through a store that admission proves lossless (FR-012a; e.g. the constant `10 / 2`
  into a Decimal field).

**Bounded exact domain (soundness)**

- **FR-007**: Admission MUST determine, for every expression evaluated in the exact domain, an
  upper bound on the size of all its intermediate and final exact values, derived only from what
  the runtime guarantees about inputs and from literals, and composed through derived values.
- **FR-008**: Admission MUST reject a module in which any such bound exceeds the runtime's
  supported exact representation, with an error at the expression stating the required and the
  supported size.
- **FR-009**: For every admitted module, the runtime MUST NOT be able to raise an exact-size
  overflow on any reachable path; the verifier MUST reason about exact values only for admitted
  modules, and no verification result may depend on values the runtime cannot represent.

**Shared semantics**

- **FR-010**: Evaluation, replay, trace, and SMT verification MUST use identical numeric semantics
  for all exact values; exact values in traces and records MUST be written as finite decimals
  when one exists and as reduced fractions otherwise.
- **FR-011**: Comparisons between compatible fixed-scale values, exact quantities, ratios, and
  numbers MUST be exact and MUST NOT require a rescale.
- **FR-012**: General decimal arithmetic MUST participate in the exact domain: `Decimal ± Decimal`
  and `Decimal × Decimal` yield exact decimal values, `Decimal ÷ Decimal` yields an exact ratio,
  and no operation rounds to 28 digits. Decimal (and nominal-without-scale) values remain exact
  finite decimals as inputs and in state; their input format is unchanged.
- **FR-012a**: A value MUST be stored in a general decimal (a field of type `Decimal` or of a
  nominal type without scale) implicitly only when admission proves, from types and literals
  alone, that the value is always exactly representable in Decimal's bounded representation (a
  finite decimal of at most 28 significant digits). Otherwise — a non-terminating ratio, or a
  possible digit overflow — admission MUST fail and name `rescale`; the author stores it through
  an explicit rescale to a fixed-scale target. The rule MUST NOT depend on whether the value came
  from a literal, a copy, or a computation: an exact value such as `10 / 2` that is provably
  representable is stored like the literal `5`.
- **FR-012b**: The same admission-time analysis MUST establish exact-size bounds (FR-007),
  finite-decimal representability and digit bounds for general decimals (FR-012a), and grid
  membership for fixed-scale targets, so that the runtime never discovers a representation
  problem after admission.
- **FR-012c**: Admission MUST establish representational validity only — well-typedness, lossless
  conversions, absence of implicit precision loss, and exact values within the runtime's
  representation — using types, literals, and type-level static bounds. It MUST NOT use entity
  constraints, invariants, or preconditions. Value-dependent safety (magnitude overflow, division
  by zero, invariants, postconditions) remains the verifier's job over the valid reachable domain.

**Compatibility and interfaces**

- **FR-013**: The semantic break MUST be versioned and visible: modules whose meaning changes get
  new behavior versions (new hash encodings for the changed operations), and their golden records
  and verification expectations are regenerated with every change reviewed and recorded.
  Semantically unchanged nodes and modules (no decimal arithmetic) MUST keep their canonical
  semantic hashes and behavior versions, even though their wire and record envelopes change
  version; identical wire or record bytes are not a goal.
- **FR-014**: Exact ratios MUST be available through the Python DSL (as a declarable derived type),
  the wire IR (with its schema), the CLI, and the Python binding, alongside exact quantities.
- **FR-015**: `docs/verification.md` and `PRINCIPLES.md` MUST describe the closed exact domain and
  its bound; the "open risks" of feature 003 concerning exact size and Money ÷ Money MUST be
  marked resolved.
- **FR-016**: The verifier MUST model decimal arithmetic exactly as the runtime now does; the
  rounding-interval model of feature 002 remains only where the runtime still rounds (an explicit
  rescale), so no check is inconclusive merely because of decimal arithmetic.

### Key Entities

- **Exact domain**: the set of values that are exact rationals during evaluation — exact decimal
  values, exact quantities of a fixed-scale type (`Exact<T>`), and exact ratios — plus the exact
  lifts of fixed-scale values, integers, and decimals that enter it.
- **Decimal**: an exact finite decimal value `c · 10^−s` with an integer coefficient `|c| < 10^28`
  (at most 28 significant digits) and `0 ≤ s ≤ 28` — the input and stored format — no longer an
  arithmetic that rounds.
- **Exact ratio**: an exact, dimensionless rational produced by dividing values of the same
  numeric type (plain decimals, or a nominal type with or without scale); never an input; becomes
  a stored value only through `rescale`, or through a store admission proves lossless.
- **Exact size bound**: for each exact expression, the maximum size of its intermediate and final
  values given the runtime's input guarantees; compared at admission with the runtime's supported
  exact representation.
- **Lossy boundary**: an explicit behavior operation (today: `rescale`) that leaves the exact
  domain with a named rounding mode.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of admitted modules are free of runtime exact-size overflows: randomized
  evaluation of admitted fixture modules with inputs drawn from the full accepted ranges (at least
  10,000 cases) produces no such overflow.
- **SC-002**: 100% of fixture expressions whose bound exceeds the supported representation are
  rejected at admission with the expression and both sizes named; every existing 003 fixture
  expression is admitted.
- **SC-003**: In every module, no evaluation produces a value that was rounded without an explicit
  rescale step in the trace — including ratios and general decimal arithmetic.
- **SC-004**: Every fixture of features 001–003 is accounted for: those without decimal arithmetic
  keep their behavior versions (semantic hashes), decision outcomes, and verification outcomes
  (their wire and record files change only in version metadata); every semantically changed one is
  listed in the implementation review with the old and new value and the reason.
- **SC-007**: No check in the verification fixtures is inconclusive because of decimal
  arithmetic; the feature 002 rounding-interval fixtures are decided under exact semantics.
- **SC-005**: Verification of a ratio property (User Story 3, scenario 4) is proven for the
  correct model and refuted with a confirmed counterexample for the incorrect one.
- **SC-006**: Admission time of the example modules grows by no more than 10% (or 50 ms, whichever
  is larger) due to bound analysis.

## Assumptions

- The runtime's supported exact representation stays as in feature 003 (512 bits for numerator
  and denominator); arbitrary-precision runtime arithmetic is out of scope.
- Bounds are conservative: an expression may be rejected even if its actual values would always
  fit; authors can split it with an explicit rescale. Ordinary money arithmetic (a handful of
  operations) is far below the bound.
- Exact ratios exist for every same-type division: plain decimals, nominals without scale, and
  fixed-scale nominals.
- `rescale` accepts an exact ratio like a decimal: the author decides which fixed-scale type (for
  example a four-decimal percentage type) the ratio becomes.
- Out of scope: currency semantics and exchange rates, persistence, arbitrary-precision runtime
  arithmetic, governance and attestation changes.
- Features 001–003 are not an external compatibility promise: the project is in its early design
  phase, so the semantic break is taken now rather than keeping general decimals as a permanent
  exception. It changes behavior versions of affected modules, and the implementation review
  records every affected fixture.
