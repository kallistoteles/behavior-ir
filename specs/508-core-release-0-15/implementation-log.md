# Implementation log: Core Release 0.15.0 alignment (feature 508)

## Baseline on 0.13.0 (T001, 2026-10-17)

- `main` at `62bb349` (507 merged); branch `508-core-release-0-15`.
- `scripts/semantic-snapshot.py` (new): a canonical snapshot of every compat reference case
  (receipts without `core_release`, BehaviorHash) and of the 506 and 507 reference histories
  (steps, positions, every state identity, command streams); `--compare` lists every difference.
- Archived on 0.13.0, before any change: `evidence/0.13.0-semantic.json`,
  `0.13.0-compat-print.json`, `0.13.0-506-full-report.json` (C19 blocked by
  behavior-ir-core#35) and `0.13.0-507-full-report.json`.

## Pin and API (T002, T003)

- `core-release.json`: v0.15.0, commit `6b93898a…`, assets and sha256 from the release's
  `SHA256SUMS`; `Cargo.toml` `rev` and `Cargo.lock` (8 lines) follow. `check-core-pin` and
  `fetch-core` pass.
- The build failed exactly at the 5 places research R1 predicted (`crates/behavior-py/src/lib.rs`:
  authorize, verify, verify_migration, authorize_migration). A new `canonical_err` maps Core's
  `CanonicalError` to the engine error `CANONICALIZATION_FAILED`; nothing is defaulted.

## Release-induced test changes (T004–T008)

The Python suite on 0.15.0 before the fixes: 8 failed, 566 passed. Every failure was caused by
the release:

- `test_core_alignment.py` (4): the expected engine 0.13.0 and verifier 0.9.0 → 0.15.0 and
  0.10.0;
- `test_command_documents.py` (1): fresh authenticated proofs under the 0.9-only policies →
  the `-verifier-0.10` policy fixtures (Q9);
- `test_source_locations.py` (3): the strict xfails for behavior-ir-core#31 now pass, so the
  markers were removed, and the docstrings there and in `python/behavior/location.py` describe
  the 0.14 contract.

`test_binding_equivalence.py`: the #29 snapshot-completion workaround (`EXTRA_ROWS`,
`complete()`) was removed (Q10). The binding now evaluates Core's unchanged `bindings/current/`
requests and compares with the published expectations by **exact** JSON equality, as their
README requires; all ten cases are equal.

## Semantic comparison (T009, Q8.1)

`evidence/0.15.0-semantic.json` is **byte-identical** to `0.13.0-semantic.json`: 0 differences
across 15 reference cases (22 receipts), 24 steps of the 506 history and 10 of the 507 history,
including every state identity and both command streams (`semantic-comparison.txt`). A
fault-injected snapshot (one position changed) yields exactly that difference and exit 1.

## Verification comparison (T010, T011, Q8.2)

- 506 readiness registry emptied: #29 and #31 were fixed in Core 0.14.0 (features 026, 027) and
  #35 in Core 0.15.0 (feature 028). The C19 test's special case for the #35 limitation was
  removed. A Core limitation is now `blocked` only through an explicit readiness entry, and on
  `verified` the test also requires the module `sha256:a04ba37…531779` and both uniqueness
  postconditions `proven`.
- 506 full: 39 passed, "full conformance: complete". C19: verified, verifier 0.10.0, z3 4.16.0,
  attestation `sha256:d862a010…`.
- 507 full: 61 passed, 12 obligations passed, no status change; SMT evidence identical.
- The only status change across both suites is C19 blocked → passed
  (`evidence/verification-comparison.md`).

## Baselines and compat (T012, T013)

- `compat.json`: `--write` recorded 9 tuples for 0.15.0, each with case hashes identical to its
  0.13.0 tuple. The 0.13.0 tuples were then removed under the gate's rule for an unpinned Core
  release. Their `supersedes` lists (the v1 history of the 0.13.0 era) go with them; that
  history stays in git, and copying it would wrongly claim 0.15.0 tuples superseded 0.13.0 ones.
  The gate and its self-tests pass.
- Pinned baselines retaken for 0.15.0 after checking that they differ from 0.13.0 only in
  release metadata: the compat print (22 receipts, equal without `core_release`) and the 506
  quick report (equal without `core_release`, `engine` and `readiness`). The two baseline tests
  are unchanged and pass. The provenance is in `specs/507-work-queue/baseline/README.md`.

## Docs (T014)

`models/README.md` (C19 proven since 0.15.0), the 506 quickstart rows for full and complete,
and the 506 contract notes that called #31 and the C19 limitation current. Historical spec,
research and evidence texts of 506 are unchanged.

## Gates and complete runs (T015)

- `scripts/gates.sh`: `gates: OK` (9 min 3 s). Python 577 passed, 4 skipped (the 3 former
  strict xfails for #31 are now ordinary passing tests). 506 quick: 37 passed, 2 unsupported.
  507 quick: 56 passed, 5 unsupported; runtime 31.36 s of 35 s.
- `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh`: `gates: OK` (17 min 24 s). 506 full: 39
  passed, complete. 507 full: 61 passed, 12 obligations, complete.
- Independent runs: `conformance.sh 506 complete` (39 passed, complete) and
  `conformance.sh 507 complete` (61 passed, 12 obligations, complete).
- Disk after the runs: 1.1 GB free (the build directory is about 4.8 GB).

## Review fixes (PR #26)

- **[P2] JSON types in the semantic comparison.** `--compare` used Python equality for scalars,
  so a type change (0 → false, 1 → true, 1 → 1.0) compared equal. It now compares the JSON type
  first. The new `scripts/tests/test_semantic_snapshot.sh` failed first on three such changes
  and now passes; it also covers list length, a missing key and a string-for-number change.
  The archived 0.13.0 → 0.15.0 comparison still reports 0 differences under the stricter rule;
  the reviewer's case (`history_506.states[0].position` 0 → false) reports 1 difference and
  exit 1.
- **[P3] Executable script.** `scripts/semantic-snapshot.py` is committed with mode 100755, as
  documented; the self-test checks it.
