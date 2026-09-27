# Data Model: Entity Lifecycle

Additions to features 001–005. Unchanged documents keep their tags and bytes; new fields are
optional and omitted when empty (research R7, R9, R13).

## Behavior IR (wire 0.5)

| Form | Wire | Semantics |
|---|---|---|
| Reference field type | `{"t": "ref", "entity": T}` | `Id<T>` marked as reference, plus a synthesized constraint `exists(field)` |
| Creation effect | `{"create": T, "id": <expr Id<T>>, "fields": {f: <expr>}, "loc"}` | `∅ → T#id` with a complete value |
| Removal effect | `{"remove": <state param>, "loc"}` | `T#id → ∅` |
| Existence | `{"op": "exists", "args": [<Id<T> or Option<Id<T>>>]}` | Bool, on S (preconditions, incoming rules) or S' (postconditions, outgoing rules) |
| Referenced | `{"op": "referenced", "args": [<Id<T>>]}` | Bool: some `Ref` to it survives in S (S' form: `incoming'` non-empty) |

Admission errors: `CREATE_INCOMPLETE`, `UNKNOWN_FIELD`, `TYPE_MISMATCH` (identity type),
`LIFECYCLE_CONFLICT`, and `UNSUPPORTED_IR_VERSION` (lifecycle forms in 0.4).

## Evaluation request and decision record

```json
"facts": {                                   // request (plain evaluate) and record (observed only)
  "existence":  [{"entity": "Customer", "id": "c1", "exists": true}],
  "identities": [{"entity": "Account",  "id": "a9", "used": false}],
  "references": [{"entity": "Customer", "id": "c1",
                  "incoming": [{"entity": "Account", "id": "a1", "field": "owner"}]}]
},
"lifecycle": [                               // record only, in effect order
  {"op": "create", "entity": "Account", "id": "a9", "value": {…complete…}},
  {"op": "remove", "entity": "Account", "id": "a1", "value": {…last value…}}
]
```

- **Record version:** a record with `facts` or `lifecycle` has `record_version "0.5"`; every other
  record stays `"0.4"`.
- **Supplied facts must form a valid snapshot** (FR-010g). Otherwise the evaluation error is
  `INCONSISTENT_FACTS`.
- **New decision results and reasons:**
  - `ENTITY_ID_ALREADY_USED` (result);
  - `DANGLING_REFERENCE` (reason, with the surviving references; result `DENY`);
  - `LIFECYCLE_CONFLICT` (result);
  - `UNKNOWN_FACT` (evaluation error: a needed fact was not supplied).

## Evaluation facts (observed reads)

Current-state facts (existence, incoming references) and history facts (ever used) together
with the state form the **evaluation snapshot**. `referenced(id)` is the projection
`incoming(id) ≠ ∅`.

```text
FieldRead(entity, id, field, value)          -- feature 005
ExistenceRead(entity, id, exists)
IdentityRead(entity, id, used)
ReferenceRead(entity, id, incoming: [(entity, id, field)])
```

- **At the evaluated state only:** facts are read at S. S' facts are derived from them plus ΔS:
  - `exists'(x) = (exists(x) ∨ created(x)) ∧ ¬removed(x)`;
  - `incoming'(x)` = `incoming(x)` − removed sources − retargeted fields + created or retargeted
    references to x.
- **Only observed facts are recorded,** in canonical order (entity, id, then field).

## Persistence (feature 005 documents, extended)

Commit bundle (optional fields):
- `read_set.existence`, `read_set.identities` and `read_set.references`, equal to the record's facts.
- `write_set.lifecycle: [{op, entity, id}]`.

Transition record (optional fields):

```json
"created": [{EntityVersion, revision 1, created_at = position}],
"removed": [{"entity", "id", "last_revision", "last_content_hash"}],
"ref_changes": [{"target": {entity, id}, "source": {entity, id}, "field", "op": "add" | "drop"}]
```

Rules:
- **Registry:** an identity is *used* at p iff some version of it has `created_at ≤ p`. It is keyed
  by the entity type's nominal name and the id, never the declaration hash. Removal
  never deletes versions.
- **Existence:** at p, `exists_at(key, p)` iff it is used at p and not removed at or before p.
- **State identity:** a creation inserts the new content hash; a removal removes the last content
  hash.
- **Genesis:** seed `Ref` fields must point at seed entities, and the initial index is derived from
  the seed.

Store errors (additions): `ENTITY_ID_ALREADY_USED`, `DANGLING_REFERENCE`, `ENTITY_NOT_FOUND` (for
removal of an absent entity), and `BUNDLE_INVALID` (facts that differ from those observed at the
parent).

## Backend primitives (additions)

| Primitive | Contract |
|---|---|
| `removed_at(key) → Option<position>` | The position of the key's removal. Set once, inside a commit, and never changed |
| `incoming_at(target, position) → [RefEdge]` | The surviving references to `target` as of `position` (derived index) |
| `used_at(key, position) → bool` | Default: `version_at(key, position).is_some()` |
| `commit(…, removals, ref_changes, …)` | Removals and index edge changes join the same atomic compare-and-set |

The derived index is stored as edges `{target, source, field, added_at, dropped_at?}`. `dropped_at`
is set at most once, atomically with the commit that drops the edge. It is not state content: it is
reconstructible from entity versions, and conformance checks that the two never diverge.

## Verification model

Per entity type T, three uninterpreted functions over identities at S:
- `ex_T` (exists);
- `used_T`, with the axiom `ex_T(x) → used_T(x)`;
- `refd_T`: "some unbound entity holds a `Ref` to x".

Creation assumes `¬used_T(id)`. Removal requires `¬refd_T(id)`, and no surviving bound or created
entity may reference the removed identity. New check kind: `referential_integrity` (blocking).
Counterexamples carry concrete facts in the request's `facts` section.
