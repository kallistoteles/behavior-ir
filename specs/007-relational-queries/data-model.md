# Data Model: Relational Queries and Set Semantics

Additions to features 001–006. Unchanged documents keep their tags and bytes; new fields are
optional and omitted when empty (research R8).

## Behavior IR (wire 0.6)

| Form | Wire | Type |
|---|---|---|
| Select | `{"op": "select", "entity": T, "loc"}` | `Query<T>` |
| Where | `{"op": "where", "args": [q], "param": "o", "body": <Bool expr>, "loc"}` | `Query<T>` |
| Set algebra | `{"op": "union" \| "intersection" \| "difference", "args": [q1, q2], "loc"}` | `Query<T>` (same `T`) |
| Count | `{"op": "count", "args": [q], "loc"}` | `Int` |
| Quantifiers | `{"op": "any" \| "all", "args": [q], "param", "body": <Bool expr>, "loc"}` | `Bool` |
| Sum | `{"op": "sum", "args": [q], "param", "body": <additive numeric expr>, "loc"}` | type of repeated exact addition; `sum(∅)` is its exact zero |
| Min, max | `{"op": "min" \| "max", "args": [q], "param", "body": <ordered expr>, "loc"}` | `Option<E>` |
| Unique | `{"op": "unique", "args": [q], "param", "body": <key expr>, "loc"}` | `Bool` |
| Module invariant | `invariants[]` entry without `entity` and `param`: `{"name", "body", "loc"}`; a closed state expression (no inputs, context, parameters or invocation captures) | — |

- **Lambda bodies** (`body`) see the candidate as `param` plus the enclosing scope. They are
  candidate-local (research R2).
- **`Query<T>`** is accepted only by the forms above; it is never stored, compared, bound, returned
  by a derived value or written as a literal.
- **Admission errors** (new): `QUERY_NOT_ALLOWED` (a relational form in an entity constraint or a
  per-entity invariant, or in a lambda body), `NON_LOCAL_PREDICATE` (in a lambda body only: `exists`,
  `referenced`, a derived value over anything but the candidate, or a non-field use of an
  enclosing entity; a module invariant referring to a parameter is `UNKNOWN_PARAM`, since it has
  none), `TYPE_MISMATCH` (different entity types in set algebra; `sum` without `add`;
  `min`/`max` without an order; a query used as a value), `UNSUPPORTED_IR_VERSION` (007 forms in
  0.4/0.5).

## Query identity

```text
definition hash = expression hash of the query, with the candidate encoded as a fixed marker
captures        = canonical values of the capture reads, sorted by (param, field)
instance id     = sha256("behavior.query_instance.v1" ‖ definition hash ‖ canonical captures)
```

## Evaluation facts (additions)

```json
"facts": {
  "queries": [{"instance": "sha256:…", "definition": "sha256:…", "entity": "Order",
               "captures": [{"read": "customer.id", "value": "c1"}],
               "members": [{"id": "o2"}, {"id": "o7"}]}],
  "fields":  [{"entity": "Order", "id": "o2", "field": "amount", "value": "10.00"}]
}
```

- `queries` records membership only (FR-009); `fields` records the member values actually read
  (FR-009a). Both are sorted canonically (instances by id, members by id; fields by entity, id,
  field).
- All query facts describe the evaluated state S, including the baselines used to derive S'
  results: `Q(captures_S', S') = Q(captures_S', S) + exact effects of ΔS`, where the baseline is an
  observed fact against S even though its captures were evaluated in S' (research R4).
- **Size**: a query fact lists its complete membership (even for `count`); member field reads come
  on top.
- **Record version**: a record with `queries` or `fields` is `record_version "0.6"`.
- **Snapshot check** for supplied facts: research R5 (`INCONSISTENT_FACTS`).
- **Trace**: a step that evaluated a relational form shows the instance id in its `reads`.

## Commit bundle and transition record

- `read_facts` gains the same `queries` and `fields` sections, plus `result_hash` per query instance
  (sha256 of the canonical member list), for future fine-grained conflict detection (FR-014,
  FR-015).
- Transition records are unchanged (queries never change state).

## Backend primitives (additions)

| Primitive | Contract |
|---|---|
| `keys_at(entity_type, position) → [EntityKey]` | Every entity of the type that exists at the position, in any order (order is never semantic) |
| `keys_by_field_at(entity_type, field, value, position) → Option<[EntityKey]>` | Entities of the type whose field equals the value at the position, or `None` if not indexed (default) |

Both are derived from entity versions and removals, never part of the state identity, and checked
against a scan by conformance (`query_index_consistency`).

## Module-level invariants

- Checked over the seed at genesis, and on S' of every transition unless a sound dependency
  analysis proves it unaffected (research R7).
- **Dependency signature**: the queried entity types and, per type, the fields read by predicates,
  keys and aggregate bodies.
- **Failure**: `DENY` with `INVARIANT_VIOLATED` naming the invariant; trace phase
  `invariant_global`.

## Verification model

Per query instance: `count_Q`, `sum_Q` per sum body, `sat_Q,P` per quantifier predicate, `m_Q` per
`min`/`max` body, over S, constrained by the classified known candidates (bound entities). S' for a
fixed instance is derived with exact deltas; a changed instance gets fresh S summaries; `min`/`max`
lose precision after removals or changes. Counterexamples are concretized with explicit unknown-member
slots and confirmed by the runtime (research R10).
