# Feature Specification: Scalable and Mathematically Sound Safety Certification

**Feature Branch**: `511-safety-certification-scale` (proposed; no branch created)

**Created**: 2026-10-19

**Status**: Proposed — investigation specification; no implementation selected

**Type**: Investigation and correctness remediation

**Repository / baseline**: `kallistoteles/behavior-ir`, published Behavior 0.13.1;
Core 0.15.0, Core verifier 0.10.0; Safety checker 2 and policy semantics 1.

**Input**: The supplied specification requires investigation of concrete Safety certification's
mathematical correctness, complete module/policy coverage and practical scalability. It does
not prescribe a proof representation, algorithm, evaluation strategy or architecture.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Establish the cause of a noncertifying realistic module (Priority: P1)

An application author needs to know why a supported module cannot be certified, without a
resource failure being mistaken for evidence that its safety property is false.

**Why this priority**: The reported failure blocks deployments requiring certification. Choosing
a remedy before establishing its cause risks concealing the limitation or changing the guarantee.

**Independent Test**: Reproduce the failure with exact identities and resource observations,
then distinguish the responsible stage and causal factors from alternative explanations.

**Acceptance Scenarios**:

1. **Given** the reported recursion-depth failure, **When** the investigation reproduces it,
   **Then** its input identities, complete input dimensions, failing stage, exact diagnostic and
   measured resource behavior are recorded separately from configured limits and hypotheses.
2. **Given** a larger resource budget permits an input to finish, **When** that observation is
   assessed, **Then** it is an experiment, not sufficient evidence of root cause or resolution.
3. **Given** original application material is unavailable for public sharing, **When** a neutral
   reproduction is produced, **Then** its relationship to the reported failing class and any
   remaining difference from the original input are explicit; no independent reproduction of
   an unavailable exact original is claimed.

---

### User Story 2 - Rely on exact, complete mathematical certification (Priority: P1)

An author or reviewer needs a successful certificate to cover the exact admitted module and
complete expected Safety policy set, including handwritten and Model-generated behavior.

**Why this priority**: Scalability has no value if the resulting mathematical claim omits a
transition, operation, policy or required premise.

**Independent Test**: Inspect positive certificates and adversarial input mutations for complete
coverage, exact identities, valid theorem statements and machine-checked obligations.

**Acceptance Scenarios**:

1. **Given** a supported composition with authored/generated actions and multiple policies,
   **When** certification succeeds, **Then** every action, operation and applicable policy is
   accounted for and all necessary obligations are machine-checked for the exact identities.
2. **Given** an unauthorized mutation, missing mandatory guard, incorrect relationship or
   incomplete expected inventory/policy set, **When** certification is attempted, **Then** no
   certificate for the requested complete module/policy set is produced.
3. **Given** a changed module, policy set, theorem statement or proof premise, **When** old or
   altered evidence is presented, **Then** it cannot establish certification of the changed
   request merely because a hash, supplied success flag or different theorem checks.

---

### User Story 3 - Certify supported modules at demonstrated practical scale (Priority: P1)

An author needs supported, realistically sized modules to certify within a documented capacity,
and a reviewer needs measurements explaining how cost grows across independent input dimensions.

**Why this priority**: Small reference cases do not establish that the certification mechanism
is usable for real modules. Acceptance must address the failing class and underlying growth.

**Independent Test**: Apply a reproducible, domain-neutral scaling corpus to the declared capacity
and compare its complete correctness/coverage results with the unremediated baseline.

**Acceptance Scenarios**:

1. **Given** the independently reproduced failing class with valid premises, **When** remediation
   is validated, **Then** it receives complete machine-checked certification within the justified
   acceptance envelope, together with other independently scaled supported cases.
2. **Given** paired supported modules differing in irrelevant complexity, **When** both complete
   within the declared capacity, **Then** their relevant mathematical conclusions agree while
   certificates remain bound to each distinct exact module and policy identity.
3. **Given** variations in actions, operations, ownership policies, dependencies, guards and
   encoded data size, **When** costs are measured, **Then** the evidence distinguishes each
   dimension, their relevant interactions and necessary proof work from incidental overhead.

---

### User Story 4 - Interpret limits and conditional guarantees accurately (Priority: P2)

An author needs to distinguish computational exhaustion, unsupported language, a failed premise
and a demonstrated safety violation, and understand which deployment obligations remain external.

**Why this priority**: A noncertifying result must be actionable without weakening the guarantee
or presenting an unresolved obligation as a proof.

**Independent Test**: Compare controlled failure classes and inspect certificate assumptions,
exclusions and resource observations.

**Acceptance Scenarios**:

1. **Given** exhaustion of a finite proof resource, **When** certification stops, **Then** it
   remains noncertifying and is distinguished from unsupported semantics and violated premises.
   A failed premise alone is not reported as a demonstrated unsafe Core transition.
2. **Given** successful transition-preservation certification, **When** its scope is reported,
   **Then** initial validity, migrations, approved-module authorization and deployment continuity
   remain separate obligations; the trusted extraction/correspondence boundary remains explicit.

---

### User Story 5 - Preserve existing Safety and Core contracts (Priority: P2)

Existing consumers need the investigation and subsequent remediation to preserve Safety v1's
meaning, compatibility obligations and Core-only execution, storage and replay.

**Why this priority**: A faster result must retain the guarantee consumers already rely on.

**Independent Test**: Apply existing reference, integrity, compatibility, determinism and
state-transparency checks alongside the new scaling and adversarial cases.

**Acceptance Scenarios**:

1. **Given** previously supported v1 references and compositions, **When** the remediation is
   assessed, **Then** their guarantees remain valid and required regression checks remain green;
   any new evidence/checker identity follows the existing versioning contract.
2. **Given** otherwise identical ordinary behavior with and without Safety policies/artifacts,
   **When** Core executes and replays its history without proof tooling, **Then** Safety adds no
   runtime state, event stream, execution semantics or replay dependency.

### Edge Cases

- A large module contains mostly irrelevant actions: the complete inventory still accounts for
  them; irrelevance to a particular property is justified, not inferred from a manual shortlist.
- One action has many irrelevant effects and one relevant lifecycle operation. The established
  v1 operation restriction still applies; scaling is not permission for mixed relevant effects.
- Shared parent lifecycle operations affect several dependencies: every affected policy must be
  checked, while inventory occurrences and identities remain exact.
- Two policies overlap, or another Model generates a protected writer: restrictions compose
  without broadening permissions or omitting generated actions.
- A value-preserving or unreachable unauthorized assignment remains structurally unauthorized.
  Adding it is not an irrelevant representation change under FieldOwnership.
- A valid certificate for an explicitly different policy subset must not certify the expected
  full policy set; omitted expected policies cannot be hidden behind that subset's success.
- A source guard is true but simultaneous effects invalidate the resulting property: existing
  v1 rejection/unsupported boundaries remain intact.
- A structurally larger representation changes the BehaviorHash: semantic comparisons do not
  demand identical evidence across different exact modules.
- Kernel acceptance of a weakened or unrelated statement cannot replace the intended guarantee;
  missing proof premises and incomplete action/effect data remain noncertifying.
- Empty inventories, policies with no authorized writers and empty selected lifecycle sets are
  assessed under the existing v1 contract, without vacuous system-safety claims.
- Recursion, time, memory or heartbeat exhaustion occurs before later stages run: outcomes and
  unrun obligations remain explicit, with no partial certificate.
- Actual counters are unavailable or sampled: missing measurements and sampling limits must be
  stated; configured ceilings are not observations that other resources were unexhausted.
- A larger budget, a warmer environment or a smaller input changes completion: the observation
  does not establish a different mathematical truth or prove that scalability is resolved.
- The original input is private or unavailable: neutral evidence must neither copy application
  internals nor imply that an unavailable exact input was reproduced.

## Requirements *(mandatory)*

### Problem and Objective

A real-world supported module has been **reported**, not independently reproduced in this
specification, to return `Inconclusive` when concrete proof checking exceeds `maxRecDepth=4096`.
The report says memory, heartbeat and execution-time limits were not exhausted. Those claims
require investigation; the diagnostic alone does not establish a causal analysis or a violation
of the module's policies. The report's exact original module and resource artifacts have not
been supplied with this specification.

The objective is to establish the actual limiting factors and resolve the limitations preventing
certification of realistic supported modules, while preserving complete mathematical guarantees
for the exact admitted module and policy set. Increasing a limit alone does not satisfy this
objective. The existing proof implementation is not an architectural constraint.

### Mathematical Correctness Contract

Let M be an admitted Core module, A(M) its complete action set, P the complete expected applicable
Safety policy set, S a valid Core state, T_M(a,S,S') a successful ordinary Core transition, and
I_P the state invariants required by the applicable policies. Successful certification establishes,
under explicit justified assumptions:

\[
\forall a\in A(M),\ \forall S,S':\quad
I_P(S)\land T_M(a,S,S')\Rightarrow I_P(S').
\]

The complete module must additionally satisfy all structural ownership and mutation restrictions.
FieldOwnership permissions are structural; this formula does not convert them into runtime
ownership state or exempt unreachable/value-preserving assignments. Initial validity and the
other external boundaries below are not silently included in ordinary-transition preservation.

### Functional Requirements

The parenthetical references trace to the supplied specification's requirement families.
They identify intent, not predetermined implementation components.

#### Complete and sound certification

- **FR-001** (MC-001): Certification MUST account for the complete admitted action inventory,
  including authored and generated actions; size, origin and optimization MUST NOT omit actions.
- **FR-002** (MC-002): Every assignment, creation and removal MUST be accounted for under every
  applicable policy. Exclusion from a particular obligation requires justified irrelevance.
- **FR-003** (MC-003): Every policy in the expected applicable set MUST contribute to the result;
  a subset result MUST NOT be presented as certification of the complete set.
- **FR-004** (MC-004): A required premise violation MUST prevent certification, regardless of
  input scale. Performance changes MUST NOT weaken structural restrictions or safety guarantees.
- **FR-005** (MC-005): Evidence MUST bind the exact admitted Core module, complete policy set and
  checked obligations; the semantic extraction/interpretation trust boundary MUST remain explicit.
- **FR-006** (MC-006): Every necessary proof obligation MUST satisfy the existing machine-checking,
  expected-theorem integrity, transitive axiom-audit and independent fresh-recheck contract.
  Accepted logical assumptions and trusted components MUST remain explicit and auditable.
- **FR-007** (MC-007): Transition preservation MUST remain distinct from initial-state validity,
  migration safety, host authorization and enforced approved deployment segments.

#### Practical capacity and reproducibility

- **FR-008** (source scalability requirement 1): Supported, realistically sized modules MUST be
  demonstrated to certify when their premises hold; valid mathematical claims do not become false
  merely because the complete supported inventory or applicable policy set grows.
- **FR-009** (source scalability requirement 2): Remaining structural/evaluation ceilings MUST
  have explained purpose, consequences and measured behavior. Finite resource limits are allowed;
  an unexplained ceiling blocking the reproduced realistic failing class is not a resolution.
- **FR-010** (source scalability requirement 3): Cost growth MUST be characterized separately for
  action count, operation count, ownership-policy count, lifecycle-dependency count, guard count/
  complexity and extracted/encoded data size, including relevant interactions.
- **FR-011** (source scalability requirement 4): Equivalent relevant safety claims and structural
  authorization premises MUST NOT acquire contradictory mathematical conclusions solely from
  irrelevant complexity. Resource exhaustion may be Inconclusive, never incorrectly Certified.
- **FR-012** (source scalability requirement 5): Remaining practical capacity MUST be measurable,
  documented and reproducible. Exhaustion, unsupported constructs, violated premises and actual
  safety counterexamples MUST be distinguishable; all required incomplete obligations fail closed.
- **FR-013** (source scalability requirement 6): Identical module, complete policies, checker and
  pinned proof environment MUST reproduce semantic evidence for completed checks. Resource use
  and diagnostic environment differences MUST NOT silently change certificate meaning.
- **FR-014**: The investigation MUST define an empirically justified acceptance envelope before
  selecting a remediation for final evaluation. It MUST include the independently reproduced
  failing class and neutral dimension-varying cases; it MUST NOT exclude failures post hoc,
  reclassify supported semantics as Unsupported, or choose an arbitrary passing size as capacity.
  This specification does not invent an action threshold, asymptotic bound or resource budget.

#### Investigation before remediation selection

- **FR-015** (INV-001): Establish the precise cause of the recursion-depth failure, considering
  mathematical obligations, representation/data size, reduction behavior, implementation and
  interactions; the error message alone MUST NOT determine the explanation.
- **FR-016** (INV-002): Identify the operations responsible for measured cost growth and distinguish
  necessary mathematical work from incidental computational overhead, with supporting evidence.
- **FR-017** (INV-003): Explain which concrete certification steps are logically necessary for the
  claimed guarantee and which belong to the present implementation, without prescribing a remedy.
- **FR-018** (INV-004): Assess completeness and correspondence from the actual admitted module to
  checked obligations at large sizes and across every supported Safety composition.
- **FR-019** (INV-005): Determine whether the limitation is specific to FieldOwnership,
  LifecycleDependency or shared infrastructure, and how action/dependency structure affects it.
- **FR-020** (INV-006): Assess failure classification and whether any failure, partial check,
  missing input or resource condition can incorrectly produce a certificate.
- **FR-021** (INV-007): For any contemplated remedy, identify and justify changes to trusted
  components, assumptions, proof integrity and independent verifiability. No expansion may be
  silent, and proof shortcuts or weakened guarantees are not acceptable remediation.

#### Required evidence and regression coverage

- **FR-022** (TEST-001): Record a reproducible failure with release/Core/proof-tool identities;
  action, operation, ownership-policy and dependency counts; guard/data dimensions; actual failing
  stage and exact diagnostic; time, memory, heartbeat and recursion observations. State configured
  limits separately. Missing counters or approximate measurements MUST be identified and their
  evidential limits resolved before ruling out a proposed cause.
- **FR-023** (TEST-002): Establish repeatable domain-neutral scaling cases independently varying
  the relevant dimensions, with exact input identities, environment and measurement scope.
- **FR-024** (TEST-003): Supported positive cases within the justified acceptance envelope MUST
  receive valid complete machine-checked certificates; any failure remains an acceptance failure.
- **FR-025** (TEST-004): Unauthorized operations, missing guards, incorrect relationships, omitted
  expected policies and incomplete action/effect coverage MUST NOT certify the expected request.
- **FR-026** (TEST-005): Mutations weakening/removing premises, changing the certified identity or
  breaking correspondence MUST fail certification, with valid positive controls.
- **FR-027** (TEST-006): Evidence MUST cover FieldOwnership-only, LifecycleDependency compositions,
  multiple dependencies, overlapping restrictions and authored/generated actions.
- **FR-028** (TEST-007): Existing supported Safety references, proof-integrity checks, compatibility
  and determinism guarantees MUST remain valid. Required regressions reuse the existing ecosystem
  gates; established published artifacts and compatibility records MUST NOT be rewritten.
- **FR-029** (TEST-008): Safety certification MUST remain state-transparent: no change to ordinary
  Core semantics/IR, application persistence, event history or deterministic Core replay; no
  runtime dependency on proof tooling, policy documents or certificates may be introduced.
- **FR-030**: Required outcomes MUST include supported root-cause analysis, mathematical correctness/
  trust assessment, complexity characterization, remedy acceptance requirements, positive/negative/
  mutation/scaling validation evidence and explicit remaining computational/mathematical limits.
- **FR-031**: Investigation findings MUST precede remediation selection. Completion requires
  evidence addressing the underlying limitation, not only a successful original input under an
  increased resource ceiling. This specification selects no solution and begins no implementation.

### Scope Boundaries

This work concerns certification of existing FieldOwnership and LifecycleDependency v1 patterns
in `behavior-ir`. It does not authorize new Safety families, broader lifecycle language, changed
FieldOwnership semantics, Core execution changes, persistent Safety state, Store/replay changes,
new authorization/governance policies or unrelated SMT-verifier improvements. The separately
reported `unique(Option)` Core-verifier concern is not investigated or resolved here; this spec
makes no finding about its soundness or its relation to optional-value runtime semantics.

All v1 restrictions remain applicable, including immutable required references, complete final-
module analysis and the existing relevant-operation/guard restrictions. A fundamental obstacle
outside this scope must be reported separately, not incorporated as an implicit Core change.
Reproduction uses public domain-neutral inputs; no private application access or code is required.
Previously published 0.13.1 artifacts, tags, receipts and attestations remain immutable.

### Key Entities *(include if feature involves data)*

- **Certification request**: exact admitted module, complete expected Safety policies and the
  identities establishing which guarantee is requested.
- **Coverage account**: complete actions, operation occurrences and applicable policies, including
  generated actions and justified irrelevant projections.
- **Proof evidence**: intended theorem/premises, checked results, exact bindings, permitted logical
  assumptions and explicit trusted correspondence; separate from run measurements.
- **Failure reproduction**: exact input/environment, failing stage/diagnostic and observations
  sufficient to distinguish the reported symptom from its established cause.
- **Scaling case and observation**: neutral supported input, controlled complexity dimensions,
  measurement conditions, costs, outcome and proof/coverage evidence.
- **Acceptance envelope**: empirically justified supported capacity and environment, including the
  reproduced failing class, evaluation cases, limits and known exclusions.
- **Remediation assessment**: evidence-based comparison against the same mathematical and capacity
  requirements, with any proposed trust-boundary change explicitly identified.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The reported failing class has at least one independently repeatable neutral
  reproduction, and its causal explanation is supported by observations distinguishing the
  responsible factors from the initial diagnostic and alternative explanations.
- **SC-002**: For 100% of accepted evaluation requests, the coverage account matches the complete
  exact action/operation inventory and expected policy set, with zero omitted relevant obligations.
- **SC-003**: Every required obligation in every issued evaluation certificate is machine-checked
  under the established integrity contract; there are zero certificates from incomplete or
  inconclusive required checks, weakened propositions or unsupported proof shortcuts.
- **SC-004**: The reproduced failing class and all valid cases in the justified acceptance envelope
  certify successfully. Raising a budget for one input alone cannot satisfy this outcome.
- **SC-005**: Each of the six specified complexity dimensions has reproducible growth observations,
  measured scope and explained cost contributors; interactions and measurement gaps are explicit.
- **SC-006**: Zero deliberately invalid, incomplete or identity/premise-mutated evaluation requests
  certify the expected complete request; every negative-control group has a passing valid control.
- **SC-007**: Every evaluated exhaustion, unsupported construct, failed premise and confirmed
  safety violation is correctly distinguished, with no exhaustion presented as a mathematical
  counterexample or proof and no failed required check presented as conditional certification.
- **SC-008**: Repeated completed checks of identical inputs reproduce semantic evidence. Paired
  cases with equivalent relevant semantics/structural premises yield no contradictory claims
  solely because irrelevant complexity changes, while exact identities remain distinct.
- **SC-009**: All existing required reference, composition, proof-integrity, compatibility,
  determinism and Core state-transparency checks pass, with zero new runtime/replay dependencies.
- **SC-010**: The six required investigation outcomes are reviewable, residual capacity and trust
  limits are explicit, and no remedy has silently expanded the trusted boundary or claim scope.

## Assumptions

- This is the specification step for a new investigation. Planning may organize reproducible
  experiments and analysis; it must not turn an unverified diagnosis into an implementation plan.
  Findings alone do not complete the feature's eventual remediation/validation acceptance criteria.
- The reported recursion-depth symptom and claims about other resources are input evidence to
  verify, not conclusions already established by this specification. Original private artifacts
  need not be copied upstream; the gap between exact original and neutral reproductions is stated.
- Reproduction starts from released Behavior 0.13.1 / Core 0.15.0 and its locked proof environment
  (Lean 4.30.0). Any contemplated environment/checker change must be identified and assessed;
  no particular future proof construction, reduction strategy or algorithm is selected here.
- Existing Core public semantic contracts, Model authority boundaries and constitution constraints
  remain governing requirements. Proof extraction/encoding implementation is not assumed formally
  verified merely because a mathematical theorem or its concrete premises pass checking.
- Practical capacity and numeric performance objectives are investigation outputs justified against
  the failing class and neutral evidence before remedy comparison; no linear-complexity promise,
  benchmark threshold or convenient maximum input size is invented at specification time.
- Kernel correctness, permitted logical axioms, semantic correspondence, initial validity and host
  deployment enforcement remain distinct classes of assurance. Timing and finite tests do not
  upgrade any assumption into a universal mathematical proof.
