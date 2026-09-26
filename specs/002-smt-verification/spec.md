# Feature Specification: SMT Verification of Behavior Modules

**Feature Branch**: `002-smt-verification`

**Created**: 2026-09-25

**Status**: Draft

**Input**: User description: "SMT-based verification of behavior modules (the next feature after
001-verifiable-behavior-ir). The engine translates the admitted semantic IR to a solver and proves
properties before a behavior version is used, with no verification annotations written by authors
(PRINCIPLES.md §10): invariant preservation per action with concrete counterexamples;
postconditions; dead actions and impossible preconditions; always-true or always-false rules and
conditions; reachable evaluation errors (division by zero, overflow). Results are deterministic,
cite node hashes, are cached by content hash, are classified as blocking or warnings, model exact
integer and decimal semantics, and report undecided checks as inconclusive, never as passed.
Available from Rust, the CLI, and the Python binding. Process state machines and temporal model
checking, rule-conflict analysis for decision tables, and permissions are out of scope."

## Scope of This Feature

Feature 001 made behavior an admitted, typed, content-addressed module that the engine evaluates
deterministically. Evaluation shows what happens for one input. This feature shows what can
happen for **every** input: the engine derives verification problems from the meaning of
invariants, preconditions, effects, and postconditions and answers them with a solver.

- **In scope**: five checks per behavior version (invariant preservation, postcondition
  validity, satisfiable preconditions, vacuous or redundant conditions, reachable evaluation
  errors); counterexamples as concrete state, input, and context; a verification report with
  blocking findings and warnings, issued as a **verification attestation** bound to the exact
  behavior hash; **waiver attestations** for individual blocking findings; an **execution
  policy** that decides, from the attestations, whether a proposed transition may be committed;
  **entity constraints** (validity rules of a type, checked on every incoming entity in any role
  and on proposed new state, with the new result `INVALID_CONTEXT`) as a construct next to the
  existing state invariants; caching of results by content hash; access from the engine library,
  the command line, and the Python binding.
- **Out of scope (later features)**: process state machines and temporal properties ("PAID only
  after APPROVED"), conflict and gap analysis for decision tables, permissions as their own
  construct, verification across several behavior versions (migrations), and the commit itself
  (persisting a change set belongs to a later data-layer feature; this feature decides whether a
  commit is permitted).

The separation this feature introduces:

```text
BEHAVIOR → evaluate → proposed ΔS + trace
BEHAVIOR → VERIFICATION → findings / proofs → verification attestation
GOVERNANCE → waiver attestations
EXECUTION POLICY (attestations + waivers) → commit permitted or refused
```

Evaluation determines a proposed transition; policy determines whether it may be committed. The
verifier describes what can be proven; governance and policy decide which evidence is required.

| Layer | Answers |
|-------|---------|
| Behavior | what the system may do |
| Verification | what we can prove |
| Governance | what exceptions humans accept |
| Policy | what evidence is required to commit |

## Clarifications

### Session 2026-09-25

- Q: Should evaluation refuse behavior versions that are not verified? → A: No. Evaluation and
  commit/deploy are kept apart. The evaluator always evaluates admitted behavior and produces
  `S × I × C → ΔS + trace`. Verification is a separate attestation bound to the exact behavior
  hash, and an execution/deployment policy decides whether a proposed transition may be
  committed. The decision record contains the behavior hash, the verification status or
  attestation, and the policy outcome.
- Q: Can a reviewer waive a blocking finding? → A: Yes, but a waiver never changes the
  verification result and never makes a version "verified". A waiver is a separate attestation
  (behavior hash, finding hash, verification profile, verifier version, reviewer, rationale,
  created at, optional expiry), i.e. governance evidence that a policy may choose to accept. It is
  bound to the exact behavior hash and finding hash, so a semantic change or a changed finding
  invalidates it automatically. The policy decides which finding kinds are waivable. (Reviewer
  identity is no longer a field of the waiver; see the signature answer below.)
- Q: Should an entity's invariants also hold, and be checked at evaluation, for entities passed as
  input or context, not only for state parameters? → A: Yes, with a distinction between two kinds
  of rules. **Entity constraints** define what a valid instance of a type is (e.g. an employee's
  approval limit is not negative) and apply to every incoming entity regardless of its role:
  invalid state is `INVALID_STATE`, invalid input is `INVALID_INPUT`, invalid context is
  `INVALID_CONTEXT`. **State invariants** remain properties of the system state S and S' only and
  are not applied to input or context. The runtime and the verifier share exactly one contract:
  the verifier may assume exactly what the runtime guarantees — no more, no less.
- Q: How should verification handle inexact decimal division, where the engine rounds to 28
  digits? → A: Model decimal operations conservatively as exact arithmetic plus a rounding-error
  bound derived from the runtime's actual decimal semantics (precision, scale, rounding mode),
  not a hard-coded constant; propagate the bounds through composed expressions. A property is
  proven only if it holds for the whole resulting interval, disproven (counterexample) if it fails
  for the whole interval, and inconclusive if the interval crosses the boundary.
- Q: How is the execution policy described? → A: As a separate declarative, immutable,
  content-addressed governance object (required verification status, waivable and forbidden
  finding kinds, accepted reviewer identities or roles, expiry rules, required verification
  profiles). Its hash is part of every commit authorization, together with the behavior,
  verification, and waiver attestations used. The policy evaluator stays small and
  deterministic. Policy is not modelled as Behavior IR yet, but the object model is kept so that a
  future policy IR can replace the evaluator without changing the audit model. A changed policy
  applies to new commits only; earlier authorizations remain explained by the policy that was in
  force.
- Q: What applies when the same entity (same `id`) is bound to two state parameters in one call?
  → A: Forbidden in this version. Distinct state parameters must bind to distinct entity
  identities; a duplicate is a binding error (`INVALID_BINDING`, reason
  `STATE_ALIAS_NOT_ALLOWED`), not invalid state, since each entity may be valid on its own. The
  verifier may assume pairwise-distinct state identities. Effects are normalized to semantic
  identity (entity type, id, field) so duplicate writes to the same state cell are detectable.
  Explicit aliasing semantics can be added later; hidden aliasing is never allowed.
- Q: How can one trust who wrote a waiver? → A: With signed attestations. A waiver is an
  immutable, content-addressed governance object that states what is accepted and contains no
  reviewer name; identity is proven by a detached Ed25519 signature over the domain-separated
  waiver hash. The execution policy maps trusted public keys to reviewer principals and roles
  and accepts only signatures that satisfy it. Signatures stay outside the waiver's content hash,
  so one waiver can carry several independent signatures. Identity claims are data; authority
  requires evidence.

## User Scenarios & Testing *(mandatory)*

Actors:

- **Behavior author**: writes and changes rules and actions; wants to know before deployment
  whether a change can break the model.
- **Reviewer**: approves behavior changes; wants proof rather than a list of passing examples.
- **Deployment pipeline**: runs verification on every change and refuses to ship blocking
  findings.
- **Auditor**: needs to know whether the behavior version behind a decision was verified, and
  what was proven.

### User Story 1 - Prove that every action preserves the invariants (Priority: P1)

The author runs verification on a behavior module. For each action and each invariant that
applies to its state, the engine asks whether any valid starting state, any input, and any
context can satisfy the preconditions and still produce a new state that breaks the invariant.
If so, the report shows a concrete counterexample: the starting state, input, and context, the
proposed changes, and which invariant fails. If not, the pair is proven.

**Why this priority**: invariants define valid state. Today they are enforced at runtime (a
violating transition is denied), which is safe but surfaces the flaw only when a user hits it.
Proving preservation turns "the engine will refuse it" into "it cannot happen", which is the core
promise of the concept.

**Independent Test**: verify the purchase-approval model from `examples/tryout/demo.py`: the
action that adds the purchase amount to `project.spent` without a budget precondition must be
reported with a counterexample (a purchase larger than the remaining budget); after adding the
precondition `purchase.amount <= remaining(project)`, the same pair must be proven.

**Acceptance Scenarios**:

1. **Given** an action whose effects can break an invariant, **When** verified, **Then** the report
   contains a blocking finding for that action and invariant with a counterexample, and evaluating
   the action on that counterexample yields `DENY` because of the same invariant.
2. **Given** an action whose preconditions guarantee the invariant, **When** verified, **Then** the
   pair is reported as proven.
3. **Given** an invariant over an entity the action does not change, **When** verified, **Then** the
   pair is proven without needing a counterexample search that depends on the action's effects.
4. **Given** the same module verified twice on different machines, **When** the reports are
   compared, **Then** they are identical, including counterexample values.

---

### User Story 2 - Verification attestations, waivers, and the execution policy (Priority: P2)

Verification produces a report and an attestation for the behavior version: **verified** when no
blocking finding remains, otherwise **not verified**. A reviewer may add a waiver attestation for
a specific blocking finding; it never changes the verification result. An execution policy then
decides, for a proposed transition, whether it may be committed, based on the verification
attestation and any waivers it accepts. Checks the solver cannot decide within the
time budget are reported as **inconclusive** and block verification. Results are cached by the
content hashes of the items involved, so after a change only the affected checks run again. The
deployment pipeline uses the status to decide whether a version may ship; decision records made
with a verified version say so.

**Why this priority**: findings are only actionable if they gate something. Attestations make
"verified" a property of a content-addressed version rather than a claim in a pull request, and
the policy keeps governance decisions (such as accepting an inconclusive check) visible and
separate from what was proven.

**Independent Test**: verify a module, change one action, verify again, and confirm that only
checks involving that action are recomputed while the report for the rest is taken from the
cache; confirm the status is "not verified" when a blocking finding or an inconclusive check
exists and "verified" otherwise; add a waiver for an inconclusive finding and confirm the status
stays "not verified" while a policy that accepts such waivers permits the commit, and that the
waiver stops applying after a semantic change.

**Acceptance Scenarios**:

1. **Given** a module with no blocking findings, **When** verified, **Then** its status is
   "verified" and the report lists every check with its outcome.
2. **Given** a check that exceeds the time budget, **When** verified, **Then** it is reported as
   inconclusive and the status is "not verified".
3. **Given** a verified module where only a comment or a rule name changed, **When** verified again,
   **Then** every result comes from the cache and the status is unchanged.
4. **Given** a module where one action changed, **When** verified again, **Then** only checks that
   involve that action are recomputed.
5. **Given** an unverified behavior version, **When** an action is evaluated, **Then** evaluation
   proceeds normally, and the commit authorization for that transition cites the behavior hash,
   "unverified", and the policy hash, with the decision refused under a policy that requires
   verification.
6. **Given** a not-verified version whose only blocking finding is inconclusive and a valid waiver
   for that finding, **When** the policy accepts waivers for inconclusive findings, **Then** the
   verification result is still "not verified", and the policy outcome is "commit permitted",
   citing the waiver.
7. **Given** a waiver for a finding, **When** the behavior changes semantically (new behavior hash)
   or the finding changes (new finding hash), **Then** the waiver no longer applies.
8. **Given** a waiver for a finding kind the policy does not allow to be waived, **When** the policy
   is evaluated, **Then** the waiver is ignored and the commit is refused.
9. **Given** a waiver with an expiry in the past, **When** the policy is evaluated, **Then** the
   waiver is ignored.
10. **Given** a waiver signed by a key the policy does not trust, or with a signature that does not
    verify, **When** the policy is evaluated, **Then** the waiver is ignored.
11. **Given** one waiver signed independently by two trusted reviewers, **When** its content is
    hashed, **Then** both signatures refer to the same waiver hash, and the commit authorization
    records the signature it relied on.
12. **Given** a commit authorization made under one policy, **When** the policy is changed,
    **Then** the earlier authorization still cites and is explained by the old policy hash, and
    new decisions use the new policy.

---

### User Story 3 - No allowed transition can fail its postconditions or hit an evaluation error (Priority: P3)

For each action the engine proves that whenever the invariants hold and the preconditions pass,
the postconditions hold on the new state, and that no expression evaluated along the way can
divide by zero or overflow. Otherwise the report gives a counterexample.

**Why this priority**: a failed postcondition or an evaluation error means an action the
preconditions allowed still ends in `DENY` or `ERROR`. That is a defect in the behavior, found
today only at runtime.

**Independent Test**: verify the project-margin model from feature 001: `flag_project_unguarded`
must be reported with a counterexample where `revenue = 0` (division by zero), while
`flag_project`, whose guard short-circuits before the division, must be proven free of
evaluation errors.

**Acceptance Scenarios**:

1. **Given** an action with a postcondition its effects do not guarantee, **When** verified,
   **Then** a blocking finding shows a counterexample, and evaluating it yields `DENY` because of
   that postcondition.
2. **Given** a division whose divisor can be zero on an allowed path, **When** verified, **Then**
   a finding names the expression and gives a counterexample that yields `ERROR` when evaluated.
3. **Given** a division guarded by a condition that short-circuits, **When** verified, **Then** no
   evaluation-error finding is reported for it.
4. **Given** arithmetic that can exceed the representable range, **When** verified, **Then** a
   finding reports the possible overflow with a counterexample.

---

### User Story 4 - Find actions that can never run and conditions that never matter (Priority: P4)

The engine reports actions whose preconditions can never hold together with the invariants
(dead actions), individual preconditions that are always true given the invariants and the
earlier preconditions (redundant), and rules or conditions that are always true or always false
(vacuous).

**Why this priority**: these findings point at mistakes (an impossible combination such as
`amount > 100 and amount < 50`) and at noise in the rules. They matter, but a dead action cannot
corrupt state, so they come after the safety checks.

**Independent Test**: add an action with preconditions `amount > 100` and `amount < 50` and a
rule that compares a non-negative amount with `>= 0`; verify and confirm a dead-action finding
and a vacuous-rule finding, each with an explanation.

**Acceptance Scenarios**:

1. **Given** an action whose preconditions contradict each other or the invariants, **When**
   verified, **Then** a finding marks the action as dead and names the conflicting conditions.
2. **Given** a precondition implied by the invariants and earlier preconditions, **When** verified,
   **Then** a warning marks it as redundant.
3. **Given** a rule that is true for every valid state, **When** verified, **Then** a warning marks
   it as always true.

---

### Edge Cases

- A module with no invariants or no actions: verification succeeds with an empty set of
  preservation checks and the status "verified".
- Unconstrained text, ids, and enums: the solver may choose any value of the declared type,
  including values not seen in tests; enum values are limited to the declared ones.
- Optional fields: counterexamples may use "none" wherever the type allows it.
- Decimal division that is inexact (e.g. `2 / 3`): the value lies within its exact result plus or
  minus the rounding bound; a rule such as `margin < 0.05` is proven or disproven when the whole
  interval lies on one side of `0.05`, and inconclusive when the interval crosses it.
- Several rounding operations in one expression: their bounds add up along the expression, so a
  deeply composed ratio may be inconclusive where a single division is proven.
- Very large or deeply nested expressions: checks that exceed the time budget are inconclusive,
  never passed.
- A counterexample that does not reproduce when evaluated (a modelling error): verification
  reports an internal error for that check and marks it inconclusive; a wrong proof is never
  reported as passed.
- A counterexample needs an employee with a negative approval limit: if an entity constraint
  forbids that, the verifier cannot use it, because the runtime would reject such a context with
  `INVALID_CONTEXT`; without such a constraint, the counterexample is legitimate and shows that a
  constraint is missing.
- `transfer(from: Account, to: Account)` called with the same account twice: `INVALID_BINDING`
  (`STATE_ALIAS_NOT_ALLOWED`) before any precondition runs; verification never considers that
  case, because it cannot reach evaluation.
- An effect produces a state entity that breaks its entity constraint: the transition is `DENY`,
  and verification reports the action with a counterexample, like an invariant violation.
- A derived value referenced by an invariant: the derived value's definition is part of the
  check; changing it invalidates cached results of every check that uses it.

## Requirements *(mandatory)*

### Functional Requirements

**Checks**

- **FR-001**: For every action and every state invariant or entity constraint that applies to one
  of its state parameters, the system MUST decide whether a valid starting state, a valid input,
  and a valid context exist such that all preconditions hold and the rule fails on the proposed
  new state. "Valid" means exactly what the runtime checks before evaluating (FR-026).
- **FR-002**: For every action and each of its postconditions, the system MUST decide whether the
  postcondition can fail on the proposed new state when the invariants hold on the starting state
  and all preconditions hold.
- **FR-003**: For every action, the system MUST decide whether its preconditions can hold together
  with the invariants on the starting state; if not, the action is dead.
- **FR-004**: The system MUST identify preconditions implied by the invariants and the earlier
  preconditions (redundant), and rules or conditions that are true for every valid state or
  false for every valid state (vacuous).
- **FR-005**: The system MUST decide whether any division by zero or arithmetic overflow can occur
  while evaluating an action from a valid starting state, respecting the engine's evaluation
  order: preconditions in order with short-circuit `and`/`or`, then effects, then postconditions.
- **FR-006**: The checks MUST be derived from the admitted module alone; authors MUST NOT need to
  write verification annotations.

**Results**

- **FR-007**: Every check MUST end as **proven**, **counterexample**, or **inconclusive**
  (time budget exceeded, or a result the model cannot decide exactly).
- **FR-008**: Every counterexample MUST give concrete values for the starting state, input, and
  context, and MUST be confirmed by evaluating it with the engine before it is reported; the
  evaluation's decision record MUST be included in the finding.
- **FR-009**: Findings MUST be classified: invariant violations, postcondition violations,
  reachable evaluation errors, and inconclusive checks are **blocking**; dead actions, redundant
  preconditions, and vacuous rules or conditions are **warnings**.
- **FR-010**: A behavior version MUST be marked **verified** only when it has no blocking finding.
  Nothing else, including waivers, can change a verification result.
- **FR-011**: Each finding MUST cite the content hashes of the action, invariant, condition, and
  expression it concerns, and the source locations for humans.
- **FR-012**: Reports MUST be deterministic: the same behavior version and time budget produce
  byte-identical reports, including counterexample values.
- **FR-013**: Integer and decimal semantics MUST be modelled so that no check is reported as proven
  unless it holds under the runtime's arithmetic. Integer operations and exact decimal operations
  are modelled exactly. A rounding decimal operation is modelled as its exact value plus a
  guaranteed error bound, derived from the runtime's precision, scale, and rounding mode (at most
  one unit in the last place as the initial contract) and propagated through the expressions that
  use it.
- **FR-013a**: A property involving rounded values MUST be proven only if it holds for every value
  in the resulting interval, reported as a counterexample only if a concrete counterexample is
  found and confirmed by evaluation, and reported as inconclusive when the interval crosses the
  property's boundary.

**Entity constraints and the shared contract**

- **FR-025**: Authors MUST be able to declare **entity constraints**: rules over one entity that
  define what a valid instance of its type is. The runtime MUST check them on every incoming
  entity, whatever its role, before preconditions are evaluated, and report violations by role:
  `INVALID_STATE` for state, `INVALID_INPUT` for input, `INVALID_CONTEXT` for context. They MUST
  also hold for every state entity on the proposed new state; a violation there is `DENY`.
- **FR-025a**: **State invariants** (the existing invariants of feature 001) remain properties of
  the system state: checked on state entities on S and S' only, never on input or context.
- **FR-026**: The verifier MUST assume exactly what the runtime guarantees before evaluation, no
  more and no less: type correctness of all values; entity constraints on every state, input, and
  context entity; state invariants on state entities. Every counterexample MUST therefore pass the
  runtime's input and state checks.

**Identity of state**

- **FR-027**: When two state parameters of an action are bound to entities with the same identity
  (same entity type and `id`), the runtime MUST reject the call before any check with the result
  `INVALID_BINDING` and reason `STATE_ALIAS_NOT_ALLOWED`, naming both parameters.
- **FR-028**: Every entry of a change set MUST identify the semantic state cell it changes (entity
  type, entity id, field) in addition to the parameter name used for readability; two effects that
  target the same state cell MUST be detected.
- **FR-029**: The verifier MUST treat distinct state parameters as distinct state cells, because the
  runtime guarantees distinct identities (FR-026, FR-027).

**Attestations, waivers, and policy**

- **FR-018**: Verification MUST issue a **verification attestation** bound to the exact behavior
  hash, the verification profile (checks and time budget), and the verifier version, stating the
  result and listing every finding by its finding hash.
- **FR-019**: Every finding MUST have a **finding hash** derived from its kind and the content
  hashes it cites (not from counterexample values or messages), so it is stable across runs and
  changes when the finding changes.
- **FR-020**: A **waiver** MUST state the behavior hash, finding hash, verification profile,
  verifier version, rationale, and optionally an expiry, and nothing about who wrote it. Its
  identity is a content hash of its canonical content. A waiver MUST apply only when all bindings
  match the current verification attestation, it has not expired, and it carries at least one
  signature the policy accepts.
- **FR-020a**: A **signed attestation** MUST consist of the waiver hash, the signer's key id, and a
  detached Ed25519 signature over the domain-separated waiver hash (tag
  `behavior.waiver.v1`). Signatures MUST NOT be part of the waiver's content hash; a waiver may
  carry several independent signatures.
- **FR-020b**: The reviewer's identity and roles MUST be derived from the signing key through the
  execution policy's list of trusted keys; a reviewer name claimed anywhere else MUST be ignored.
  A signature that does not verify, or whose key the policy does not trust, MUST be ignored.
- **FR-021**: Waivers MUST NOT change the verification result or status; they are evidence for the
  execution policy only.
- **FR-022**: An **execution policy** MUST decide, for a proposed transition, whether a commit is
  permitted, from the behavior hash, the verification attestation, and the waivers it is given.
  The policy MUST declare which finding kinds are waivable and which are never waivable; by
  default only inconclusive checks are waivable.
- **FR-022a**: The execution policy MUST be a declarative document, not executable code: required
  verification status (e.g. verified, or verified-or-waived), waivable and forbidden finding
  kinds, trusted reviewer keys with their principals and roles, the roles required per finding
  kind, expiry rules, and required verification profiles.
  It MUST have a content hash computed from its canonical meaning, like behavior.
- **FR-022b**: Every policy decision MUST produce a **commit authorization**: the behavior hash, the
  hash of the proposed transition (its canonical decision record), the policy hash, the
  verification attestation and waivers used, the decision (allow or refuse), and the reasons. The
  authorization MUST itself be content-addressed so it can be audited later.
- **FR-022c**: A waiver MUST be accepted only if it carries a valid signature from a key the
  policy trusts with a sufficient role, its finding kind is waivable under the policy, and its
  bindings match (FR-020, FR-020a, FR-020b).
- **FR-022e**: When a waiver is accepted, the commit authorization MUST record the waiver hash, the
  signature, and the signer's key id that were used, so the whole chain can be verified again
  later.
- **FR-022d**: Changing the policy MUST NOT invalidate earlier commit authorizations; they remain
  explained by the policy hash they cite. New decisions are evaluated against the policy given.
- **FR-023**: Evaluation MUST NOT depend on verification: admitted behavior is always evaluated
  and produces the decision record of feature 001. The commit authorization (FR-022b) links that
  record to the verification status (or "unverified" when no attestation is given), the policy,
  and the waivers relied on.
- **FR-024**: Policy decisions MUST be deterministic given their inputs; the current time used to
  check waiver expiry MUST be supplied by the caller and recorded.

**Operation**

- **FR-014**: Check results MUST be cached by the content hashes of everything the check depends
  on (the action, the invariants and derived values it uses, the types involved) together with
  the verifier version, so unchanged checks are not recomputed after a change elsewhere.
- **FR-015**: The time budget per check MUST be configurable and recorded in the report.
- **FR-016**: Verification MUST be available from the engine library, the command line (with
  distinct exit codes for verified, not verified, and usage errors), and the Python binding.
- **FR-017**: The verification report MUST be a canonical document that can be stored next to the
  behavior version and compared across runs.

### Key Entities

- **Check**: one verification question about one action (and invariant or condition), of kind
  invariant preservation, postcondition, satisfiable preconditions, redundancy/vacuity, or
  evaluation error.
- **Finding**: the outcome of a check that is not proven: kind, severity (blocking or warning),
  cited hashes and locations, and a counterexample or explanation.
- **Counterexample**: concrete starting state, input, and context, plus the decision record
  produced by evaluating them.
- **Verification report**: every check of a behavior version with its outcome, the time budget,
  the verifier version, and the resulting status.
- **Entity constraint**: a validity rule of an entity type; holds for every instance the runtime
  accepts, in any role, and for state entities after every transition.
- **State invariant**: a rule over the system state; holds for state entities before and after
  every transition, and is not applied to input or context.
- **State cell**: one field of one entity, identified by entity type, entity id, and field name;
  what a change-set entry changes.
- **Verification status**: verified or not verified, per behavior version.
- **Verification profile**: the checks run and the time budget; part of every attestation.
- **Verification attestation**: the signed-off result of one verification run for one behavior
  hash, profile, and verifier version; lists findings by finding hash.
- **Waiver**: the acceptance of one blocking finding for one behavior hash, with rationale and
  optional expiry; content-addressed, contains no reviewer identity, never changes the
  verification result.
- **Signed attestation**: a detached signature by one key over one waiver hash; the policy turns
  a trusted key into a reviewer principal and roles.
- **Execution policy**: a declarative, content-addressed document stating which evidence is
  required to commit: verification status, waivable and forbidden finding kinds, accepted
  reviewers, expiry rules, required profiles.
- **Commit authorization**: the content-addressed result of applying a policy to one proposed
  transition: behavior hash, transition hash, policy hash, attestations and waivers used,
  decision, and reasons.
- **Result cache**: stored check outcomes keyed by the content hashes they depend on.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of seeded defects in a reference suite (invariant-breaking effects, unguaranteed
  postconditions, reachable division by zero, reachable overflow, contradictory preconditions)
  are found, each with a counterexample that reproduces when evaluated.
- **SC-002**: 0 checks are reported as proven for a seeded defect (no false proofs), and every
  counterexample in the suite reproduces (no false alarms reported as blocking).
- **SC-003**: Verifying the example modules of features 001 and 002 (invoice, project margin,
  purchase approval) completes in under 10 seconds on a developer laptop.
- **SC-004**: After changing a single action in a module of 50 actions, re-verification recomputes
  only the checks that involve that action and finishes in under 20% of the time of a full run.
- **SC-005**: Running verification twice on the same version yields byte-identical reports in
  100% of runs, across machines.
- **SC-006**: A reviewer can understand any finding — what can go wrong, for which values, and in
  which source line — from the report alone in under 2 minutes.
- **SC-007**: 100% of waivers stop applying when the behavior hash or the finding hash changes, when
  they have expired, or when the policy does not allow the finding kind, in the reference suite.

## Assumptions

- Verification runs on admitted modules only; admission (feature 001) is unchanged.
- The solver is an embedded SMT solver used by the engine; which one is a planning decision.
- State, input, and context values are unconstrained apart from their declared types and the
  invariants on the starting state; there is no knowledge of actual data.
- Nominal types, enums, ids, options, integers, and decimals are modelled with the exact rules of
  the engine (declared operations, 64-bit integer range, 28-digit decimals).
- The default time budget per check is a few seconds; exceeding it is inconclusive and blocking.
- Rounding error bounds follow the engine's decimal semantics (28 significant digits, the
  engine's rounding mode); at most one unit in the last place per rounding operation is the
  initial contract and may be tightened when the rounding mode allows it.
- The verification report, attestations, waivers, and cache are files next to the behavior
  version; a shared or remote cache is a later concern. Waivers are signed (FR-020a);
  verification attestations are produced deterministically by the engine and are
  content-addressed, and signing them is a later concern.
- Key management (creating, rotating, and revoking reviewer keys) happens outside the engine;
  the engine only verifies signatures against the keys the policy lists.
- The engine does not persist change sets; "commit permitted" is the policy's answer, and the
  host performs or refuses the commit.
- Attestations are governance artifacts and may carry timestamps and reviewer identities; the
  engine's own outputs (reports, records) stay free of wall-clock time except the caller-supplied
  time used for waiver expiry.
- The specification is written in English to match the other project documents.
