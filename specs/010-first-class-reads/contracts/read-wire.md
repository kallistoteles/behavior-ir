# Contract: Read Wire Forms (wire IR 0.7)

## Module `reads` section

A module that has a `reads` section needs `ir_version: "0.7"`. Earlier versions are refused with
`NeedsReadVersion` naming the section. Modules without reads are unchanged, at any accepted
version.

```json
{
  "ir_version": "0.7",
  "...": "enums, nominals, entities, derived, invariants, constraints, actions as in 0.6",
  "reads": [
    {
      "name": "open_total",
      "params": [{"name": "customer", "role": "state", "type": {"entity": "Customer"}}],
      "body": {"value": { "...": "any 0.6 expression" }},
      "loc": {"file": "lab.py", "line": 40}
    },
    {
      "name": "order_view",
      "params": [{"name": "order", "role": "state", "type": {"entity": "Order"}}],
      "body": {"project": {"over": {"op": "param", "param": "order", "loc": {"file": "lab.py", "line": 67}}, "param": "o",
                           "items": [{"field": "status"}, {"field": "amount"},
                                     {"derived": "total"}]}},
      "loc": {"file": "lab.py", "line": 48}
    },
    {
      "name": "active_cultures",
      "params": [],
      "body": {"project": {"over": {"select": "Culture", "...": "where/union/... as in 0.6"},
                           "param": "c",
                           "items": [{"field": "name"}, {"field": "stage"}, {"derived": "age"}]}},
      "loc": {"file": "lab.py", "line": 55}
    }
  ]
}
```

- `params[].role` is one of `state`, `input` or `context`.
- `body` has exactly one key: `value` or `project`.
- `project.over` is a wire expression: a `param` expression naming a `state` entity parameter, or
  a query expression (`select`, or a `where` / set-algebra lambda over one).
- `items[]` has exactly one key: `field` or `derived`, naming a non-empty item. `id` is implicit.
  Admission, not decoding, refuses names that are not items (a reference path such as
  `customer.name` is `UNKNOWN_PROJECTION_ITEM`, with guidance).
- Decoding is strict, as everywhere else: an unknown key, a wrong shape or a wrong arity is a
  `DECODE_ERROR` with its JSON path.

## Read document (ad-hoc read)

```json
{"ir_version": "0.7", "read": { "...": "one WRead, as above" }}
```

A read document is admitted against a module (`admit_read`). It is never added to the module and
never changes the behavior version.

## Admission errors

| Code | When |
|---|---|
| `UNKNOWN_PROJECTION_ITEM` | An item is neither a field of `T` nor a derived value over `Entity<T>` alone; includes reference paths |
| `DUPLICATE_PROJECTION_ITEM` | An item is listed twice, or a field and a derived item share a name |
| `INVALID_PROJECTION` | `over` is neither a query nor a `state` parameter of entity type |
| `READ_CALL_NOT_ALLOWED` | A module expression references a declared read (message: "declared reads are capability entry points; move the shared computation into a derived value and use it from both") |
| `DUPLICATE_CAPABILITY` | A read has the same name as an action or a derived value |
| `DECODE_ERROR` at `params[i].role` | A read parameter has a role other than `state`, `input` or `context` (the existing behavior of parameter decoding) |
| existing codes | `TYPE_MISMATCH`, `UNKNOWN_TYPE`, `UNKNOWN_PARAM`, `NON_LOCAL_PREDICATE`, … apply to read bodies as to derived values |

## Hashing

- `H(behavior.read.v1, name-independent content: params (role, type), body)`. A value body hashes
  its expression. A projection body hashes `H(behavior.projection.v1, over, member,
  [item: field name | derived hash])`.
- A name-table entry `(Kind::Read = 8, name) → hash` exists for declared reads only.
- The module hash is unchanged for a module without reads.
- New hash vectors in `tests/fixtures/hash_vectors.json` cover one value read, one entity
  projection and one query projection.

## JSON Schema

`schema/wire-ir-0.7.schema.json`: the 0.6 schema plus `reads`, and the read document.
