# Feature Specification: Ecosystem Release and Skills Update

**Feature Branch**: `509-ecosystem-release`

**Created**: 2026-10-17

**Status**: Draft

**Repository**: `kallistoteles/behavior-ir`

**Target**: Behavior `0.13.0`, bundling Core `0.15.0` and verifier `0.10.0`

**Input**: Publish the capabilities already built as a coherent, immutable, reproducible release.
Update the three existing consumer Skills and release documentation, reuse the established
release process, and defer noncritical improvements. Publication is a separate explicit step
after the release changes are merged; preparation alone does not complete this feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Prepare a consistent release (Priority: P1)

As a maintainer, I can identify exactly what Behavior 0.13.0 contains, explain its version under
the existing policy, and prepare it using the release process already in this repository.

**Why this priority**: Users need one consistent release identity and dependable artifacts.
A new release process or unrelated engineering work would delay that outcome.

**Independent Test**: Review the candidate's version declarations, README, versioning guidance,
release notes and release evidence. They agree on the product and bundled Core versions, and
the existing required checks pass for the release candidate.

**Acceptance Scenarios**:

1. **Given** the preceding published ecosystem release is 0.12.0, **When** the candidate is
   reviewed against the existing versioning policy, **Then** the choice of 0.13.0 is justified,
   the exact Core 0.15.0 pin is retained, and verifier 0.10.0 is identified correctly.
2. **Given** the release metadata and documentation, **When** a consumer compares them,
   **Then** they agree and the notes explain StateMachine, RequestFlow, Approval, WorkQueue,
   Model composition and the relevant Core verification improvements since 0.12.0.
3. **Given** a reproducible candidate, **When** the established release checks run,
   **Then** all required checks pass, including complete conformance for 506 and 507, without
   replacing, weakening or bypassing the existing acceptance requirements.

---

### User Story 2 - Use accurate consumer Skills (Priority: P1)

As a consumer, including an agent using the shipped Skills, I can choose and author the supported
Models, interpret verification evidence honestly, and integrate the results with a host and
Core Store using concise, executable guidance for Behavior 0.13.0.

**Why this priority**: A correct package is hard to use if its Skills describe an older API,
claim capabilities it does not have, or omit capabilities now available.

**Independent Test**: Review all three existing Skills against the released public capabilities
and run their essential examples through the existing package and Skill-example checks.
No external application build or manual agent evaluation is required.

**Acceptance Scenarios**:

1. **Given** an author choosing a Model, **When** they consult `behavior-authoring`, **Then**
   they can distinguish StateMachine, RequestFlow, Approval and WorkQueue, find an executable
   public-API example for each, and understand composition, ownership and generated entities.
2. **Given** verification evidence, **When** a consumer consults `behavior-verification`,
   **Then** they can distinguish local SMT proofs, global safety arguments, tests and host
   assumptions; INCONCLUSIVE is never described as proven, and the resolved Approval
   query-congruence limitation is no longer presented as an open blocker.
3. **Given** a host using Models and Core Store, **When** it follows `behavior-application`,
   **Then** the guidance preserves atomic commits, replay, trusted context and durable command
   intents, and explains that WorkQueue manages work-state transitions rather than scheduling
   or physical execution.
4. **Given** either a refused attempt or a lost commit acknowledgement, **When** the host follows
   the retry guidance, **Then** it distinguishes a new evaluation against current state from
   resubmitting the original bundle to recover the original committed event.
5. **Given** the built package, **When** the existing Skill-example checks run, **Then** the
   essential examples execute successfully against that package and the three Skills identify
   Behavior 0.13.0 and reflect its supported public API and semantic-gap guidance.

---

### User Story 3 - Publish and verify the immutable release (Priority: P1)

As a maintainer, I can publish the merged, validated release in an explicitly authorized step.
As a consumer, I can obtain its artifacts and verify that they correspond to the immutable
release identity and bundled Core.

**Why this priority**: Publication is the deliverable. A prepared branch, successful local
build or draft release does not make the release available to consumers.

**Independent Test**: After the release changes are merged and publication is explicitly
requested, use the existing publishing and artifact-verification process to verify the
annotated release tag, the published artifacts and their recorded identities and checksums.

**Acceptance Scenarios**:

1. **Given** implementation is underway or the release changes are not yet merged, **When**
   preparation and validation finish, **Then** the candidate is ready for publication but
   the release tag has not been pushed and the release has not been published.
2. **Given** the release changes are merged and publication is explicitly authorized,
   **When** the existing publishing process runs, **Then** the annotated immutable `v0.13.0`
   tag identifies the merged release commit and the verified release artifacts are published.
3. **Given** published artifacts, **When** the existing artifact verification runs,
   **Then** the artifacts match the release manifest, checksums and exact bundled Core identity;
   repeat builds of the same release source satisfy the existing reproducibility checks.
4. **Given** an existing published tag or a publication failure, **When** recovery is needed,
   **Then** the existing release policy governs recovery without moving a published tag or
   rewriting previously published records or attestations; completion is withheld until the
   intended release is published and verified.

### Edge Cases

- If the existing versioning policy contradicts 0.13.0, version selection is a blocker to
  resolve explicitly before publication; the policy is not silently redefined to fit the target.
- A required gate failure, an incorrect artifact or a seriously misleading semantic claim
  blocks publication even if the rest of the release is ready. A failed mandatory check cannot
  be reclassified as a nonblocking finding to permit release.
- A minor defect, example gap outside essential coverage or optional improvement is recorded
  for a subsequent release when correct release usage remains possible.
- A missing solver or other unavailable prerequisite must not turn an unproven result into a
  proof or make incomplete conformance count as complete.
- If an essential Skill example uses an unavailable or changed public capability, correct the
  guidance or example within the current capabilities; adding a new capability is not the
  default remedy.
- A clock, worker identity, backend or migration assumption remains an assumption even when
  local verification succeeds. WorkQueue makes no promise of physical execution or exactly-once
  external effects.
- If publication is not authorized, implementation stops at a documented ready-for-publication
  state. Feature 509 remains incomplete until publication and verification finish.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The release MUST target Behavior 0.13.0 and document why the change from the
  latest published ecosystem release, 0.12.0, meets the existing versioning policy. Any
  incompatible version decision MUST be resolved explicitly before publication.
- **FR-002**: The release MUST retain the exact Core 0.15.0 pin adopted by Feature 508,
  including its immutable commit and published asset identities, and identify verifier 0.10.0.
  Behavior, Core and verifier versions MUST remain distinguishable.
- **FR-003**: Release metadata, README, versioning documentation, release notes, the installed
  package's reported versions and artifact manifests MUST agree on the candidate's identity.
- **FR-004**: Release notes MUST summarize the consumer-visible changes since 0.12.0:
  StateMachine, RequestFlow, Approval, WorkQueue, Model composition and Core verification
  improvements, including any adaptation consumers need under the existing versioning policy.
- **FR-005**: Preparation, build, tagging, publishing and verification MUST reuse the
  established release infrastructure and supported distribution targets. This feature MUST
  NOT introduce a second release process.
- **FR-006**: The existing `behavior-authoring`, `behavior-verification` and
  `behavior-application` Skills MUST retain their established structure, identify Behavior
  0.13.0, and accurately describe the released public capabilities. Existing semantic-gap
  guidance MUST be updated where the release now supplies the capability.
- **FR-007**: `behavior-authoring` MUST explain when to use each of the four Models and cover
  essential authoring, composition, ownership and generated entities, using concise executable
  examples through the released public API. Models MUST be described as lowering to Core
  semantics, without independent runtime meaning.
- **FR-008**: `behavior-verification` MUST describe verifier 0.10.0 and remove obsolete claims
  that the Approval query-congruence issue fixed in Core 0.15.0 remains an open limitation.
  This correction MUST NOT imply that every query or obligation is universally provable.
- **FR-009**: Verification guidance MUST distinguish local SMT proofs, global safety
  arguments, tests and host assumptions. It MUST preserve the meaning of actual outcomes and
  MUST never present INCONCLUSIVE or unavailable evidence as proven.
- **FR-010**: `behavior-application` MUST cover Models interacting with the host and Core
  Store, preserving atomic commits, replay, trusted context and durable command intents.
  It MUST describe WorkQueue as governing work-state transitions, without scheduling or
  physically executing work.
- **FR-011**: Application guidance MUST distinguish ordinary retries that evaluate a new
  attempt against current state from recovery after a lost acknowledgement by resubmitting
  the original bundle and recognizing the original committed event.
- **FR-012**: Essential Skill examples MUST execute against the built release through the
  existing package and Skill-example checks. Essential coverage includes each Model and the
  verification and application distinctions in FR-007–FR-011; examples MAY combine topics.
  Exhaustive examples for every method or possible composition are not required.
- **FR-013**: The candidate MUST pass `scripts/gates.sh`, complete conformance for both
  Features 506 and 507, and `scripts/release-check.sh`, including its existing package,
  Skill-example, reproducibility and artifact checks. Existing required checks MUST NOT be
  weakened, waived or replaced. Complete conformance requires every required check and
  obligation to pass, with none blocked, unsupported or otherwise unresolved.
- **FR-014**: Release artifacts MUST pass the existing reproducibility and artifact
  verification process and identify the exact release commit and bundled Core. Their
  published checksums and manifests MUST match the actual artifacts.
- **FR-015**: Implementation MUST prepare and validate the release without pushing the
  release tag or publishing. Publication MUST be a separate explicit step after the release
  changes are merged, using the annotated immutable `v0.13.0` tag on the merged release commit.
- **FR-016**: Previously published tags, records and attestations MUST remain immutable.
  Fresh verification evidence MUST identify the verifier that produced it; historical evidence
  MUST NOT be rewritten or silently upgraded.
- **FR-017**: Findings MUST be classified as blocking when they concern incorrect release
  artifacts, broken required gates, unsound semantic claims, incompatible version metadata
  or documentation that seriously misrepresents supported behavior.
- **FR-018**: Nonblocking minor bugs, nonessential example gaps, usability concerns,
  performance improvements and documentation polish MUST be recorded for later releases.
  They MUST NOT expand Feature 509 unless they prevent correct release usage.
- **FR-019**: Each significant additional suggestion MUST state its benefit and be classified
  as necessary for this release, requiring explicit approval, or deferred. An optional
  suggestion MUST NOT become required scope automatically.
- **FR-020**: Planning MUST challenge every proposed new task with: “Is this necessary to
  release 0.13.0 correctly, or can it wait until 0.13.1?” Work that can wait MUST be deferred;
  optional work requiring approval MUST remain outside the committed scope until approved.
- **FR-021**: Feature completion MUST include the verified published release and documented
  nonblocking findings. Preparation, successful validation or merge alone MUST NOT be
  reported as completion of Feature 509.

### Scope Boundaries

This feature releases the capabilities already built. New Core semantics, new Models,
additional host runtime capabilities, release-process redesign, extra distribution targets,
external application tests, separate consumer-application reviews and extensive manual agent
evaluations are outside scope. Existing clean-environment package and Skill-example checks
remain required; the exclusion of new external application testing does not remove them.
Only repairs necessary for correct release usage or passing existing mandatory checks belong
in the release work. Optional suggestions follow FR-019 and FR-020.

### Key Entities

- **Release candidate**: A source commit, product version, exact bundled Core identity,
  consumer Skills, documentation and generated artifacts being prepared for publication.
- **Consumer Skill**: One of the three existing versioned guidance packages, including
  its executable examples and its stated capabilities and limitations.
- **Release evidence**: Results of the required checks, complete conformance, reproducibility
  comparisons and artifact verification, associated with the candidate being released.
- **Published release**: The annotated immutable tag, its merged source commit and the
  artifacts available through the established publishing process with matching manifests
  and checksums.
- **Release finding**: A concrete issue or suggestion with its impact, classification and
  disposition for this release or a subsequent release.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every release version declaration and consumer-facing release reference agrees
  on Behavior 0.13.0, exact Core 0.15.0 and verifier 0.10.0 where applicable, with a recorded
  version-policy rationale and no conflicting current release claims.
- **SC-002**: All three consumer Skills identify the correct release and cover their essential
  topics; every essential example executes successfully against the built release using the
  existing checks, including at least one executable example for each of the four Models.
- **SC-003**: All existing required release checks pass, and both 506 and 507 report complete
  conformance with zero blocked, failed, inconclusive, unverifiable or unsupported required
  checks or obligations.
- **SC-004**: Repeated builds of the same release source satisfy the existing byte
  reproducibility requirements, and all published artifact checksums and identities match
  the verified release evidence.
- **SC-005**: The release tag is pushed only after merge and explicit publication
  authorization; the published annotated `v0.13.0` tag and every required artifact are verified
  successfully, with zero changes to previously published tags, records or attestations.
- **SC-006**: Every discovered nonblocking finding has a recorded disposition for a subsequent
  release; every significant suggestion has a benefit and one of the three decisions in
  FR-019. No optional work is silently added to the release requirements.

## Assumptions

- The latest published ecosystem release is 0.12.0, as stated in the release brief. The existing
  version policy and the change in verifier outcomes support a minor release; confirmation
  belongs to preparation rather than a redesign of that policy.
- Feature 508 supplies the Core 0.15.0 adoption, verifier 0.10.0 and evidence for the resolved
  Approval issue. It is a release dependency and must be included in the merged release source.
  Feature 509 does not repeat that adoption or change its pin.
- The four Models and their composition are already implemented. The primary consumers are
  release maintainers and users or agents consulting the three shipped Skills.
- Existing publishing destinations, supported artifacts, platform coverage and verification
  procedures remain the release contract; no additional platform or publishing service is
  requested.
- Maintainers supply the existing publication access when publication is explicitly requested.
  A request to implement the preparation tasks is not itself authorization to push the release
  tag or publish artifacts.
- Existing deterministic examples and package checks provide the required validation of Skills.
  Demonstrating that an external AI agent can build an entire application is not an acceptance
  condition.
