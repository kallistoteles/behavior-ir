# Tasks: Adopt Core Update

**Input**: [spec](spec.md), [plan](plan.md), [research](research.md), [data model](data-model.md), [contract](contracts/adoption.md), [quickstart](quickstart.md).

**Checkout**: `/home/kalle/repos/behavior-ir-512`, branch `512-adopt-core-update`. Paths below are relative to that checkout. Preserve all existing work in `/home/kalle/repos/behaviour-ir`.

**Tests**: Explicitly required by the spec and constitution. Observe meaningful behavioral failures before fixes. Published-baseline failures are expected support evidence, not green candidate acceptance. Checklists remain read-only during implementation.

## Phase 1: Setup

- [X] T001 Isolate the feature from published main, verify unchanged user-work boundaries, feature selection and existing ignore coverage, and record the source/checkout in `specs/512-adopt-core-update/evidence/README.md`.
- [X] T002 Select and checksum the published Behavior 0.14.0 wheel and exact installed environment, verify reported Core 0.15.0/verifier 0.10.0, and retain the artifact/source/tool identities in `specs/512-adopt-core-update/evidence/baseline-environment.json`; do not use the older developer venv.
- [X] T003 Capture the unchanged complete semantic reference snapshot with `scripts/semantic-snapshot.py` in `specs/512-adopt-core-update/evidence/baseline-semantic.json`, record historical baseline digests and available resource/tool limits in `specs/512-adopt-core-update/evidence/README.md`.

## Phase 2: Foundational preparation and release dependency

**Purpose**: Prepare independent controls while preserving the released dependency graph. No story adoption begins before T007.

- [X] T004 Add neutral public Python fixtures in `specs/512-adopt-core-update/probes/adoption_cases.py` for free/joint/retarget removal in both profiles, exact filtered incoming-count guards, allowed runtime/replay and unsafe controls; execute the six support assertions red on the published baseline and retain complete reports in `specs/512-adopt-core-update/evidence/baseline-removals.json`.
- [X] T005 [P] Add the bounded retained-Store creation probe in `specs/512-adopt-core-update/probes/growth.py`, run initial populations 64/256/1024 × three repetitions × eight creations against the published baseline, verify 1000/1001 and unrelated-update controls, and freeze the baseline-derived improvement/resource criterion in `specs/512-adopt-core-update/evidence/growth-profile.json` before candidate measurements. Retain setup/evaluate/commit/whole/output-size/replay observations separately in `baseline-growth.json`.
- [X] T006 Add first-failing metadata-only receipt-change tests plus semantic/nested/multiplicity negative controls in `python/tests/test_pinned_identity_baseline.py`; normalize only top-level `core_release` in `scripts/tests/test_pinned_identity_baseline.sh`, preserve `specs/507-work-queue/baseline/compat-print.txt` bytes, and record red/green outputs in `specs/512-adopt-core-update/evidence/`.
- [X] T007 Verify a published Core release containing 029/030, its immutable commit, manifest, verifier and all declared asset hashes; record provenance in `specs/512-adopt-core-update/evidence/core-release.json`. If absent, record the observed release inventory and leave this and all dependent tasks incomplete; do not substitute main, publish Core implicitly or change the ecosystem pin.

## Phase 3: US1 — Install the exact updated Core (P1)

**Goal/independent test**: A clean package install reports and contains exactly the selected published Core and assets; override/mismatch controls reject invalid candidates.

- [X] T008 [US1] After T007, update `core-release.json`, `Cargo.toml`, `Cargo.lock`, current version assertions and package release metadata for Behavior 0.15.0 and the actual selected Core/verifier, using only the supported public released dependency graph.
- [X] T009 [US1] Exercise and adapt exact-identity/build/fetch controls in `python/tests/test_versions.py`, `scripts/tests/test_check_core_pin.sh` and `scripts/fetch-core.sh` only where necessary; retain mismatched asset, graph/override and CLI-version refusals and record the new package build identity.
- [X] T010 [US1] Run the installed-package identity controls through the existing `scripts/release-check.sh` path and record actual candidate/CLI/conformance checksums in `specs/512-adopt-core-update/evidence/README.md` without publishing.

## Phase 4: US2 — Public Python benefits (P1)

**Goal/independent test**: Six positive removal/profile cases have actual proved Ref checks plus allowed execution, negative controls remain honest, and growing creation satisfies the frozen whole-path criterion and boundary checks.

- [X] T011 [US2] Integrate the T004 fixtures into `python/tests/test_core_update_adoption.py` and the existing test gate; require all six nonempty proven inventories, allowed commits/replay, guarded refusal, extra incoming fields/rows, same-action invalid creation and truthful unsupported/exhaustion controls. Adapt binding/report access only if a concrete test exposes a public integration defect.
- [X] T012 [US2] Run the identical T005 workload with the new candidate, retain `candidate-growth.json`, compare against the unchanged frozen `growth-profile.json`, and assert both the genuine 1000/1001 boundary and unaffected update through the public Store path; document output-sized and fallback costs.
- [X] T013 [US2] Check consumer-path conflicts, retries, complete results and replay using `python/tests/test_store.py` and the new adoption tests; preserve Core-owned reference execution and record the supported optimization limits in `specs/512-adopt-core-update/evidence/README.md`.

## Phase 5: US3 — Compatibility and trustworthy evidence (P1)

**Goal/independent test**: Zero unexplained semantic drift; intended verification/dependency evidence changes explicit; historical artifacts stay exact and stale evidence gains no fresh authority.

- [X] T014 [US3] Capture `candidate-semantic.json`, require an empty semantic comparison against T003 and preserved version contracts, and add new-Core ordinary/Safety compatibility tuples in `models/fixtures/compat.json` and `models/fixtures/safety-compat.json` with the existing writer while retaining established entries; explain expected dependency-bound inventory hashes separately. Keep same-Core inventory checks exact; normalize only selected current release metadata in the historical conformance-byte comparison.
- [X] T015 [US3] Refresh current Core verification examples/fixtures and explicit verifier-policy references using the released public API in `skills/behavior-verification/examples/approval_verification.py`; exercise fresh-versus-archival/context/cache controls in `python/tests/test_verify.py` and `python/tests/test_store.py`, retaining immutable old evidence and exact action scope.
- [X] T016 [US3] Regenerate and independently check fresh Safety bundles under the new exact dependency identities through existing `release/safety_smoke.py` and `python/tests/test_safety_evidence.py`; preserve all required proof/axiom/fresh-kernel checks and reject stale/wrong-context bundles without changing policy semantics or theorems solely for the pin update.

## Phase 6: US4 — Skills and release guidance (P2)

**Goal/independent test**: All three Skills and essential examples accurately describe the installed candidate; complete conformance and existing release checks assess its exact source.

- [X] T017 [US4] Update the three `skills/behavior-*/SKILL.md` documents, essential versioned examples, `README.md`, `docs/versioning.md` and `release/notes/0.15.0.md`; document recognized Ref guards/checks, measured global-rule scope, current precision limits, evidence boundaries and normal retry versus lost-acknowledgement recovery without expanding Models.
- [X] T018 [US4] Execute essential Skill examples and fresh certification/reverification from a clean installed candidate through the existing `scripts/release-check.sh`; retain the tested package checksum and preserve ordinary Core replay without Lean/Safety artifacts.
- [X] T019 [US4] Run `scripts/conformance.sh 506 complete` and `scripts/conformance.sh 507 complete`, retaining full reports and existing timing requirements; do not weaken checks or label blocked scenarios complete.

## Phase 7: Final acceptance

- [X] T020 Run existing `scripts/gates.sh`, required Safety proof/integrity checks and full `scripts/release-check.sh` with reproducibility/artifact verification against the exact declared Core release; record source and artifact digests and actual raw results in `specs/512-adopt-core-update/evidence/`.
- [X] T021 Close FR-001–016, SC-001–007 and all 12 story scenarios in `specs/512-adopt-core-update/evidence/README.md`; record blockers and deferred nonblocking findings, validate all links/commands, and report release readiness only when every preceding task passes. No tag, release publication, unrelated Core/SMT work or new governance is authorized.

## Dependencies and parallel opportunities

```text
T001 → T002 → T003 → {T004, T005, T006} → T007 [published Core gate]
T007 → T008 → T009 → T010 [US1]
T010 → T011 → T012 → T013 [US2]
T013 → T014 → T015 → T016 [US3]
T016 → T017 → T018 → T019 [US4]
T019 → T020 → T021
```

T004/T005 use independent probe files; T006 is an independent gate regression. Avoid concurrent heavyweight measurements or builds. Within US1, fetch/metadata review may be read concurrently; US2 proof/runtime checks and growth measurements are independently testable but measured runs stay serial. US3 semantic and historical evidence can be reviewed independently after new-pin build. US4 Skill documents can be reviewed independently, followed by serial clean-install/conformance checks.

## Implementation strategy and coverage

The smallest consumer-deliverable increment is US1 after a qualifying release exists; it does not complete the feature. Until then, complete preparation without false new-release claims. Every later phase preserves the initial baseline and records evidence from the exact tested candidate.

| Requirements | Tasks |
|---|---|
| FR-001/002; SC-001 | T001/002/007–010 |
| FR-003/004; SC-002 | T004/011/013 |
| FR-005/006; SC-004/005 | T005/012/013 |
| FR-007/008; SC-003 | T003/006/014 |
| FR-009; US3.2 | T015 |
| FR-010/011; SC-006 | T016 |
| FR-012/013; US4.1 | T017/018 |
| FR-014–016; SC-007; US4.2 | T019–021 |

**Inventory**: 21 tasks: setup 3, foundation 4, US1 3, US2 3, US3 3, US4 3, final 2. All start unchecked; completion requires actual evidence. No automatic delegation is implied by `[P]`.
