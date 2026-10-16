# Feature Specification: End-to-End Behaviour Conformance

**Feature Branch**: `506-e2e-conformance`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "Feature 506 — End-to-End Behaviour Conformance: executable
evidence that the behaviour-ir ecosystem preserves its declared semantics across Author → Lower →
Admit → Evaluate → Commit → Replay → Verify, on a purchase approval workflow composed of
StateMachine (501), RequestFlow (502) and Approval (503), under the declaration identity of 505
and the pinned Core release. No new Model, no new Core semantics."

## Context

Features 501–505 each prove their own guarantees in isolation. This feature proves that they
still hold when the whole system is composed and run end to end: one realistic workflow is
authored, lowered, admitted, evaluated, committed, replayed and verified, and every step is
checked against independently stated expectations.

Conformance is judged by authoritative semantic outputs (decisions, effects, refusal reasons,
committed history, identities and verification results), never by successful execution, by the
absence of errors, or by two implementations agreeing with each other.

### Principles

- **P1 One semantic authority.** Core defines entities, expressions, predicates, transitions,
  invariants, query evidence, evaluation and state validity. The ecosystem authors, composes,
  lowers and calls Core. Nothing in the ecosystem reinterprets a Core decision.
- **P2 Evaluation is pure.** Evaluation persists nothing and executes no command. It yields a
  deterministic result: permitted state effects, command intents and evidence.
- **P3 Commit is authoritative.** An allowed evaluation does not imply a successful commit. The
  Store independently enforces its commit contract: history-head consistency, state validity
  and freshness of observed facts.
- **P4 Replay is reproducible, in two distinct senses.**
  - *Data replay* reconstructs committed state and history from the initial state and the
    durable history.
  - *Behavior replay* reproduces each recorded decision from its own historical inputs.

  Neither is re-execution against today's state, context, module versions or newly observed
  facts. A decision is reproducible only when its versioned semantics, historical inputs and
  historical evidence are all available. Missing historical inputs are never synthesized or
  reconstructed from current state.
- **P5 Verification is independent of execution success.** The recorded execution is checked
  against explicit invariants and expected outcomes.
- **P6 No implicit world completion.** Direct evaluation needs a complete parent snapshot. A
  Store may supply it from its authoritative state. Neither the binding nor the test harness
  infers missing entity universes.
- **P7 No compatibility obligations.** The project is pre-stable. No shim or fallback is added
  to keep older calling patterns.

### Contract

Let `B` be an admitted module, `S_h` the complete parent state at history head `h`, `a` an
action, `I` its explicit input, `C` its explicit context and `F_h` the facts it observes.

- **Evaluation**: `Eval(B, a, S_h, I, C, F_h) = R`. When allowed, `R = Allowed(ΔS, K, E)`, with
  state effects `ΔS`, command intents `K` and the evidence `E` the Core profile requires.
- **Candidate**: `S′ = Apply(S_h, ΔS)`.
- **Commit**: allowed only when the pinned release's commit predicate holds,
  `CommitValid(B, h, S_h, F_h, R, S′)`. The Store then atomically publishes a new head `h′` and
  the state `S′`. This feature tests that public predicate and never redefines it.
- **Determinism**: identical authoritative inputs and versions give equal semantic results.
  Incidental runtime properties are excluded.
- **Preservation**: `Valid_B(S_h) ∧ CommitValid(…) ⇒ Valid_B(S′)`.
- **History**: `Parent(h′) = h`, and the recorded transition matches the committed effects.
  Equal state content at two positions does not make them the same history head.
- **Data replay**: for a valid committed history `H = (r₁, …, r_n)` from `S₀`, with
  `S_i = Apply(S_{i−1}, ΔS_i)`, `ReplayData(S₀, H) = S_n`, the state at the recorded head, with
  equal identities. This shows that the state follows from the history. It does not show that
  the decisions were correct.
- **Behavior replay**: for every recorded decision `r_i`, with historical module `B_i`, parent
  state `S_{i−1}`, input `I_i`, context `C_i`, evidence `F_i` and recorded result `R_i`,
  `Eval(B_i, S_{i−1}, I_i, C_i, F_i) ≡ R_i`, where `≡` is Core's documented semantic result
  equality. A record must contain, or unambiguously reference, everything reproduction needs.
  A hash without its material is not enough.
- **Static verification**: for the admitted module `B`, Core's verifier proves supported
  obligations, for example inductive invariant preservation
  `∀ S, I, C, S′: Inv_B(S) ∧ T_a(S, I, C, S′) ⇒ Inv_B(S′)`, together with postconditions and
  evaluation-error checks. These hold only within the verifier's modeled fragment and
  assumptions, so a green result is never a claim beyond them. Approval's global safety still
  needs its other premises as well: a valid base state and complete ownership.
- **Evidence integrity** (recorded evidence meets its integrity requirements) and **historical
  commit validity** (the recorded commit met its commit contract, so far as historical data
  allows it to be checked) are separate obligations. A reproduced ALLOW does not prove that the
  head was still current at commit time.
- **Authoring conformance**: for one workflow definition `P`, authored through the Python binding
  (`L_P`) and through the Rust model authority (`L_R`),
  `Meaning(Core(L_P(P))) = Meaning(Core(L_R(P)))`. With the same canonical representation and
  versions, the identities are equal too: `Hash(Core(L_P(P))) = Hash(Core(L_R(P)))`.
- **Execution conformance**: for one admitted module `B`, a complete parent state `S`, input `I`
  and context `C`, `Normalize(Eval_Python(B, S, I, C)) = Normalize(Eval_CLI(B, S, I, C))`.
- The two are separate contracts. One can hold while the other is broken. Each has its own
  explicitly defined normalization, which never discards a semantically meaningful difference.

Four notions are kept apart:
- **Reproducibility**: the same operation repeated gives the same authoritative result.
- **Conformance**: an implementation meets a stated contract.
- **Equivalence**: two paths give equivalent semantic results.
- **Verification**: a recorded execution satisfies independently stated properties.

Agreement between two paths does not by itself prove conformance.

## Clarifications

### Session 2026-10-16

- Q: Which execution paths does "Python and Rust agree" cover: independent authoring through
  the Rust model authority and the Python binding (A), the Python-built module executed through
  the released Core command-line binary (B), or both (C)? → A: Both, as two separate obligations
  with separate normalization and separate reporting:
  - **authoring conformance**: the same workflow, constructed independently in Python and
    through the Rust model authority, gives the same admitted module;
  - **execution conformance**: the Python-authored module, evaluated through the Python engine
    interface and through the released, version-pinned Core command-line binary in a separate
    process, gives the same results.

  Both execution paths use the same Core engine. They are independent consumers, not
  independent implementations, so independently specified expected outcomes remain the
  oracle.
- Q: Should replay (C13) cover only state reconstruction from recorded changes, or also
  re-evaluating each recorded decision from its own recorded evidence? → A: Both, as separate
  checks with separate outcomes:
  - **data replay** reconstructs the state from the initial state and the committed history;
  - **behavior replay** reproduces each recorded decision from its historical module version,
    parent state, input, context, evidence and profile, never from current ones.

  Evidence integrity and historical commit validity are further separate obligations. A
  reproduced decision does not prove that its commit was valid. Missing historical inputs or
  an unsupported capability yield an explicit UNVERIFIABLE or UNSUPPORTED outcome, never a pass.
- Q: Should the lifecycle's "Verify" step also include static SMT verification of the admitted
  reference module, or only verification of the recorded execution? → A: Both, as independent
  obligations with separately reported outcomes:
  - **static verification** uses Core's pinned `verify` contract on the reference module;
  - **execution verification** checks data replay, behavior replay, evidence integrity and the
    invariants of committed states.

  Static verification proves specified properties within the verifier's formal model and
  assumptions, not universal correctness. Execution verification proves properties of the
  recorded history. Neither substitutes for the other. An unavailable solver is UNSUPPORTED,
  and an unknown or timed-out result is INCONCLUSIVE; neither is a pass. The full CI
  conformance gate requires the solver, while the quick local suite may omit static
  verification.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The purchase approval workflow runs end to end (Priority: P1)

A maintainer runs the reference purchase approval application and sees the whole lifecycle
complete on the pinned Core release:

1. authoring;
2. dependency resolution;
3. lowering;
4. admission;
5. a checked valid initial state;
6. creating a request;
7. eligible approvals, committed;
8. the derived outcome;
9. the transition to a terminal state;
10. reading state and history;
11. replay;
12. verification.

Each step's result matches its independently specified expectation.

**Why this priority**: This is the feature's purpose. Without it there is no evidence that the
models compose correctly across their public boundaries.

**Independent Test**: Run the reference application from its quickstart. Every step reports
its expected outcome, and the final verification passes.

**Acceptance Scenarios**:

1. **C01.** **Given** the authored workflow, **When** it is lowered and admitted, **Then**
   admission succeeds without manual repair, and the module contains only ordinary Core
   constructs.
2. **C03.** **Given** a checked valid initial state (`Valid_B(S₀)` verified, not assumed),
   **When** a requester creates a purchase request, **Then** the commit succeeds and the
   resulting state is valid.
3. **C04.** **Given** a request in review, **When** an eligible approver approves, **Then** the
   decision is committed.
4. **C05.** **Given** decisions that reach the approval threshold, **When** the outcome is
   derived, **Then** it is APPROVED, and the request's transition to its approved terminal state
   commits. The approval itself never moves the request.
5. **C06.** **Given** decisions that reach the rejection threshold, **When** the outcome is
   derived, **Then** it is REJECTED, and the request's transition to its rejected terminal state
   commits.
6. **Given** a successful transition that declares command intents, **When** it commits,
   **Then** exactly those intents appear in the committed command stream. Nothing executes
   them.

---

### User Story 2 - Refusals and safety are enforced at the right boundary (Priority: P1)

Every invalid attempt in the workflow is refused by the boundary responsible for it, and leaves
no committed trace. A business-rule denial, invalid input and an infrastructure failure are
distinguishable.

**Why this priority**: Safety that is never exercised is not demonstrated. Negative controls
prove that enforcement is active.

**Independent Test**: Run each refusal scenario against a known state and compare the outcome,
its reason and the unchanged state and history with the expectation.

**Acceptance Scenarios**:

1. **C07.** **Given** an approver who already decided, **When** they decide again, **Then** the
   attempt is denied, and neither the state nor the history changes.
2. **C08.** **Given** an actor who is not eligible, **When** they decide, **Then** the attempt is
   denied, and neither the state nor the history changes.
3. **Given** an invalid input or an action not enabled in the current state, **When** it is
   attempted, **Then** it is refused, with a reason distinguishable from a business-rule denial.
4. **C09.** **Given** a direct evaluation with an incomplete parent snapshot, **When** it is
   evaluated, **Then** it is rejected as an invalid snapshot, and no consumer-side completion
   happens.
5. **C12.** **Given** a parent state from which an approval decision would produce an invalid
   child state (both thresholds reached, or a duplicate decision for one subject and actor),
   **When** the decision is evaluated with its guards bypassed, **Then** the Approval validity
   laws refuse the child state.

---

### User Story 3 - The Store's commit contract holds under contention (Priority: P1)

Concurrent and stale candidates cannot commit as if they were current. A failed commit
publishes nothing. Creating a store again under the supported contract recovers correctly.

**Why this priority**: An allowed evaluation is only a proposal. The commit is where safety
becomes durable, so its contract has to be demonstrated under the conditions that challenge it.

**Independent Test**: Evaluate two candidates on one parent, commit both, and inspect the
outcomes and the history. Repeat with a stale query observation and with store re-creation.

**Acceptance Scenarios**:

1. **C10.** **Given** two decisions evaluated against the same parent head, **When** both are
   committed, **Then** at most one commits, and the history contains no double commit.
2. **C11.** **Given** a candidate whose observed query facts have since changed, **When** it is
   committed, **Then** the commit is rejected, and the stale observation authorizes nothing.
3. **Given** a commit that fails for any reason, **When** state, history and the command stream
   are read, **Then** none of the candidate's effects or intents appear.
4. **C17.** **Given** a store being created for the workflow, **When** creation is repeated
   under the supported contract, **Then** the result is the same store, without duplication or
   divergence.

---

### User Story 4 - Replay reconstructs, and verification detects tampering (Priority: P2)

An auditor reconstructs the committed state from the initial state and the durable history and
gets exactly the current state. Verification of the recorded evidence fails when any record has
been altered.

**Why this priority**: Replay and verification turn a run into evidence. They come after the
run itself works.

**Independent Test**: Replay the reference history, compare with the store's current state,
alter one recorded item, and verify again.

**Acceptance Scenarios**:

1. **C13a, data replay.** **Given** the complete reference history, **When** it is replayed
   from the initial state, **Then** the reconstructed state and its identities equal those at
   the recorded head.
2. **C13b, behavior replay.** **Given** the complete reference history, **When** every recorded
   decision is re-evaluated from its historical module, parent state, input, context, evidence
   and profile, **Then** each reproduced result equals the recorded one under Core's semantic
   result equality. No current state, context, module version or newly observed fact is used.
3. **C14, negative controls.** **Given** any of the following, **When** the record is verified
   or replayed, **Then** the outcome is an integrity or reproduction failure, never a pass:
   - a recorded decision or its evidence altered in any way;
   - a record presented with the wrong historical module identity;
   - a record with a required context or observation missing;
   - a recorded result that does not match its reproduction.
4. **Given** a record whose historical inputs are incomplete, or a replay the pinned release
   does not support, **When** the suite runs, **Then** the check is reported as UNVERIFIABLE or
   UNSUPPORTED, never as passed.
5. **C19, static verification.** **Given** the admitted reference module, **When** Core's
   verifier runs on it, **Then** every supported proof obligation is reported with its own
   outcome (proven, failed, warning or inconclusive), together with the module identity,
   assumptions, verifier version and solver configuration. A failed obligation is a failure,
   and a warning keeps its own classification.

---

### User Story 5 - Results are deterministic and agree across supported paths (Priority: P2)

The same construction and the same run give the same identities and results every time. The
supported construction and evaluation paths agree on the authoritative results.

**Why this priority**: Reproducibility and cross-path agreement make the evidence
transferable. They build on stories 1–4.

**Independent Test**: Build and run the workflow twice, and through each supported path, then
compare the normalized results. Change one referenced declaration and check the identity
change.

**Acceptance Scenarios**:

1. **C02.** **Given** the same authored definitions, **When** they are lowered twice, **Then**
   the module, model and generated identities are equal.
2. **C15.** **Given** the same initial state and inputs, **When** the workflow runs twice,
   **Then** decisions, effects, refusal reasons, committed history and identities are equal.
3. **C16a, authoring conformance.** **Given** the reference workflow constructed independently
   through the Python binding and through the Rust model authority, from the same explicit
   definition (neither derived from the other's output), **When** both are lowered and admitted,
   **Then** the admitted modules are semantically equal and their module and model identities,
   generated declarations and actions are equal.
4. **C16b, execution conformance.** **Given** the Python-authored admitted module, identical
   complete parent snapshots, inputs, contexts and profiles, **When** each operation is
   evaluated through the Python engine interface and through the released Core command-line
   binary in a separate process, **Then** decisions, effects, refusal reasons, command intents
   and identities are equal after the execution normalization.
5. **C18.** **Given** a change to a declaration the models reference, **When** the workflow is
   rebuilt, **Then** the affected model identities change as the 505 contract requires, and the
   unaffected ones do not.

---

### User Story 6 - An auditable report, in CI, with documented scope (Priority: P3)

Every run of the conformance suite produces a machine-readable report. Each check is listed as
passed, failed, unsupported or blocked, with the reason and the owning layer. The suite runs
automatically in CI, and a quickstart explains what is and is not demonstrated.

**Why this priority**: The report makes the evidence auditable over time. It wraps the
scenarios rather than adding new guarantees.

**Independent Test**: Run the suite, read the report, and check that every scenario C01–C18
appears with a status, and that nothing unsupported or blocked is reported as passed.

**Acceptance Scenarios**:

1. **Given** a suite run, **When** the report is read, **Then** it identifies the Core release,
   the model versions and the profiles used, and gives every check a status.
2. **Given** a check whose required public capability is missing, or that depends on an open
   upstream issue, **When** the suite runs, **Then** the check is reported as unsupported or
   blocked, with the owning layer. It is never reported as passed.

---

### Edge Cases

- **Equal state at two positions**: a history may return to an equal state content at a later
  position. History identity is the head, not the content.
- **Concurrent decisions that would cross both thresholds**: at most one commits. The child
  state laws and the commit contract together keep threshold exclusion.
- **An approval reaching its outcome without the request moving**: the outcome is derived. The
  request stays in review until its own transition commits.
- **A refused evaluation that would have emitted intents**: no intent is published.
- **Replay of a history that includes refused attempts**: refused attempts are not in the
  history, so replay reconstructs only committed transitions.
- **Open upstream issues**:
  - behavior-ir-core#29, binding conformance fixtures that lack complete parent snapshots;
  - behavior-ir-core#31, mixed source-location relativization.

  While these are open, the cases that depend on them are reported as blocked. The suite does
  not work around them.
- **Solver unavailable, or no answer**: without a solver, static verification is UNSUPPORTED. An
  unknown or timed-out result is INCONCLUSIVE. Neither is a pass.
- **Unsupported capability**: if the pinned release lacks a public capability a check needs, the
  check is unsupported, and the gap is recorded with its owning layer.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 Complete executable reference.** One executable reference application MUST exercise
  the complete lifecycle of User Story 1, without modifying generated IR and without bypassing
  public interfaces.
- **FR-002 Authoritative composition.** The application MUST compose StateMachine, RequestFlow
  and Approval through their published model contracts, and MUST NOT recreate their semantics
  in application code.
- **FR-003 Deterministic lowering.** Repeated construction from the same definitions MUST give
  identical lowered artifacts and semantic identities.
- **FR-004 Declaration identity.** Every model type reference MUST resolve under the 505
  contract. Missing, inconsistent or mistyped references MUST be rejected.
- **FR-005 Admission.** The generated module MUST be admitted without manual repair. An invalid
  module MUST be rejected with Core's structured error.
- **FR-006 Complete snapshots.** Every direct evaluation MUST use a complete, consistent parent
  snapshot. An incomplete snapshot MUST be shown to be rejected.
- **FR-007 Store-backed evaluation.** The application MUST evaluate against authoritative Store
  state, with no consumer-side completion of entity universes.
- **FR-008 Allowed transitions.** An allowed action MUST produce exactly its expected effects
  and command intents.
- **FR-009 Refused transitions.** A refused action MUST commit no state change and publish no
  intent. Business-rule denial, invalid input and infrastructure failure MUST be
  distinguishable.
- **FR-010 Approval safety.** The workflow MUST preserve:
  - decision uniqueness per subject and actor;
  - threshold exclusion;
  - eligibility;
  - immutable decision ownership;
  - Approval's global safety conditions.
- **FR-011 Conflict detection.** A candidate evaluated against an obsolete head MUST NOT commit
  as if it were evaluated against the current head.
- **FR-012 Evidence freshness.** Where a transition depends on query observations, a stale
  observation MUST NOT authorize a transition that is no longer valid.
- **FR-013 Atomic publication.** A failed commit MUST NOT partially publish a transition or its
  intents.
- **FR-014 Replay.** Two obligations, reported independently:
  - **(a) Data replay.** The application MUST reconstruct the committed state from the
    recorded initial state and the committed history, and MUST check that the state and its
    identities match the recorded head.
  - **(b) Behavior replay.** For every recorded decision the replay contract covers, the
    application MUST re-evaluate using the exact historical admitted module and semantic
    version, the historical parent state (or its authoritative reconstruction), the original
    action, input and context, the original recorded evidence including query observations, and
    the original profile. It MUST compare the result with the recorded one using Core's
    documented semantic equality.

  Current state, context, module versions or newly observed facts MUST NOT replace historical
  inputs. Missing historical evidence MUST NOT be synthesized or rebuilt from current state.
  Both use Core's public replay contracts, and no second evaluator is introduced.
- **FR-015 Evidence verification.** Evidence integrity MUST be verified separately from
  decision reproducibility. The documented integrity relationships among module identity,
  decision evidence, committed effects and history MUST be checked. Historical commit validity
  is a separate obligation, checked only so far as the historical data supports it. A
  successful behavior replay MUST NOT be reported as proof that the original commit was valid.
- **FR-025 Static verification.** The admitted reference module MUST be verified through Core's
  pinned public `verify` contract, covering the supported obligations: invariant preservation,
  postconditions and evaluation-error checks where applicable. The result MUST record the
  module identity, the verifier version, the solver configuration, the assumptions and the
  supported fragment. Proven, failed, warning, inconclusive and unsupported outcomes MUST stay
  distinct. No other verifier is implemented in the ecosystem, and nothing beyond the
  verified properties and assumptions is claimed. Static verification and execution
  verification (FR-014, FR-015 and the invariants of committed states) are reported
  independently.
- **FR-016 Cross-path conformance.** Two separate obligations, reported separately so that a
  failure can be localized:
  - **(a) Authoring conformance.** The reference workflow MUST be constructed independently
    through the Python binding and through the Rust model authority, from the same explicitly
    specified definition. Neither construction may be derived from the other's output. The
    admitted modules MUST be semantically equal, with equal module identity, model identities,
    generated declarations and actions.
  - **(b) Execution conformance.** The Python-authored admitted module MUST be evaluated
    through the public Python engine interface and through the released, version-pinned Core
    command-line binary in a separate process. The inputs are identical: complete parent
    snapshots, inputs, contexts and profiles. Decisions, effects, refusal reasons, command
    intents and applicable identities MUST agree.

  Each obligation MUST have its own explicitly defined normalization, which discards nothing
  semantically meaningful. Where the public contract guarantees exact identity, the
  comparison is of canonical bytes or hashes. Neither path is claimed to be an independent
  implementation of Core semantics, both use the same engine, and neither introduces a second
  evaluator.
- **FR-017 Negative controls.** The suite MUST deliberately introduce invalid or inconsistent
  conditions and show that the expected boundary rejects each one.
- **FR-018 Backend conformance.** Every backend the application uses MUST pass the pinned
  release's backend-conformance suite.
- **FR-019 Deterministic environment.** The suite MUST NOT depend on wall-clock time, random
  process state, the working directory, network access or iteration order. Property-test
  randomness MUST be seeded.
- **FR-020 Executable documentation.** A quickstart MUST run the reference application and its
  verification suite, and MUST state which guarantees are demonstrated and which are out of
  scope.
- **FR-021 Conformance report.** Every suite run MUST produce a machine-readable report in the
  project's existing canonical JSON conventions. The report identifies the Core release, the
  model versions and the profiles, and gives each check exactly one status: passed, failed,
  inconclusive (no definite answer, such as a solver timeout), unverifiable (historical inputs
  incomplete), unsupported (capability missing in the pinned release or environment) or blocked
  (open upstream issue). Each status has a reason, and every status other than passed and
  failed names its owning layer. None of them counts as passed. A verifier warning is reported
  as a warning, never as a passed proof.
- **FR-022 Readiness checks.** Cases that depend on behavior-ir-core#29 or #31 MUST be tied to
  an explicit readiness check. While the issue is open, they MUST be reported as blocked and
  MUST NOT be repaired inside this feature.
- **FR-023 Independent oracles.** Every scenario MUST state its preconditions, operation,
  expected result and authoritative verification method. Expected outcomes and invariants
  MUST be specified independently. One path's output MUST NOT be the only oracle for another
  path.
- **FR-024 Boundaries.** This feature MUST NOT introduce a Model, change Core semantics, define
  a Store protocol or a serialization format, execute commands, schedule work, deliver
  messages, build a user interface, or add a compatibility layer. A missing public capability
  is recorded with its owning layer, not worked around.

### Key Entities

- **Reference workflow**: the purchase approval application. A requester, a purchase request
  with its lifecycle (StateMachine and RequestFlow), eligible approvers, immutable decisions
  with thresholds and a derived outcome (Approval), terminal transitions, and optional command
  intents.
- **Scenario**: one of C01–C18, with its preconditions, operation, independently specified
  expected result, verification method and level.
- **Level**: one of four, each with its authoritative source of truth:
  - 1, model conformance: construction, identity, admission;
  - 2, evaluation conformance: snapshots, guards, postconditions, query evidence, decisions;
  - 3, Store conformance: commits, conflicts, re-creation, atomic publication, state and history;
  - 4, replay and verification: data replay, behavior replay, evidence integrity, historical
    commit validity and static verification, each separate.
- **Conformance manifest**: the pinned Core release, model versions and profiles a run is
  valid for.
- **Conformance report**: a run's statuses per check, with reasons and owning layers.
- **Readiness check**: the condition that releases a blocked check, tied to an upstream issue.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All scenarios C01–C19 (C13 as C13a and C13b, C16 as C16a and C16b) have a status
  in every report. Every scenario not tied to
  an open upstream issue passes.
- **SC-002 (VO-001, VO-002)**: The initial state, and every state after each successful commit
  in the reference histories, is checked valid: 100% of committed states.
- **SC-003 (VO-003, VO-004)**: In every committed reference state, no subject has two decisions
  from one actor, and no subject has both thresholds reached: 0 violations.
- **SC-004 (VO-005)**: Every action of the final admitted module respects Approval's decision
  ownership: 0 unauthorized creations, writes or removals.
- **SC-005 (VO-006)**: 100% of the deliberately failed commits leave state, history and the
  command stream unchanged.
- **SC-006 (VO-007)**: Data replay of every reference history equals the state and identities
  at its recorded head (100%). Behavior replay reproduces 100% of the recorded decisions the
  replay contract covers. The two are reported independently.
- **SC-007 (VO-008)**: Two independent constructions and runs give equal identities and results:
  100% agreement.
- **SC-008 (VO-009)**: Authoring conformance holds for the reference workflow: equal module and
  model identities. Execution conformance holds on 100% of the compared operations, allowed and
  refused alike, after the execution normalization. The report gives each obligation its own
  status.
- **SC-009 (VO-010)**: Every scenario's expected outcome is stated independently of any
  implementation's output: 21 of 21 (C01–C19 with C13a, C13b, C16a and C16b).
- **SC-010**: Every negative control is rejected at its specified boundary: 100%. This includes
  the replay controls of C14: altered evidence, the wrong historical module identity, missing
  context and a mismatched recorded result.
- **SC-011**: The suite runs in CI on every change and produces the report, and 0 inconclusive,
  unverifiable, unsupported or blocked checks are counted as passed.
- **SC-012**: In the designated CI environment, static verification of the reference module
  passes every supported proof obligation. A fully passing conformance gate requires both
  static verification and execution verification to pass.

## Assumptions

- The pinned Core release is 0.13.0 (feature 504), with the model kinds and identity rule of
  features 501–505.
- The Store's existing public operations, the Python binding's public interface and the pinned
  release's backend-conformance suite cover the lifecycle:
  - evaluating, committing and reading state and history;
  - the command stream;
  - replaying behavior and data;
  - verification.

  `/speckit-clarify` and the plan confirm the exact operations. Any missing operation is
  recorded as a gap rather than built here.
- The reference workflow uses the in-memory backend. Other backends are out of scope unless
  they already exist and pass the backend-conformance suite.
- The Python binding delegates every semantic operation to the Rust engine. Cross-path
  agreement compares independently constructed inputs and their authoritative results, never a
  second business-rule engine.
- The released Core command-line binary is the one the pinned release ships, already bundled
  with the package. No development build is used for execution conformance.
- The suite has two modes, following the project's existing tiers:
  - a quick local integration run, which may omit static verification and the full property
    depth;
  - the full conformance run in CI and release, which requires the solver and includes static
    verification, replay and evidence verification.
