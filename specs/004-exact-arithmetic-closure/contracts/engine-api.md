# Contract: Engine API Changes (feature 004)

Extends the 001–003 engine-api contracts. Semantics: [numeric-semantics.md](numeric-semantics.md).

## Wire IR 0.4

- `ir_version` must be `"0.4"`. Documents with `"0.1"`, `"0.2"`, or `"0.3"` are rejected with
  `UNSUPPORTED_IR_VERSION` and a message: *"wire IR 0.1–0.3 used rounded decimal arithmetic; 0.4 is
  exact. Re-serialize from the DSL and declare a scale where computed values are stored."*
- Everything of 0.3 is allowed (constraints list required, `scale`, `rescale`, declared derived
  types).
- Types: `{"t": "exact"}` is the dimensionless exact type; `{"t": "exact", "name": "<nominal>"}`
  accepts any decimal nominal (with or without scale). Both only as declared derived types.
- Schema: `schema/wire-ir-0.4.schema.json`. The serializer always writes `"0.4"`.

## Admission errors

`EXACT_BOUND_EXCEEDED` (new), `LOSSY_CONVERSION` (extended to unprovable stores into general
decimals), `UNSUPPORTED_IR_VERSION` (0.1–0.3), `TYPE_MISMATCH` (mixed units, `SEK ÷ JPY`,
`ratio ± amount`). `EXACT_NOT_FIXED_SCALE` is retired for declared types (any decimal nominal may
be exact) and kept for `rescale` targets without a scale.

## Hashing

Unchanged tags. Type code `0x24` for dimensionless `Exact`; `0x23` for `Exact<T>` of any decimal
nominal. Expressions whose type changes get new hashes through the encoded type. Frozen vectors are
regenerated with review; vectors without decimal arithmetic keep their values.

## Records

`record_version "0.4"` for all records. Wire and record envelopes change version; semantic
identity (behavior versions, item hashes) of unchanged modules does not. Exact values in reads and
derived values use the 003 text form (`"0.67"`, `"1/3"`).

## Rust

```rust
// semantic::types
pub enum Unit { Dimensionless, Nominal(Arc<NominalInfo>) }
pub enum Type { /* … */ Exact(Unit) }          // replaces Exact(Arc<NominalInfo>)
// admit
pub(crate) mod bounds;                          // Facts { nb, db, scale, cd }, analysis
```

`admit`, `evaluate`, `replay`, `verify`, `authorize`: signatures unchanged.

## CLI

No new commands. `admit`/`eval`/`replay`/`verify` require 0.4 documents.

## Python

`Exact[Decimal]` (dimensionless) and `Exact[T]` for any decimal nominal as derived return
annotations. `model.to_wire_json()` writes 0.4. No other API change.

## Verifier

Version `0.4.0`. Decimal arithmetic encoded exactly; rounding only at `rescale`; overflow
obligations only for integers and fixed-scale ranges.
