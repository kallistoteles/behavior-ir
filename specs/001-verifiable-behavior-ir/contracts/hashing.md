# Contract: Semantic Content Hashing v1

How the engine computes the identity of every semantic node. Identity depends only on semantic
content: not on JSON, not on source locations, not on names for humans (research R4, R5).
Only the Rust engine implements this; other components obtain hashes from it.

## Hash function

```text
hash(node) = SHA-256( tag ‖ 0x00 ‖ body(node) )      -- 32 bytes
display    = "sha256:" + lowercase hex
```

| Tag | Node |
|-----|------|
| `behavior.enum.v1` | enum declaration |
| `behavior.nominal.v1` | nominal type declaration |
| `behavior.entity.v1` | entity declaration |
| `behavior.expr.v1` | any expression node (predicates/conditions are Bool expressions and use this tag) |
| `behavior.effect.v1` | effect |
| `behavior.derived.v1` | derived value or rule |
| `behavior.invariant.v1` | invariant |
| `behavior.action.v1` | action |
| `behavior.module.v1` | module (behavior version) |

A change to any encoding below requires new tags (`…v2`); old hashes stay valid for old data.

## Primitive encodings

| Item | Bytes |
|------|-------|
| `u8` | 1 byte |
| `u32` | 4 bytes big-endian |
| `i64` | 8 bytes big-endian two's complement |
| `str` | `u32` byte length ‖ UTF-8 bytes |
| `list<X>` | `u32` count ‖ items |
| `ref` | 32-byte child hash |
| `decimal` | `str` of the normalized plain form (`"50000"`, `"0.05"`, `"0"`) |

## Types (encoded inline)

| Type | Bytes |
|------|-------|
| Bool / Int / Decimal / String | `0x01` / `0x02` / `0x03` / `0x04` |
| Option(T) | `0x10` ‖ type(T) |
| Enum | `0x20` ‖ `ref` (enum declaration hash) |
| Nominal | `0x22` ‖ `ref` (nominal declaration hash) |
| Id(E) | `0x21` ‖ `str` entity name |
| Entity (parameters only) | `0x30` ‖ `str` entity name |

Enums and nominal types are referenced by declaration hash because their meaning is their
declaration (values, underlying type, ops; a nominal's name is part of its declaration).
Entities are referenced by name because `Id(E)` means "identity of an E", independent of E's
fields.

## Declarations

```text
enum     = str name ‖ list<str> values
nominal  = str name ‖ type underlying ‖ u8 ops_bitmask   (order=1, add=2, scale=4, ratio=8)
entity   = str name ‖ list<field>          field = str name ‖ type   (implicit id first)
param    = str name ‖ u8 role ‖ type       (role: read=0, state=1, input=2, context=3)
```

## Expressions

```text
expr = u8 opcode ‖ type(result) ‖ operands
```

| Opcode | Node | Operands |
|--------|------|----------|
| 0x01 | Lit | value (below) |
| 0x02 | Field | `str` param ‖ `str` field |
| 0x03 | Param | `str` param |
| 0x04 | DerivedRef | `ref` derived ‖ `list<str>` args |
| 0x10–0x15 | eq ne lt le gt ge | `ref` a ‖ `ref` b |
| 0x20–0x23 | add sub mul div | `ref` a ‖ `ref` b |
| 0x30 / 0x31 | And / Or | `list<ref>` |
| 0x32 | Not | `ref` |
| 0x33 | In | `ref` ‖ `list<value>` |
| 0x40 / 0x41 / 0x42 | IsNone / IsSome / ValueOr | `ref` / `ref` / `ref` ‖ `ref` |
| 0x50 / 0x51 / 0x52 / 0x53 | Some / ToDecimal / Wrap / Unwrap | `ref` (Wrap's target is its result type) |

Literal values: Bool `u8` (0/1); Int `i64`; Decimal `decimal`; String, Enum, Id `str`;
Nominal → value of its underlying type; Option → `u8 0` for none, or `u8 1` ‖ inner value.

## Behavior items

```text
effect    = str param ‖ str field ‖ ref value
derived   = u8 kind (derived=0, rule=1) ‖ list<param> ‖ ref body
invariant = str entity ‖ str param ‖ ref body
action    = list<param> ‖ list<ref> preconditions ‖ list<ref> effects ‖ list<ref> postconditions
module    = list<entry>      entry = u8 kind ‖ str name ‖ ref item
            kinds: enum=1, nominal=2, entity=3, derived=4, invariant=5, action=6
            entries sorted by (kind, name bytes)
```

Names of derived values, invariants, and actions appear only in module entries.

## Conformance

- `tests/fixtures/hash_vectors.json` lists wire IR snippets with their expected hashes; it is
  generated once during implementation, reviewed, and frozen. Any change to it requires new
  tags.
- Property tests: hash is unchanged by (a) source locations, (b) renaming a derived value,
  invariant, or action (item hash), (c) reordering entities/enums/nominals/derived/invariants/
  actions in the wire document, (d) writing an implicit conversion explicitly. Hash changes when
  preconditions are reordered, a literal changes, or a field is renamed.
