# Contract: Wire IR JSON Encoding v0.1

The untrusted exchange form of behavior, produced by frontends (Python DSL now) and admitted by
the engine into the semantic Behavior IR ([data-model.md](../data-model.md)). Wire IR is a
serialization: it carries names, source locations, and may leave conversions implicit. Its
bytes are **not** the behavior's identity; see [hashing.md](hashing.md).

A JSON Schema (`schema/wire-ir-0.1.schema.json`) is written during implementation and used in
contract tests.

## Canonical emission (for clean Git diffs and golden tests)

1. UTF-8, no BOM, compact, no trailing newline; object keys sorted by byte value.
2. Identifiers ASCII `[A-Za-z_][A-Za-z0-9_]*`.
3. No floating-point numbers. Integers as JSON integers (signed 64-bit). Decimals as normalized
   plain strings.
4. Lists keep declaration order; optional keys are omitted, never `null` (except the `none`
   literal value, below).

The engine accepts non-canonical wire IR; canonical emission only matters for diffs and golden
files. The canonical writer is the engine's serializer (research R17): it writes admitted
modules with explicit conversions and declarations sorted by name.

## Document

```json
{
  "ir_version": "0.1",
  "enums":    [{"name": "InvoiceStatus", "values": ["pending", "approved"], "loc": {...}}],
  "nominals": [{"name": "Money", "underlying": {"t": "decimal"}, "ops": ["add", "order", "ratio", "scale"], "loc": {...}}],
  "entities": [{"name": "Invoice", "loc": {...}, "fields": [
      {"name": "amount", "type": {"t": "nominal", "name": "Money"}, "loc": {...}},
      {"name": "status", "type": {"t": "enum", "name": "InvoiceStatus"}, "loc": {...}},
      {"name": "approved_by", "type": {"t": "option", "of": {"t": "id", "entity": "User"}}, "loc": {...}}
  ]}],
  "derived":    [{"name": "margin", "kind": "derived", "params": [{"name": "project", "type": {"t": "entity", "name": "Project"}}], "body": <expr>, "loc": {...}}],
  "invariants": [{"name": "non_negative_amount", "entity": "Invoice", "param": "invoice", "body": <expr>, "loc": {...}}],
  "actions": [{
      "name": "approve_invoice",
      "params": [
        {"name": "invoice", "role": "state",   "type": {"t": "entity", "name": "Invoice"}},
        {"name": "actor",   "role": "context", "type": {"t": "entity", "name": "User"}}
      ],
      "preconditions":  [{"expr": <expr>, "loc": {...}}],
      "effects":        [{"target": {"param": "invoice", "field": "approved_by"}, "value": <expr>, "loc": {...}}],
      "postconditions": [{"expr": <expr>, "loc": {...}}],
      "loc": {...}
  }]
}
```

`loc` is `{"file": "<path relative to module root, / separators>", "line": <n>}`. The implicit
`id` field is **not** written in `fields`; declaring a field named `id` is `RESERVED_NAME`.

## Types

`{"t": "bool" | "int" | "decimal" | "string"}`, `{"t": "option", "of": <type>}`,
`{"t": "enum", "name": …}`, `{"t": "nominal", "name": …}`, `{"t": "id", "entity": …}`,
`{"t": "entity", "name": …}` (parameters only).

## Expression nodes

Every node has `op` and `loc`. Result types are not written; the engine infers them.

| `op` | Keys |
|------|------|
| `lit` | `type`, `value` (`null` only for the `none` literal of an option) |
| `field` | `param`, `field` |
| `param` | `param` (whole value of a non-entity input/context parameter) |
| `derived` | `name`, `args` (parameter names) |
| `eq` `ne` `lt` `le` `gt` `ge` `add` `sub` `mul` `div` | `args` (2) |
| `and` `or` | `args` (≥ 2) |
| `not` `is_none` `is_some` `some` `to_decimal` `unwrap` | `args` (1) |
| `value_or` | `args` (2) |
| `in` | `args` (1), `values` (non-empty list of literal values) |
| `wrap` | `nominal` (name), `args` (1) |

`some` and `to_decimal` may be written explicitly or left implicit; admission makes them
explicit either way, so both forms get the same hash.

## Compatibility

- `ir_version` other than `"0.1"` → `UNSUPPORTED_IR_VERSION`.
- Unknown keys → `DECODE_ERROR` (strict decoding; a typo cannot silently drop behavior).
- New node kinds or keys require a new `ir_version`.
