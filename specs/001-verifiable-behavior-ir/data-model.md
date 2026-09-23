# Data Model: Behavior IR v0.1

The logical model of the **semantic Behavior IR** inside the Rust engine, and of the engine's
results. The untrusted wire form is in [contracts/ir-encoding.md](contracts/ir-encoding.md);
hashing is in [contracts/hashing.md](contracts/hashing.md).

## Types

```text
Type = Bool | Int | Decimal | String
     | Option(Type)              -- inner type is not an Option
     | Enum(name)
     | Id(entity)
     | Nominal(name)             -- declared: underlying primitive + ops
```

Entities are not value types for fields (no nested entities); they appear as parameter types.

### Nominal type declaration

| Field | Rules |
|-------|-------|
| `name` | unique among nominal types, enums, and entities; part of the type's identity |
| `underlying` | `Bool`, `Int`, `Decimal`, or `String` |
| `ops` | subset of `order`, `add`, `scale`, `ratio`; `order`/`add`/`scale`/`ratio` require a numeric underlying type |

### Typing rules

Let `N` be a nominal type over numeric `P`. "Num" means `Int` or `Decimal`.

| Operation | Allowed operands → result |
|-----------|---------------------------|
| `==`, `!=` | same type both sides → Bool (after conversions below) |
| `<` `<=` `>` `>=` | Num × Num → Bool; `N × N` → Bool if `N` has `order` |
| `+`, `-` | Int × Int → Int; Num × Num → Decimal; `N × N` → `N` if `add` |
| `*` | Int × Int → Int; Num × Num → Decimal; `N × Int` or `Int × N` → `N` if `scale`; `N × Decimal`/`Decimal × N` → `N` if `scale` and `P = Decimal` |
| `/` | Num × Num → Decimal (zero divisor → error); `N / Int` or `N / Decimal` → `N` if `scale` and `P = Decimal`; `N / N` → Decimal if `ratio` |
| `and`, `or` (2+ args), `not` | Bool → Bool |
| `in` | `T` × non-empty literal list of `T` → Bool |
| `is_none`, `is_some` | `Option(T)` → Bool |
| `value_or(x, d)` | `Option(T)` × `T` → `T` |
| `wrap(N, e)` | `P` → `N` (explicit conversion) |
| `unwrap(e)` | `N` → `P` (explicit conversion) |

Implicit conversions accepted in wire IR and made explicit on admission:

- `Int` → `Decimal` where an operand of `Decimal` is required (`to_decimal` node).
- `T` → `Option(T)` in effects and in `==`/`!=` against `Option(T)` (`some` node).
- `none` literal takes the `Option(T)` type required by its position.

Everything else is a type error, in particular: a nominal type mixed with its underlying type
or another nominal type; `Id(A)` vs `Id(B)` or `String`; `Option(T)` where `T` is required.

## Module (semantic)

| Field | Content |
|-------|---------|
| `enums`, `nominals`, `entities` | information model items |
| `derived`, `invariants`, `actions` | behavior items |
| `name_table` | `(kind, name) → item hash`, sorted by `(kind, name)` |
| `evaluation_order` | derived names in topological order, ties by name |
| `hash` | module hash = behavior version |

## Enum

`name`, `values` (non-empty, unique strings). Hashed with name and values in order.

## Entity

`name`, `fields` (non-empty after the implicit `id: Id(name)` field, which always comes first;
names unique; `id` is reserved). Field types: any value type except entities.

## Parameter

| Field | Rules |
|-------|-------|
| `name` | unique within its declaration; part of the hash |
| `role` | `state`, `input`, or `context` (actions); derived values and invariants have state-like read parameters with role `read` |
| `type` | an entity (required for `state`), or any value type (`input`, `context`) |

## Expr (semantic; every node has `type` and `hash`, plus `loc` outside the hash)

| Node | Content |
|------|---------|
| `Lit(value)` | type-tagged literal; `none` for options |
| `Field(param, field)` | read of a parameter's field (or the whole value for non-entity input/context parameters: `Param(param)`) |
| `DerivedRef(item_hash, args)` | args are parameter names, matched by position |
| `Cmp(op, a, b)` | `eq ne lt le gt ge` |
| `Arith(op, a, b)` | `add sub mul div` |
| `And(args)`, `Or(args)`, `Not(a)` | |
| `In(a, values)` | |
| `IsNone(a)`, `IsSome(a)`, `ValueOr(a, d)` | |
| `Some(a)`, `ToDecimal(a)`, `Wrap(nominal, a)`, `Unwrap(a)` | explicit conversions |

## Derived (includes rules)

`kind` (`derived` or `rule`; a rule's body is Bool), `params` (read parameters, entity-typed,
≥ 1), `body`, `result_type`, `hash` (name excluded; bound in `name_table`).

## Invariant

`entity`, `param`, `body` (Bool), `hash`. Applies to every **state** parameter of that entity
type, on `S` and on `S'`.

## Action (transition)

| Field | Rules |
|-------|-------|
| `params` | ≥ 1 state parameter; any number of input/context parameters |
| `preconditions` | ordered Bool predicates over S, I, C |
| `effects` | ordered `Set(state_param.field, expr)`; target fields unique; target cannot be `id` |
| `postconditions` | ordered Bool predicates over S', I, C |
| `hash` | name excluded; bound in `name_table` (the name is the capability name) |

## Admission result (ValidationResult)

`ok`, `behavior_version` (only when `ok`), `errors` (list of `{code, message, loc,
related_locs}` sorted by file, line, code), `evaluation_order`, `items` (name → hash, for
display).

Error codes: `DECODE_ERROR`, `UNSUPPORTED_IR_VERSION`, `UNKNOWN_ENTITY`, `UNKNOWN_TYPE`,
`UNKNOWN_FIELD`, `UNKNOWN_DERIVED`, `UNKNOWN_PARAM`, `DUPLICATE_NAME`, `RESERVED_NAME`,
`TYPE_MISMATCH`, `ARITY_MISMATCH`, `NOT_BOOLEAN`, `OP_NOT_ALLOWED` (operation not declared by a
nominal type), `CYCLE`, `DUPLICATE_ASSIGNMENT`, `EFFECT_ON_READONLY`, `EMPTY_IN`,
`INVALID_LITERAL`, `NESTED_OPTION`.

## Evaluation request

`action`, `state` (param → entity object including `id`), `input` (param → value), `context`
(param → value), `data_version`, optional `git_revision`.

Value encoding: Bool → bool; Int → integer; Decimal → normalized string or integer; String →
string; Enum → value string; Id → string; Nominal → encoding of its underlying type; Option →
`null` for none, else the inner encoding; entity → object of fields.

## Decision record

| Field | Content |
|-------|---------|
| `record_version` | `"0.1"` |
| `behavior_version`, `git_revision`, `data_version` | identifiers |
| `action` | `{name, hash}` |
| `state`, `input`, `context` | as evaluated (normalized encoding) |
| `result` | `ALLOW`, `DENY`, `ERROR`, `INVALID_INPUT`, `INVALID_STATE` |
| `reasons` | `{code, message, loc}` list |
| `trace` | ordered `{phase, name?, hash, expr_text, reads, outcome, loc}`; phase ∈ `invariant_pre`, `precondition`, `effect`, `postcondition`, `invariant_post` |
| `derived` | `{name, hash, phase_state: "S" \| "S'", value}` in evaluation order |
| `changes` | ΔS: `{param, field, old, new}` list, empty unless `ALLOW` |

## Result state machine

```text
request ──shape/type check fails──► INVALID_INPUT
   ▼
invariants on S ──false──► INVALID_STATE        (error ► ERROR)
   ▼
preconditions (S, I, C) ──false──► DENY          (error ► ERROR)
   ▼
effects → ΔS ──error──► ERROR
   ▼
S' = apply(S, ΔS)
postconditions (S', I, C), invariants on S' ──false──► DENY   (error ► ERROR)
   ▼
ALLOW (ΔS returned)
```
