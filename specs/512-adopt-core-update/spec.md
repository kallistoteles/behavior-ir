# Feature Specification: Adopt Core Update

**Feature Branch**: `512-adopt-core-update` (proposed; specification creation does not switch the current checkout)

**Created**: 2026-10-26

**Status**: Implemented — candidate validated; publication is a separate explicit step

**Repository**: `kallistoteles/behavior-ir`

**Target**: Release-ready Behavior `0.15.0`, adopting a published Core release containing Features 029 and 030, including verifier `0.11.0`.

**Input**: Adopt the Core update through the existing ecosystem release process: update the exact pin and bundled assets, demonstrate the new capabilities through Python, preserve semantic compatibility, refresh certification and governance evidence, and update consumer Skills. Keep the work narrowly scoped.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install the exact updated Core (Priority: P1)

A consumer installs one Behavior package and obtains the released Core containing scalable validation with global rules and supported Ref-target-removal verification. The package identifies exactly which Core and verifier it contains.

**Why this priority**: Consumers cannot benefit from the Core work until the ecosystem distributes a consistent, verifiable package.

**Independent Test**: Install the candidate outside the checkout and compare its reported identities and bundled assets with the selected immutable Core release and ecosystem release metadata.

**Acceptance Scenarios**:

1. **Given** a published Core release containing 029/030, **When** the ecosystem adopts it, **Then** the dependency pin, bundled command-line tool, conformance assets and reported versions all identify that exact release and its verified checksums.
2. **Given** an unreleased branch, mismatched asset or active development override, **When** required release checks run, **Then** the candidate is rejected rather than distributed under a released Core identity.
3. **Given** a clean installation, **When** a consumer uses the documented public package, **Then** the new capabilities are available without requiring a Core checkout or a separately selected engine.

---

### User Story 2 - Use the Core improvements through Python (Priority: P1)

An application author can verify supported reference-safe removals and can retain a genuine global rule while growing a Store, using the ordinary public Python workflow.

**Why this priority**: A dependency update is useful only if the benefits reach the consumer's actual execution path.

**Independent Test**: Exercise neutral reference-removal examples and repeated creation workloads through the public package, with explicit allowed, refused and unsupported controls.

**Acceptance Scenarios**:

1. **Given** the supported guarded free-removal, joint-removal and retarget/removal patterns, **When** preservation is verified, **Then** each produces nonempty proven reference checks and also permits its independently valid runtime transition. Empty checks or unreachable later checks cannot substitute for this evidence.
2. **Given** a missing guard, an additional surviving referrer or a newly created invalid reference, **When** verification and execution are exercised, **Then** the unsafe reference obligation is not falsely proven and runtime reference integrity remains enforced.
3. **Given** a maximum of 1,000 entities, **When** creation genuinely affects that rule, **Then** entity 1,000 is accepted and entity 1,001 is refused without changing the committed state. An update unrelated to the rule also retains correct behavior.
4. **Given** a supported growing creation workload, **When** complete evaluate/commit cycles are measured at increasing live populations, **Then** the updated package satisfies a predeclared improvement criterion against Behavior 0.14.0. Necessary result/history output work and remaining full-state work are reported separately.

---

### User Story 3 - Retain behavior and trustworthy evidence (Priority: P1)

A maintainer can distinguish unchanged application behavior from intentionally improved verification outcomes and can establish which evidence applies to the new dependency environment.

**Why this priority**: Stronger verification must not silently change Model meaning or promote historical evidence into fresh authority.

**Independent Test**: Compare the reference Models and histories with the published baseline, then check newly produced evidence and deliberate attempts to reuse evidence from incompatible contexts.

**Acceptance Scenarios**:

1. **Given** unchanged reference definitions, **When** they are rebuilt and executed with the new pin, **Then** their established semantic identities, applicable version contracts, decisions, resulting states, command intents and replay results remain unchanged after normalizing only declared nonsemantic release metadata.
2. **Given** new and historical Core verification evidence, **When** fresh authorization is requested, **Then** only evidence meeting the exact current subject, verifier and policy requirements is eligible; archival replay and interpretation retain their original contracts and bytes.
3. **Given** a Safety Model module, **When** evidence is produced and independently checked against the new package, **Then** it binds the exact module, complete policies and dependency identities, and preserves the existing proof-integrity requirements and explicit assumptions. A dependency change does not automatically validate an old certificate.

---

### User Story 4 - Follow accurate guidance and review a releasable candidate (Priority: P2)

Consumers can use the three established Skills to understand the supported capabilities and their limits. Maintainers can review a candidate using the existing release checks.

**Why this priority**: Accurate guidance and reproducible artifacts make the Core improvements usable without starting another feature-development project.

**Independent Test**: Execute essential Skill examples from a clean installed candidate and review the required release/conformance results alongside consistent release documentation.

**Acceptance Scenarios**:

1. **Given** the updated Skills, **When** an author follows the reference-removal and Store guidance, **Then** the examples execute and the guidance accurately distinguishes supported verification, conservative limitations, conditional Safety certification and host responsibilities.
2. **Given** the prepared candidate, **When** the existing release process is exercised, **Then** required gates and complete conformance pass, reproducible artifacts identify their inputs, and nonblocking findings are recorded without expanding this feature. Tag publication remains a separate explicit action after merge.

### Edge Cases

- Core 030 is merged but no Core release containing it has been published.
- The released Core version, verifier identity or available asset names differ from expectations.
- A reference-removal report has no relevant checks, only inconclusive checks, or a later check made unreachable by an earlier refusal.
- Several incoming reference fields/types or equal-body actions require distinct coverage and scope.
- A global rule or operation is outside Core 029's supported optimization domain and requires ordinary reference execution.
- A Store is reopened, another writer commits, or a conflict occurs during the measured workload; continuity and retry behavior must remain Core-owned.
- Runtime output grows with state size even after redundant validation work is removed.
- A new Model compatibility tuple is needed for the new Core release while established historical tuples remain immutable.
- Archived Core attestations or Safety bundles identify the previous dependency environment.
- Semantic snapshots agree while verification reports legitimately differ; these are separate comparisons.

## Requirements *(mandatory)*

### Scope and release boundary

The baseline is published Behavior 0.14.0, bundling Core 0.15.0 and verifier 0.10.0. Core 029 supplies supported incremental global-rule validation; Core 030 supplies complete-world reference preservation verification. Adoption depends on an immutable Core release containing both. Approval, merge or passing branch CI alone does not satisfy that dependency.

This feature prepares and validates the ecosystem release candidate. Merging, creating/pushing a release tag and publishing distributions require a separate explicit instruction. The expected ecosystem version is 0.15.0 under the existing minor-bump rule for changed verifier outcomes; the exact Core release version and commit are selected from published artifacts rather than invented here.

### Functional Requirements

- **FR-001**: Adopt exactly one published Core release containing 029/030 through the supported public Core dependency. Required gates and release artifacts MUST reject branch/path overrides and mismatched Core identities or assets.
- **FR-002**: Keep dependency metadata, bundled engine/conformance assets, checksums, package version reporting and release metadata consistent with that exact release. Confirm the published verifier identity, expected to be 0.11.0, before recording it as supported.
- **FR-003**: Demonstrate all three mandatory supported reference-removal patterns through the public Python surface, covering both admitted profiles used by Core 030. Each positive MUST contain actual relevant proven checks and an independently allowed transition; unsafe counterparts MUST retain their refusal behavior.
- **FR-004**: Preserve complete verification reports and honest failure classifications, including the new reference-world obligations. Unsupported forms, solver exhaustion and unconfirmed candidates MUST NOT become proof or fresh authorization.
- **FR-005**: Demonstrate the supported global-rule benefit through repeated Python evaluate/commit cycles at no fewer than three increasing live-state populations, with at least three measured repetitions per population under recorded, comparable conditions. Compare with the published baseline and freeze the improvement criterion and resource profile before evaluating the final candidate. Identify setup, necessary output/history costs and remaining full-state operations; an isolated validator timing or constant-true rule is insufficient.
- **FR-006**: Exercise a genuinely affected maximum-cardinality rule, including successful creation of entity 1,000 and refused creation of entity 1,001, plus an unaffected update. Preserve decisions, errors, refusal behavior, committed state and replay meaning. Unsupported optimization shapes MUST retain correct Core fallback behavior; no blanket constant-time claim is permitted.
- **FR-007**: Compare semantic compatibility separately from verification improvements. Established Model meanings and version contracts, semantic hashes, generated declarations, applicable source maps, reference decisions, state histories and command intents MUST have zero unexplained differences. Normalize only explicitly identified nonsemantic release metadata.
- **FR-008**: Preserve published fixtures, receipts, records and evidence. Add compatibility entries for the new Core release as required by the existing rules without rewriting established expected identities to hide a difference. An unexplained semantic difference blocks adoption.
- **FR-009**: Generate fresh Core verification fixtures and policy references for the adopted verifier where required. Preserve exact-current fresh eligibility, complete subject/action scope and cache invalidation across module, schema, profile and verifier contexts. Historical interpretability MUST NOT confer new authorization.
- **FR-010**: Rebuild and independently check Safety evidence against the exact adopted dependency identity using the established certification contract. Retain complete action/policy coverage, permitted assumptions, theorem/axiom integrity and fresh independent proof checking. Keep initial-state validity, migrations and deployment authorization separate from transition preservation.
- **FR-011**: Preserve FieldOwnership, LifecycleDependency and existing behavioral Model semantics. A pin update alone MUST NOT introduce new Safety language, theorem claims, runtime state or replay dependencies. Any necessary binding adaptation MUST remain access to public Core semantics rather than an ecosystem implementation of those semantics.
- **FR-012**: Update the three existing consumer Skills, their essential executable examples and semantic-gap guidance for the adopted release. Explain recognized removal guards and reference checks, current verification limitations, incremental validation's supported scope, atomic commits, conflict retries and lost-acknowledgement recovery. Retain the distinction between Core verification and Safety certification; re-evaluate precision-debt claims against observed pinned outcomes.
- **FR-013**: Align README, versioning guidance, release notes, current version assertions and package metadata on the selected ecosystem/Core/verifier versions and necessary consumer adaptations. Use the established versioning policy; do not rewrite historical documentation as if it described the new release.
- **FR-014**: Pass the existing required gates and release process, including `scripts/gates.sh`, complete 506/507 conformance, existing Safety proof/integrity checks, essential Skill-example checks, semantic/Model compatibility and `scripts/release-check.sh`. Required checks MUST use the declared released Core; none may be weakened or bypassed to accept the candidate.
- **FR-015**: Validate a clean installed package and the existing reproducibility/artifact checks. Record the exact tested candidate and dependency identities so older evidence cannot be presented as a run of changed source. Preserve the existing build/tag/publishing workflow.
- **FR-016**: Treat incorrect artifacts, unsound claims, incompatible evidence, unexplained semantic changes and failing mandatory checks as blocking. Record noncritical improvements for later releases. Do not add external application reviews, production integrations or extensive manual agent evaluations.

### Key Entities *(include if feature involves data)*

- **Core Release Identity**: The published Core version, immutable tag/commit, verifier identity and asset checksums selected for the candidate.
- **Ecosystem Release Candidate**: The package, bundled tools/assets, Skills, metadata and reproducibility evidence reviewed together.
- **Compatibility Baseline**: Published reference identities, Model version contracts and runtime histories, distinct from intentionally changing verification results.
- **Current and Historical Evidence**: Core attestations and separate Safety artifacts with their exact subjects, dependency identities, policies and original verification conditions.
- **Consumer Validation Record**: The executed examples, growing-state measurements and required checks, bound to the precise candidate and environment.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every installed component and release declaration identifies the same selected Core release and ecosystem candidate; all mismatched-asset and override controls are rejected.
- **SC-002**: All six mandatory reference-removal pattern/profile cases expose nonempty proven reference coverage and valid execution controls, with zero false proofs in the corresponding unsafe cases.
- **SC-003**: Every established reference Model and the complete 506/507 histories have zero unexplained semantic differences from the baseline; intended verification changes are recorded separately.
- **SC-004**: Growing-state measurements cover at least three live populations and three repetitions each, meet the frozen baseline-derived improvement criterion for the supported case, and account for the full consumer transaction path and residual costs.
- **SC-005**: The 1,000-entity boundary case accepts the last valid creation and refuses the first invalid creation without advancing committed state; the unaffected-update control also passes.
- **SC-006**: Fresh evidence passes every required integrity and independent verification check under the adopted dependencies; all required stale/wrong-context controls reject fresh authority while immutable historical controls retain their documented results.
- **SC-007**: All three Skills' essential examples execute from a clean installed candidate, and every required release, conformance, compatibility and reproducibility check passes for the identified candidate.

## Assumptions

- Behavior 0.14.0 is the published baseline. If another release appears before implementation, record and justify any revised baseline/version selection before evaluation.
- Core 029/030 correctness and supported-language guarantees are established upstream. This feature validates ecosystem adoption and correspondence without redesigning their implementations or repeating the entire Core investigation.
- The existing public binding already retains a Core Store instance. Change the wrapper only if investigation shows a specific integration defect; measure the consumer path rather than assuming automatic performance gains.
- Existing semantic snapshots, compatibility checks, conformance scenarios and release infrastructure are reused. A small neutral growth workload is required, not a new benchmarking framework or long-duration production test program.
- No new consumer authorization mechanism, proof cache, backend requirement, Model family, ownership-transfer support or Core change is authorized. Unrelated `unique(Option)` verifier work remains separate.
- Policy meanings, generic Safety theorems and proof-contract versions need not change solely because the Core pin changes. Any discovered required change must follow existing versioning rules and be justified before proceeding beyond adoption scope.
- Development environment limitations and nonblocking findings are recorded separately. Existing uncommitted user work and unrelated Spec Kit changes are preserved.
