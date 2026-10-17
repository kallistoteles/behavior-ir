# Feature Specification: Core Release 0.15.0 alignment

**Feature Branch**: `508-core-release-0-15`

**Created**: 2026-10-17

**Status**: Draft (design tree settled with the user, Q1–Q10)

**Input**: User description: "ny core release 0.15.0" — adopt the immutable Core Release 0.15.0
(tag `v0.15.0`, commit `6b93898a9375113f7a8946f23ab5535319b6f5d6`, engine 0.15.0, verifier
0.10.0, Z3 4.16.0), successor to feature 504 (Core 0.13.0). Directly from 0.13.0; no Core change.

## Context

Core 0.15.0 includes 0.14.0:

- **0.14.0**:
  - checked canonical artifact APIs (021): `verify` and artifact `to_json_string` return a
    `Result` with a canonicalization error;
  - executable binding conformance (026): complete current binding fixtures; fixes
    behavior-ir-core#29;
  - portable Builder source locations (027); fixes behavior-ir-core#31.
- **0.15.0**: verifier 0.10.0 with guarded query-summary congruence (028), which fixes
  behavior-ir-core#35. The unchanged Approval reference module of feature 506 (BehaviorHash
  `sha256:a04ba37…531779`, exactly the module Core 028 proves) now verifies its two uniqueness
  postconditions.

Both releases state that BehaviorHash and SchemaHash rules, Wire, record and Store formats,
runtime semantics and historical replay are unchanged. What changes is verification: fresh
attestations, cache keys and proof policies identify verifier 0.10.0, and outcomes may improve.

This feature is a **release adoption and conformance** feature only. Its result is evidence that
the same semantics now verify more strongly, with behavior unchanged.

### Decisions (design tree, 2026-10-17)

- Q1: Start from the latest `main` (507 merged).
- Q2, Q3: Pin 0.15.0 directly. No 0.14 step, no shims, no new model features, no Core changes.
  Migrate directly (no backward compatibility before stable).
- Q4: Semantic identities must be preserved: an unexplained ModelHash or BehaviorHash difference
  stops the work. It is never overwritten.
- Q5: C19 may pass only when the pinned 0.15.0 verifier proves the **unchanged** Approval module,
  confirmed by its exact BehaviorHash. Approval lowering and guards are not changed.
- Q6: Ordinary gates keep `full` mode; a blocker needs an explicit readiness registration.
  Adoption is complete only after independent successful `complete` runs of 506 and 507.
- Q7: Remove the readiness entries for the resolved Core issues #29, #31 and #35, with their
  provenance in the implementation log. Update stale documentation that calls them open.
  Historical evidence is not rewritten.
- Q8 (corrected by the user): two distinct comparisons.
  1. *Semantic compatibility*: ModelHash, BehaviorHash, declarations, generated actions,
     runtime decisions, applicable source maps and stable semantic artifacts are compared, with
     only non-semantic release metadata normalized. Any unexplained difference blocks.
  2. *Verification and governance*: the intended change (C19 blocked → passed) is asserted
     explicitly. Proof results, reports, fresh attestation and cache identities and policy
     references may differ and are not required to equal 0.13.0's.

  Both are archived separately before new pinned baselines are accepted. The existing baseline
  tests are not weakened.
- Q9: Fresh fixtures and proof policies move directly to verifier 0.10.0, with fresh evidence
  regenerated. Historical 0.9 attestations are never upgraded or reinterpreted.
- Q10: Remove the #29 snapshot-completion workaround and use Core's published current binding
  fixtures directly, without reproducing their semantics in the ecosystem.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Build and run on Core 0.15.0 (Priority: P1)

A developer installs the ecosystem and gets exactly Core 0.15.0: the engine, CLI and conformance
assets, with their release identity checked.

**Why this priority**: Nothing else can be checked until the ecosystem builds and runs on the new
release.

**Independent Test**: The pin checks, a fresh fetch, the build and the unit suites pass, and
`engine-info` reports engine 0.15.0 and verifier 0.10.0.

**Acceptance Scenarios**:

1. **Given** the pin set to 0.15.0, **When** the Core assets are fetched, **Then** their
   checksums match the published release.
2. **Given** the new engine, **When** the bindings call verification or serialize artifacts,
   **Then** a canonicalization failure surfaces as a typed error, never a crash or a silent
   default.

---

### User Story 2 - Prove semantic compatibility (Priority: P1)

A reviewer sees that every model built on 0.13.0 has the same identity and behavior on 0.15.0.

**Why this priority**: The adoption must not change behavior. This is the guarantee the user
asked for.

**Independent Test**: A semantic comparison of all reference cases, the 506 and 507 reference
histories and the stable artifacts between 0.13.0 and 0.15.0 is empty after normalizing only
release metadata.

**Acceptance Scenarios**:

1. **Given** the 0.13.0 baseline, **When** every reference case is built on 0.15.0, **Then**
   every ModelHash, BehaviorHash, generated declaration and action, and source map is identical.
2. **Given** an unexpected difference, **When** the comparison runs, **Then** it fails and names
   the difference, and no fixture or baseline is updated.
3. **Given** an empty comparison, **When** the new pinned baselines are recorded, **Then** the
   comparison is archived as their justification, and the established 0.13.0 compat tuples are
   replaced by 0.15.0 tuples with identical hashes.

---

### User Story 3 - C19 verifies the unchanged Approval module (Priority: P1)

The 506 suite reports C19 as passed because verifier 0.10.0 proves the exact module that 0.13.0
could not.

**Why this priority**: This is the reason for the release.

**Independent Test**: A 506 run in complete mode ends "full conformance: complete", with C19's
evidence naming the module's BehaviorHash, the verifier and solver versions and both proven
uniqueness checks.

**Acceptance Scenarios**:

1. **Given** the unchanged module with BehaviorHash `sha256:a04ba37…531779`, **When** C19 runs
   in full mode, **Then** the attestation is `verified` and both action-qualified uniqueness
   postconditions are proven.
2. **Given** any other result, **When** C19 runs, **Then** it is reported exactly as it is, and
   508 stops without changing Approval.
3. **Given** an empty readiness registry, **When** 506 and 507 run in complete mode, **Then**
   both are complete.

---

### User Story 4 - Fresh verification evidence and current fixtures (Priority: P2)

Tests that authorize verified behavior use 0.10.0 proof policies, and binding equivalence runs on
Core's current fixtures.

**Why this priority**: Needed for a green adoption, but secondary to semantics and C19.

**Independent Test**: The governance and binding tests pass with the 0.10.0 policies and the
current fixtures, and with no ecosystem completion of snapshots.

**Acceptance Scenarios**:

1. **Given** a fresh attestation from verifier 0.10.0, **When** it is authorized under the 0.10
   policies, **Then** authorization succeeds. A 0.9-only policy is not used for fresh proofs.
2. **Given** the current binding fixtures, **When** the equivalence tests run, **Then** every
   request gives its published expectation with no snapshot completion by the ecosystem.

### Edge Cases

- **A hash differs** between 0.13.0 and 0.15.0: stop and investigate. Neither the fixture nor the
  baseline is edited.
- **C19 is still inconclusive**, or newly inconclusive elsewhere: it is reported exactly, Approval
  is not changed, and 508 stops for a decision.
- **507 SMT outcomes change**: they may only improve. A regression (proven → anything else) stops.
- **Source locations change** for marker/absolute mixes or Rescale (Core 027): allowed only in
  wire locations and fresh legacy read ids, not in any hash. Where a check or test compares them,
  the change is explained.
- **The strict xfails for behavior-ir-core#31 start passing**: they are removed, not loosened.
- **Governance fixture keys valid only until 2026-11-01**: tests use fixed time; no test may
  depend on the wall clock.
- **Disk space**: a cold build needs about 4.6 GB, and the build directory is cleaned between
  heavy runs if space runs low.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The ecosystem MUST pin Core 0.15.0 exactly: tag, commit `6b93898a…`, CLI and
  conformance asset names, and checksums from the published `SHA256SUMS`. The public Rust
  dependency MUST point at the same commit.
- **FR-002**: The bindings MUST handle the `Result`-returning verification and artifact
  serialization of 0.14.0. A canonicalization failure MUST surface as a typed error.
- **FR-003**: The semantic comparison (Q8.1) MUST cover every compat reference case and the
  506 and 507 reference histories. It MUST normalize only release metadata (Core release
  identity, engine/verifier versions, readiness texts), and it MUST be empty before any baseline
  or fixture is replaced. It MUST be archived.
- **FR-004**: The verification comparison (Q8.2) MUST record the expected changes explicitly:
  C19 blocked → passed; verifier 0.9.0 → 0.10.0 in fresh attestations and caches; 507 SMT
  results unchanged or improved. Any other verification change stops the work.
- **FR-005**: The compat manifest MUST replace the 0.13.0 tuples with 0.15.0 tuples whose case
  hashes are identical, using the gate's existing rule for tuples of an unpinned Core release.
- **FR-006**: The pinned baselines of 507 (identity print and 506 quick report) MUST be retaken
  only after FR-003 and FR-004, with the archived comparisons cited. Their tests MUST keep their
  strictness.
- **FR-007**: C19 MUST pass only on the unchanged Approval module with BehaviorHash
  `sha256:a04ba37ab89e051e448abb760bd754bbb0955125fd22ba8da927b8529a531779`. Its evidence MUST
  record the verifier, solver and attestation identities and both proven uniqueness checks.
- **FR-008**: The 506 readiness registry MUST contain no entry for #29, #31 or #35. Their
  resolution is recorded in the implementation log. Documentation that calls them open is
  updated; historical evidence is unchanged.
- **FR-009**: Tests that authorize fresh proofs MUST use the verifier-0.10 policy fixtures.
  Historical 0.9 attestations are neither upgraded nor reinterpreted.
- **FR-010**: Binding equivalence MUST use Core's current binding fixtures directly. The
  snapshot-completion workaround MUST be removed.
- **FR-011**: The strict expected-failure markers for behavior-ir-core#31 MUST be removed, and
  location documentation updated to the 0.14 contract.
- **FR-012**: The ordinary gates MUST pass in quick and full mode. Independent `complete` runs of
  506 and 507 MUST succeed before the adoption is declared complete.
- **FR-013**: No Core change, no model feature, no compatibility shim and no change to Approval,
  StateMachine, RequestFlow or WorkQueue lowering.

### Key Entities

- **Core pin**: the release identity the ecosystem builds against.
- **Semantic comparison**: the archived 0.13.0 → 0.15.0 diff of semantic identities and runtime
  outcomes, with release metadata normalized.
- **Verification comparison**: the archived record of expected verification changes.
- **Pinned baselines**: the identity print and the 506 quick report, retaken per pin.
- **Readiness registry**: the 506 list of open upstream blockers, empty after this feature.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of reference cases (22 receipts) and both reference histories show zero
  semantic differences between 0.13.0 and 0.15.0.
- **SC-002**: 506 in complete mode: every check passed, including C19. 507 in complete mode:
  61 checks and 12 obligations passed.
- **SC-003**: The ordinary gates pass in quick and full mode, with the 507 quick runtime within
  its 35 s budget.
- **SC-004**: Zero ecosystem workarounds for #29, #31 or #35 remain, and zero readiness entries.
- **SC-005**: The diff touches no model lowering and no Core file, and `compat.json` changes only
  in its Core-release keys.

## Assumptions

- Core 0.15.0's statements about unchanged hashes and formats hold. Where they do not, FR-003
  stops the work.
- The governance-v2 fixture keys are used only with fixed test time.
- Wire `loc` and fresh legacy read-id changes from Core 027 do not occur for the ecosystem's
  modules (Python passes absolute paths or markers). If they do, they are explained, never
  hashed.
