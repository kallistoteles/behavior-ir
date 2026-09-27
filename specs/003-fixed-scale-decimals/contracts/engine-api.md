# Contract: Engine API Additions (feature 003)

Extends `specs/001-verifiable-behavior-ir/contracts/engine-api.md` and
`specs/002-smt-verification/contracts/engine-api.md`. Semantics:
[numeric-semantics.md](numeric-semantics.md).

## Wire IR 0.3

Only `ir_version "0.3"` documents may use the following; 0.1/0.2 documents are unchanged. The
serializer writes the lowest version that suffices.

```json
"nominals": [{"name": "Money", "underlying": {"t": "decimal"}, "ops": ["add", "order", "scale"],
              "scale": 2, "loc": {...}}]

{"t": "exact", "name": "Money"}                        // type; derived declarations only

{"op": "rescale", "nominal": "Money", "rounding": "half_even", "args": [<expr>], "loc": {...}}

"derived": [{"name": "fee", ..., "type": {"t": "nominal", "name": "Money"}}]   // optional
```

Admission errors (new): `SCALE_NOT_DECIMAL`, `SCALE_OUT_OF_RANGE` (not 0–28), `OFF_GRID_LITERAL`,
`LOSSY_CONVERSION` (e.g. `Money(decimal_expr)`, `Exact<T>` where `T` is required; the message
names `rescale`), `UNKNOWN_ROUNDING`, `EXACT_NOT_FIXED_SCALE` (`exact` of a non-fixed-scale
nominal), `DECLARED_TYPE_MISMATCH`, `EXACT_FIELD` (entity field of type `exact`).
Schema: `schema/wire-ir-0.3.schema.json`.

## Hashing

- Nominal without `scale`: unchanged (`behavior.nominal.v1`).
- Nominal with `scale`: `behavior.nominal.fixed.v1` over
  `str name ‖ type underlying ‖ u8 ops ‖ u8 scale`.
- Type `Exact<T>`: new type code, followed by `T`'s declaration hash.
- `rescale`: new expression op code over `arg hash ‖ T decl hash ‖ u8 rounding`
  (`half_even 0, half_up 1, down 2, up 3, floor 4, ceiling 5`).
- Derived declared `type`: not hashed.
- Every feature 001/002 hash vector is unchanged.

## Records

Record version `"0.3"` for modules at wire 0.3; otherwise `"0.2"` unchanged. In 0.3 records:
fixed-scale values use the fixed text form everywhere (state, input, context, reads, derived,
changes); trace steps may carry `"rescales": [{"rescale", "exact", "rounding", "scale",
"result"}]`; input problems may use `OFF_GRID` / `OUT_OF_RANGE`. Replay compares bytes as today.

## Rust

```rust
// behavior-core
pub mod exact;                      // Exact (bounded rational), Rounding, round(), text form
pub enum Rounding { HalfEven, HalfUp, Down, Up, Floor, Ceiling }
// semantic::types
pub struct NominalInfo { /* … */ pub scale: Option<u8> }
pub enum Type { /* … */ Exact(Arc<NominalInfo>) }
// semantic::expr
pub enum ExprKind { /* … */ Rescale { arg: Box<Expr>, target: Arc<NominalInfo>, rounding: Rounding } }
// builder
impl Builder {
    pub fn declare_nominal(&mut self, name, underlying, ops, scale: Option<u8>, loc) -> …;
    pub fn rescale(&mut self, arg: Node, nominal: &str, rounding: Rounding, loc) -> Result<Node, BuildError>;
    pub fn add_derived(&mut self, …, declared: Option<Type>) -> …;
}
```

`evaluate`, `replay`, `admit`, `verify`, `authorize` keep their signatures.

## CLI

No new commands. `admit`, `eval`, `replay`, `verify` accept 0.3 documents.

## Python

```python
from behavior import Exact, Rounding, nominal, rescale

Money = nominal("Money", Decimal, scale=2, ops={"order", "add", "scale", "ratio"})

@derived
def fee(invoice: Invoice) -> Money:                  # declared type (optional)
    return rescale(invoice.amount * Decimal("0.25"), Money, Rounding.HALF_EVEN)

@derived
def theoretical_fee(invoice: Invoice) -> Exact[Money]:
    return invoice.amount * Decimal("0.25")
```

`Rounding` has exactly `HALF_EVEN`, `HALF_UP`, `DOWN`, `UP`, `FLOOR`, `CEILING`; `rescale` has no
default. `nominal(..., scale=)` requires `Decimal`. Values cross the boundary as `Decimal` on the
way in; decisions expose the record's canonical text, so fixed-scale values read `"100.50"`.

## Verifier

Verifier version `0.3.0` (check keys change, outcomes for 001/002 fixtures do not). Encoding per
research R11: fixed-scale values as integers on their grid, `Exact<T>` as exact reals, rescale as
linear rounding constraints; general decimals keep the feature-002 model. Counterexample values
of fixed-scale fields have exactly `scale` digits.
