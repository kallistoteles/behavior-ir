# Data Model: RequestFlow Model

**Feature**: 502-request-flow | Builds on feature 501's data model (`ModelInput`, fragments,
receipts). All semantic choices are fixed here. Implementers choose only names and layout.

Notation: `f` is the flow id (a semantic id), `E = EncodeEntityName(f)`, `T_s` is the
authored status enum, `A` is the actor entity, `Q := Cases(T_s)`.

## 1. RequestFlowInput: what a binding may construct (raw, no authority)

| Field | Type | Notes |
|---|---|---|
| `flow_id` | string | validated as a semantic id by Rust |
| `actor_entity` | string | the name of an authored entity `A` |
| `status_enum` | string | the name of an authored enum `T_s` (never generated) |
| `assignee` | bool | enables `assignee: Option<Id<A>>` |
| `payload` | list of `(name, WType)` in any order | a mapping; order is never semantic |
| `creations` | list of `CreationInput` | ≥ 1 |
| `transitions` | list of `TransitionInput` (feature 501, unchanged) | |
| `display`, `source` | metadata | provenance only |

`CreationInput`:

| Field | Type | Notes |
|---|---|---|
| `creation_id` | string | semantic id, unique against creation *and* transition ids |
| `to_state` | string | `q_c`, a case of `T_s` |
| `inputs`, `context` | lists of `(name, WType)` | mappings |
| `state` | list of `(name, entity name)` | `S_c`: existing entities only (never the request being created), a mapping; `E_c` writes only through these; aliasing per Core binding rules (research R5) |
| `display`, `source` | metadata | |

Fragments: one `FragmentSource` per flow, called once per creation scope and once per
transition scope. A creation scope returns `CreationFragments`:

```text
CreationFragments {
  initializer:    [(field, Node)]              // V_c: requester, payload, optional assignee
  guards:         [(Node, Loc)]                // G_c, author order
  effects:        [(param, field, Node, Loc)]  // E_c over S_c, author order
  postconditions: [(Node, Loc)]                // P_c, author order
  commands:       [CommandEmissionSpec]        // K_c, author order
}
```

Transition scopes return feature 501's `TransitionFragments`, unchanged.

## 2. ResolvedRequestFlow: the authoritative representation

```text
ResolvedRequestFlow {
  model_kind:        "request_flow"
  semantic_version:  1
  flow_id:           SemanticId                    // f
  entity:            String                        // E = EncodeEntityName(f), derived
  actor:             String                        // A
  status_enum:       String                        // T_s; Q not stored
  assignee:          bool
  payload:           SortedMap<FieldId, WType>     // Π
  schema:            [(FieldId, WType)]            // derived: see §3
  creations:         SortedMap<SemanticId, ResolvedCreation>
  machine:           ResolvedStateMachine          // SM(f), resolved by the 501 resolver
}

ResolvedCreation {
  creation_id:  SemanticId
  to_state:     Case                               // q_c ∈ Q
  params:       [WParam]                           // see §4
  action_name:  String                             // encode(f, creation_id)
}
```

Derived facts. Each one is computed in exactly one place and never stored twice:
- `E = EncodeEntityName(f)` (research R3).
- `schema` (§3).
- `Q_entry = { c.to_state }`. It exists only as `machine.entry_states`, computed once in
  `sm_input` (research R6). The flow input has no entry-state field.
- `machine` = `state_machine::resolve(sm_input(RF), decls)`.

## 3. The generated request entity

```text
E {
  requester:  Id(A)
  status:     Enum(T_s)
  assignee:   Option(Id(A))      // iff assignee enabled
  <payload>:  <declared type>
}
```

- **Field order:** all fields sorted by field name (code point) (research R2).
- **Never `Ref`:** no existence or removal semantics for actors (spec clarification).
- **Location:** the flow's source location (relative path rule). It is not part of any hash.

Resolution errors (all blocking):

| Rule | Error |
|---|---|
| `flow_id` and every creation and transition id are semantic ids | `INVALID_SEMANTIC_ID` |
| `actor_entity` is an authored entity | `UNKNOWN_ENTITY` |
| `status_enum` is an authored enum | `UNKNOWN_ENUM` |
| payload field ∉ {`requester`, `status`, `assignee` (if enabled)} and is a Core identifier | `RESERVED_FIELD` / `INVALID_PARAMETER` |
| payload field declared twice | `INVALID_PARAMETER` |
| at least one creation | `NO_CREATION` |
| creation ids ∪ transition ids pairwise distinct | `CAPABILITY_ID_COLLISION` |
| `to_state` ∈ `Cases(T_s)` | `UNKNOWN_STATE` |
| creation parameter names distinct, Core identifiers, not `request_id` | `INVALID_PARAMETER` |
| state binding types are authored or generated entities | `UNKNOWN_ENTITY` |
| the embedded machine's resolution errors (501) | as in 501, with `model_ref` under the flow |

**Note (forward references, found during implementation).** When authored declarations
reference the request entity (law D5), they cannot be admitted before P2 declares it. P2 then
declares the request entity first and resolves against the completed declarations. If, in that
case, the actor or the status enum is also missing, the declarations as a whole cannot be
admitted, and the error is `CORE_ADMISSION_FAILED` carrying Core's own errors (`UNKNOWN_ENTITY`
"unknown entity \`A\`" or `UNKNOWN_TYPE` "unknown enum \`T\`"), not `UNKNOWN_ENTITY` /
`UNKNOWN_ENUM`. Core's public API names the missing type only in its message text, and an
inadmissible builder can't be queried, so the model authority can't attribute the error
without parsing messages. This is existing Core semantics without a structured public error
field, not a missing semantic. Without forward references, the table above applies unchanged.
Pinned by `forward_references_keep_model_errors` in
crates/behavior-models/tests/request_flow_staging.rs.

## 4. Creation parameters (fixed, model-semantic)

Terminology: **state bindings (`S_c`)** are the creation's Core state parameters. This
document uses "state bindings" for `S_c` throughout.

For creation `c`, in this order:
1. `(request_id, Role::Input, Id(E))`: the explicit request identity (spec FR-010);
2. inputs `(name, Role::Input, type)`, sorted by name;
3. state bindings `(name, Role::State, Entity(X))`, sorted by name;
4. context `(name, Role::Context, type)`, sorted by name.

`request_id` is a reserved parameter name in creations. The binding binds the author's
callables by position in this order.

## 5. Creation lowering (lowerer version 1)

```text
action   encode(f, c)
params   §4
pre      G_c
create   E { id: request_id, status: lit(T_s, q_c), <V_c field values> }   // exactly one
effects  E_c
post     P_c
commands K_c                                       // command profile only
```

- `status` is generated, from `q_c` only.
- `V_c` must provide exactly `{requester} ∪ payload ∪ ({assignee} if enabled)`. The static
  errors are `INITIALIZER_MISSING_FIELD`, `INITIALIZER_UNKNOWN_FIELD`,
  `INITIALIZER_SETS_STATUS` and `INITIALIZER_DUPLICATE_FIELD`.
- Values are typed by Core admission; a mismatch is `FRAGMENT_INVALID`.

## 6. Ownership matrix

See research R7. New error codes (stage `compose`): `UNAUTHORIZED_REQUEST_CREATION`,
`UNAUTHORIZED_REQUESTER_WRITE`. One creation per creation action is guaranteed by
construction (§5) and asserted, so it is not an error code. New error codes (stage `set`):
`DECLARATION_COLLISION`.

## 7. Identity

`RequestFlowHash = tagged_hash("behavior.model.v1", CanonicalRequestFlowDocument)`:

```json
{
  "format": "behavior.model.request_flow.v1",
  "model_kind": "request_flow",
  "model_semantic_version": 1,
  "flow_id": "purchase_request",
  "actor": { "entity": "User" },
  "status_enum": { "name": "PurchaseRequestStatus", "enum_hash": "sha256:…" },
  "assignee": true,
  "payload": [ { "name": "amount", "type": <WType> }, … ],           // sorted by name
  "creations": [                                                     // sorted by id
    { "id": "create", "to": "Draft",
      "inputs": […], "state": […], "context": […],                   // sorted by name
      "initializer": { "<field>": "sha256:<Core expr hash>", … },    // map
      "guards": [ "sha256:…" ], "effects": [ "sha256:…" ],
      "postconditions": [ "sha256:…" ], "commands": [ "sha256:…" ] } ],
  "state_machine": { "model_hash": "sha256:…", "document": <501 canonical document> }
}
```

- **The enum is referenced by its Core hash, never copied.** Transitions appear only inside
  the embedded machine's document, never in a second encoding.
- **The entity name and schema are derived, not hashed separately.** They are functions of
  the hashed facts.
- **Fragment hashes** are read from the admitted creation action: `preconditions()`,
  `effects()`, `postconditions()`, `command_emissions()`, and `creates()[0].fields()` for the
  initializer. Positions follow §5, which has no generated preconditions, effects or
  postconditions: every one is the author's.

## 8. Receipt (one per supplied flow)

The 501 receipt fields with `model_kind: "request_flow"`, plus
`embedded: [{kind: "state_machine", id: f, model_semantic_version, lowerer_version,
model_hash}]`. Source-map entries are listed in research R9.

## 9. Versions

| Constant | Initial | Changes when |
|---|---|---|
| `REQUEST_FLOW_SEMANTIC_VERSION` | 1 | the meaning or admitted structure of RequestFlow changes |
| `REQUEST_FLOW_LOWERER_VERSION` | 1 | the same flow intentionally lowers differently (schema layout, creation layout, names) |
| embedded StateMachine versions | 501's | part of a flow's compatibility tuple (research R10) |
