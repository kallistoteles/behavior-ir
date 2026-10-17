# Tasks: Core Release 0.15.0 alignment

**Input**: spec.md (Q1–Q10), plan.md (R1–R9). Tests: the existing suites, plus the semantic
comparison. Each fix of a release-induced failure is preceded by observing that failure.

## Phase 1: Baseline on 0.13.0 (before any change)

- [X] T001 Add `scripts/semantic-snapshot.py` (snapshot and `--compare`). Capture, on the 0.13.0 pin, `evidence/0.13.0-semantic.json`, `evidence/0.13.0-compat-print.json`, `evidence/0.13.0-506-full-report.json` (C19 blocked) and `evidence/0.13.0-507-full-report.json`, under specs/508-core-release-0-15/.

## Phase 2: Pin (US1)

- [X] T002 [US1] Set core-release.json to v0.15.0 (commit `6b93898a…`, assets and sha256 from `SHA256SUMS`), the root Cargo.toml `rev`, and Cargo.lock; run `scripts/check-core-pin.sh` and `scripts/fetch-core.sh`.
- [X] T003 [US1] Build. Record the compile errors, then handle the `Result` values in crates/behavior-py/src/lib.rs (authorize, verify, verify_migration, authorize_migration), mapping a canonicalization error to the engine error type (FR-002).

## Phase 3: Release-induced test changes (US1, US4)

- [X] T004 [US1] Run the Python suite and record which tests fail on 0.15.0 and why, in implementation-log.md.
- [X] T005 [US1] python/tests/test_core_alignment.py: engine 0.15.0, verifier 0.10.0.
- [X] T006 [US4] python/tests/test_command_documents.py: use the `policy-verifier-0.10.json` and `evidence-policy-verifier-0.10.json` fixtures (FR-009).
- [X] T007 [US4] python/tests/test_source_locations.py: remove the strict `CORE_31` xfails. Update the docstrings in python/behavior/location.py and the test module (FR-011).
- [X] T008 [US4] python/tests/test_binding_equivalence.py: remove the snapshot-completion workaround and run the equivalence against `tests/fixtures/bindings/current/` requests and expectations (FR-010).

## Phase 4: Semantic comparison (US2)

- [X] T009 [US2] On 0.15.0, capture `evidence/0.15.0-semantic.json` and run `scripts/semantic-snapshot.py --compare` against the 0.13.0 snapshot. It must report 0 differences; otherwise stop and investigate (FR-003). Archive the output as `evidence/semantic-comparison.txt`.

## Phase 5: Verification comparison (US3)

- [X] T010 [US3] Remove the #29, #31 and #35 entries from models/examples/purchase_approval/report.py (FR-008). Update the C19 test docstring and its known-limitation path, which no longer applies.
- [X] T011 [US3] Run 506 full: C19 `passed`, with evidence of module hash `a04ba37…`, verifier 0.10.0 and both uniqueness checks proven (FR-007). Run 507 full: 12 obligations passed. Archive `evidence/0.15.0-506-full-report.json`, `evidence/0.15.0-507-full-report.json` and `evidence/verification-comparison.md` (FR-004).

## Phase 6: Baselines and compat (US2)

- [X] T012 [US2] Record the 0.15.0 compat tuples and remove the 0.13.0 tuples (unpinned Core release), checking that every case hash is identical to the 0.13.0 tuple's (FR-005).
- [X] T013 [US2] Retake specs/507-work-queue/baseline/compat-print.txt and 506-report-quick.json for the 0.15.0 pin. Add a provenance note naming the 508 comparisons, and keep both baseline tests unchanged in strictness (FR-006).

## Phase 7: Docs and gates

- [X] T014 Update models/README.md (C19 now proven on 0.15.0), the readiness texts, and specs/506 comparison notes that call #31 open (stale-doc fixes only; no historical evidence rewritten).
- [X] T015 Run `scripts/gates.sh` in quick and full mode, then `conformance.sh 506 complete` and `conformance.sh 507 complete` (FR-012). Record the results in implementation-log.md.
