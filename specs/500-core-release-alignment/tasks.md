# Tasks: Current Core Release alignment

**Input**: [spec.md](spec.md), [plan.md](plan.md), research/data model and Python contract.
Tests are mandatory under FR-007 and the owning constitution.

## Phase 1: Setup

- [x] T001 Inspect canonical repositories, constitutions and core012/013 contracts; preserve user changes in isolated checkouts.
- [x] T002 Specify and plan feature500 in specs/500-core-release-alignment/; validate requirements and hooks.

## Phase 2: Foundational

- [x] T003 Run core scripts/release-check.sh and exact-revision scripts/check-consumer.sh; record evidence in implementation-log.md.
- [x] T004 Construct final validated core artifacts and publish immutable v0.12.0 using canonical scripts/workflow.

## Phase 3: User Story 1 — Released engine alignment

Independent test: installed package release identity and legacy examples/conformance.

- [x] T005 [P] [US1] Write and observe failing current-release/format/refusal tests in python/tests/test_core_alignment.py and existing store/migration/conformance tests.
- [x] T006 [US1] Update exact published core pin and Cargo resolution in core-release.json, Cargo.toml and Cargo.lock.
- [x] T007 [US1] Update package/public CLI format metadata and intentional legacy refusal expectations in api/public-api.json and python/tests/.

## Phase 4: User Story 2 — Current native document capabilities

Independent test: invocation/commands frozen release fixtures match bytes and altered inputs fail.

- [x] T008 [P] [US2] Write and observe failing fixtures/refusal/replay tests in python/tests/test_invocation.py and test_command_documents.py.
- [x] T009 [US2] Add checked native import/invocation/history/context/evidence access in crates/behavior-py/src/lib.rs.
- [x] T010 [US2] Add Python result/store/module wrappers and typing in python/behavior/; preserve detached diagnostics and canonical bytes.
- [x] T011 [US2] Verify collected release fixtures, duplicate/refusal/paging paths and legacy authoring parity through python/tests/.

## Phase 5: Polish and delivery

- [x] T012 [P] Update README.md/docs/versioning.md and public API manifest with actual capabilities and authoring limits.
- [ ] T013 Run canonical ecosystem scripts/gates.sh and scripts/release-check.sh; record exact-revision verification in implementation-log.md.
- [ ] T014 Review focused diff, publish upgrade branch and deliver verified changes to the user checkout without modifying existing Spec Kit changes.

## Dependencies and parallel execution

T001→T002. Red test tasks T005/T008 may run while core T003/T004 validates. Official pin
T006 follows publication T004. Native T009 and Python T010 follow observed red and can be
coordinated by file ownership; T011 follows both. Documentation T012 can accompany wrappers.
T013 follows all implementation; T014 follows required checks. No unchecked checklist edits.

## Implementation strategy

First validate/publish current core and align the supported graph; then add native current
document access, confirm byte parity/refusals, and deliver the verified ecosystem source.
