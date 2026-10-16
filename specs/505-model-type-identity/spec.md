# Feature Specification: Declaration identity of named types in StateMachine and RequestFlow hashes

**Feature Branch**: `505-model-type-identity`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "issue #20" — kallistoteles/behavior-ir#20, *StateMachine and
RequestFlow model hashes ignore the declarations of named parameter types*.

## Context

A behavior model's hash (StateMachineHash, RequestFlowHash) is the content identity of what its
author chose. Together with the model's version tuple (model semantic version, lowerer version,
Core Release), it pins the module the model lowers to: under one tuple, one model hash must
mean one behavior.

Today the StateMachine (feature 501) and RequestFlow (feature 502) documents record each input,
context and payload type by its name only. A parameter of enum type `Mode` is hashed as the word
`Mode`. If `Mode` changes from `{Standard, Fast}` to `{Standard, Slow}` and no guard mentions the
parameter, the model hash stays the same, while the invocations the generated action accepts,
the action's own hash and the module's BehaviorHash all change. Two different behaviors then
share one model hash under one version tuple.

The same holds for the entities a model is about: a StateMachine records its subject entity, and
a RequestFlow its actor entity and its creations' state bindings, by name only.

Feature 503 fixed this for Approval: its subject, its actor and every named type in its payload,
inputs and context are carried with their Core declaration identity. This feature applies that
rule to StateMachine and RequestFlow, so all three model kinds follow one rule:

```text
ModelIdentity = H(Canonical(ModelDefinition, ReferencedDeclarations))
```

A model's identity depends on the exact declarations it references, and on nothing else in the
module. This is deliberately conservative. Adding an unused field to a referenced entity changes
the model hash, even though the model's observable behavior does not change, because the model
hash records exact declaration dependencies, not behavioral equivalence.

## Clarifications

### Session 2026-10-16

- Q: Should the subject, actor and state-binding entities also carry declaration identity, or
  only the parameter and payload types named in issue #20? → A: All type references carry
  declaration identity, consistently for StateMachine, RequestFlow and Approval: subjects,
  actors, state bindings, parameter types and payload types. The dependency is on each
  referenced declaration's identity, never on the whole module schema. Dependency minimization
  and behavioral-equivalence analysis are out of scope.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A StateMachine's hash follows the declarations it references (Priority: P1)

An auditor compares two builds of a state machine by their StateMachineHash. When the subject
entity's declaration changes, or a transition's input or context parameter has an enum, nominal
or entity type whose declaration changes, the hash changes, even if no guard, effect or command
mentions the parameter or the changed field.

**Why this priority**: This is the defect. Without it, equal hashes under an equal version tuple
do not guarantee equal behavior, which is the promise model identity exists to keep.

**Independent Test**: Build the same machine twice, with one difference in the declaration of an
enum used only as a context parameter's type, and compare the two StateMachineHashes.

**Acceptance Scenarios**:

1. **Given** a transition with an unused context parameter of enum type `Mode`, **When** `Mode`'s
   cases change, **Then** the StateMachineHash changes.
2. **Given** a transition with an input of a nominal type, **When** that nominal's declaration
   changes (its operations, scale or underlying type), **Then** the StateMachineHash changes.
3. **Given** a transition with an input of identity type `Id<Customer>`, **When** `Customer`'s
   declaration changes, **Then** the StateMachineHash changes.
4. **Given** the same machine, **When** an enum, nominal or entity that no parameter of the
   machine names changes, **Then** the StateMachineHash does not change.
5. **Given** the same machine, **When** only display names, descriptions, source locations or
   declaration order differ, **Then** the StateMachineHash does not change.
6. **Given** a machine over subject `Order`, **When** `Order` gains a field the machine never
   uses, **Then** the StateMachineHash changes (exact declaration dependency).
7. **Given** a parameter of enum type `Mode`, **When** `Mode` is renamed (same cases, new name),
   **Then** the StateMachineHash changes.

---

### User Story 2 - A RequestFlow's hash follows its actor, payload, input, context and state types (Priority: P1)

The same guarantee holds for a request flow: its actor entity, its payload fields, its creations'
inputs, context and state bindings, and its embedded state machine all carry the declaration
identity of every named type they use.

**Why this priority**: RequestFlow embeds a state machine and adds a payload, so it has the same
defect in more places. It is the model most approvals range over.

**Independent Test**: Build the reference purchase-request flow twice, changing only the
declaration of an enum used as a payload field type (or as a creation's context type), and
compare the RequestFlowHashes.

**Acceptance Scenarios**:

1. **Given** a payload field of enum type, **When** the enum's cases change, **Then** the
   RequestFlowHash changes.
2. **Given** a creation with an unused input of nominal type, **When** the nominal's declaration
   changes, **Then** the RequestFlowHash changes.
3. **Given** a transition of the flow with an unused context parameter of enum type, **When** the
   enum's cases change, **Then** the embedded machine's hash and the RequestFlowHash both
   change.
4. **Given** the same flow, **When** only unrelated declarations or metadata change, **Then** the
   RequestFlowHash does not change.
5. **Given** a flow with actor `User`, **When** `User`'s declaration changes, **Then** the
   RequestFlowHash changes.
6. **Given** a creation with a state binding of entity `Budget`, **When** `Budget`'s declaration
   changes, **Then** the RequestFlowHash changes.

---

### User Story 3 - The change is versioned, pinned and otherwise invisible (Priority: P2)

A maintainer upgrades to this release. The model hashes of StateMachine and RequestFlow change,
so their model semantic versions change and the compatibility manifest pins the new values. What
the models lower to does not change at all.

**Why this priority**: The project's compatibility gate requires a changed model hash to come
with a version change. The guarantee that lowering is untouched is what makes the change safe.

**Independent Test**: Run the compatibility gate and compare every reference case's BehaviorHash
with its value before this feature.

**Acceptance Scenarios**:

1. **Given** every StateMachine and RequestFlow reference case, **When** they are built, **Then**
   every BehaviorHash, generated action hash and generated declaration is byte-identical to its
   value before this feature, and only the model hashes differ.
2. **Given** the three Approval reference cases, **When** they are built, **Then** every
   ApprovalHash and BehaviorHash is unchanged. Only the recorded versions of their co-supplied
   models change.
3. **Given** the compatibility manifest, **When** the gate runs, **Then** it passes with the
   tuples for the new versions. It fails if a pinned model hash is edited, or if the new hashes
   appear without the version change.

---

### Edge Cases

- **Nested and wrapped types**: an `Option` of a named type, and an exact value of a nominal,
  carry the named type's identity like the bare type does.
- **One type in many places**: a type used by several parameters is recorded with each of them;
  the result does not depend on which parameter is visited first.
- **A parameter with only built-in types** (`Bool`, `Int`, `Decimal`, `String`, a dimensionless
  exact value) carries an explicit empty set of named types, so the document shape is the same
  for every parameter.
- **The flow's own request entity**: the subject of its embedded state machine, and possibly a
  parameter type (for example `Id<PurchaseRequest>`). Its declaration identity is recorded like
  any other entity's. It is generated from the flow's payload, so it changes exactly when the
  payload does.
- **The machine's state enum** used again as a parameter type: recorded with the parameter as
  well; its identity is already part of the document through the state field.
- **A reference that does not resolve to exactly one declaration of the expected kind** (missing,
  ambiguous, or naming a declaration of another kind) cannot occur in an admitted module. If it
  did, the build fails with a blocking identity error rather than producing a hash without it.
- **Dependency order**: a model referencing the same declarations in a different order, or a
  module declaring them in a different order, has the same hash.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The StateMachine canonical document MUST record the Core declaration identity of
  its subject entity and, for every input and context parameter of every transition, of every
  named type the parameter's type mentions: enums, nominals and entities, including inside
  `Option` and in exact values of a nominal.
- **FR-002**: The RequestFlow canonical document MUST record the declaration identity of its
  actor entity, of every state-binding entity of every creation, and of every named type
  mentioned by every payload field and every creation's input and context parameters. Its
  embedded state machine follows FR-001.
- **FR-003**: There MUST be one rule for all model kinds, with one authoritative implementation:
  StateMachine, RequestFlow and Approval record named types the same way. Approval's canonical
  document and every ApprovalHash MUST stay byte-identical.
- **FR-004**: Declaration identities MUST be the ones Core assigns to the admitted module's
  declarations, read from the admitted module. The models MUST NOT compute their own type
  identities or copy declarations into their documents.
- **FR-005**: The recorded named types MUST be keyed and ordered deterministically by type name,
  independent of parameter order, declaration order and supply order.
- **FR-006**: The model hash MUST depend on each referenced declaration's identity, and MUST NOT
  depend on declarations the model does not reference (never on the module's whole schema), nor
  on display names, descriptions, source locations or declaration order. Model identity is exact
  declaration dependency: a change anywhere in a referenced declaration, including a field the
  model does not use, changes the model hash. Dependency minimization and behavioral equivalence
  are out of scope.
- **FR-007**: The model semantic versions of StateMachine and RequestFlow MUST each increase,
  because their canonical documents change. Their lowerer versions MUST stay the same, because
  their lowering does not change.
- **FR-008**: What every model lowers to MUST be unchanged: generated declarations, actions,
  derived items and every module's BehaviorHash stay byte-identical for every reference case.
- **FR-009**: The compatibility manifest MUST pin every StateMachine and RequestFlow reference
  case under its new version tuple, and every Approval reference case under its new co-supplied
  versions. The tuples of the previous versions are removed rather than kept alongside: before
  the first stable release there is no backward compatibility (docs/versioning.md), and one
  authoritative tuple per case is kept.
- **FR-010**: Every type reference MUST resolve to exactly one declaration of the expected kind in
  the admitted module. A missing, ambiguous or inconsistent reference MUST fail the build with a
  blocking identity error naming the model, the referring item and the type.
- **FR-011**: An entity reference that is not a parameter type (the StateMachine's subject, the
  RequestFlow's actor, a creation's state binding) MUST be recorded as the entity's name together
  with its declaration identity, in the same shape Approval uses for its subject and actor.
- **FR-012**: This feature MUST NOT change behavior-ir-core and MUST NOT use raw Wire JSON as an
  interface.

### Key Entities

- **Canonical model document**: the content a model's hash is computed from. Each parameter or
  payload field in it has a name, a type and the set of named types it mentions with their
  declaration identities.
- **Declaration identity**: the identity Core assigns to an admitted enum, nominal or entity
  declaration. It changes exactly when the declaration's semantic content changes.
- **Version tuple**: model kind, model semantic version, lowerer version and Core Release (with
  embedded and co-supplied model versions where they apply). It keys the compatibility manifest.
- **Reference case**: a model or composition pinned in the compatibility manifest.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For each of StateMachine and RequestFlow, 100% of the tested changes to a
  referenced declaration change the model hash. The tested changes are: enum cases, nominal
  operations, nominal scale and the fields of an entity used as an identity type, each in an
  unused parameter; an unused field added to the subject (StateMachine), the actor and a
  state-binding entity (RequestFlow); and a renamed parameter type.
- **SC-002**: 0 of the tested unrelated changes (a declaration no parameter names, display names,
  descriptions, source locations, declaration order) change any model hash.
- **SC-003**: 100% of the reference cases keep a byte-identical BehaviorHash, and 100% of the
  Approval reference cases keep a byte-identical ApprovalHash.
- **SC-004**: The compatibility gate passes on the new manifest, rejects 100% of the tested
  edits to a pinned model hash, and rejects a changed model hash under an unchanged version.
- **SC-005**: Across at least 1,000 randomized permutations of parameter, declaration and supply
  order, every model hash is identical (the existing determinism properties keep passing with
  the new documents).

## Assumptions

- The rule and its representation are the ones Approval introduced in feature 503 (a `types` map
  from type name to declaration identity on every parameter and payload field). StateMachine and
  RequestFlow adopt it as is, so the three kinds share one representation.
- The new model semantic versions are 2 for both StateMachine and RequestFlow. A RequestFlow
  embeds a state machine, so its tuple's embedded version also becomes 2.
- The Approval kind keeps its versions. Its compatibility tuples change only because they record
  the versions of co-supplied StateMachine and RequestFlow models.
- Built modules, stores, records and evidence are not affected: model hashes appear only in
  lowering receipts and in the compatibility manifest.
- Core Release 0.13.0 remains the pin. The declaration identities used are already public in the
  admitted module.
- Out of scope: source-location relativization (behavior-ir-core#31), any change to how models
  lower, and new model kinds.
