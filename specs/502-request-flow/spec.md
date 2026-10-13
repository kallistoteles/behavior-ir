# Feature Specification: RequestFlow Model

**Feature Branch**: `502-request-flow`

**Created**: 2026-10-12

**Status**: Draft

**Input**: User description: "RequestFlow as the next Behavior Model, built on the implemented
StateMachine rather than in parallel. RequestFlow owns request creation and request conventions;
StateMachine remains the single authority for status evolution. Entry states are derived from
the creation capabilities instead of being declared. No Core change."

**Depends on**: feature 501 (Behavior Models infrastructure and StateMachine), branch
`501-model-lowering-state-machine` (PR #16). This branch is stacked on it.

## Context

RequestFlow is a Behavior Model for persistent requests that an actor creates, that carry
domain data, and that move through a finite set of statuses. It lowers completely to ordinary
Behavior IR:

```text
RequestFlow
    ├── request entity (schema)
    ├── creation actions
    └── StateMachine over the request's status   ← the existing, authoritative lowerer
            └── transition actions
                    ↓
             ordinary Behavior IR → behavior-ir-core (unchanged)
```

```text
RF = (id, A, E, Q, Π, 𝒞, 𝒯)
  id  stable semantic flow identity          A   actor identity type (Id<Actor>)
  E   the generated request entity type      Q   := Cases(status enum)
  Π   payload schema FieldId → CoreType      𝒞   creation specifications (≥ 1)
  𝒯   StateMachine transitions

c = (id_c, q_c, S_c, I_c, C_c, G_c, V_c, E_c, P_c, K_c)     q_c ∈ Q, fixed initial status
Q_entry := { q_c | c ∈ 𝒞 }                             derived, never declared
R_T     := { (q, q_t) | t ∈ 𝒯, q ∈ F_t }               the status transition relation

L_RF(RF) = Compose( Schema(RF), Create(RF), L_SM(SM(RF)) )
SM(RF)   = (E, status, Q_entry, 𝒯)                     lowered by the existing StateMachine lowerer
Meaning_RF(RF) := Meaning_Core(L_RF(RF))
```

**Central guarantee (lifecycle theorem).** Every ordinary request status trace begins at a
declared creation state and follows only declared StateMachine edges:
`trace ∈ Q_entry · R_T*`. The proof is by induction. Creation sets `q_0 = q_c ∈ Q_entry ⊆ Q`.
The embedded StateMachine owns every ordinary write of `status`, so each later change is a
declared transition with `q_i ∈ F_t` and `q_{i+1} = q_t`, which means `(q_i, q_{i+1}) ∈ R_T`.

Governing principles, used as review tests:

- One semantic kernel, many authoring models.
- RequestFlow adds request structure; StateMachine defines status evolution.
- Request creation is RequestFlow semantics, not StateMachine semantics.
- Authority is explicit behavior, never implied by labels such as requester or assignee.
- A request status change must correspond to a declared StateMachine transition.
- The same semantic fact is not represented twice when it can be derived.
- Lifecycle notation `⊥ → q_c` is mathematics only: `⊥` (nonexistence) is never a
  StateMachine state. The Core operation is creation.

## Clarifications

### Session 2026-10-12

- Q: Where does the status enum's identity come from: an author-declared enum, or an enum the
  flow generates? → A: The author declares it. The status enum `T_s` is an ordinary nominal
  Core enum that the flow references, and only the request entity is generated. Exactly as in
  StateMachine, `Q := Cases(T_s)`, so RequestFlow and its embedded StateMachine share one state
  domain by construction. There is no separate `Q_RF` to keep in step. RequestFlowHash commits
  to `T_s`'s semantic Core identity (its enum hash) and does not copy its cases. Changing the
  enum (for example adding `Cancelled`) therefore changes the flow's identity. Enums can be
  shared: two flows over the same `T_s` have `Q_A = Q_B` but may have different transition
  relations `R_A ≠ R_B`, because a machine owns its field, not the enum. Principle: reference
  the semantic type; do not duplicate its definition. RequestFlow owns the generated request
  entity, not the status enum type.
- Q: What name should the generated request entity get: derived automatically from the flow id,
  or stated explicitly by the author? → A: Derived, through a canonical injective encoding
  `EncodeEntityName : FlowId → CoreEntityName`, with `f₁ ≠ f₂ ⇒ EncodeEntityName(f₁) ≠
  EncodeEntityName(f₂)`. It is not an author-supplied semantic input, because it can be derived
  exactly from the flow id. Plain PascalCase is *not* injective under the semantic-id grammar
  (`a_1b` and `a1b` both give `A1b`), so the v1 encoding capitalizes each run that starts with
  a letter and keeps `_` before each run that starts with a digit (`purchase_request` →
  `PurchaseRequest`, `a_1b` → `A_1b`, `a1b` → `A1b`). Injectivity proof: runs contain only
  `[a-z0-9]`, so uppercase letters mark exactly the starts of letter-led runs and `_` exactly
  the starts of digit-led runs, and decoding splits there and lowercases. Every result is a
  Core identifier. Changing the flow id is a deliberate public semantic rename (capabilities
  and entity together); display names stay non-semantic. A clash with an independently
  declared entity is a blocking composition error, never renamed. Python gets a typed handle
  to the generated entity (for example `flow.request_entity`, usable in `Id[...]` and as a
  parameter type), so authors never write the name.
- Q: Should the requester and the optional assignee be plain identities (`Id<Actor>`) or
  references (`Ref<Actor>`)? → A: `Id<A>` for both: `requester ∈ Id(A)` and
  `assignee ∈ Option(Id(A))`. They record actor identity and imply neither
  `requester(r) = a ⇒ exists(a)` nor `assignee(r) = Some(a) ⇒ exists(a)`, nor any
  removal blocking. Historical attribution needs identity, not continued existence (people
  leave, accounts are removed, identities come from external providers). If a domain needs the
  actor to exist at creation or assignment time, it adds an explicit `exists(...)` guard. If it
  needs persistent referential integrity and removal blocking, it models that explicitly with a
  `Ref<A>` relationship or a composed model; RequestFlow's built-in fields don't provide it. No
  per-flow `Id`/`Ref` option exists in v1, because that would create two RequestFlow semantics.
- Q: When a module contains a RequestFlow, should the build report one receipt or two (flow and
  embedded state machine)? → A: One receipt per supplied model:
  `Receipts(M) = { Receipt(m) | m ∈ Models_supplied }`. The embedded machine `S = SM(R)` is a
  deterministic component of the flow, not a member of the model set (`S ∉ Models_supplied`).
  So the id rule (FR-027) needs no exception for it. The flow receipt (kind `request_flow`)
  carries an *embedded-model identity record* for `S` (kind, id, semantic version, ModelHash
  `H(S)`), not a nested full receipt: a receipt describes a complete lowering in module
  context, and `S` only contributes to the flow's one composed lowering. Source-map entries
  carry the chain flow construct → state-machine construct → Core action. Identity law: for
  fixed versions, `H(R₁) = H(R₂) ⇒ H(S(R₁)) = H(S(R₂))` and
  `⇒ BehaviorHash(L_RF(R₁)) = BehaviorHash(L_RF(R₂))`. The compatibility manifest pins, per
  reference flow, its ModelHash, embedded state-machine hash and BehaviorHash in one entry.
- Q (raised in planning): Core effects target only state parameters, so a creation with only
  input and context could never have the extra effects `E_c`. Should creations get state
  bindings, or should `E_c` be dropped? → A: Add state bindings `S_c` and keep `E_c`:
  `c = (id_c, q_c, S_c, I_c, C_c, G_c, V_c, E_c, P_c, K_c)`. A creation is an ordinary Core
  transition that additionally creates the request:
  `Δ_c = CreateEntity(request, q_c, V_c) ∪ E_c(s_c, i, γ)`, committed atomically. Without
  `S_c`, "create request and reserve budget" would need two non-atomic actions, which is a
  real loss of meaning and not just syntax. `Enabled_c(S, s_c, i, γ)` iff every binding in
  `S_c` binds validly against the snapshot, `request_id` is fresh under Core's lifecycle
  rules, every guard in `G_c` holds, `V_c` is valid, and the resulting state after both the
  creation and `E_c` is valid. `S_c` holds only *existing* entities. The request being created
  is never a member, because it does not exist before the transition. Aliasing between state
  bindings follows Core's ordinary binding rules; RequestFlow adds no alias semantics.
  Ownership checks (status, requester) apply to effects through `S_c` too. `S_c` is a
  semantic mapping canonicalized in Rust, and it is part of RequestFlowHash.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Author a request flow and run its lifecycle as ordinary behavior (Priority: P1)

A behavior author declares a request flow:
- the actor type;
- the status enum;
- payload fields;
- optional assignee support;
- one or more creation capabilities, each with a fixed initial status;
- the transitions.

They build the module and run a request through it with the public Behavior engine: create it,
submit it, start review, return it for changes, resubmit, review again and approve. Every step is
an ordinary Core action, and Core has no RequestFlow concept.

**Why this priority**: This is the minimum useful product: a request lifecycle authored as one
readable model that runs entirely on ordinary Core semantics.

**Independent Test**: Author `purchase_request` (Draft → Submitted → InReview →
{Approved, Rejected, NeedsChanges}, NeedsChanges → Submitted, plus Cancelled). Build it and
drive the full cycle through the public engine. Inspect the admitted module for model-specific
constructs.

**Acceptance Scenarios**:

1. **Given** a flow with creation `create → Draft`, **When** `purchase_request__create` is
   invoked with a fresh request identity, the requester and every payload value, **Then** one
   request exists with status Draft, the given requester and payload, and an empty assignee.
2. **Given** a created request, **When** the transitions submit → start_review →
   return → resubmit → start_review → approve are invoked in order, **Then** each commits and
   the status follows exactly those edges.
3. **Given** a request in Draft, **When** `approve` is invoked, **Then** Core returns its
   ordinary precondition denial, and there is no RequestFlow-specific outcome.
4. **Given** the lowered module, **When** it is inspected, **Then** it contains:
   - only ordinary entities, enums, actions and expressions;
   - one request entity;
   - one action per creation capability and one per transition, all named
     `<flow id>__<local id>`;
   - nothing that names a RequestFlow or a StateMachine.
5. **Given** the same flow, **When** its transitions are compared with a standalone
   StateMachine built from the same transitions over the same entity and field, **Then** the
   generated transition actions are identical. The flow reuses the StateMachine lowering and
   does not reimplement it.

---

### User Story 2 - The lifecycle guarantee cannot be bypassed (Priority: P1)

A behavior author mixes a request flow with hand-written behavior. The build rejects any
hand-written creation of the request entity, any write of the status outside the flow's
transitions, and any write of the requester after creation. Entry states are exactly the
creation targets.

**Why this priority**: Without these rules the lifecycle theorem does not hold, and RequestFlow
would be a set of helpers rather than a model.

**Independent Test**: Build the flow together with hand-written actions that create a request,
force a status, rewrite the requester, and update the assignee. Check that the first three are
rejected with exact errors and the fourth is accepted.

**Acceptance Scenarios**:

1. **Given** creations `create_draft → Draft` and `import_submitted → Submitted`, **When** the
   flow is built, **Then** the embedded state machine's entry states are exactly
   {Draft, Submitted}, and no separate entry-state declaration exists or is accepted.
2. **Given** a hand-written action that creates the request entity, **When** the module is
   built, **Then** the build fails with an unauthorized-creation error naming the action.
3. **Given** a hand-written action, a creation's extra effects or a transition's extra effects
   that write the status, **When** the module is built, **Then** the build fails.
4. **Given** any ordinary action, including the flow's own transitions and creations, that
   writes the requester of an existing request, **When** the module is built, **Then** the
   build fails with a requester-immutability error.
5. **Given** a transition or hand-written action that sets the assignee, **When** the module
   is built, **Then** it is accepted: the assignee has no ownership rule.
6. **Given** a creation initializer that sets the status, omits a payload field, sets an
   unknown field, or omits the requester, **When** the flow is lowered, **Then** lowering fails
   with an error naming the creation and the field.

---

### User Story 3 - Explicit authority, data, time and notifications (Priority: P2)

An author expresses who may do what only through explicit guards. They put domain data in
typed payload fields, take time from explicit input or context, and emit notifications as
ordinary command intents on creations and transitions.

**Why this priority**: RequestFlow must stay free of hidden semantics. Authority, time and
delivery are where hidden behavior most easily creeps in.

**Independent Test**: Build a flow whose `approve` requires `context.actor == request.assignee`,
whose `cancel` requires `context.actor == request.requester`, whose creation stores
`created_at = context.now`, and which emits a command on creation and on approve. Check the
allowed and denied outcomes and the emitted intents through the public engine.

**Acceptance Scenarios**:

1. **Given** a request whose requester is A and assignee is B, **When** A invokes `approve`
   on a flow without an approve guard, **Then** the transition commits. Requester and assignee
   grant and deny nothing by themselves.
2. **Given** `approve` guarded by `context.actor == request.assignee`, **When** A and then B
   invoke it, **Then** A is denied and B's attempt commits.
3. **Given** a creation guard that is false or fails to evaluate, **When** the creation is
   invoked, **Then** Core's ordinary denial or evaluation-error outcome is returned and no
   request exists.
4. **Given** commands declared and emitted on creation and on approve, **When** each commits,
   **Then** the intents are part of the same commit. A denied step emits nothing.
5. **Given** a timestamp payload field initialized from context, **When** two creations run
   with different context times, **Then** each request stores its own given time. The lowered
   module contains no clock.

---

### User Story 4 - Deterministic identity, provenance and reproducibility (Priority: P2)

An auditor or tool needs:
- a stable identity for each flow (RequestFlowHash);
- a receipt linking it to the executed module (BehaviorHash);
- a source map that leads from each generated action back through the embedded state machine
  to the flow construct that produced it.

The same flow must always produce the same module, independent of declaration order.

**Why this priority**: RequestFlow inherits feature 501's reproducibility contract. Without it,
nothing built on RequestFlow can be audited.

**Independent Test**: Build the reference flow with every unordered collection permuted and with
only metadata changed. Compare RequestFlowHash and BehaviorHash, and check the source map and the
compatibility gate.

**Acceptance Scenarios**:

1. **Given** two declarations differing only in the order of payload fields, creations,
   transitions, input or context parameters or source states, **When** both are built, **Then**
   RequestFlowHash, the module and BehaviorHash are identical.
2. **Given** declarations differing only in display names, descriptions or source locations,
   **When** both are built, **Then** both hashes are identical.
3. **Given** a semantic difference (payload type, creation target, initializer, guard,
   transition, assignee support), **When** both are built, **Then** RequestFlowHash differs.
4. **Given** a build, **When** the receipt is inspected, **Then** it holds RequestFlowHash,
   model kind, model semantic version, lowerer version, Core Release, BehaviorHash, the
   canonical flow document (which includes the embedded state machine's identity) and a source
   map. Each transition action maps to both its flow transition and its state-machine
   transition, and each creation action to its flow creation.
5. **Given** the compatibility manifest, **When** a change alters a reference flow's lowered
   output without a version bump, **Then** the gate fails.

---

### User Story 5 - Flows compose with each other, with state machines and with hand-written behavior (Priority: P3)

Several flows, standalone state machines and hand-written behavior live in one module. Their
order doesn't matter, and every collision is an explicit error.

**Why this priority**: Real systems hold several request types. It reuses feature 501's
composition infrastructure.

**Independent Test**: Compose `purchase_request`, `leave_request` (both with `submit`), a
standalone state machine on another entity, and hand-written reads over the request entities,
in every order. Then try a duplicate flow id, a creation/transition id collision, and a
hand-written item named like a generated one.

**Acceptance Scenarios**:

1. **Given** two flows that both define `submit`, **When** they are composed, **Then** both
   `purchase_request__submit` and `leave_request__submit` exist.
2. **Given** a flow whose creation id equals one of its transition ids, **When** it is built,
   **Then** the build fails with a capability collision. Nothing is renamed.
3. **Given** a flow and a standalone state machine with the same id, or two flows with the same
   id, **When** they are composed, **Then** the build fails with a duplicate-model error.
4. **Given** hand-written reads, actions or other entities that reference the generated
   request entity, **When** the module is built, **Then** they are accepted (subject to the
   ownership rules of User Story 2).
5. **Given** a declaration elsewhere with the generated entity's name, **When** the module is
   built, **Then** the build fails with a declaration collision.

---

### Edge Cases

- **No creation capability**: blocking error (a flow needs at least one entry).
- **Two creations with the same target status**: allowed. Entry states are a set.
- **A status no creation targets and no transition reaches**: a non-blocking StateMachine
  diagnostic (`UNUSED_STATE` or `UNREACHABLE_STATE`), never an error.
- **Cycles** (`NeedsChanges → Submitted`): allowed. RequestFlow is not a DAG abstraction.
- **Payload field named `requester`, `status` or `assignee`** (the latter only with assignee
  support): blocking error.
- **Assignee disabled, but an initializer or effect sets `assignee`**: blocking error (unknown
  field).
- **A creation with a dynamic initial status**: not expressible. The initial status is a fixed
  part of the creation specification.
- **Request identity**: always explicit creation input. Freshness (an unused identity) is
  Core's ordinary creation semantics. RequestFlow generates no identities.
- **Creation's extra effects**: they may change other existing entities, but never the status
  or requester of the request being created. Creating a second request of the same flow in the
  same action is rejected.
- **Removal**: not a transition. Ordinary Core removal semantics apply unchanged. Domains that
  keep records model Cancelled, Archived or Closed as statuses.
- **Migrations**: separate Core semantics, outside composition. They are not RequestFlow
  creation.
- **Actor existence**: a requester or assignee need not exist as an entity, and removing an
  actor is never blocked by requests. `exists(...)` guards give point-in-time checks; persistent
  integrity needs an explicit `Ref` relationship outside the built-in fields.
- **A guard that fails to evaluate** (creation or transition): Core's ordinary evaluation error.
- **Core change apparently needed**: stop, and classify it as missing public facade exposure
  or genuinely missing Core semantics (as in feature 501). No Python workaround.

## Requirements *(mandatory)*

### Functional Requirements

**Model and schema**

- **FR-001**: A RequestFlow MUST be resolved, validated, identified and lowered by the existing
  Rust model authority (`behavior-models`), as a new model kind. Bindings only construct input
  and trace Core fragments in scopes the authority opens. Python MUST NOT implement creation
  normalization, entry-state derivation, RequestFlowHash, transition lowering, capability
  naming or ownership rules.
- **FR-002**: Each flow MUST generate exactly one request entity type. Its name is
  `EncodeEntityName(flow id)`, a canonical injective encoding (Clarifications). It never comes
  from display names, source locations, declaration position or an author-supplied name. The
  request entity is the only declaration a flow generates. The entity name is derived, so
  RequestFlowHash does not hash it as a separate input.
- **FR-003**: The request entity MUST have:
  - `requester: Id<A>`;
  - `status` of the flow's status enum;
  - `assignee: Option<Id<A>>`, if and only if assignee support is configured;
  Requester and assignee are identities, never `Ref`. RequestFlow adds no existence or removal
  rule for actors (Clarifications).
  - one field per payload entry with its declared type.
  The request's identity is Core entity identity and is not duplicated as a field.
- **FR-004**: The status field MUST reference an author-declared nominal Core enum `T_s`,
  which the flow does not generate, copy or rename. The status domain is `Q := Cases(T_s)`, as
  in StateMachine, and no separate state set exists. RequestFlow owns the generated request
  entity, not the enum type. `T_s` may be shared with other flows, state machines and fields.
- **FR-005**: Payload is a finite mapping `FieldId → CoreType`. Field identities MUST NOT
  collide with the reserved fields `requester`, `status` and (with assignee support)
  `assignee`. RequestFlow MUST NOT impose mutability rules on payload fields.
- **FR-006**: The generated declarations MUST be available to ordinary hand-written behavior
  in the same module: reads, queries, actions and references from other entities. Composition
  MUST treat them as contributed declarations under the identical-or-error rule (feature 501,
  composition contract §2.3). A same-named declaration is an error, never an overwrite.

**Creation**

- **FR-007**: A flow MUST declare at least one creation. Each creation has a local semantic id,
  a fixed initial status `q_c ∈ Q`, state-binding, input and context parameter mappings
  (`S_c`, `I_c`, `C_c`), and ordered Core fragments for guards, extra effects, postconditions
  and command emissions. `S_c` binds existing entities only. The request being created is
  never in `S_c`, and aliasing follows Core's ordinary binding rules. Extra effects `E_c`
  target only `S_c`. It also has an
  initializer `V_c` that gives the requester, every payload field and, with assignee support,
  the assignee.
- **FR-008**: Each creation MUST lower to one ordinary action `encode(flow id, creation id)`:
  - it takes the new request identity as an explicit input parameter;
  - its preconditions are `G_c`;
  - it creates the request with `status = lit(q_c)` (generated, exactly once) plus the
    initializer's values;
  - it applies `E_c` (over `S_c`), then checks `P_c`, then emits `K_c`. The creation and
    `E_c` are one atomic Core transition.
- **FR-009**: The initializer MUST set every required generated field exactly once, MUST NOT set
  `status`, and MUST NOT name unknown fields. Violations are blocking errors naming the creation
  and the field.
- **FR-010**: The creation's extra effects MUST NOT write the status or requester of any
  request of the flow. A creation action MUST NOT create a second entity of the flow's request
  type.
- **FR-011**: Ordinary creation of the request entity type MUST originate only from the flow's
  creation actions. Any other ordinary creation of that type is a blocking composition error.
  This is stronger than standalone StateMachine, which does not own creation.

**Entry states and the embedded StateMachine**

- **FR-012**: Entry states MUST be derived as `Q_entry := { q_c | c ∈ 𝒞 }`. A flow MUST NOT
  accept a separate entry-state declaration.
- **FR-013**: Each flow MUST produce exactly one StateMachine
  `SM(RF) = (E, status, Q_entry, 𝒯)` with the flow's semantic id as its machine id. That
  machine MUST be resolved and lowered by the existing authoritative StateMachine
  implementation. RequestFlow MUST NOT reproduce StateMachine normalization, transition
  validation, ownership, state-field lowering or transition action generation.
- **FR-014**: Transitions MUST have exactly StateMachine semantics:
  `Enabled_t = Existing(r) ∧ status(r) ∈ F_t ∧ ⋀G_t`, with no additional hidden predicate.
  Transition inputs, context, guards, effects, postconditions and commands are as in
  StateMachine.
- **FR-015**: The embedded StateMachine's ownership MUST apply. Only its transitions write
  `status` after creation, and the composition pass checks every write (feature 501, FR-017).

**Requester and assignee**

- **FR-016**: The requester MUST be initialized only by the creation's explicit typed
  expression. After creation, no ordinary action may write it, including the flow's own
  transitions and creations. A violation is a blocking composition error.
- **FR-017**: Requester and assignee MUST NOT grant or deny anything by themselves. Every
  authority rule is an explicit guard.
- **FR-018**: With assignee support, the assignee MUST be an ordinary field with no ownership
  rule. It may be initialized at creation and changed by any otherwise valid effect.

**Names, fragments and time**

- **FR-019**: Creation ids and transition ids MUST be semantic ids (feature 501 grammar) and
  mutually unique within a flow. Generated action names are `encode(flow id, local id)`. A
  collision is a blocking error, and nothing is renamed.
- **FR-020**: Every open-ended part MUST be a typed ordinary Core fragment, traced in a scope
  the authority opens with parameter names it chooses: creation guards, initializer
  expressions, transition guards, extra effects, postconditions and command emissions. No
  RequestFlow expression language exists.
- **FR-021**: RequestFlow MUST NOT read a clock or generate identities. Time values and request
  identities come only from explicit input or context.
- **FR-022**: RequestFlow MUST NOT add reads, queries, schedules, automatic progression,
  approval quorums, history logs, delivery semantics or any runtime outcome.

**Identity, composition and reproducibility**

- **FR-023**: Each flow MUST have a RequestFlowHash: a tagged hash of a canonical document. The
  document contains the flow id, actor type, generated schema, the semantic Core identity of
  `T_s` (its enum hash, never a copy of its cases), payload fields and types, assignee configuration, every creation with its fragments' Core semantic
  identities, and the embedded StateMachine's canonical document or ModelHash, not a second
  encoding of StateMachine semantics. Display names, descriptions, source locations and
  declaration order are excluded.
- **FR-024**: Payload fields, creations, transitions, input and context mappings and
  source-state sets MUST be canonicalized by semantic identity. Their declaration order MUST
  NOT affect RequestFlowHash, the embedded StateMachine's ModelHash, BehaviorHash or generated
  capability order.
- **FR-025**: A successful build MUST return exactly one receipt per supplied model, so one
  per flow. The embedded state machine gets no receipt of its own. The flow receipt has the
  receipt fields of feature 501, an embedded-model identity record for the embedded machine
  (kind, id, semantic version, ModelHash), and a source map. The source map maps creation actions to flow creations, and
  each transition action to both its flow transition and its state-machine transition. It maps
  generated fields to the flow's schema. Source locations are checkout-independent.
- **FR-026**: RequestFlow MUST have its own model semantic version and lowerer version. The
  compatibility manifest MUST pin each reference flow's ModelHash, embedded state-machine
  ModelHash and BehaviorHash per version tuple, as one entry, enforced by the existing
  `models compat` gate.
- **FR-027**: Composition MUST be order-independent and MUST report explicit errors for:
  duplicate model ids (flows and state machines share one id namespace), generated
  declaration collisions, generated capability collisions, unauthorized creation, unauthorized
  status writes and unauthorized requester writes.

**Binding and boundaries**

- **FR-028**: The Python binding MUST let a flow and its creations and transitions be declared
  readably, including payload, assignee support, initializers, guards, effects, postconditions
  and command emissions. It MUST expose a typed handle to the generated request entity (for
  example `flow.request_entity`) that is usable in hand-written Python behavior as a parameter
  type and in `Id[...]`. Authors never write the generated name.
- **FR-029**: This feature MUST NOT change behavior-ir-core. If a construct seems to need one,
  work stops and the gap is classified as missing public facade exposure or genuinely missing
  Core semantics.
- **FR-030**: Before implementation, planning MUST establish with evidence that:
  - the public engine can build the generated entity and creation actions;
  - unauthorized creation and requester writes are detectable on the admitted module;
  - the StateMachine lowerer can be invoked directly;
  - fragments stay Core fragments;
  - generated names are injective;
  - no Core change is needed.

### Key Entities

- **RequestFlow**: `(id, A, E, Q, Π, 𝒞, 𝒯)`. A model kind of the model authority.
- **Request entity**: the generated entity: requester, status, optional assignee and payload.
- **Creation specification**: `(id_c, q_c, S_c, I_c, C_c, G_c, V_c, E_c, P_c, K_c)`. It
  lowers to one atomic creation action. `S_c` binds existing entities that `E_c` may change.
- **Initializer** (`V_c`): typed Core expressions for requester, payload and optional assignee.
  It never includes status.
- **Embedded StateMachine**: `SM(RF) = (E, status, Q_entry, 𝒯)`, lowered by the existing
  StateMachine implementation.
- **RequestFlowHash and receipt**: the flow's content identity and provenance. One receipt per
  supplied flow, with an embedded-model identity record (not a receipt) for its state machine.
- **Reference flows**: the flows pinned by the compatibility manifest.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The reference lifecycle (create, submit, start_review, return, resubmit,
  start_review, approve, plus cancel) runs end-to-end through the public engine, and 100% of
  steps produce ordinary Core outcomes.
- **SC-002**: For every status × transition pair of the reference flow, a transition commits
  iff the status is in its source set and its guards hold. For every creation, the new request
  has exactly its declared initial status. Both hold in 100% of cases.
- **SC-003**: Across ≥ 1,000 randomized valid invocation traces, every observed status change is
  an edge of `R_T`, and every trace starts in `Q_entry`.
- **SC-004**: 100% of bypass attempts are rejected at build time with errors naming the
  offending action and field, and no module is produced. The attempts are: hand-written
  creation of the request type, status write outside transitions (hand-written, creation
  effect, transition effect), requester rewrite, initializer setting status, and a missing or
  unknown initializer field.
- **SC-005**: Across ≥ 1,000 randomized permutations of every unordered collection, and of the
  order in which flows, state machines and hand-written behavior are supplied, RequestFlowHash,
  the module and BehaviorHash are identical.
- **SC-006**: The transition actions of a flow are identical, by Core action hash, to those of
  a standalone StateMachine with the same transitions over the same entity and field.
- **SC-007**: The lowered modules contain zero RequestFlow- or StateMachine-specific constructs
  and are admitted by the public engine. behavior-ir-core is unchanged.
- **SC-008**: Every blocking error this feature introduces has at least one test producing it.
- **SC-009**: The compatibility gate rejects 100% of tested unversioned changes to a reference
  flow's lowered output.
- **SC-010**: A property test over the complete flow-id grammar (≥ 1,000 cases, including
  digit-led runs) shows that `EncodeEntityName` is injective, decodable and always a valid Core
  identifier.

## Assumptions

- RequestFlow is a new model kind in the existing `behavior-models` crate and reuses its
  infrastructure: semantic ids, encoding, error catalogue, composition, ownership pass, identity,
  receipts, compatibility gate and Python glue.
- This is the first model kind that contributes a declaration: the request entity only, since
  the status enum is authored (FR-004). The feature 501 protocol is extended accordingly during
  planning: authored declarations plus model-produced declarations form the declaration
  environment; then hand-written behavior is traced and model behavior lowered; then
  composition. StateMachine's semantics and pinned hashes do not change.
- The actor type `A` is an ordinary entity declared by the author. RequestFlow references it
  and never generates it.
- The request identity input parameter has a fixed, model-semantic name (for example
  `request_id`), so it can be referenced in guards and is part of the capability interface.
- Requester immutability covers ordinary actions only. Migrations remain separate Core
  semantics.
- The `request_flow_reference` test generator from feature 501 stays as a StateMachine
  composition test. The real RequestFlow gets its own reference flows in the manifest.
- Out of scope (non-goals): workflow runtime, BPMN, automatic progression, timers, scheduling,
  parallel branches, subflows, retries, sagas, external jobs, implicit role lookup or
  authorization, approval quorums, comments, attachments, own history log, reads/views, and a
  second expression language.
