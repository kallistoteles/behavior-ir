# Feature Specification: Approval Model

**Feature Branch**: `503-approval-model`

**Created**: 2026-10-13

**Status**: Draft

**Input**: User description: "Feature 503 — Approval Model. A model that records immutable actor
decisions (APPROVE / REJECT) about an existing subject and derives whether the subject has
reached an approval or rejection threshold. Approval represents decisions and aggregation; it
never moves a RequestFlow or StateMachine between states itself. Approval determines a fact;
StateMachine decides what may change because of that fact. No Core change expected."

**Depends on**: feature 502 (RequestFlow and declaration staging), branch `502-request-flow`
(PR #17). This branch is stacked on it.

## Context

Approval is a Behavior Model for collecting decisions about a subject that already exists,
such as a purchase request, a document or a release. It lowers completely to ordinary Behavior
IR:

```text
Approval
    ├── DecisionChoice enum   { APPROVE, REJECT }        (generated)
    ├── Decision entity       subject, actor, choice, payload   (generated)
    ├── derived predicates    approve_count, reject_count, pending, approved, rejected
    └── approve / reject actions  → each creates exactly one Decision and carries the
                                    validity laws as child-state checks (postconditions):
                                    one decision per (subject, actor); thresholds not both reached
                    ↓
             ordinary Behavior IR → behavior-ir-core (unchanged)
```

```text
AM = (id, S, A, Π, k_A, k_R, Open, Eligible, 𝒦)
  id        stable semantic approval identity
  S         subject entity type            A    actor identity type (Id<A>, never Ref<A>)
  Π         payload schema FieldId → CoreType
  k_A ≥ 1   approval threshold             k_R ∈ ℕ>0 ∪ {⊥}   optional rejection threshold
  Open      authored Core predicate: is the decision window open for the subject
  Eligible  authored Core predicate: may this actor decide on this subject
  𝒦         the two decision capabilities {approve, reject}

d = (id_d, subject_d, actor_d, choice_d, payload_d)     subject_d ∈ Id(S), actor_d ∈ Id(A)
D(s)  = { d | subject_d = s }
A(s)  = |{ d ∈ D(s) | choice_d = APPROVE }|          R(s) = |{ d ∈ D(s) | choice_d = REJECT }|

Outcome(s) = REJECTED  if k_R ≠ ⊥ ∧ R(s) ≥ k_R
             APPROVED  else if A(s) ≥ k_A
             PENDING   otherwise

CanApprove(s, a) = Exists(s) ∧ Open(s) ∧ Pending(s) ∧ Eligible(s, a) ∧ ¬HasDecision(s, a) ∧ G_A(s, a)
CanReject(s, a)  = Exists(s) ∧ Open(s) ∧ Pending(s) ∧ Eligible(s, a) ∧ ¬HasDecision(s, a) ∧ G_R(s, a)
                   (shared policy: Open, Eligible; choice-specific policy: G_A, G_R)
Δ_c = CreateEntity(Decision, (decision_id, id(s), a, c, payload))     and nothing else (plus command intents)

L_Approval(AM) = Compose( Decl(AM), Defs(AM), Actions(AM) )
  Defs = the five derived values (the queries);  Inv(AM) is carried by Actions(AM) as their
  child-state checks (FR-007, FR-008), not as separate module-wide rules
Meaning_Approval(AM) := Meaning_Core(L_Approval(AM))
```

**Validity laws** (every valid ordinary Approval state):

- **Uniqueness:** `∀ s, a: |{ d | subject_d = s ∧ actor_d = a }| ≤ 1`.
- **Threshold consistency:** `¬(A(s) ≥ k_A ∧ k_R ≠ ⊥ ∧ R(s) ≥ k_R)`.

**Central guarantees.**

- **Approval soundness.** In a valid state, `approved(s) ⇒` at least `k_A` *distinct* actors
  have recorded APPROVE for `s`. This follows from uniqueness: each actor contributes at most
  one decision.
- **Terminal stability.** If `Outcome_σ(s) ≠ PENDING` and `σ →* σ'` by ordinary behavior, then
  `Outcome_σ'(s) = Outcome_σ(s)`. Proof: decisions are never modified or removed by ordinary
  behavior, and no new decision for `s` can be created once the outcome is terminal. So
  `D_σ'(s) = D_σ(s)`, and the counts and the outcome are unchanged.
- **Veto.** With `k_R = 1`, one valid rejection makes the outcome REJECTED. By terminal
  stability, no later approval can reverse it. Veto needs no special primitive.

Governing principles, used as review tests:

- Decisions are state; outcome is derived.
- Approval does not own the subject's lifecycle.
- Authorization is explicit behavior, not implied by the word "approver".
- Recorded decisions are immutable audit facts under ordinary behavior.
- Eligibility is checked when a decision is recorded; it is not silently re-evaluated later.
- Approval outcome may guard another model, but never triggers that model implicitly.
- One semantic kernel, many authoring models.
- Concurrency control prevents stale transitions from composing accidentally; validity
  invariants ensure that no execution path may commit a semantically impossible Approval state.
- Different semantic origins may collide; collisions are errors, never implicit renaming.

**Terminology (locked).**

| Term | Meaning |
|---|---|
| DENIED | the invocation or its evaluation did not permit the action (precondition denial, invalid resulting state, or commit conflict) |
| REJECT | a recorded decision choice |
| REJECTED | the derived Approval outcome |

**Three-layer terminality.** Consider `A(s) = k_A − 1` and `R(s) = k_R − 1`, so the outcome is
PENDING. Actor 1 approves and actor 2 rejects, and both evaluate against the same parent state
`S`. If both results could commit against `S`, the child would have `A(s) = k_A` and
`R(s) = k_R`, so both `approved(s)` and `rejected(s)` would hold, which Approval forbids. Three
protections apply together. None of them is an alternative to the others:

| Situation | Protection | Outcome |
|---|---|---|
| Fresh second decision, evaluated on the new head after a terminal commit | terminal guard `Pending(s)` | DENIED |
| Stale concurrent commit, evaluated on the same parent `S` as an already-committed decision | Core's concurrency contract: `Commit(C₁, S) = Success ⇒ Head ≠ S ⇒ Commit(C₂, S) = Conflict` | conflict, nothing committed |
| Any ordinary transition producing a child with both thresholds reached | resulting-state validity: the child-state check of the decision action, which with structural ownership is the only ordinary writer of decisions (FR-008b) | the child state is refused |

The same layering protects uniqueness: the `¬HasDecision` guard, then the concurrency contract,
then the uniqueness validity law.

The two propositions `Outcome(s) = APPROVED` (Approval semantics) and `status(s) = Approved`
(RequestFlow/StateMachine semantics) are intentionally different. They may differ for a while:
the outcome is APPROVED while the request is still IN_REVIEW, until someone explicitly invokes
the transition. That state is valid.

## Clarifications

### Session 2026-10-13

- Q: Which words name a refused attempt, a negative decision and a negative outcome? → A: Locked
  terminology: DENIED = the invocation or its evaluation did not permit the action;
  REJECT = a recorded decision choice; REJECTED = the derived Approval outcome. "Rejected" is
  never used for a refused attempt.
- Q: Is preventing a state with both thresholds reached under concurrency a choice between
  Core's commit semantics and a validity invariant? → A: No. All three protections are
  required, and planning shows why together they suffice: (1) the terminal guard denies a
  fresh second decision on the new head; (2) Core's concurrency contract lets at most one commit
  succeed against a given parent state (`Commit(C₁, S) = Success ⇒ Head ≠ S ⇒
  Commit(C₂, S) = Conflict`), and planning verifies that this is Core's stated contract, not an
  implementation effect; (3) the threshold-consistency law is resulting-state validity (MUST,
  not "where expressible") as a backstop for any execution or storage path. Principle:
  concurrency control prevents stale transitions from composing accidentally; validity
  invariants ensure that no execution path may commit a semantically impossible Approval state.
- Q: How must the decision guards relate to Core's observed evidence under concurrency? → A:
  Planning MUST demonstrate, using the current Store/Query contract, that every fact affecting
  `Pending`, `HasDecision`, `ApproveCount` and `RejectCount` is part of the observed evidence
  or concurrency validation of a decision action. A decision action reads exactly the queries
  its own created entity changes, so this is the first model that stresses query semantics
  under writes. If it cannot be shown, implementation stops.
- Q: How are model declaration dependencies ordered, and who owns name collisions? → A: Model
  declarations form a DAG `G_D = (M, E)`, where `M_i → M_j` means `M_j` needs a declaration
  produced by `M_i` (for example `RequestFlow → Approval`, because
  `Decision.subject : Id<RequestFlowEntity>`). The build proceeds only if `G_D` is acyclic. Ready
  nodes are processed in canonical semantic model-id order, so supply and Python iteration order
  never matter. The collision rule ("different semantic origins may collide; collisions are
  errors, never implicit renaming") belongs to the common declaration composition, not to
  Approval.
- Q (raised in planning): Core module invariants cannot express either law (a query predicate
  cannot contain another query, per-entity invariants cannot query, and `unique` takes one
  scalar key with no tuples). How is resulting-state validity provided? → A: Action-scoped
  child-state checks, documented as an inductive preservation proof, not as a workaround. A
  decision action changes `Count(s, a)` only for its own `(s*, a*)` and the counts only for its
  own `s*`, and structural ownership excludes every other ordinary writer. So the local checks
  `Count_S′(s*, a*) ≤ 1` and `¬(A_S′(s*) ≥ k_A ∧ R_S′(s*) ≥ k_R)` preserve both global laws:
  `P(S₀) ⇒ ∀n: P(S_n)` over ordinary reachable states. The base-state assumption is explicit:
  empty genesis satisfies it, and seeded, imported or migrated decisions must establish it
  separately. No Core proposal is opened. Grouped or module-wide quantification is recorded as
  a possible future generalization, only if further models show the same need ("Core changes
  require semantic necessity, not model convenience").
- Q: Are the v1 simplifications kept? → A: Yes. A decision action is exactly
  `CreateEntity(Decision)` plus command intents, with no other state effects. There is one shared
  `Eligible(subject, actor)` and the choice-specific guards `G_A` and `G_R`, written as
  `CanApprove` and `CanReject` (Context).

## User Scenarios & Testing *(mandatory)*

Terminology (locked, see Context): **DENIED** = the invocation or its evaluation did not permit
the action; **REJECT** = a recorded decision choice; **REJECTED** = the derived outcome. In
scenarios, "is denied" always means DENIED.

### User Story 1 - Record decisions and derive a threshold outcome (Priority: P1)

A behavior author declares an approval over an existing subject type: the actor type, the
approval threshold and an optional rejection threshold. Actors record decisions through the two
generated capabilities `<approval id>__approve` and `<approval id>__reject`, and anyone can read
the derived counts and outcome. Each actor decides at most once per subject, and a terminal
outcome never changes.

**Why this priority**: This is the minimum useful product: a reusable, auditable "enough people
said yes" fact that runs entirely on ordinary Core semantics.

**Independent Test**: Declare `doc_approval` over an authored `Document` entity with actor
`User`. Using the public engine, drive approve/reject sequences under the thresholds
(1, ⊥), (2, ⊥), (2, 1), (3, 2) and (1, 1), and check the counts, the outcome and every denial.

**Acceptance Scenarios**:

1. **Given** `k_A = 1`, **When** one actor approves, **Then** the outcome is APPROVED.
2. **Given** `k_A = 2`, **When** one actor approves, **Then** the outcome is PENDING with an
   approve count of 1.
3. **Given** `k_A = 2`, **When** two different actors approve, **Then** the outcome is
   APPROVED.
4. **Given** an actor who has decided on a subject, **When** the same actor attempts a second
   approval with a fresh decision identity, **Then** the attempt is denied and the counts are
   unchanged.
5. **Given** an actor who has approved, **When** the same actor attempts to reject, **Then**
   the attempt is denied. Decisions cannot be changed in v1.
6. **Given** `k_R = 1`, **When** the first actor rejects, **Then** the outcome is REJECTED
   (veto).
7. **Given** no rejection threshold, **When** actors reject, **Then** their rejections are
   recorded and counted, but the outcome stays PENDING until `k_A` approvals are recorded.
8. **Given** an outcome of APPROVED, **When** any actor attempts to approve or reject, **Then**
   the attempt is denied. The same holds for an outcome of REJECTED.
9. **Given** any sequence of approve and reject attempts, **When** it is executed, **Then** the
   approval and rejection thresholds are never both reached.
10. **Given** the lowered module, **When** it is inspected, **Then** it contains only ordinary
    Core items (enums, entities, derived values and actions with their pre- and
    postconditions) and no item kind, field or runtime construct beyond those. Generated names
    that contain the approval id (for example `PurchaseRequestApprovalDecision`) are ordinary
    names and are allowed. The module is admitted by the public engine.

---

### User Story 2 - Decisions are immutable audit facts owned by the approval (Priority: P1)

A behavior author mixes an approval with hand-written behavior. The build rejects any
hand-written creation of the decision entity, any write of a decision field and any ordinary
removal of a decision. The approval's own actions create exactly one decision and change
nothing else, in particular nothing on the subject.

**Why this priority**: Without these rules the soundness and stability theorems do not hold,
and recorded decisions would not be trustworthy audit facts.

**Independent Test**: Build the approval together with hand-written actions that create a
decision, rewrite a decision's choice, rewrite its payload, and remove a decision. Check that
each is rejected with an exact error. Then compare subject state before and after each
successful decision.

**Acceptance Scenarios**:

1. **Given** a hand-written action that creates the decision entity, **When** the module is
   built, **Then** the build fails with an unauthorized-creation error naming the action.
2. **Given** a hand-written action that writes any decision field (subject, actor, choice or a
   payload field), **When** the module is built, **Then** the build fails with an error naming
   the action and the field.
3. **Given** a hand-written action that removes a decision entity, **When** the module is
   built, **Then** the build fails with an unauthorized-removal error naming the action.
4. **Given** a successful approve or reject, **When** the subject's fields are compared before
   and after, **Then** they are identical, and the only state change is the new decision.
5. **Given** an actor whose eligibility later becomes false, **When** the counts are read,
   **Then** that actor's recorded decision still counts. Eligibility is checked only when a
   decision is recorded.
6. **Given** a decision identity that is already in use, **When** a decision is attempted with
   it, **Then** Core's ordinary freshness rule denies it. The approval generates no
   identities.
7. **Given** an actor identity with no existing actor entity, **When** that actor decides and
   no authored guard requires existence, **Then** the decision is recorded. Actors are
   identities, not references.

---

### User Story 3 - Explicit window, eligibility, payload and commands (Priority: P2)

An author decides when decisions may be made (the open window), who may make them
(eligibility, plus optional per-choice guards), what each decision carries (a typed payload such
as a reason), and which notifications are emitted. All of these are explicit Core fragments.
Nothing is implied by the words "approver", "requester" or "assignee".

**Why this priority**: Approval must stay free of hidden authorization and hidden coupling.
This is where such semantics would most easily creep in.

**Independent Test**: Build an approval with `open_when = subject.status == IN_REVIEW`,
`eligible = actor != subject.requester`, a `reason: Option<String>` payload, a `decided_at`
payload from context, and a command on approve. Check every allowed and denied outcome and
every emitted intent through the public engine.

**Acceptance Scenarios**:

1. **Given** an open-window predicate that is false for the subject, **When** an actor
   attempts a decision, **Then** the attempt is denied and nothing is recorded.
2. **Given** an eligibility predicate that is false for the actor, **When** the actor attempts
   a decision, **Then** the attempt is denied.
3. **Given** an eligibility predicate that fails to evaluate, **When** a decision is attempted,
   **Then** Core's ordinary evaluation-error outcome is returned and nothing is recorded.
4. **Given** an approval without an eligibility restriction, **When** any actor decides,
   **Then** the decision is accepted. The approval adds no implicit rule such as
   `actor != requester` or `actor == assignee`.
5. **Given** two decisions with different payloads but the same choices, **When** the counts
   and the outcome are read, **Then** the payload has no effect on them.
6. **Given** a command emitted on approve, **When** an approve commits, **Then** the intent is
   part of the same commit. **When** an approve is denied, **Then** no intent is emitted.
7. **Given** a `decided_at` payload initialized from context, **When** two decisions run with
   different context times, **Then** each stores its own time. The lowered module contains no
   clock.

---

### User Story 4 - Approval composes with RequestFlow, StateMachine and other approvals (Priority: P2)

An author guards a RequestFlow transition (`IN_REVIEW → APPROVED`) with `approval.approved(request)`,
where the approval's subject is the flow's generated request entity. Approval never invokes the
transition. Several approvals over the same or different subjects coexist in one module.

**Why this priority**: Composition with a lifecycle is the main use. It is also the main new
infrastructure need: one model's generated declaration (the decision entity) depends on another
model's generated declaration (the request entity).

**Independent Test**: Compose `purchase_request` (RequestFlow) with `purchase_request_approval`
(subject = the flow's request entity, `open_when = status == IN_REVIEW`) and guard
`finalize_approved` and `finalize_rejected` with `approved` and `rejected`. Drive the full
cycle. Add a second approval `legal_review` over the same subject, supply the models in every
order, and construct a declaration cycle.

**Acceptance Scenarios**:

1. **Given** a request IN_REVIEW with too few approvals, **When** `finalize_approved` is
   invoked, **Then** it is denied. **After** the threshold is reached, **Then** it commits.
2. **Given** an outcome of APPROVED, **When** no transition is invoked, **Then** the request
   status is still IN_REVIEW. Approval changes no status.
3. **Given** an approval whose subject is a RequestFlow request entity, **When** the module is
   built, **Then** the request entity is declared before the decision entity that references
   it, whatever the order in which the models were supplied.
4. **Given** two approvals that both have `approve` and `reject`, **When** they are composed,
   **Then** four distinct capabilities exist (`<id>__approve`, `<id>__reject` per approval), and
   the two approvals' decisions and outcomes are independent.
5. **Given** models whose generated declarations depend on each other in a cycle, **When** the
   module is built, **Then** the build fails with the same declaration-cycle error, naming the
   models involved, in every supply order.
6. **Given** hand-written reads, guards or actions that use the approval's derived predicates
   or reference the generated decision entity, **When** the module is built, **Then** they are
   accepted (subject to the ownership rules of User Story 2).
7. **Given** a generated name that equals another declaration or capability, **When** the
   module is built, **Then** the build fails with a collision error. Nothing is renamed.

---

### User Story 5 - Deterministic identity, provenance and reproducibility (Priority: P2)

An auditor or tool needs:
- a stable identity for each approval (ApprovalHash);
- one receipt linking it to the executed module (BehaviorHash);
- a source map from each generated action back to the approval construct.

The same approval must always produce the same module, independent of declaration order.

**Why this priority**: Approval inherits feature 501's reproducibility contract. Without it,
nothing built on Approval can be audited.

**Independent Test**: Build the reference approvals with every unordered collection permuted,
with the models supplied in every order, and with only metadata changed. Compare ApprovalHash
and BehaviorHash, and check the receipt, the source map and the compatibility gate.

**Acceptance Scenarios**:

1. **Given** two declarations differing only in the order of payload fields, input or context
   parameters, or the order in which models are supplied, **When** both are built, **Then**
   ApprovalHash, the decision entity, the module and BehaviorHash are identical.
2. **Given** declarations differing only in display names, descriptions or source locations,
   **When** both are built, **Then** both hashes are identical.
3. **Given** a semantic difference (threshold, payload type, subject type, actor type, open
   window, eligibility, a per-choice guard, actor expression, payload initializer or command),
   **When** both are built, **Then** ApprovalHash differs.
4. **Given** a build, **When** the receipts are inspected, **Then** there is exactly one
   receipt per supplied approval, holding ApprovalHash, model kind, model semantic version,
   lowerer version, Core Release, BehaviorHash, the canonical approval document, the identities
   of the generated enum and entity, and a source map from `Approval.<id>.approve` / `.reject`
   to the Core actions.
5. **Given** the compatibility manifest, **When** a change alters a reference approval's
   lowered output without a version bump, **Then** the gate fails.
6. **Given** every StateMachine and RequestFlow reference case already pinned, **When** they
   are built with the extended declaration staging, **Then** every hash is byte-identical to
   its pinned value.

---

### Edge Cases

- **`k_A = 0` or `k_R = 0`**: blocking error. Thresholds are at least 1.
- **No rejection threshold**: rejections are recorded and counted but never make the outcome
  terminal. If no approver ever appears, the outcome stays PENDING; that is a domain matter.
- **Both thresholds reachable by one decision** (for example `k_A = 1, k_R = 1`): not a
  problem. The first committed decision makes the outcome terminal, and every later attempt is
  denied.
- **Concurrent decisions on stale evidence**: two actors evaluate against the same parent state
  and both observe PENDING (for example `A = k_A − 1`, `R = k_R − 1`, one approves, one
  rejects). At most one commits against that parent. The other gets Core's commit conflict and,
  if retried on the new head, is DENIED by the terminal guard. The resulting-state validity
  laws also refuse any child with both thresholds reached or with a duplicate
  (subject, actor) pair. These are three layers, not alternatives (Context).
- **Subject removed after decisions**: the decisions remain and keep their subject identity.
  Core's permanent-identity rule ensures that no later subject reuses that identity, so stale
  decisions never attach to a new subject.
- **Actor removed after deciding**: the decision remains and keeps counting. Actors are
  identities.
- **Open window closes after decisions**: the decisions remain and keep counting. Only new
  decisions are affected.
- **Payload field named `subject`, `actor`, `choice` or `id`**: blocking error (reserved field;
  `id` is Core's implicit entity identity).
- **Author parameter named `subject` or `decision_id`**: blocking error (reserved parameter).
- **A decision action with effects on existing state**: not expressible. Decision actions have
  no extra effects in v1.
- **Subject is another approval's decision entity** (a review of a decision): allowed if it
  forms no cycle. A cycle among generated declarations is a blocking error.
- **Migrations, genesis seeds and imports**: separate Core semantics. Decision immutability and
  the Approval laws cover ordinary behavior reachable from a valid Approval base state. A seed,
  import or migration that introduces decisions must establish the laws itself (FR-008b). A
  seeded decision is not a valid Approval state by default.
- **Core change apparently needed**: stop, and classify the gap as "existing Core semantics but
  missing public API" or "genuinely unrepresentable semantic requirement". No Python or Wire
  JSON workaround.

## Requirements *(mandatory)*

### Functional Requirements

**Model and generated declarations**

- **FR-001**: An Approval MUST be resolved, validated, identified and lowered by the existing
  Rust model authority (`behavior-models`), as a new model kind. Bindings only construct input
  and trace Core fragments in scopes the authority opens. Bindings MUST NOT implement counting,
  outcome, thresholds, uniqueness, ownership, ApprovalHash or capability naming.
- **FR-002**: Each approval MUST generate exactly two declarations:
  - an enum `<E>DecisionChoice` with exactly the cases `APPROVE` and `REJECT`;
  - an entity `<E>Decision`.

  Here `E = EncodeEntityName(approval id)`, the injective encoding of feature 502. Within the
  Approval kind the two generated names are injective in the approval id. Names never come from
  display names, source locations, declaration position or Python object identity.
- **FR-003**: The decision entity MUST have exactly:
  - `subject: Id<S>`;
  - `actor: Id<A>`;
  - `choice` of the generated choice enum;
  - one field per payload entry with its declared type.

  Fields are in canonical order by field name, never by authoring order. `subject` and `actor`
  are identities, never `Ref`, and imply no existence or removal rule. The decision's identity
  is Core entity identity.
- **FR-004**: The subject `S` and the actor `A` MUST be entity types in the declaration
  environment. Each may be authored or generated by another model. Approval references them and
  never generates, copies or changes them.
- **FR-005**: Payload is a finite mapping `FieldId → CoreType`. Field identities MUST NOT be
  `subject`, `actor`, `choice` or `id` (Core's implicit entity identity). Payload fields are immutable like every other decision field
  (FR-016).
- **FR-006**: The thresholds MUST satisfy `k_A ≥ 1` and, if present, `k_R ≥ 1`. Otherwise the
  build fails.

**Validity and derived values**

- **FR-007**: The uniqueness law (at most one decision per subject and actor) MUST be lowered
  to ordinary Core resulting-state validity: a child-state check in each decision action for
  its own `(subject, actor)` pair, `Count_S′(s*, a*) ≤ 1`. Together with structural ownership
  (FR-016), this check preserves the global law inductively over every ordinary transition.
- **FR-008**: The threshold-consistency law MUST be lowered to ordinary Core resulting-state
  validity: a child-state check in each decision action for its own subject,
  `¬(A_S′(s*) ≥ k_A ∧ R_S′(s*) ≥ k_R)`. It preserves the global law inductively in the same
  way. It is the backstop of the three-layer terminality (Context), not an alternative to the
  terminal guard or to Core's concurrency contract.
- **FR-008b**: The guarantee MUST be stated and tested as an inductive preservation theorem.
  Let `P(S) = P_U(S) ∧ P_T(S)`. A decision action changes only the group `(s*, a*)` and the
  subject `s*`, and no other ordinary action can create, write or remove a decision (FR-016).
  So `P(S) ⇒ P(S′)` for every ordinary transition, and `P(S₀) ⇒ ∀n: P(S_n)`. Base state: an
  empty decision relation satisfies `P` trivially. Genesis seeds, imports and migrations that
  introduce decisions are separate semantics. They must establish `P` themselves, and they are
  not valid Approval states by default.
- **FR-008a**: Terminality and uniqueness MUST hold under concurrency through all three layers
  together:
  - the generated `Pending` and `¬HasDecision` guards deny decisions evaluated on a head where
    they are false;
  - Core's concurrency contract lets at most one commit succeed against a given parent state;
  - the validity laws (FR-007, FR-008) refuse any child state that violates them.

  Every fact that affects `Pending`, `HasDecision`, `ApproveCount` and `RejectCount` MUST be
  part of the observed evidence or concurrency validation of each decision action.
- **FR-009**: `approve_count(s)`, `reject_count(s)`, `pending(s)`, `approved(s)` and
  `rejected(s)` MUST be defined by the model authority as ordinary Core query and predicate
  semantics, exactly per the Outcome definition, including rejection precedence. Indexes may
  accelerate them but never define them. The outcome is never stored.
- **FR-010**: The derived values MUST be usable as ordinary expressions wherever Core accepts
  them:
  - guards of RequestFlow and StateMachine transitions and creations;
  - hand-written guards, reads and actions;
  - the approval's own open-window and eligibility predicates.

  The binding MUST expose them as typed handles (for example `approval.approved(request)`) that
  produce the authority's definition and nothing else.

**Decision capabilities**

- **FR-011**: Each approval MUST generate exactly two capabilities:
  - `encode(approval id, approve)`, which records APPROVE;
  - `encode(approval id, reject)`, which records REJECT.

  The choice is fixed by the capability and is never caller input.
- **FR-012**: Each decision capability MUST have:
  - the subject as an existing-entity state binding named `subject`;
  - the new decision identity as an explicit input named `decision_id`;
  - the author's input and context parameters (mappings);
  - an explicit, typed actor expression of type `Id<A>` (commonly the context actor), which the
    authority receives as a Core expression and never infers from a parameter name;
  - a payload initializer that sets every payload field exactly once and nothing else;
  - optional per-choice guards (`G_A` on approve, `G_R` on reject) and command emissions `K_c`.

  The actor expression, inputs, context, initializer, guards and commands are per capability.
- **FR-013**: `approve` MUST be enabled exactly when `CanApprove(s, a)` holds, and `reject`
  exactly when `CanReject(s, a)` holds:
  - the subject binds;
  - `Open` holds;
  - the outcome is PENDING;
  - `Eligible` holds;
  - the actor has no decision on the subject;
  - the choice's own guards (`G_A` for approve, `G_R` for reject) hold;
  - the resulting state is valid.

  Evaluation errors follow Core's ordinary semantics. The `Pending` and "no decision yet"
  checks MUST be generated guards (layer 1 of FR-008a). The validity laws (FR-007, FR-008)
  stay authoritative.
- **FR-014**: The state change of a successful decision MUST be exactly one creation:
  `CreateEntity(Decision, (decision_id, id(subject), actor, c, payload))`. Decision
  capabilities MUST NOT write the subject or any other existing entity. They MUST NOT create
  any other entity, and they have no extra effects.
- **FR-015**: Open and Eligible MUST be explicit authored Core predicates over the subject,
  the actor, inputs, context and the current state (through queries). Open may be the constant
  true. Approval MUST NOT add implicit rules. It does not assume `actor == assignee` or
  `actor != requester`, does not look up roles or managers, and does not couple to any
  RequestFlow status.

**Ownership**

- **FR-016**: Approval MUST own creation, every field, and ordinary removal of its decision
  entity. Any other ordinary action that creates a decision, writes a decision field or
  removes a decision is a blocking composition error naming the action (and the field, for
  writes). Approval MUST NOT own, read for ownership purposes or restrict the subject's entity,
  fields or lifecycle.
- **FR-017**: The ownership rules MUST be checked in final composition over the admitted
  module, together with every other model's rules. Approval's rules MUST NOT conflict with
  RequestFlow or StateMachine ownership, because they concern disjoint generated
  declarations.

**Declaration staging**

- **FR-018**: Declaration staging (feature 502 phase P2) MUST support dependencies between
  model-produced declarations. A model's declaration may reference declarations that another
  model produces, as the decision entity references a RequestFlow request entity. Model
  declarations form a DAG `G_D = (M, E)`, where `M_i → M_j` means `M_j` needs a declaration
  produced by `M_i`. Declarations MUST be produced in a topological order of `G_D` in which
  ready nodes are taken in canonical semantic model-id order, never in supply or Python
  insertion order. The common model infrastructure owns this. No model kind orders
  declarations itself.
- **FR-019**: If `G_D` has a cycle, the build MUST fail with a blocking error that names the
  models involved and is the same in every supply order.
- **FR-020**: P2 MUST accept generated enums as well as generated entities. All of feature
  502's staging laws (D1–D6) MUST continue to hold, including authored forward references to
  generated declarations (D5).
- **FR-021**: The extension MUST NOT change any pinned StateMachine or RequestFlow output. Every
  existing compatibility-manifest case MUST build to its pinned hashes.

**Identity, composition and reproducibility**

- **FR-022**: Each approval MUST have an ApprovalHash: a tagged hash of a canonical document
  that contains at least:
  - the approval id;
  - the semantic Core identity of the subject and actor types (by reference, never a copy);
  - `k_A` and `k_R`;
  - the payload fields and their types;
  - the Core semantic identities of Open, Eligible, the actor expressions, the payload
    initializers, the per-choice guards and the command emissions;
  - the input and context mappings.

  Display names, descriptions, source locations, Python object identity and declaration order
  are excluded. Generated names and schemas are derived, so they are not hashed as separate
  inputs.
- **FR-023**: For a fixed tuple (ApprovalHash, model semantic version, lowerer version, Core
  Release), BehaviorHash MUST be immutable. Approval MUST have its own model semantic version
  and lowerer version. The compatibility manifest MUST pin each reference approval's
  ApprovalHash, its generated declaration identities and its BehaviorHash per version tuple,
  enforced by the existing `models compat` gate.
- **FR-024**: A successful build MUST return exactly one receipt per supplied approval, with the
  receipt fields of feature 501, the identities of the generated enum and entity, and a source
  map from `Approval.<id>.approve` / `.reject` to the generated Core actions. Generated
  declarations get no receipt of their own. Source locations are checkout-independent.
- **FR-025**: Composition MUST be order-independent and MUST report explicit errors for:
  - duplicate model ids (approvals, flows and state machines share one id namespace);
  - generated declaration collisions;
  - generated capability collisions;
  - declaration cycles;
  - unauthorized decision creation, field writes and removal.

  Nothing is ever renamed. The collision rule belongs to the common declaration composition,
  not to Approval: different semantic origins may collide, and every collision is an error.

**Binding and boundaries**

- **FR-026**: The Python binding MUST let an approval be declared readably. That covers the
  subject, actor, thresholds, payload, open window, eligibility, and per capability the actor
  expression, payload initializer, inputs, context, guards and commands. It MUST accept a
  generated entity handle (for example `flow.request_entity`) as the subject. It MUST expose
  typed handles to the generated decision entity and to the derived predicates.
- **FR-027**: Approval MUST NOT read a clock, generate identities, schedule anything, trigger
  transitions of other models, or add delegation, stages, weights, quorum percentages,
  deadlines, escalation, withdrawal, abstention, a history log or any runtime outcome. Time and
  identities come only from explicit input or context.
- **FR-028**: This feature MUST NOT change behavior-ir-core and MUST NOT use raw Wire JSON as
  an interface. If a construct seems to need either, work stops and the gap is classified as
  "existing Core semantics but missing public API" or "genuinely unrepresentable semantic
  requirement".
- **FR-029**: Before implementation, planning MUST establish with evidence that:
  1. the generated enum and entity can be declared entirely through the public engine;
  2. model-produced declarations can depend deterministically on other model-produced
     declarations;
  3. the uniqueness law can be expressed faithfully with existing Core query and validity
     semantics;
  4. the counts can be expressed with existing Core query semantics;
  5. the terminal and "no decision yet" guards observe exactly the query facts Core's
     optimistic concurrency needs;
  6. Core's concurrency contract guarantees that at most one commit succeeds against a given
     parent state (`Commit(C₁, S) = Success ⇒ Commit(C₂, S) = Conflict`). Planning cites the
     contract, not an implementation effect. With layers 1 and 3 (FR-008a), this means two
     concurrent actors cannot both commit, on stale PENDING evidence, a state that violates
     terminality, threshold consistency or uniqueness;
  7. no raw Wire JSON is needed;
  8. decision ownership (creation, writes, removal) is detectable on the admitted module;
  9. the derived predicates can be consumed as RequestFlow and StateMachine guards;
  10. no Core change is required;
  11. using the current Store/Query contract, every fact affecting `Pending`, `HasDecision`,
      `ApproveCount` and `RejectCount` is part of the observed evidence or concurrency
      validation of a decision action.

  If any gate fails, planning stops and classifies the problem before implementation.

### Key Entities

- **Approval**: `(id, S, A, Π, k_A, k_R, Open, Eligible, 𝒦)`. A model kind of the model
  authority.
- **Decision choice enum**: generated, with the cases APPROVE and REJECT.
- **Decision entity**: generated. It holds subject identity, actor identity, choice and
  payload, and is immutable once created.
- **Decision capability**: `approve` or `reject`. It has a fixed choice, an actor expression,
  inputs, context, a payload initializer, per-choice guards and commands, and lowers to one
  action that creates exactly one decision.
- **Derived predicates**: approve_count, reject_count, pending, approved, rejected. These are
  ordinary Core queries and predicates, never stored.
- **Model declaration dependency graph**: model-produced declarations and the generated
  declarations they reference, resolved in a deterministic topological order.
- **ApprovalHash and receipt**: the approval's content identity and provenance. There is one
  receipt per supplied approval.
- **Reference approvals**: the approvals pinned by the compatibility manifest.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The reference composition (purchase request created, submitted, put in review;
  approvals and a rejection recorded; finalize_approved or finalize_rejected) runs end-to-end
  through the public engine. 100% of its steps produce ordinary Core outcomes.
- **SC-002**: For every threshold configuration in {(1, ⊥), (2, ⊥), (2, 1), (3, 2), (1, 1)} and
  every decision sequence of up to 6 attempts by up to 4 actors, the observed outcome equals the
  Outcome definition after every step in 100% of cases.
- **SC-003**: Across at least 1,000 randomized decision sequences (any actors, choices,
  thresholds, open and eligibility results, including concurrent pairs on the same snapshot):
  - no subject ever has two decisions from one actor;
  - no state has both thresholds reached;
  - once the outcome is terminal it never changes;
  - `approved(s) ⇒ A(s) ≥ k_A` and `rejected(s) ⇒ k_R ≠ ⊥ ∧ R(s) ≥ k_R`.

  Each layer is also tested on its own:
  - a fresh second decision is DENIED by the guard;
  - a stale concurrent commit against an already-advanced parent gets a commit conflict;
  - a child state with both thresholds reached, or with a duplicate (subject, actor), is
    refused by validity even when the guards are bypassed.
- **SC-004**: 100% of bypass attempts are rejected at build time with errors naming the
  offending action (and field, where relevant), and no module is produced. The attempts are:
  hand-written decision creation, writes of each decision field, decision removal, a reserved
  payload field, a reserved parameter name, and a zero threshold.
- **SC-005**: For every successful decision in the tests, the subject's state is identical
  before and after, and exactly one new decision exists.
- **SC-006**: Across at least 1,000 randomized permutations of every unordered collection, and
  of the order in which approvals, flows, state machines and hand-written behavior are
  supplied, ApprovalHash, the generated declarations, the module and BehaviorHash are
  identical.
- **SC-007**: Every existing StateMachine and RequestFlow compatibility-manifest case builds to
  its pinned hashes after the staging extension: 100% byte-identical.
- **SC-008**: The lowered modules contain zero Approval-specific constructs, are admitted by
  the public engine, and behavior-ir-core is unchanged.
- **SC-009**: Every blocking error this feature introduces has at least one test producing it,
  and every acceptance scenario above has at least one test.
- **SC-010**: The compatibility gate rejects 100% of tested unversioned changes to a reference
  approval's lowered output.

## Assumptions

- Approval is a new model kind in the existing `behavior-models` crate, registered in the model
  kind registry of feature 502. It reuses semantic ids, `encode`, `EncodeEntityName`, the error
  catalogue, composition, the ownership pass, identity, receipts, the compatibility gate and the
  Python glue.
- Approval is the first model kind whose generated declarations may depend on another model's
  generated declarations, and the first that generates an enum. The feature 502 staging protocol
  is extended accordingly: P2 becomes a deterministic topological pass over model declaration
  dependencies.
- The generated names `<E>DecisionChoice` and `<E>Decision` are injective within the Approval
  kind, because `EncodeEntityName` is injective and the two suffixes cannot be confused. They
  can still equal a name generated by another kind, for example a RequestFlow with id
  `x_decision` and an approval with id `x`. That is a blocking collision error, consistent with
  "never rename".
- Decision capabilities have no extra effects and no extra state bindings in v1. Their only
  postconditions are the two child-state checks of FR-007 and FR-008 (uniqueness always,
  threshold consistency iff `k_R ≠ ⊥`). Open and Eligible read further state through ordinary
  queries. The capability interface
  names `subject` and `decision_id` are fixed and model-semantic.
- One eligibility predicate applies to both choices. Choice-specific restrictions are expressed
  with the per-choice guards `G_A` and `G_R`.
- Decision immutability and the ownership rules cover ordinary behavior only. Migrations remain
  separate Core semantics.
- Out of scope (non-goals): automatic RequestFlow transitions, ordered stages, approval chains,
  delegation, substitute approvers, weighted votes, percentage quorum, "all currently eligible
  users", implicit role or manager lookup, deadlines, escalation, timers, withdrawal, decision
  changes, abstention, notifications beyond ordinary command intents, external command
  execution, a second event log, and a second expression language.
