# Data model: Declaration identity of named types (feature 505)

## 1. Shared reference shapes (one implementation, `crates/behavior-models/src/identity.rs`)

**Entity reference**, for a subject, an actor, or a state binding with its `name` added:

```json
{ "entity": "PurchaseRequest", "entity_hash": "sha256:…" }
```

**Typed parameter**, for an input, context parameter or payload field:

```json
{ "name": "mode", "type": <WType>, "types": { "<type name>": "sha256:…" } }
```

- `types` maps every named type the `type` mentions to its Core declaration identity. Named
  types are `Enum(n)`, `Nominal(n)`, `Exact(Some(n))` (a nominal), and `Id(n)`, `Ref(n)` and
  `Entity(n)` (an entity), also inside `Option`. It is empty for built-in types only.
- Keys are type names, ordered by code point (a `BTreeMap`). A type mentioned twice appears
  once.
- Lists of typed parameters are sorted by name, as before.

**Resolution** (contract `declaration-references.md`): `(kind, name)` is looked up in the
admitted module's name table. A missing name, or one declared only under another kind, is a
blocking `MODEL_IDENTITY_FAILED` error naming the model, the referring item and the type.

## 2. StateMachine document v2

```json
{
  "format": "behavior.model.state_machine.v2",
  "model_kind": "state_machine",
  "model_semantic_version": 2,
  "model_id": "order",
  "subject": { "entity": "Order", "entity_hash": "sha256:…" },          // was { "entity" }
  "field": { "name": "status", "enum": "OrderStatus", "enum_hash": "sha256:…" },
  "subject_param": null,
  "entry_states": [ … ],
  "transitions": [
    { "id": "submit", "from": [ … ], "to": "…",
      "inputs":  [ <typed parameter>, … ],                                // was { name, type }
      "context": [ <typed parameter>, … ],
      "guards": [ … ], "effects": [ … ], "postconditions": [ … ], "commands": [ … ] } ]
}
```

## 3. RequestFlow document v2

```json
{
  "format": "behavior.model.request_flow.v2",
  "model_kind": "request_flow",
  "model_semantic_version": 2,
  "flow_id": "purchase_request",
  "actor": { "entity": "User", "entity_hash": "sha256:…" },             // was { "entity" }
  "status_enum": { "name": "…", "enum_hash": "sha256:…" },
  "assignee": true,
  "payload": [ <typed parameter>, … ],                                    // was { name, type }
  "creations": [
    { "id": "create", "to": "…",
      "inputs":  [ <typed parameter>, … ],
      "state":   [ { "name": "budget", "entity": "Budget", "entity_hash": "sha256:…" } ],
      "context": [ <typed parameter>, … ],
      "initializer": { … }, "guards": [ … ], "effects": [ … ], "postconditions": [ … ],
      "commands": [ … ] } ],
  "state_machine": { "model_hash": "sha256:…", "document": <StateMachine document v2> }
}
```

The embedded machine's subject is the flow's generated request entity. Its `entity_hash` is a
function of the payload.

## 4. Approval document (unchanged)

`behavior.model.approval.v1` and semantic version 1, byte-identical. Its subject, actor and typed
parameters already have the shapes of §1, which now come from the shared implementation.

## 5. Versions and tuples

| Kind | Semantic version | Lowerer version | Format |
|---|---|---|---|
| state_machine | 1 → **2** | 1 | `…state_machine.v2` |
| request_flow | 1 → **2** (embedded `state_machine` 2) | 1 | `…request_flow.v2` |
| approval | 1 | 1 | `…approval.v1`; `co_supplied` versions follow the table |

Manifest after the change: one tuple per case. The v1 tuples (and the old approval tuples) are
removed, and each replacement lists the keys it replaces in `supersedes` (contract
`compat-supersession.md`). Every BehaviorHash, every generated hash and every
ApprovalHash equals its value before this feature. Only StateMachine and RequestFlow model
hashes differ, including the embedded machine's hash recorded with a flow.
