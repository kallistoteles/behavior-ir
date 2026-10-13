# Research: RequestFlow Model

**Feature**: 502-request-flow | **Date**: 2026-10-12 | **Builds on**: feature 501 (merged design,
branch `501-model-lowering-state-machine`)

All Core facts are read from the pinned Core Release 0.12.0 (`b525da4…`) in the cargo checkout.
Nothing in behavior-ir-core was changed. Paths are relative to `crates/behavior-core/src/`
unless stated otherwise.

## R1. Declaration staging: the new build protocol (FR-006, the main architectural question)

**Finding.** Feature 501's protocol was "trace all ordinary behavior, then compose models".
A hand-written action, read or entity that names the request entity must be type-checked
against a declaration that already exists. The builder computes its declaration environment
lazily (`ensure_decls`, `builder.rs:503`), the first time a node is built or a scope is
pushed. So the request entity must be in the builder **before any hand-written tracing
starts**.

**Decision.** The protocol has two model calls with a fixed order:

```text
P1  authored declarations      enums, nominals, entities, commands (binding, unchanged)
P2  model declarations         declare_models(builder, models)       ← new, Rust
      for each model in canonical id order: Decls(model) → identical-or-error merge → builder
P3  hand-written tracing       derived, invariants, constraints, actions, reads (binding, unchanged)
P4  model behavior + checks    compose(builder, models, root)        ← 501, extended
      resolve → set checks → Creates + L_SM(SM) lowering → finish → final pass → identity
```

- `Decls(m)` is computed from the model input and the P1 declarations alone, without lowering
  anything, so the entity handle never requires running lowering. `Decls(StateMachine) = ∅`.
- P2 for a set containing only StateMachines is a no-op: no builder call and no snapshot. So
  every 501 build is byte-for-byte the same sequence of builder calls as before, and pinned
  hashes cannot move. A regression test asserts this for every 501 manifest case (plan test
  table, row "SM regression").
- P4 re-derives `Decls(m)` and requires the builder to hold an identical declaration
  (same name, same field list and types, by Core entity hash). This catches a binding that
  skipped P2 or declared something else under that name. P4 never declares anything.

**Alternatives rejected.**
- Declaring entities inside `compose` (P4): hand-written behavior would be traced before the
  entity exists, so it would fail.
- Running full lowering early to obtain the entity: rejected per the planning brief.
- A builder token passed from P2 to P4: unnecessary, because identical-or-error re-derivation
  is stateless and stricter.

## R2. Canonical declaration merge and entity field order (FR-024)

**Finding.** An entity's Core hash covers its fields **in the order given**
(`admit/hash.rs:141`, `e.str(name).u32(len)` then each field in sequence). The module hash is
order-independent (a sorted name table). `declare_entity` rejects a second declaration of the
same entity name (`builder.rs:487`).

**Decisions.**
- **Generated field order:** all fields of the request entity sorted by field name (code
  point): reserved and payload fields together. Payload declaration order therefore cannot
  reach the entity hash.
- **Merge rule (common infrastructure, `composition::declarations`):** contributions are
  processed in canonical model-id order. For each contributed declaration `d` with name `n`:
  - if no existing item is named `n`, declare `d`;
  - if an existing entity named `n` has exactly `d`'s field list (names, order and wire types,
    and therefore the same Core entity hash), it is accepted as identical and not declared again;
  - anything else (a different entity, or an enum, nominal, command, derived or action with
    name `n`) → `DECLARATION_COLLISION { name, model_ref, existing_kind }`.
  Existing items are read from a `Builder::finish` snapshot of P1 (`name_table()`,
  `entities()`). The order of authored declarations is irrelevant, because only the sorted
  name table and per-entity hashes enter BehaviorHash.
- The status enum is never contributed (FR-004). It must exist in P1, otherwise
  `UNKNOWN_ENUM`.

## R3. EncodeEntityName: canonical, injective, Core-valid (FR-002, SC-010)

`ID = [a-z][a-z0-9]*(_[a-z0-9]+)*`. Split `f` into runs `r₁ … r_k` at `_`.
`EncodeEntityName(f) = cap(r₁) ‖ enc(r₂) ‖ … ‖ enc(r_k)`, where `cap(r)` uppercases the first
character and `enc(r) = cap(r)` if `r` starts with a letter, else `"_" ‖ r`.

- **Plain PascalCase is not injective:** `a_1b` and `a1b` both give `A1b`.
- **Proof of injectivity.** Runs contain only `[a-z0-9]`, so uppercase letters in the output
  occur exactly at the starts of letter-led runs, and `_` occurs exactly at the starts of
  digit-led runs. `r₁` always starts with a letter. Decoding splits before each uppercase
  letter and at each `_`, then lowercases the first character of each run, which recovers
  `f`. Distinct inputs therefore give distinct outputs.
- **Core-valid:** the output starts with `[A-Z]` and contains only `[A-Za-z0-9_]`, which
  matches `wire::is_identifier`.
- **Tests:** a proptest over the generator `[a-z][a-z0-9]{0,4}(_[a-z0-9]{1,4}){0,4}` (≥ 2,000
  cases, digit-led runs included, fixed seed) checks injectivity, decodability and validity,
  plus a golden table (`purchase_request` → `PurchaseRequest`, `a_1b` → `A_1b`, `a1b` → `A1b`,
  `x_2_y` → `X_2Y`).

## R4. Creation lowering through the public builder (FR-008)

**Finding.** `Builder::add_action` takes creations as `WLifecycle::Create { entity, id: WExpr,
fields: Vec<(String, WExpr)> }` (`builder.rs:1147`). `WLifecycle` is public; `WExpr` values
come only from `Node::into_wire()`. The Python binding uses exactly this
(`crates/behavior-py/src/lib.rs`, `add_action`). Core admission type-checks the whole creation
in `finish`.

**Decision.** Creation lowering builds every value as a Core `Node` in the creation's scope:
the generated `lit(T_s, q_c)` and the author's initializer nodes. It then converts them with
`into_wire()` **only** to fill `WLifecycle::Create`. This is the public builder's input type
for creations, not a model interface or JSON. Feature 501's public-surface guard forbids
`into_wire` everywhere in `crates/behavior-models/src`. It is narrowed: `into_wire` is allowed
in exactly one file, `src/request_flow/create.rs`, and `to_wire_json` and serde
serialization stay forbidden everywhere. The guard test is updated and negative-tested.

**Classification.** This is not a gap. It is the existing public API for creations, the same
one the Python binding uses.

## R5. Creation extra effects need state parameters (FR-007, a finding)

**Finding.** Core effects target only **state** parameters (`(param, field, value)`, checked
by `effect_target`). A creation whose parameters are only inputs and context therefore cannot
have any effect, so `E_c` from the specification would always be empty. Core does allow state
parameters on creating actions (`examples/accounts/behavior.py: open_account(owner: Customer,
…)`).

**Decision.** A creation specification may also declare **state bindings** `S_c`
(`ParameterId → entity type`): existing entities the creation reads or changes. `E_c` targets
them. This is the only way to give the specification's `E_c` any meaning. Every effect still
goes through the ownership pass:
- writing `status` or `requester` of any request (including when `S_c` binds a request) is
  rejected by the existing rules (R7);
- payload and assignee writes are ordinary.
Parameter order: the request identity input, then the other inputs sorted by name, then
`S_c` sorted, then context sorted. This is recorded in the data model. **Confirmed by the
author (spec Clarifications):** `c = (id_c, q_c, S_c, I_c, C_c, G_c, V_c, E_c, P_c, K_c)`, and
`Create ∪ E_c` is one atomic Core transition. Constraints:
- `S_c` binds existing entities only, never the request being created;
- aliasing follows Core's binding rules;
- `E_c` writes only through `S_c` and stays under the ownership pass;
- `S_c` is canonicalized and enters RequestFlowHash.

## R6. Reusing the StateMachine implementation exactly (FR-013, FR-015)

**Decision.** RequestFlow derives a StateMachine `ModelInput` and hands it to the existing
code paths. Nothing is copied.

```text
sm_input(RF) = ModelInput {
  model_id:       RF.id,
  subject_entity: EncodeEntityName(RF.id),
  state_field:    "status",
  subject_param:  Some(RF.id),         // explicit: equals the StateMachine default for every id
  entry_states:   { q_c | c ∈ 𝒞 },     // derived here, the only source
  transitions:    RF.transitions        // passed through unchanged
}
```

- **Resolution:** `state_machine::resolve::resolve`.
- **Lowering:** `state_machine::lower::lower_transition` with the flow's `FragmentSource`
  (transition fragments are traced exactly as for a standalone machine).
- **ModelHash:** `identity::canonical_document` and `identity::model_hash`.
- **Diagnostics:** `source_map::diagnostics`.
- **Ownership** (status writes and entry-state check): the existing `composition::final_pass`,
  with the embedded machine as a member of the *internal* set of composed machines (not of
  the supplied model set, R9).

**Proof obligation (SC-006).** Over the same entity, field and transitions, the flow's
transition actions equal a standalone StateMachine's by Core action hash. A test builds both.

Entry states cannot diverge from creation targets: `entry_states` is computed from `𝒞` in one
function, and the flow input type has no entry-state field at all.

## R7. Ownership matrix and its enforcement over the admitted module (FR-010, FR-011, FR-015, FR-016)

| Field or operation on request type `E` | Who may | Enforced by |
|---|---|---|
| create `E` | the flow's creation actions only | final pass: every `creates()` with `entity() = E` must be in `CreationActions(RF)` → else `UNAUTHORIZED_REQUEST_CREATION` |
| `E.status` (after creation) | embedded StateMachine transitions only | existing 501 rule (`UNAUTHORIZED_OWNED_FIELD_WRITE`) |
| `E.status` at creation | `lit(q_c)` generated by the flow | initializer check (`INITIALIZER_SETS_STATUS`) plus the existing entry check |
| `E.requester` (after creation) | nobody | final pass: any effect `(p, requester)` with `p : E` → `UNAUTHORIZED_REQUESTER_WRITE` |
| `E.requester` at creation | the initializer `V_c` | initializer check (missing or duplicate) |
| `E.assignee` | any ordinary effect | not checked (an ordinary field) |
| payload fields | any ordinary behavior | not checked |
| a creation action creating > 1 `E` | — | impossible by construction (one `Create`); `debug_assert!` in creation lowering plus a structural test, no error code |
| removal of `E` | Core lifecycle | not a write, not checked (spec edge case) |

Completeness follows feature 501's argument: in an admitted module only `effects()` and
`creates()` change entity state, and the pass enumerates both over every action.

**`Id<A>` stays identity (FR-003).** The requester is `WType::Id(A)` and the assignee is
`Option(Id(A))`, never `Ref`. The pass adds no existence checks. A golden schema test asserts
the generated field types.

## R8. Static initializer completeness (FR-009)

Before any Core call, Rust checks each creation's initializer keys against the generated
schema: `required = {requester} ∪ payload ∪ ({assignee} if enabled)`.
- missing → `INITIALIZER_MISSING_FIELD`;
- unknown → `INITIALIZER_UNKNOWN_FIELD`;
- `status` → `INITIALIZER_SETS_STATUS`;
- a key twice (possible through binding input) → `INITIALIZER_DUPLICATE_FIELD`.

The initializer's *values* are Core nodes, type-checked against the field types with
`Builder::check_effect`-equivalent typing. That happens through Core admission at `finish`,
which reports a type mismatch as `FRAGMENT_INVALID`. Every structural error is static and
reported per creation and field.

## R9. One receipt per supplied model; embedded identity record (FR-025)

- Supplied set = what the binding passes. `DUPLICATE_MODEL_ID` is checked over it; an embedded
  machine is never a member, so no exception exists.
- The internal composition set = supplied StateMachines ∪ `{ SM(RF) | RF supplied }`. It is
  used for ownership, so an embedded machine and a standalone machine claiming the same
  `(E, status)` still give `CONFLICTING_OWNERSHIP`.
- **Receipt of a flow** (`behavior.model_receipt.v1`, kind `request_flow`): the 501 fields,
  plus `model` (the flow's canonical document) and
  `embedded: [{ kind: "state_machine", id, model_semantic_version, lowerer_version,
  model_hash }]`. Its `source_map` entries:
  - creations: `{core: action, model: "<id>/creation/<c>"}`;
  - transitions: `{core: action, model: "<id>/transition/<t>",
    embedded: "state_machine:<id>/transition/<t>"}`;
  - the entity: `{core: {kind: "entity", name}, model: "<id>/request_entity"}`;
  - fields: `<id>/request_entity/<field>`.
  Locations follow the 501 relative-path rule.
- Python: `module.model_receipts` returns one `LoweringReceipt` per supplied model. Its type
  gains an optional `embedded` field. The `from_dict` drift check is updated deliberately
  (the field is optional and present only for flows).

## R10. Compatibility manifest for a declaration-producing model (FR-026)

**Finding.** The 501 gate requires every case to share one version tuple, and established
tuple entries must stay byte-identical.

**Decision.**
- **Tuple key** = the set of receipts' (kind, semantic version, lowerer version) plus the Core
  Release, plus, for flows, the embedded machine's (kind, semantic version, lowerer version).
  A StateMachine lowerer bump therefore also requires a new RequestFlow tuple, because a
  flow's BehaviorHash depends on it.
- The gate groups cases by tuple instead of requiring a single tuple. The key of 501's
  StateMachine-only tuple is computed exactly as before (no `embedded` member when absent), so
  its established entry stays byte-identical.
- RequestFlow case entries pin `behavior_hash`, `models: {flow id: model_hash}`,
  `embedded: {flow id: state_machine_hash}` and `entities: {flow id: entity name}`.
- New reference cases: `rf_purchase_request` (the full reference lifecycle with assignee,
  payload, two creations, a command), `rf_two_flows_local_submit`, and
  `rf_with_standalone_machine`.

## R11. Python handle to the generated entity (FR-028)

- `RequestFlow.request_entity` is a Python class carrying an `EntityDecl`. Its **name** comes
  from Rust (`_engine.encode_entity_name(flow_id)`, the R3 function exposed read-only), so it
  is usable in annotations and in `Id[...]` before the module is built.
- Its **fields** mirror the Rust-derived schema. Python assembles them from the declared
  payload, actor and enum types, and P2 returns the authoritative schema `[(field, type)]`.
  The binding compares the two and raises on any difference (the same drift-check pattern as
  `LoweringReceipt`). So Python never decides the schema; it only holds a typed view.
- P2 runs inside `BehaviorModule`, after authored entities are declared and before
  `derived`, `invariants`, `actions` and `reads` are traced.

## R12. Lifecycle property (SC-003)

A property test (Python, through the public engine) generates random sequences of invocations
over the reference flow: random creations, random transitions on random existing requests,
some invalid. It applies the committed outcomes to an in-test state and checks the invariant
`trace ∈ Q_entry · R_T*`: the first status is in `Q_entry`, and every status change observed
in a committed decision is an `R_T` edge. Fixed seed, ≥ 1,000 traces, hypothesis-free (a
seeded `random.Random` loop), so it needs no new dependency.

## R13. Core gap classification

| Need | Public API | Gap? |
|---|---|---|
| Declare the generated entity | `Builder::declare_entity` | no |
| Read existing declarations (merge, identical check) | `Builder::finish(&self)` snapshot → `name_table()`, `entities()`, `EntityItem::{fields, field_type}`, `hash()` via name table | no |
| Creation actions | `add_action` / `add_action_with_commands` with `WLifecycle::Create` (R4) | no |
| Detect creations and requester writes | `ActionItem::creates()`, `effects()`, `params()` | no |
| StateMachine reuse | internal (`behavior-models`) | no |

**No Core change is needed.** No semantic gap and no facade-exposure gap was found.
