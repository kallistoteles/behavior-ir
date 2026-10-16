# Implementation log: End-to-End Behaviour Conformance (feature 506)

## Baseline (T001)

- Branch `506-e2e-conformance`, from `main` `d817840` (after feature 505).
- Core pin: 0.13.0, `15094972191aa0ae79705df0397c551a18745399`.
- `behavior engine-info` (the bundled released CLI): engine 0.13.0, verifier 0.9.0, records
  0.4–0.8, wire IR 0.1–0.7 (accepted 0.4–0.8), store documents up to `commit_bundle.v2` and
  `transition_record.v2`.

## Findings during implementation

- **Ecosystem bug fixed: items with parameters under the command profile.** A module with a
  command (and therefore the command profile) failed to build with `SCOPE_MISMATCH` when it
  also had an entity invariant, an entity constraint or a hand-written action with
  parameters. Core checks under this profile that a condition's or effect's bindings belong to
  the open scope, but the binding added each item after closing its scope
  (`python/behavior/module.py` `_trace_invariant`, `_trace_action`). Each item is now added
  before its scope closes. Regression test:
  `python/tests/test_query_command_profile.py::test_items_with_parameters_build_under_the_command_profile`.
  It failed first on the invariant and then on the action; reads were already correct.
- **C19, static verification, is INCONCLUSIVE.** Core's verifier 0.9.0 (z3 4.16.0) reports the
  Approval uniqueness postcondition of `purchase_request_approval__approve` and `__reject` as
  `inconclusive (counterexample_not_reproduced)`. The solver finds a model in which the child
  state has two decisions for one subject and actor, and evaluation cannot reproduce it. Such a
  parent already holds a duplicate, which is unreachable by induction (503's argument). The
  verifier does not know the inductive premise, because Approval's laws are child-state
  postconditions, not declared state invariants.

  **Decision (2026-10-16): B.** The issue is filed as behavior-ir-core#35, which asks how Core
  should formally verify an inductive preservation theorem (base state, complete ownership,
  local preservation, preservation by other actions) and leaves the mechanism open. Option A,
  declaring the laws as module invariants, was rejected for three reasons:
  - 503 established that Core's module-invariant language cannot express the grouped laws;
  - it would change Approval's semantics;
  - Approval's semantics and hashes are to stay unchanged.

  C19 runs and reports `blocked` by core#35 with the exact solver result, never `passed`. Any
  other verifier result keeps its own status. The integration gate (`full`, which CI runs)
  passes with the recorded blocker. Full conformance (`scripts/conformance.sh complete`, and
  the report's `complete` field) stays not passed until C19 has a supported proof or the
  requirement is formally revised.
- **An `InMemoryBackend` handle is not shared between stores.** A second `Store.create` on the
  same Python handle starts a separate store at position 0, because each Store holds its own
  copy. Core's creation-recovery contract is covered for it by `run_conformance` (FR-018).
  C17 runs on the Python reference backend (`DictBackend`), whose handle is the backend itself.

## Deviations, with reasons

- **C12** cannot reach the child-state postconditions through any public interface: the
  generated guards deny first. `scripts/conformance.sh` runs the model authority's own
  child-state tests (`cargo test -p behavior-models --lib child_state`, which evaluate the
  postcondition-only layout through Core) and records the result in `c12-rust.json`. C12 passes
  on that evidence, and without it it is `unsupported`. This resolves research risk 1.
- **C14.stored_transition and FR-009.infrastructure** use the Python reference backend
  (`python/tests/test_store_conformance.py::DictBackend`), because the `InMemoryBackend`
  handle is opaque. This resolves research risk 3.
- **FR-018** (`run_conformance`, about 12 s) runs in full mode only, like C19. Quick mode
  reports both as `unsupported` (owner: environment). CI runs full mode.
- **FR-004.references** is refused by the binding's definition check (`BehaviorDefinitionError:
  … is not an @entity class`), naming the offending type rather than the approval.
- **C16b request bytes.** A refused request is recorded verbatim, so the CLI is given the same
  canonical bytes the binding sends. With other whitespace the records would differ in the
  embedded request text only.
- **The reference workflow** adds a declared read `request_outcome` (the request's status and
  the three Approval predicates), so outcomes are read through Core's derived predicates.
- **Quick mode** took about 42 s, against a target of about 30 s, and now takes about 32 s
  (T032). The read-only prefix tests share one store per step, the wire IR is parsed once, and
  the shared walk compares the exact head (position, state, record) after each refused attempt
  instead of querying the command stream. Commands are published only with a commit record, and
  the stream is checked once against the expected intent.

## Quickstart (T028)

| # | Command | Result |
|---|---|---|
| — | `python -m models.examples.purchase_approval.run` | Exit 0. All 24 operations of H match; final position 13; one intent `RequestApproved{r1, alice}`; data and behavior replay ok |
| 1 | `scripts/conformance.sh quick` | Exit 0: 36 passed, 2 unsupported (C19 and FR-018 run in full mode) |
| 2 | `scripts/conformance.sh full` | Exit 0: 37 passed, 1 blocked (C19 by behavior-ir-core#35, with the exact solver result); "full conformance: NOT complete" |
| 2b | `scripts/conformance.sh complete` | Exit 1, as specified until core#35 is resolved |
| 3 | report summary | Quick: `failed` 0. Full: `failed`, `inconclusive`, `unverifiable` and `unsupported` all 0 |
| 4 | `scripts/tests/test_conformance_report.sh` | OK |
| 5 | `scripts/gates.sh` | `gates: OK` |
| 6 | quick mode run twice, and once from another working directory | Byte-identical reports (FR-019) |

## Gates (T029)

- `scripts/gates.sh` (quick): `gates: OK` in 7 m 50 s. pytest gives 547 passed, 4 skipped and 3
  xfailed; conformance gives 36 passed and 2 unsupported. Log SHA-256
  `116302f7bd358b2dc57fc6353d679ca53bb1425ef15d037621cf1f307897a56d`.
- `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh` (full, as CI runs it): `gates: OK` in 12 m 45 s.
  Conformance gives 37 passed and 1 blocked (C19 by behavior-ir-core#35), and full conformance
  is not complete. Log SHA-256
  `4c3ccc9328be10b6fb00c6df65d0c61cfae17edbd989fab7e9eb1f5a885f1283`.
- No change to behavior-ir-core, Approval, the model kinds or the Store protocol (FR-024). The
  only change to package code is the binding fix for items with parameters under the command
  profile (Findings).

## Convergence (T030–T033)

- **T030 (FR-025):** C19's report entry carries `evidence` for every outcome: the verified
  module's BehaviorHash (asserted equal to the attested one), the attestation hash, the
  verifier and solver versions, the profile, every check's outcome (`cached` excluded, as it
  depends on the run) and the findings.
- **T031 (C16b):** step 5 (DENY: a second decision by one approver) is compared between Python
  `evaluate` and `behavior eval`, alongside steps 4 and 6, the incomplete snapshot and the
  missing input.
- **T032 (performance):** quick mode went from about 42 s to about 32 s, as described above.
- **T033 (PR notes):** the binding change in `python/behavior/module.py` is a separate ecosystem
  bug fix that 506 did not request but needed. Under the command profile, entity invariants,
  constraints and hand-written actions are now added before their scope closes. Its regression
  test is
  `python/tests/test_query_command_profile.py::test_items_with_parameters_build_under_the_command_profile`.
  The PR description lists it on its own.

## Review fixes

- **[P2] A report also when the Rust half fails.** `scripts/conformance.sh` exited at once,
  under `set -e`, when the C16a Rust test failed or did not compile, and left no report. It now
  records the failure (`c16a-rust.failed`, with the log), and C16a reports `failed` with the log
  tail. The pytest suite still runs, the report is written, and the script then exits non-zero.
  A missing report is itself an error. Checked with a `cargo` that always fails: the report has
  C16a and C12 `failed`, every other scenario ran, and the exit status is 1.
- **[P2] The lifecycle command checks the whole final-state oracle.** `run.py` compared only
  the final position and the command stream. It now compares the request statuses, the
  decisions, the roles, the budget state, the position and the command stream with
  `expected.json`, and exits 1 on any difference. Checked with the expected final status of
  r1 changed to Rejected: exit 1.
- **[P2] pytest failures propagate.** A failure outside a scenario test (an unmarked test, a
  collection error) is not in the report, and `scripts/conformance.sh` ignored pytest's exit
  status. It now fails when pytest fails, and the report is still written. Checked with an
  unmarked failing test added through `PYTEST_ADDOPTS`: exit 1 ("pytest exited 1").
- **[P2] The lifecycle command checks every step's position and commit outcome.** Each walk
  result now records its kind:
  - an ordinary step is checked on its result, its commit refusal (none expected unless stated)
    and its position;
  - a kept candidate is checked on its decision;
  - the commit of a kept candidate is checked on its refusal and position.

  `test_every_step_of_h_matches_its_expectation` uses the same rule. Checked: with step 1's
  expected position set to 999, or step 13 given an unexpected conflict, `run.py` exits 1.
- **[P2] Only the pinned CLI.** C16b and the report used `behavior._cli.binary()`, which honors
  the development override `BEHAVIOR_CLI_BINARY`, so a wrapper reporting another engine could
  pass C16b. The suite now runs the pinned bundled CLI (`python/behavior/_bin/behavior`, whose
  digest `scripts/fetch-core.sh` checks) and asserts that its `engine-info` reports the pinned
  Core release. `scripts/conformance.sh` rejects an active override (exit 2), as the
  constitution requires of official scripts.
- **[P2] Portable records agree with the Store and the expectations.** A new check,
  `C13.records_agree`, compares each portable `InvocationRecord` (T010) with the Store's
  decision at that step and with expected.json. Replay and CLI agreement alone confirm only a
  record's self-consistency. Checked: building step 6's record with actor carol (ALLOW, while
  the Store and the expectation say DENY) makes the check fail.
