# Feature Specification: Scalable Safety Certification and Independent Proof Checking

**Feature Branch**: `511-safety-certification-scale` (proposed; checkout remains on `510-safety-models`)

**Created**: 2026-10-19 | **Revised**: 2026-10-22

**Status**: Draft — ready for remediation planning; no production solution implemented

**Repository / baseline**: `kallistoteles/behavior-ir`; Behavior 0.13.1, Core 0.15.0,
verifier 0.10.0, Safety checker 2 and policy semantics 1; locked proof environment.

**Input**: Turn the measured root causes and mathematically reviewed solution proposals into
requirements for sound, complete and scalable certification. Prefer existing verified components
where they contribute to proof authority. Establish correspondence to the same or stronger Safety
guarantees without making the present proof representation an architectural constraint.

This revision continues Feature 511. The [initial investigation specification](investigation-spec.md)
is preserved. Evidence is in [root-cause-analysis.md](root-cause-analysis.md), candidate directions
in [solution-proposals.md](solution-proposals.md), and verified library findings in
[verified-algorithms-research.md](verified-algorithms-research.md). These sources justify the work;
they do not establish that a complete remedy already exists.

## Clarifications

### Session 2026-10-22 — settled design frontier

- **Q1:** A256 and A512 must certify and independently re-verify. They are regression floors;
  mixed dependencies, multiple policies, generated actions and growth measurements remain required.
- **Q2:** Start with the pinned proof distribution. New external dependencies need a concrete
  demonstrated benefit and explicit review of trust, reproducibility and distribution. Existing
  libraries are not presumed sufficient for every possible solution.
- **Q3:** Preserve mathematical guarantees and exact semantic identity, not current payloads,
  chunk boundaries or proof syntax. Representation correctness must be machine-checked. Changes
  to proof targets, evidence formats or verification contracts require explicit review/versioning.
- **Q4:** Soundness and usability are separate acceptance dimensions. Freeze separate whole-chain
  budgets for cold certification and independent re-verification in specified reference conditions.
  A timeout is not a mathematical counterexample; a correct but unusable remedy is not complete.
- **Q5:** Preserve historical artifacts and their pinned verification conditions. A new verifier
  need only support declared contract versions. Reproducibility of an older implementation does
  not establish its soundness or restore trust if it is later found unsound.
- **Q6:** Cold means installed pinned package/toolchain, a fresh workspace and no reused prior
  certification results. Record preinstalled compiled dependencies and OS cache conditions;
  installation/download costs are separate. Evidence being checked is legitimate verifier input.
- **Q7:** Prioritize investigating separate proof construction and proof checking, potentially
  using untrusted witnesses. Prove checker soundness, exact binding and independent coverage;
  preserve axiom audit and fresh kernel checking, with no arbitrary evidence-supplied code execution.
  Witness format, proof structure and algorithm remain open investigation outcomes.
- **Q8:** The first binding performance profile is x86_64-linux with recorded CPU, available cores,
  memory and process limits. Preserve other supported platforms' correctness, reproducibility
  and compatibility. Performance claims require measurements on their stated platforms.
- **Q9:** Preserve the entire supported v1 language. Distinguish language compatibility,
  mathematical completeness relative to the contract, and practical capacity. No new mandatory
  domain restrictions or reclassification as Unsupported may hide the scaling problem.
- **Q10:** The construction/search algorithm may be unverified and fallible. Its correctness
  is not a soundness assumption. The verifier independently establishes witness validity,
  complete exact-module/policy coverage, identity binding and all required proof checks.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Certify supported modules beyond the observed failure (Priority: P1)

An application author needs a complete certificate for a realistically sized supported module,
without data construction or proof evaluation preventing deployment despite valid premises.

**Why this priority**: The released certifier cannot complete the neutral supported reference
cases with 256 and 512 authored actions. A working small example does not resolve that blocker.

**Independent Test**: Certify the complete neutral references and verify their exported evidence
through the normal consumer path under the declared evaluation resource profile.

**Acceptance Scenarios**:

1. **Given** the A256 and A512 references, **When** the remedy is evaluated, **Then** both receive
   complete conditional certificates and their evidence passes independent consumer re-verification.
2. **Given** alternative representations bound to the same module and policies, **When** each is
   processed, **Then** successful evidence establishes the same or stronger guarantee through
   checked interpretation, without dropping identities or coverage.
3. **Given** a successful diagnostic component or a run with larger limits, **When** readiness is
   assessed, **Then** neither substitutes for the full certification and re-verification chain.

---

### User Story 2 - Trust mathematical equivalence of the faster checks (Priority: P1)

A reviewer needs assurance that reused algorithms, alternative representations and additional
proof lemmas establish the same guarantee for the exact module and policy set.

**Why this priority**: A fast result is unusable if it weakens ownership, overlooks duplicates,
loses an action or relies on an unacknowledged trusted computation.

**Independent Test**: Review checked equivalence/preservation results and deliberately corrupt
representations, uniqueness witnesses, permission checks and proof statements.

**Acceptance Scenarios**:

1. **Given** an alternative data representation or versioned proof target, **When** certification
   succeeds, **Then** machine-checked correspondence binds it to the same exact semantic content
   and preserved guarantees; present chunk boundaries need not remain unchanged.
2. **Given** a uniqueness witness over a reordered auxiliary list, **When** it is accepted,
   **Then** it establishes uniqueness of the complete original list, including multiplicities.
3. **Given** duplicate hashes, incomplete witnesses, a wrong comparison law, or an unauthorized
   operation, **When** certification is attempted, **Then** no certificate is issued.
4. **Given** several restrictive policies, **When** permission lookup is optimized, **Then** all
   restrictions still apply together; no policy's permission broadens another's authority.
5. **Given** an untrusted producer's witness and claimed inventory, **When** independently checked,
   **Then** the verifier establishes coverage against the admitted module and expected policy set
   itself, rather than trusting the producer's inventory or earlier acceptance.
6. **Given** a changed or fabricated witness, **When** verification is attempted, **Then** it
   cannot produce a false accepted guarantee or cause arbitrary supplied code to execute.

---

### User Story 3 - Reuse verified mathematics without overstating assurance (Priority: P1)

A maintainer needs existing verified components to reduce custom proof work, with a precise
account of their assumptions, the application-specific bridge and remaining trusted code.

**Why this priority**: The pinned proof distribution already contains useful correctness results.
Reimplementing their algorithms adds proof obligations without demonstrated benefit.

**Independent Test**: Inspect proof-authoritative components and their checked bridges, the
fallible producer boundary and all mandatory certificate integrity checks.

**Acceptance Scenarios**:

1. **Given** a needed ordering, uniqueness or membership operation, **When** a component is chosen,
   **Then** applicable existing verified components are assessed first and a custom alternative
   has a recorded mathematical or measured operational justification.
2. **Given** a verified component, **When** it is integrated, **Then** its relevant assumptions
   and connection to the exact required premise are discharged; its name or executable result
   alone does not establish certification.
3. **Given** a proof hole, forbidden axiom or weakened final statement, **When** evidence is
   checked, **Then** the existing integrity contract rejects it.
4. **Given** an erroneous or unverified construction algorithm, **When** it supplies a witness,
   **Then** only independent successful checking can establish the claim; construction failure
   remains a separate completeness/capacity issue and cannot weaken verification.

---

### User Story 4 - Know capacity, costs and failure boundaries (Priority: P2)

Authors need a reproducible capacity statement and actionable failures; reviewers need to know
whether improvement addresses data handling, proof cost or dependency replay.

**Why this priority**: Performance must be demonstrated across supported compositions rather
than inferred from one passing size or compiled algorithm speed.

**Independent Test**: Repeat the controlled scaling corpus, inspect complete stage observations
and verify classification with valid and invalid controls.

**Acceptance Scenarios**:

1. **Given** increasing actions, operations, policies, dependencies, guards and encoded data,
   **When** capacity is characterized, **Then** each dimension and material interactions have
   reproducible observations, explained limits and complete correctness checks.
2. **Given** exhaustion before later stages run, **When** the result is reported, **Then** no
   certificate is issued, the failed stage is identified and unrun obligations remain explicit.
3. **Given** identical completed requests, **When** certification is repeated, **Then** semantic
   evidence is reproducible and measurement noise does not change its meaning.
4. **Given** completed valid proof checking that exceeds an end-to-end usability budget,
   **When** assessed, **Then** mathematical validity and failed performance acceptance are
   reported separately; an aborted required proof check still cannot issue a certificate.

---

### User Story 5 - Preserve existing applications and release contracts (Priority: P2)

Consumers need existing Models, conditional safety guarantees and Core-only storage and replay
to continue working while certification improves.

**Why this priority**: The remedy must address certification without creating new domain or
runtime semantics.

**Independent Test**: Run the established compatibility, integrity, conformance, determinism
and state-transparency checks with the remediated complete chain.

**Acceptance Scenarios**:

1. **Given** unchanged ordinary behavior, **When** Safety proof construction changes, **Then**
   Core IR, BehaviorHash, existing Model receipts, runtime results and replay remain unchanged.
2. **Given** a new checker or proof artifact identity, **When** evidence is consumed, **Then**
   applicable versioning rules distinguish it and stale evidence cannot certify a changed request.
3. **Given** a conditional transition-preservation certificate, **When** deployment claims are
   made, **Then** genesis, migrations, approved modules and exact activation boundaries remain
   separate obligations rather than becoming implicitly proven.
4. **Given** historical evidence under an older contract, **When** a new verifier does not support
   that contract, **Then** incompatibility is explicit and does not rewrite the artifact or claim
   a safety counterexample. Historical verification uses its pinned conditions and stated trust.
5. **Given** a valid previously certified construction under unchanged assumptions and within
   the fixed capacity profile, **When** evaluated by the new system, **Then** it remains certifiable.

### Edge Cases

- A non-strictly sorted list contains two identical hashes; sortedness alone must not certify it.
- A witness omits or duplicates an entry. Equal sets or equal lengths alone do not prove permutation.
- A representation changes payload chunks or proof targets: correspondence must bind the new
  version to the same semantic content, rather than assume byte concatenation proves the old
  nested-value equality or require the old chunks merely because they were once in a theorem.
- A comparator equates distinct full keys, an encoding truncates values, or a hash collision is
  assumed impossible instead of checking the concrete equality premise.
- Auxiliary sorting accidentally changes action indices or the relationship between actions,
  operation occurrences and permissions.
- Overlapping policies authorize different writers: their restrictions remain conjunctive.
- Duplicate writer entries, empty writer sets, creation and removal obey the existing v1 rules;
  set conversion is not permission to accept previously invalid authoring input.
- Generated actions or many irrelevant operations increase input size. Every action remains in
  the inventory, and irrelevance is established for each applicable property.
- Shared-parent dependencies or mixed effects still obey v1's relevant-operation restrictions.
- A changed expected policy set is hidden behind a certificate for a different policy subset.
- The executable algorithm is fast while logical reduction, proof construction or fresh replay
  remains expensive; the complete pipeline determines acceptance.
- A later diagnostic follows a failed declaration and may be cascading. It is not automatically
  an independent cause or measurement of a valid instance.
- Instrumentation uses the wrong imports or loads the original module instead of a mutation.
  Such observations are invalid controls and cannot support acceptance.
- Missing counters, sampled memory, cache reuse or overlapping heavy runs limit interpretation;
  configured resource ceilings are not measurements of actual consumption.
- A witness is invalid although another valid witness may exist for the module. Rejection of
  that witness is not a proof that the module is unsafe or unprovable.
- An independently constructed or modified witness is still valid. Its untrusted origin is
  not a reason to reject it; different valid witnesses may have distinct artifact identities
  while establishing the same module/policy claim.
- A package has preinstalled compiled dependencies while no request results are reused. Its
  fresh-workspace measurement must not be mislabeled as a cold OS or tool installation.
- An old verifier is reproducible but later found unsound. Immutable historical evidence and
  reproducibility must not be presented as establishing continuing mathematical trust.

## Requirements *(mandatory)*

### Objective and Established Basis

The investigation independently demonstrated released certification failures for neutral A256
and A512 modules. An extended observation of the byte-identical A256 proof source exposed a
recursion failure in the data declaration followed by a potentially cascading heartbeat failure.
Separate controls isolated inline action-hash representation and uniqueness proof reduction.
They did not validate a complete remedy or establish a supported capacity.

This feature must remedy both input-processing and concrete-proof bottlenecks while preserving
sound, complete conditional certification. Existing verified components are preferred, but no
particular sorting algorithm, container, encoding or proof organization is prescribed. Fresh
replay cost is also measured; changing its dependency closure is required only if justified by
the acceptance profile. Raising limits alone is not a remedy. The investigation must identify
necessary mathematical obligations before selecting representation or algorithm changes. Separate
proof construction and independent checking is a priority research direction, not a mandated format.

### Mathematical Contract

For admitted module M, complete action set A(M), complete expected Safety policy set P and
successful ordinary Core transition T_M(a,S,S'), certification must establish, under explicit
justified assumptions:

\[
\forall a\in A(M),\ \forall S,S':\quad
I_P(S)\land T_M(a,S,S')\Rightarrow I_P(S').
\]

Structural ownership and mutation restrictions also remain mandatory. Initial validity is a
separate premise. The relationship between Core semantics and extracted proof data remains an
explicit trusted correspondence boundary unless separately verified; library theorems do not
remove it.

For a proposed witness w, the checking contract must establish under its declared assumptions:

\[
\forall M,P,w:\quad Check(M,P,w)=true\Rightarrow SafetyGuarantee(M,P).
\]

`SafetyGuarantee` includes the required structural rules and conditional transition preservation.
No correctness assumption about the producer may appear in this implication. The concrete verifier's
correspondence to `Check`, Core admission and extraction remain explicit trust boundaries unless
independently verified. Independent checking may use the existing pinned kernel; it does not imply
a separately implemented kernel. Invalid or incomplete witnesses cannot establish the premise.

### Separate Evaluation Dimensions

| Dimension | Required assessment |
| --- | --- |
| Soundness | Every accepted claim follows under declared assumptions, independently of the producer's correctness. |
| Coverage and completeness | Preserve the supported v1 language; state mathematical completeness conditions separately from empirically demonstrated certification and resource capacity. |
| Computational scalability | Measure construction and independent checking separately, including their complete costs, growth and binding usability budgets. |

The compatibility requirement is:

\[
Certified_{v1}(M,P)\Rightarrow Certifiable_{new}(M,P)
\]

for valid prior constructions with the same declared assumptions and within the established
capacity profile. This does not require new-format verification of old artifacts or unlimited
completeness for arbitrary true properties and unbounded modules. It requires preserving v1
expressiveness and assessing the stated conditions; finite regression tests alone do not prove
a universal completeness theorem.

### Functional Requirements

#### Exact and complete claims

- **FR-001**: Certification MUST cover the complete admitted action inventory, all assignment,
  creation and removal occurrences and every policy in the expected applicable set, including
  generated actions. Size and optimization MUST NOT omit required obligations.
- **FR-002**: Certification MUST preserve existing FieldOwnership and LifecycleDependency v1
  language coverage and premises, with the same or stronger guarantees. Stronger claims MUST NOT
  impose new mandatory domain restrictions or silently change v1 semantics. Unreachable or
  value-preserving unauthorized writes remain forbidden.
- **FR-003**: Evidence MUST identify the exact BehaviorHash, complete policy identity, theorem,
  proof/checker and toolchain identities, and checked obligations. A certificate for a subset or
  different identity MUST NOT certify the expected complete request.
- **FR-004**: Initial validity, migrations, module authorization, activation continuity and
  extraction/encoding correspondence MUST remain distinct and explicit assurance boundaries.

#### Exact data and checked algorithm correspondence

- **FR-005**: Alternative representations MUST preserve the mathematical guarantees and exact
  semantic binding to the admitted module and complete policies. Payload/chunk structures and
  concrete proof targets MAY change when justified, with machine-checked interpretation or
  correspondence and explicit contract review/versioning. No attempt to retain the current
  representation is a prerequisite; unverified decoding MUST NOT become a hidden assumption.
- **FR-006**: Any auxiliary ordering or index MUST preserve the original relationship among
  actions, operations and policies. It MUST NOT reorder ordinary Core behavior or change its
  identifiers to simplify certification.
- **FR-007**: Any replacement uniqueness check MUST establish the original full-key uniqueness
  premise. Multiplicity-preserving coverage and duplicate rejection MUST be checked; non-strict
  sortedness, set equality, truncated keys or assumed cryptographic collision freedom are insufficient.
- **FR-008**: Any replacement permission lookup MUST have a checked equivalence to the existing
  authorization rule for every supported operation and combination of policies. All applicable
  restrictions remain conjunctive; writer, creator and remover permissions remain distinct.
- **FR-009**: Start with the pinned proof distribution and assess applicable verified components
  for proof-authoritative work before custom algorithms. Record guarantees, assumptions, missing
  bridges, dependency impact and cost evidence. New external dependencies require a demonstrated
  need and explicit decision after mathematical benefit, trust, reproducibility and distribution
  review. Existing libraries need not suffice for every solution; a fallible producer algorithm
  need not itself be formally verified merely because it constructs a proposed witness.
- **FR-010**: Components used as proof authority MUST have their necessary premises and bridge
  discharged by machine-checked evidence. The checker must establish its soundness for all
  accepted witnesses, including those produced by erroneous construction algorithms. Required
  obligations that are unsupported, missing or inconclusive MUST block certification.

#### Proof integrity and simplicity

- **FR-011**: Generic safety theorems, concrete instantiations and the intended final statements
  MUST satisfy expected-type checks, transitive axiom audit and fresh kernel replay. All semantic
  obligations covered by the twelve baseline audited roots MUST remain covered, together with
  any new checker-soundness or representation obligations. Changed targets/audit roots require
  an explicit versioned mapping and review; no weaker claim or omitted obligation is permitted.
  A passing component alone MUST NOT issue a certificate.
- **FR-012**: Only the existing permitted logical axioms (`propext`, `Classical.choice`,
  `Quot.sound`) may be used. Proof holes, unchecked native-evaluation axioms, unrelated propositions
  and unreviewed proof assumptions MUST be rejected. Fresh replay uses the pinned kernel in a
  fresh environment; it MUST NOT be described as a separately implemented verifier.
- **FR-013**: Executable evaluation, test results, checksums and benchmarks MUST remain distinct
  from mathematical proof. Reuse or dependency changes MUST NOT skip required checks or silently
  expand the trusted boundary. Changes to that boundary require explicit justification and review.

#### Demonstrated scalability

- **FR-014**: The remediated full chain MUST certify the complete A256 and A512 neutral references
  and successfully re-verify their exported evidence against the expected requests. Input removal,
  partial proofs or a larger budget alone MUST NOT satisfy this requirement.
- **FR-015**: Final evaluation MUST use a documented fixed resource/environment profile and
  acceptance envelope established before final remedy comparison. Both reproduced references
  are mandatory members. Published default budgets remain the comparison baseline; any profile
  change MUST be explicit and supported by causal and scaling evidence. Separate binding total
  budgets for cold certification and cold independent re-verification MUST cover their whole
  chains and be fixed before final evaluation. Post hoc exclusion of failures, arbitrary passing
  ceilings and reclassification of supported constructs are forbidden.
- **FR-016**: Cost growth MUST be characterized separately for action count, operation count,
  ownership-policy count, lifecycle-dependency count, guard count/complexity and encoded data
  size, with material interactions. Include mixed policies, shared dependencies and generated
  actions; FO-only action scaling alone is insufficient.
- **FR-017**: Measurements MUST distinguish data processing, concrete proof construction/checking,
  identity/type/axiom auditing, fresh replay and consumer re-verification. Import/dependency costs
  and runtime-versus-kernel behavior MUST be accounted for where material. Complete wall time
  and resource use MUST be measured separately for construction and independent verification;
  per-stage limits are not whole-chain budgets. No unmeasured complexity or speed guarantee may
  be inferred from a library algorithm's description.
- **FR-018**: Performance comparisons MUST use sequential, non-overlapping heavy runs on a fixed
  recorded profile and retain exact inputs, commands, stage outcomes, configured limits and
  measured wall time, CPU and memory scope. Missing heartbeat/depth counters, sampled peaks and
  cache conditions MUST be explicit, including the actual scope/application of resource limits.
  Instrumented results cannot masquerade as normal certificates.

#### Failure controls and reproducibility

- **FR-019**: Exhaustion, unsupported patterns, violated premises and established safety
  counterexamples MUST remain distinguishable. Exhaustion MUST NOT become Certified or be
  reported as proof of a safety violation. Rejecting a witness does not prove the module unsafe
  or establish that no valid witness exists. Unrun required obligations MUST remain explicit.
  Fully checked evidence exceeding a usability budget can retain its mathematical validity,
  while performance acceptance fails; interrupted required checks cannot issue a certificate.
- **FR-020**: Positive controls MUST accompany adversarial tests for duplicate/missing witnesses,
  incorrect key laws, changed payloads, unauthorized operations, missing/incorrect guards,
  incomplete inventories/policy sets and forged/stale proof identities. None may certify the
  expected complete request. Actual proof-hole and weakened-statement mutations MUST be rejected.
- **FR-021**: Identical complete requests/configurations in the pinned environment MUST reproduce
  canonical semantic evidence; record relevant producer inputs and the witness being checked.
  Different valid witnesses may have distinct artifact identities for the same claim. Execution
  timings, environment noise, container traversal and diagnostic paths MUST NOT alter proof meaning
  or contaminate semantic identity.

#### Compatibility and delivery boundaries

- **FR-022**: Certification MUST remain state-transparent. Core IR, ordinary runtime results,
  persistence, history and deterministic replay MUST remain unchanged, with no runtime dependency
  on Safety artifacts or proof tooling and no additional runtime state or event streams.
- **FR-023**: Existing Model meanings, receipts and published compatibility fixtures MUST remain
  unchanged. Changed checker/proof/evidence identities MUST follow existing versioning rules;
  published artifacts and attestations MUST remain immutable. A verifier MUST declare supported
  contract versions and reject incompatible evidence explicitly. Historical verification uses
  its pinned components and conditions; universal backward-reading support is not required.
  Reproducibility MUST NOT be treated as establishing an older verifier's soundness or retaining
  trust after an unsoundness finding; no historical artifact may be rewritten or reinterpreted.
- **FR-024**: Existing integrity, reference, compatibility, determinism, package and 506/507
  conformance checks MUST remain green through the established gates. Reuse existing workflows;
  this feature does not create a new release mechanism or authorize publication.

#### Producer/checker separation and operational acceptance

- **FR-025**: Investigate separating proof construction from independent proof checking before
  selecting witness format, proof structure or algorithm. A producer MAY use unverified search,
  heuristics or construction. Its correctness MUST NOT be a hidden soundness assumption; its
  construction success, completeness and cost MUST be assessed separately.
- **FR-026**: The verifier MUST independently establish coverage of the exact admitted module's
  actions and relevant operations, the complete expected policy set, identity binding and witness
  validity. It MUST NOT trust producer-supplied completeness claims, earlier success flags or
  cached acceptance. Soundness of the witness-checking implication MUST be machine-checked under
  explicit assumptions; any concrete implementation/correspondence trust remains identified.
- **FR-027**: Witnesses are untrusted data. Manipulated, incomplete, erroneous or fabricated
  witnesses MUST NOT yield a false accepted guarantee, and arbitrary code from the evidence
  package MUST NOT execute. Parsing, validation and checking costs belong to verification costs;
  mandatory axiom auditing and fresh kernel checking remain required.
- **FR-028**: Cold runs MUST use the installed pinned package/toolchain, a new workspace and no
  results reused from earlier certification requests. Record preinstalled compiled dependencies,
  their declared trust/verification conditions and OS cache state; they do not replace mandatory
  checks. The evidence package is verifier input, not a prior trusted acceptance. Installation and
  downloads are measured separately, and any warm/result-reuse profile is reported separately.
- **FR-029**: The first binding performance profile MUST be x86_64-linux with specified CPU,
  available cores, memory, process limits, toolchain identity and measurement conditions. Existing
  correctness, reproducibility and compatibility requirements for other supported platforms remain.
  Performance guarantees MUST only cover measured environments; ARM requires separate evidence.
- **FR-030**: Assess language compatibility, mathematical completeness relative to the declared
  contract and practical resource capacity separately. Retain all supported v1 constructions as
  target coverage and include valid previously certified cases in regression under the same
  assumptions/profile. Do not introduce Unsupported classifications to hide difficult cases.
  State which completeness claims have proofs and which have only empirical demonstrations.

### Scope Boundaries

This is remediation of existing Safety certification in `behavior-ir`. It does not introduce
new Safety families, broader LifecycleDependency language, changed FieldOwnership permissions,
Core semantics, a new governance mechanism, private Core dependencies, application redesign or
an unrelated SMT-verifier change. Core remains pinned to 0.15.0. No external production application
or private application access is required. The original private failure is not claimed to have
been exactly reproduced by the neutral references.

### Key Entities *(include if feature involves data)*

- **Certification request**: exact admitted module and complete expected Safety policy set.
- **Logical proof input**: complete resolved operations, policy premises and exact identity anchors.
- **Verified component assessment**: reusable result, assumptions, correspondence bridge and cost limits.
- **Untrusted witness**: proposed build-time proof data checked against the original request;
  no format, ordering structure or producer algorithm is selected.
- **Safety evidence**: intended statements, discharged obligations, identities and explicit assumptions.
- **Evaluation profile and observation**: fixed environment/resource budgets, controlled workload
  dimensions, stage measurements, outcome and observation limitations; separate from semantic evidence.
- **Capacity envelope**: measured supported cases, limits and exclusions, including A256 and A512.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A256 and A512 each receive complete conditional certification and successful independent
  consumer re-verification in three consecutive sequential cold runs, within their separately
  frozen whole-chain budgets on the specified reference profile.
- **SC-002**: For 100% of accepted evaluation requests, the certified action/operation inventory,
  expected policy set and identity data match the complete original request; zero required
  obligations are omitted or replaced by a weaker claim.
- **SC-003**: Every proof-authoritative component or representation has a reviewable checked
  correspondence to its required premise and an assessment of applicable verified alternatives.
  Zero unreviewed axioms, unchecked proof shortcuts or silent trust-boundary expansions enter
  accepted evidence. No proof of the producer's search algorithm is assumed for checker soundness.
- **SC-004**: Zero deliberately invalid or identity/premise-mutated evaluation cases certify the
  expected request. Each negative-control group has a passing valid control, and missing checks
  never produce a partial certificate.
- **SC-005**: All six scaling dimensions have repeated, reproducible observations with material
  mixed-model interactions, explained cost contributors and documented residual limits. All valid
  cases in the frozen acceptance envelope certify and re-verify; an unexplained size cap is not success.
- **SC-006**: Completed identical requests reproduce semantic evidence, and paired inputs with the
  same relevant premises yield no contradictory mathematical claims due only to irrelevant size
  or representation differences. Exact module identities remain correctly distinguished.
- **SC-007**: All required existing compatibility, integrity, conformance, package, determinism
  and state-transparency checks pass. Core runtime/replay dependencies and established Model
  receipt identities have zero changes.
- **SC-008**: The final assessment explains which measured bottlenecks were remedied, how reused
  results and new bridges justify correctness, construction-versus-checking cost, the complete-chain
  resource behavior and any remaining capacity or trust limitations. Raising limits or passing one
  component alone is insufficient.
- **SC-009**: The independent checking contract has a machine-checked soundness argument for
  arbitrary proposed witnesses under declared assumptions. Every accepted evaluation witness
  passes independent exact-module/policy coverage and binding checks; zero acceptance relies
  on producer correctness or prior acceptance. Invalid tampering/fabrication controls are rejected;
  a valid independently constructed witness is not rejected merely because its producer is untrusted.
- **SC-010**: Every valid case in the fixed capacity corpus meets separate cold certification and
  cold re-verification budgets; complete wall time, resource use, installed dependencies and cache
  conditions are recorded. Other supported platforms retain existing correctness/compatibility
  checks, with no unsupported performance claims. Budget misses are distinct from mathematical failure.
- **SC-011**: The valid previously certified v1 regression corpus remains certifiable under the same
  assumptions and capacity profile, with zero newly unsupported constructions or mandatory domain
  restrictions. The assessment separately reports language compatibility, proved completeness
  conditions and empirically demonstrated practical capacity without claiming unlimited completeness.

## Assumptions

- This is a revision of Feature 511 following empirical investigation, not a new Safety Model
  feature. The refreshed remediation plan is in [plan.md](plan.md); the historical investigation
  plan is preserved in [investigation-plan.md](investigation-plan.md).
- A256/A512 refer to the existing neutral generator with respectively 256/512 distinct authored
  actions, one assignment per action, one ownership policy and no lifecycle dependencies. They
  are reproducible reference floors, not sufficient coverage of realistic compositions on their own.
- Baseline proof tooling remains the locked Lean 4.30.0 environment. Its existing verified
  sorting, permutation and membership results are candidates, not automatically adopted solutions.
  New external dependencies require the explicit need/trust/distribution assessment in FR-009;
  existing libraries are not presumed sufficient for every possible remedy.
- Published limits are baseline controls, not mathematical semantics. Separate whole-chain cold
  certification/re-verification budgets and capacity are justified by intended usage and measured
  evidence, then frozen before final comparison rather than relaxed after observing failures.
- Construction may be expensive and fallible while checking establishes the same claim. A verified
  checking method is a priority investigation direction; choosing its format or algorithm remains
  an evidence-driven planning outcome. Neither checked-result reuse nor proof caching is assumed.
- Representation and concrete targets may change under a reviewed versioned contract. The old
  nested payload shape is a baseline fact, not an architectural requirement. Mathematical guarantees,
  complete semantic identity and Core behavior remain protected independently of encoding choices.
- Historic evidence remains immutable and conditionally verifiable with its original pinned
  components, exact request, platform and contract. A new verifier may reject an unsupported old
  contract explicitly; historical reproducibility alone is not evidence of implementation soundness.
- Kernel checking, accepted logical axioms, input correspondence, initial validity and host
  enforcement are distinct assurance classes. Reusing a verified library does not formally verify
  the Rust extractor or a newly written adapter automatically.
- The existing test-first and compatibility obligations apply to later code changes. This specify
  step changes documentation only and does not mark implementation or acceptance tests complete.
