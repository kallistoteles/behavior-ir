# Data Model: Approval Model

**Feature**: 503-approval-model | Builds on the data models of features 501 (`ModelInput`,
fragments, receipts) and 502 (declaration staging, `EncodeEntityName`). All semantic choices are
fixed here. Implementers choose only names and layout.

Notation:
- `p` is the approval id (a semantic id), and `E = EncodeEntityName(p)`.
- `S` is the subject entity and `A` the actor entity.
- `X = E ++ "Decision"` is the decision entity, and `C = E ++ "DecisionChoice"` the choice enum.
- `c ∈ {approve, reject}` is a capability, and `choice(approve) = APPROVE`,
  `choice(reject) = REJECT`.

Terminology: **DENIED** means the attempt was not permitted. **REJECT** is a recorded choice.
**REJECTED** is the derived outcome. Diagnostics, test names and docs use exactly these words.

## 1. ApprovalInput: what a binding may construct (raw, no authority)

| Field | Type | Notes |
|---|---|---|
| `approval_id` | string | validated as a semantic id by Rust |
| `subject_entity` | string | name of an entity in the declaration environment (authored or model-produced) |
| `actor_entity` | string | name of an entity in the declaration environment |
| `approve_threshold` | integer | `k_A ≥ 1` |
| `reject_threshold` | integer or null | `k_R ≥ 1` or `⊥` |
| `payload` | list of `(name, WType)` in any order | a mapping; order is never semantic |
| `capabilities` | exactly the two `CapabilityInput`s `approve` and `reject` | both are always present |
| `display`, `source` | metadata | provenance only |

`CapabilityInput` (for `approve` and for `reject`):

| Field | Type | Notes |
|---|---|---|
| `inputs`, `context` | lists of `(name, WType)` | mappings |
| `display`, `source` | metadata | |

A capability has no state bindings other than `subject`, no effects and no postconditions of its
own (spec Assumptions).

One `FragmentSource` serves the whole approval. It is called once per capability scope, and the
scope returns `DecisionFragments`:

```text
DecisionFragments {
  actor:     (Node, Loc)                     // the actor expression, of type Id(A)
  open:      (Node, Loc)                     // Open: traced in this scope
  eligible:  (Node, Loc)                     // Eligible: traced in this scope, `actor` is the node above
  guards:    [(Node, Loc)]                   // G_A or G_R, author order
  payload:   [(field, Node)]                 // the initializer, payload fields only
  commands:  [CommandEmissionSpec]           // K_c, author order
}
```

`open` and `eligible` are authored once and traced once per scope. Their nodes may differ
between the two scopes only through parameter names, which are fixed (§4), so they are
identical when the two capabilities have the same inputs and context.

## 2. ResolvedApproval: the authoritative representation

```text
ResolvedApproval {
  model_kind:        "approval"
  semantic_version:  1
  approval_id:       SemanticId                       // p
  enum_name:         String                           // C, derived
  entity:            String                           // X, derived
  subject:           String                           // S
  actor:             String                           // A
  approve_threshold: u32                              // k_A ≥ 1
  reject_threshold:  Option<u32>                      // k_R ≥ 1 or ⊥
  payload:           SortedMap<FieldId, WType>        // Π
  schema:            [(FieldId, WType)]               // derived, §3
  capabilities:      SortedMap<"approve" | "reject", ResolvedCapability>
}

ResolvedCapability {
  local_id:     "approve" | "reject"
  choice:       "APPROVE" | "REJECT"
  params:       [WParam]                              // §4
  action_name:  String                                // encode(p, local_id)
}
```

Each derived fact is computed in exactly one place:
- `C` and `X` come from `EncodeEntityName(p)` plus a fixed suffix (§3).
- `schema` (§3).
- `outcome` is never stored. It exists only as the generated derived definitions (§5).

## 3. Generated declarations

```text
enum   C { APPROVE, REJECT }                         // cases in this order (Core enum hash)
entity X {
  actor:    Id(A)
  choice:   Enum(C)
  subject:  Id(S)
  <payload>: <declared type>
}                                                    // all fields sorted by field name (code point)
```

**Naming.** `C = E ++ "DecisionChoice"` and `X = E ++ "Decision"`, where
`E = EncodeEntityName(p)`.
- Within the Approval kind both maps are injective, because `EncodeEntityName` is injective
  and a suffix is appended.
- `C(p) ≠ X(q)` for all `p`, `q`: `C(p)` ends with `…Choice` and `X(q)` ends with `…Decision`, and
  no string ends with both (their last letters are `e` and `n`). With the injectivity above, the
  map `p ↦ {C(p), X(p)}` produces pairwise-distinct names across all approvals.
- Across kinds, names may collide (a RequestFlow `x_decision` and an approval `x` both give
  `XDecision`). That is a `DECLARATION_COLLISION` from the common merge, never a rename.

**Field order.** Fields are sorted by field name, so payload declaration order never reaches
the entity hash (502 research R2).

**Identities, never references.** `subject` and `actor` are `Id`, never `Ref`. They add no
existence or removal rule. Core's implicit `id: Id(X)` is the decision identity.

Resolution errors (all blocking):

| Rule | Error |
|---|---|
| `approval_id` is a semantic id | `INVALID_SEMANTIC_ID` |
| `subject_entity` and `actor_entity` are entities of the declaration environment | `UNKNOWN_ENTITY` |
| `k_A ≥ 1`, and `k_R ≥ 1` if present | `INVALID_THRESHOLD` (new) |
| payload field ∉ {`subject`, `actor`, `choice`, `id`} and is a Core identifier | `RESERVED_FIELD` / `INVALID_PARAMETER` |
| payload field declared twice | `INVALID_PARAMETER` |
| capability parameter names distinct, Core identifiers, not `subject` or `decision_id` | `INVALID_PARAMETER` |
| exactly the capabilities `approve` and `reject` | `INVALID_PARAMETER` |

## 4. Decision capability parameters (fixed, model-semantic)

For capability `c`, in this order:
1. `(decision_id, Role::Input, Id(X))`: the explicit decision identity (spec §8);
2. inputs `(name, Role::Input, type)`, sorted by name;
3. `(subject, Role::State, Entity(S))`: the existing subject;
4. context `(name, Role::Context, type)`, sorted by name.

Callables bind by position in this order. `Open` receives `(subject, …)` and `Eligible` receives
`(subject, actor, …)`. The binding contract (contracts/approval-contract.md §4) fixes this.

## 5. Generated derived definitions (added in P2, research R10)

All five take one parameter `s: Entity(S)` with role Read. Each is a `DefinitionSpec`
(contracts/declaration-dag.md §2): its signature is fixed first, and its body is added after
its `deps`. The resulting order is `approve_count`, `reject_count`, `approved`, `rejected`,
`pending`.

| Name | Kind | Body | `deps` |
|---|---|---|---|
| `p__approve_count` | Derived, `Int` | `count(where(select(X), d → d.subject == s.id ∧ d.choice == lit(C, APPROVE)))` | ∅ |
| `p__reject_count` | Derived, `Int` | as above with `REJECT` | ∅ |
| `p__approved` | Rule | `k_R = ⊥`: `approve_count(s) ≥ k_A`; else `approve_count(s) ≥ k_A ∧ ¬(reject_count(s) ≥ k_R)` | approve_count (+ reject_count if `k_R ≠ ⊥`) |
| `p__rejected` | Rule | `k_R = ⊥`: `lit(false)`; else `reject_count(s) ≥ k_R` | ∅ or reject_count |
| `p__pending` | Rule | `¬approved(s) ∧ ¬rejected(s)` | approved, rejected |

The names use the 501 `encode(p, local)` with the fixed local ids `approve_count`,
`reject_count`, `approved`, `rejected` and `pending`. They share the capability namespace of
`p`, so a collision with any other item is `GENERATED_NAME_COLLISION`.

The bodies are exactly the spec's Outcome definition, with REJECTED taking precedence:
`approved` excludes `rejected` explicitly. So `approved`, `rejected` and `pending` are mutually
exclusive and exhaustive on *every* state, valid or not.

## 6. Decision action lowering (lowerer version 1)

As in research R11:

```text
action   encode(p, c)
params   §4
pre      [ Open, p__pending(subject), Eligible,
           ¬any(select(X), d → d.subject == subject.id ∧ d.actor == actor),
           G_c… ]
create   X { id: decision_id, actor: actor, choice: lit(C, choice(c)),
             subject: subject.id, <payload initializer> }          // exactly one
effects  none
post     [ count(where(select(X), d → d.subject == subject.id ∧ d.actor == actor)) ≤ 1,   // uniqueness, on S′
           ¬(p__approve_count(subject) ≥ k_A ∧ p__reject_count(subject) ≥ k_R) ]        // only if k_R ≠ ⊥
commands K_c                                                                             // command profile only
```

- **Initializer.** It must set exactly every payload field. The static errors are
  `INITIALIZER_MISSING_FIELD`, `INITIALIZER_UNKNOWN_FIELD` (this also covers `subject`, `actor`,
  `choice` and `id`) and `INITIALIZER_DUPLICATE_FIELD`.
- **Typing.** Core admission types every value; a mismatch, or an actor expression not of type
  `Id(A)`, gives `FRAGMENT_INVALID`.
- **Delta.** `Δ = CreateEntity(X, …)` plus command intents. Nothing else is expressible.

## 7. Ownership matrix and the base-state rule

| Operation on `X` | Allowed for | Error otherwise (stage `compose`) |
|---|---|---|
| create `X` | `encode(p, approve)`, `encode(p, reject)` | `UNAUTHORIZED_DECISION_CREATION` |
| write any field of `X` | nobody, including the approval's own actions | `UNAUTHORIZED_DECISION_WRITE` |
| remove `X` | nobody (ordinary behavior) | `UNAUTHORIZED_DECISION_REMOVAL` |
| read, query, derived, reads | anyone | — |

The rules are checked over the admitted module:
- every `creates()` of entity `X`;
- every `effects()` whose parameter's type is `Entity(X)`;
- every `removes()` whose parameter's type is `Entity(X)`.

`final_pass` and the 502 checks run alongside. `C` has no ownership rule: an enum has no
state.

**Completeness (the premise of the induction).** In an admitted Core module, an action changes
state only through:
- `effects()`, which write fields;
- `creates()`, which create entities;
- `removes()`, which remove entities.

`ActionItem` has no other state-changing part (`semantic/module.rs:347-357`), and Core has no
cascading removal. `subject` and `actor` are `Id`, not `Ref`, so removing a subject or actor
touches no decision. Reads, derived values, invariants, constraints and command emissions
change nothing.

The check runs over **every** action of the final module, with no exemption by origin:
- hand-written actions;
- StateMachine transitions (including one whose subject is `X`);
- RequestFlow creations and transitions (including effects over a creation state binding of
  type `X`);
- the actions of other approvals.

The only permitted creators are `p`'s own two actions. Migrations are outside ordinary
behavior (base-state rule).

Approval owns nothing on `S` or `A`: no field, creation or removal rule.

**Base-state rule.** The Approval laws hold on every ordinary state reachable from a base state
with `P(S₀)`. A genesis seed, import or migration that contains decisions is not a valid
Approval state by default. It must establish `P` itself. Documented in the contract, §6.

## 8. Identity

`ApprovalHash = tagged_hash("behavior.model.v1", CanonicalApprovalDocument)`:

```json
{
  "format": "behavior.model.approval.v1",
  "model_kind": "approval",
  "model_semantic_version": 1,
  "approval_id": "purchase_request_approval",
  "subject": { "entity": "PurchaseRequest", "entity_hash": "sha256:…" },
  "actor":   { "entity": "User",            "entity_hash": "sha256:…" },
  "approve_threshold": 2,
  "reject_threshold": 1,                                           // null for ⊥
  "payload": [ { "name": "reason", "type": <WType> }, … ],         // sorted by name
  "capabilities": [                                                // sorted: approve, reject
    { "id": "approve",
      "inputs": […], "context": […],                               // sorted by name
      "actor": "sha256:<Core expr hash>",
      "open": "sha256:…", "eligible": "sha256:…",
      "guards": [ "sha256:…" ],                                    // G_c, author order
      "payload": { "<field>": "sha256:…" },                        // map
      "commands": [ "sha256:…" ] } ]
}
```

- **Types are referenced by name plus Core entity hash, never copied.** A change to the subject
  entity (for example a RequestFlow payload change) therefore changes ApprovalHash.
- **Generated names, the schema, the derived bodies and the generated pre- and postconditions
  are derived**, so they are not hashed separately. They are functions of the hashed facts.
- **Fragment hashes** are read from the admitted action. Fixed positions in §6 separate them:
  - preconditions `[0]` is Open, `[2]` is Eligible, and `[4..]` are the guards;
  - `creates()[0].fields()` gives `actor` and the payload;
  - `command_emissions()` gives the commands.

## 9. Receipt (one per supplied approval)

The receipt has the 501 receipt fields with `model_kind: "approval"`, plus:

```json
"generated": {
  "enum":    { "name": "…DecisionChoice", "hash": "sha256:…" },
  "entity":  { "name": "…Decision",       "hash": "sha256:…" },
  "derived": { "approve_count": "sha256:…", "reject_count": "…", "approved": "…",
               "rejected": "…", "pending": "…" }
}
```

Source-map entries are checkout-independent, with relative paths as in 501:

| Construct | Maps to |
|---|---|
| `Approval.p.approve` / `.reject` | Core action `p__approve` / `p__reject` |
| `Approval.p.approved` (and the four others) | Core derived `p__approved` (rule) → its body hash |
| `Approval.p.uniqueness` | postcondition `[0]` of both actions |
| `Approval.p.threshold_consistency` | postcondition `[1]` of both actions (if `k_R ≠ ⊥`) |
| `Approval.p.terminal_guard` | precondition `[1]` (`p__pending(subject)`) of both actions |
| `Approval.p.decision` / `.choice` | entity `X` / enum `C`, with field entries |

## 10. Versions

| Constant | Initial | Changes when |
|---|---|---|
| `APPROVAL_SEMANTIC_VERSION` | 1 | the meaning or admitted structure of Approval changes |
| `APPROVAL_LOWERER_VERSION` | 1 | the same approval intentionally lowers differently (names, layout, derived bodies, guard positions) |

The compat tuple key is `(approval, 1, 1, Core 0.12.0)`. A case that also contains RequestFlows
or StateMachines adds their versions to its key (502 compat grouping).

## 11. New error codes

| Code | Stage | Fields |
|---|---|---|
| `INVALID_THRESHOLD` | resolve | `model_ref`, `which` (`approve` / `reject`), `value` |
| `DECLARATION_CYCLE` | set | `models` (sorted) |
| `DEFINITION_CYCLE` | set | `names` (sorted) |
| `DEFINITION_MISMATCH` | set | `name`, `model_ref` (P4 found a different derived item than P2 added) |
| `UNAUTHORIZED_DECISION_CREATION` | compose | `action`, `entity`, `owner` |
| `UNAUTHORIZED_DECISION_WRITE` | compose | `action`, `entity`, `field`, `owner` |
| `UNAUTHORIZED_DECISION_REMOVAL` | compose | `action`, `entity`, `owner` |
