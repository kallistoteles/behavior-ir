---

description: "Tasks for feature 506: end-to-end behaviour conformance"
---

# Tasks: End-to-End Behaviour Conformance

**Input**: Design documents from `specs/506-e2e-conformance/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R12 and the risks), data-model.md,
contracts/reference-workflow.md, contracts/comparison.md, contracts/conformance-report.md,
quickstart.md

**Tests**: This feature is a conformance suite, so its scenarios are its tests. Constitution III
applies: each scenario test is written against its `expected.json` entry and is seen to fail
(or to report its non-passed status) before the code that makes it pass. The report machinery
gets self-tests before its implementation.

**Organization**: by user story (spec.md US1–US6). Every scenario test carries the marker
`@pytest.mark.scenario("<id>", level=<n>)`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6

Paths are relative to `~/repos/behaviour-ir`. Commands run inside `nix develop`, after
`scripts/fetch-core.sh` and `maturin develop`.

---

## Phase 1: Setup

- [X] T001 Create the package models/examples/purchase_approval/ with an empty `__init__.py`, and specs/506-e2e-conformance/implementation-log.md with a "Baseline" section recording the `main` commit (`d817840`), the Core pin (0.13.0, `1509497…`) and `behavior engine-info`.
- [X] T002 [P] In scripts/gates.sh, change the pytest gate to `python -m pytest -q python/tests models --ignore=models/examples/purchase_approval`, so the suite runs only through `scripts/conformance.sh` (research R12). Check that `scripts/gates.sh` still reaches `gates: OK`.

---

## Phase 2: Foundational (blocking all stories)

**Purpose**: the authored workflow, the independent expectations, and the report machinery
every scenario reports through.

### Report machinery (contract `conformance-report.md`)

- [X] T003 [P] Write scripts/tests/test_conformance_report.sh covering the seven self-tests of contract conformance-report.md §5. Run a throw-away pytest project in a temp directory that imports the suite's conftest (via `-p` or `rootdir`), with tests that pass, fail, `pytest.skip`, and raise each of `Inconclusive`, `Unverifiable`, `Unsupported` and `Blocked`. Then check:
  1. each outcome maps to its status;
  2. a plain skip maps to `failed`;
  3. a non-passed status without an owner is rejected;
  4. a scenario declared but not run is `failed` ("not run");
  5. a scenario under an open readiness entry is `blocked`;
  6. quick mode passes with an `unsupported` C19 and full mode fails with it;
  7. two runs give byte-identical reports.

  See them fail.
- [X] T004 Implement models/examples/purchase_approval/conftest.py:
  - the `scenario(id, level)` marker;
  - the outcome exception types `Inconclusive`, `Unverifiable`, `Unsupported` and `Blocked`, each with `reason` and `owner`;
  - the readiness registry (data-model §5): `behavior-ir-core#29` and `behavior-ir-core#31`, each with `affects: []` and the R10 reasons;
  - the declared scenario list (reference-workflow.md §3);
  - a `pytest_runtest_makereport` and `pytest_sessionfinish` hook that writes `target/conformance/506-report.json`, overridable with `BEHAVIOR_CONFORMANCE_DIR`. The report is canonical JSON with sorted keys, `separators=(",", ":")` and no timestamp, in the data-model §7 shape, with `mode` from `BEHAVIOR_FULL_PROPERTIES`.

  T003 green.

### Workflow and expectations

- [X] T005 Author the reference workflow in models/examples/purchase_approval/workflow.py, exactly as data-model §1:
  - `Role` {`Requester`, `Approver`};
  - `User` {`name: String`, `role: Role`};
  - `BudgetStatus` {`Open`, `Closed`};
  - `Budget` {`status`, `limit: Int`} with the invariant `limit >= 0`;
  - the command `RequestApproved` {`request: Id<PurchaseRequest>`, `by: Id<User>`};
  - the RequestFlow `purchase_request`: actor `User`; payload `title: String`, `amount: Int`; assignee; creation `create` → `Draft`; `submit` (requester only); `start_review` (`reviewer` input); `finalize_approved` (requires `approved(request)`, emits `RequestApproved{request, by: actor}`); `finalize_rejected` (requires `rejected(request)`);
  - the StateMachine `budget`, `close` Open→Closed;
  - the Approval `purchase_request_approval`: subject = the flow's request entity, actor `User`, `k_A = 2`, `k_R = 1`, Open `request.status == InReview`, Eligible "some `User` with id == actor and role == Approver" (a query), context `actor: Id[User]`;
  - the hand-written action `set_role(user: User, role: Input[Role])`;
  - the declared read `decisions`;
  - `module()` and `SEED` (data-model §1), and `COMMIT_TIME = "2026-10-16T00:00:00Z"`.

  Use only the published model APIs (FR-002).
- [X] T006 [P] Write models/examples/purchase_approval/expected.json by hand from contracts/reference-workflow.md §1–§3: every step's expected result, position, outcome, status and intents, and every scenario's expected value. Add a one-line `"_source"` naming the contract. Values are not copied from a run (FR-023).
- [X] T007 Write models/examples/purchase_approval/harness.py:
  - `fresh_store()`: a v2 genesis with evidence policy `none` from Core's fixtures, and `Store.create(InMemoryBackend(), …)`;
  - `step(store, op)`: evaluate → commit when allowed, returning `(decision, committed_position, error)`;
  - `snapshot(store)`: position, the state identity, and the command stream from `commands_since`;
  - the oracle predicates `unique_per_actor(rows)` and `thresholds_exclusive(rows, k_a=2, k_r=1)` over the `decisions` read (VO-003, VO-004; research R6).

  Python decides nothing that Core decides.

**Checkpoint**: an empty suite produces a report listing every declared scenario as `failed`
("not run").

---

## Phase 3: User Story 1 - The purchase approval workflow runs end to end (Priority: P1) 🎯 MVP

**Goal**: history H of reference-workflow.md §1 runs through the public interface and matches
`expected.json`.

**Independent Test**: `python -m models.examples.purchase_approval.run` exits 0, and C01, C03,
C04, C05 and C06 pass.

- [X] T008 [P] [US1] In models/examples/purchase_approval/test_l1_model.py, add a C01 test: the module builds and is admitted, and every admitted item is an ordinary enum, entity, derived item, action, read or command (no Approval construct).
- [X] T009 [P] [US1] In models/examples/purchase_approval/test_lifecycle.py, add tests driving history H through `harness.step`:
  - C03 (step 1 committed at position 1, valid state read back);
  - C04 (step 4 ALLOW, PENDING);
  - C05 (steps 7 and 10: APPROVED, `Approved`, exactly one intent `RequestApproved{r1, alice}`);
  - C06 (steps 16 and 19: REJECTED, `Rejected`, no new intent).

  After every committed step, assert `unique_per_actor` and `thresholds_exclusive`, and that steps 4, 7 and 16 leave the request status unchanged (reference-workflow.md §2). Expectations come only from expected.json.
- [X] T010 [US1] In models/examples/purchase_approval/harness.py, record portable invocation evidence along H. For steps 4 (ALLOW), 6 (DENY, not eligible) and 10 (ALLOW with an intent), also run `invoke(model, document, snapshot)` against the same parent: a requested-invocation document, plus the complete snapshot exported from the store at that position (`Store.export_seed_at` or the store's snapshot of the parent). Keep the resulting `InvocationRecord`s in `HISTORY_RECORDS`. Assert that each record's result equals the Store decision's result at that step. C13b, C14 and C16b use these records.
- [X] T011 [US1] Implement models/examples/purchase_approval/run.py, the executable lifecycle (FR-001, FR-020). It authors, admits, creates the store, runs H, reads the final state and command stream, and runs `replay_data` and `replay_behavior`. It prints each step with its expected and actual outcome, and exits non-zero on any difference. Share H's operation list with T009 through a `HISTORY` definition in harness.py.

**Checkpoint**: MVP. The full lifecycle runs and matches its stated expectations.

---

## Phase 4: User Story 2 - Refusals and safety at the right boundary (Priority: P1)

**Goal**: every refusal happens at its boundary and leaves no trace (FR-009, FR-010, FR-017).

**Independent Test**: the L2 refusal scenarios, VO-005 and VO-001 pass.

- [X] T012 [P] [US2] In models/examples/purchase_approval/test_l2_evaluation.py, add tests:
  - C07 (step 5 DENY, the snapshot unchanged);
  - C08 (step 6 DENY, the snapshot unchanged);
  - C09 (direct `evaluate` with the `User` universe removed from `facts` → the refusal code `INVALID_STATE_SNAPSHOT` from Core's documented catalogue, with no completion);
  - C09.input (approve without `decision_id` → a structured refusal, not a DENY decision).

  Each asserts the error kind, so business denial, invalid input and infrastructure failure stay distinguishable.
- [X] T013 [P] [US2] Add the C12 tests to models/examples/purchase_approval/test_l2_evaluation.py, using the 503 child-state technique (see the tests `a_second_decision_by_one_actor_is_denied_on_the_child_state` and `a_decision_reaching_both_thresholds_is_denied_on_the_child_state` in crates/behavior-models/src/approval/lower.rs). Directly evaluate a decision on seeded parents (a duplicate `(subject, actor)` row; a parent where the decision makes both thresholds hold) and expect DENY at postcondition `[0]` or `[1]`. If the reference module's guards make the postcondition unreachable without bypassing public interfaces, raise `Unsupported(reason, owner="ecosystem")` (research risk 1). Never pass silently.
- [X] T014 [P] [US2] In models/examples/purchase_approval/test_l1_model.py, add negative controls:
  - VO-005: building the module plus a hand-written action that writes a decision field fails with `UNAUTHORIZED_DECISION_WRITE`;
  - VO-001: a genesis seeding `Budget.limit = -1` is refused with `GENESIS_INVALID`;
  - S₀ satisfies `unique_per_actor` and `thresholds_exclusive`;
  - `FR-004.references`: an Approval whose subject is a class that is not a declared entity, and one whose actor is an enum, are each rejected at build time with a structured error naming the approval (FR-004; the 505 rule that every reference resolves to exactly one declaration of the expected kind).

**Checkpoint**: enforcement is shown to be active.

---

## Phase 5: User Story 3 - The Store's commit contract under contention (Priority: P1)

**Goal**: C10, C11, atomic publication, C17 and backend conformance (FR-011, FR-012, FR-013,
FR-018).

**Independent Test**: test_l3_store.py passes.

- [X] T015 [P] [US3] In models/examples/purchase_approval/test_l3_store.py, add tests:
  - C10 (steps 7–9: x4 commits; x5 raises `StateConflict` with `STATE_CONFLICT`; the history grows by exactly 1; the re-evaluated x5 is DENY by the terminal guard);
  - C11 (steps 12–15: the stale x6 conflicts after `set_role`; the re-evaluation is DENY, not eligible);
  - FR-013 (after each failed commit in H, `harness.snapshot` is unchanged, including `commands_since`).
- [X] T016 [P] [US3] In models/examples/purchase_approval/test_l3_store.py, add tests:
  - C17 (`Store.create` twice on one backend with the same genesis → the same `store_id` and history; with a different genesis → refused);
  - FR-018 (`run_conformance(InMemoryBackend)` reports `ok`, otherwise `failed` with its failed checks);
  - `FR-009.infrastructure`: a test backend wrapping the public backend protocol fails its write on the commit of step 4. The failure surfaces as an infrastructure error, which is neither a DENY decision nor an invalid-input refusal, and nothing is published (`harness.snapshot` unchanged). If the backend cannot be wrapped through public surface, raise `Unsupported(owner="ecosystem")` (research risk 3).

---

## Phase 6: User Story 4 - Replay reconstructs, and verification detects tampering (Priority: P2)

**Goal**: data replay, behavior replay, commit-history consistency, integrity controls and
static verification, each a separate check (FR-014, FR-015, FR-025).

**Independent Test**: test_l4_replay.py passes in full mode.

- [X] T017 [P] [US4] In models/examples/purchase_approval/test_l4_replay.py, add tests:
  - C13a (`replay_data(store)` succeeds, and its recomputed head identity equals `current()`);
  - C13b (`replay_behavior(store, [module()])` succeeds for every transition; every record in `HISTORY_RECORDS` (T010) replays with `replay_invocation` and matches);
  - C13.history (over `history()`: contiguous positions 1..13, each parent is the previous head, and each entry's changes equal its decision's effects).

  Missing historical inputs raise `Unverifiable`, never pass.
- [X] T018 [P] [US4] Add the C14 negative controls to models/examples/purchase_approval/test_l4_replay.py, one check each:
  - `C14.altered_facts`, `C14.altered_input` and `C14.altered_result`: a copy of a `HISTORY_RECORDS` entry (T010), mutated, makes `replay_invocation` report a mismatch;
  - `C14.wrong_module`: replay against `module()` with one changed declaration is a mismatch or refusal;
  - `C14.missing_context`: a required context field removed is refused;
  - `C14.stored_transition`: a wrapping test backend over the public backend protocol alters one stored change, and `replay_data` fails. If the backend cannot be wrapped through public surface, raise `Unsupported(owner="ecosystem")` (research risk 3).
- [X] T019 [P] [US4] Add C19 to models/examples/purchase_approval/test_l4_replay.py. In full mode, `verify(module(), Profile())`:
  - `verified` → pass;
  - a finding → fail;
  - an unknown or timeout → `Inconclusive(owner="core")`;
  - `solver_notice()` not None → `Unsupported(owner="environment")`.

  Record the attestation hash, verifier version and profile in the report entry's reason. In quick mode, raise `Unsupported("static verification runs in full mode", owner="environment")`.

---

## Phase 7: User Story 5 - Determinism and cross-path agreement (Priority: P2)

**Goal**: C02, C15, C16a, C16b and C18 (FR-003, FR-016; contract `comparison.md`).

**Independent Test**: test_l5_agreement.py and the Rust test e2e_reference pass, with both C16a
halves present.

- [X] T020 [P] [US5] Write crates/behavior-models/tests/e2e_reference.rs: construct the reference workflow independently with the Rust model authority, following data-model §1 and the fragment-source style of tests/common/ap.rs and rf.rs. Do not read Python output. Compute the authoring identity `N_A` (comparison.md §1): BehaviorHash, item name → hash for every admitted item, model hashes and versions from the receipts, and Approval's `generated`. Write it as canonical JSON to `$BEHAVIOR_CONFORMANCE_DIR/c16a-rust.json`. When the variable is unset, use the workspace root's `target/conformance/`, resolved as `concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/conformance")`, because `cargo test` runs in the crate directory. Assert it is stable across two builds.
- [X] T021 [P] [US5] In models/examples/purchase_approval/test_l5_agreement.py, add tests:
  - C02 (two builds give equal `N_A`);
  - C16a (write `c16a-python.json` and compare it with `c16a-rust.json`; raise `Unsupported("Rust half not run", owner="ecosystem")` if absent);
  - C18 (add a case to `Role` → the `purchase_request_approval` and `purchase_request` model hashes change, and the `budget` model hash does not).
- [X] T022 [P] [US5] Add C16b to models/examples/purchase_approval/test_l5_agreement.py. Write `to_wire_json()` and, for steps 4, 5, 6 and 10 and C09.input, the explicit request or invocation documents with complete parent snapshots to `tmp_path`. Run Python `evaluate`/`invoke`, and the bundled CLI `python/behavior/_bin/behavior eval|invoke` in a subprocess (`behavior._cli.binary()`). Compare `N_E` (comparison.md §2): canonical record JSON equal; refusals as `{error: code}`. A differing code is a failure, never normalized away.
- [X] T023 [P] [US5] Add C15 to models/examples/purchase_approval/test_l5_agreement.py: run H in two fresh stores and compare the record ids, positions, state identities and command streams.

---

## Phase 8: User Story 6 - Auditable report, CI and documented scope (Priority: P3)

**Goal**: one runner, gate integration, a CI artifact and executable documentation (FR-020,
FR-021, SC-011).

- [X] T024 [US6] Write scripts/conformance.sh `[quick|full]` (default `quick`, or `full` when `BEHAVIOR_FULL_PROPERTIES=1`). It exports `BEHAVIOR_CONFORMANCE_DIR` as the absolute path `$(pwd)/target/conformance` before both halves, so the Rust and Python halves write to and read from the same place:
  1. `cargo test -q -p behavior-models --test e2e_reference`;
  2. `python -m pytest -q models/examples/purchase_approval`;
  3. print `summary` from the report.

  Exit non-zero by the contract conformance-report.md §4 rules: quick fails on any `failed`; full fails on any status other than `passed` or `blocked`.
- [X] T025 [US6] Add the gate `gate "conformance" scripts/conformance.sh` after the "pytest" gate in scripts/gates.sh.
- [X] T026 [US6] In .github/workflows/ecosystem-ci.yml, after the gates step, upload `target/conformance/` with `actions/upload-artifact` pinned by full commit SHA, with the version in a trailing comment as the file's other actions have it. Use `if: always()`. Check that `scripts/check-workflows.sh` passes.
- [X] T027 [P] [US6] Add a "Purchase approval conformance (feature 506)" section to models/README.md: what the reference app is, how to run `run.py` and `scripts/conformance.sh`, which guarantees are demonstrated and which are not (quickstart.md "What is demonstrated").

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T028 Run quickstart.md scenarios 1–6. Record the results, the report summaries for both modes and any `unsupported` or `blocked` checks with their reasons in specs/506-e2e-conformance/implementation-log.md, together with deviations and the resolution of research risks 1–3.
- [X] T029 Run `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh` and `scripts/gates.sh`. Both must give `gates: OK`. Also check that `cargo clippy --all-targets -D warnings` covers e2e_reference.rs, that mypy is clean if the files fall under its scope, and that the report is byte-identical across two runs (FR-019).

---

## Dependencies & Execution Order

- **Setup** (T001–T002) comes before everything.
- **Foundational** (T003–T007) blocks all stories. T003 comes before T004. T005 comes before T006 and T007.
- **US1** (T008–T011) comes first. It holds the MVP, the shared `HISTORY` and the portable records `HISTORY_RECORDS` (T010), which US4 and US5 use.
- **US2, US3, US4 and US5** each depend on Foundational and on T010–T011 (`HISTORY` and `HISTORY_RECORDS`). They are independent of each other and live in different files.
- **US6** (T024–T027) depends on the suite existing. T024 comes before T025, which comes before T026.
- **Polish** (T028–T029) comes last.

## Parallel Opportunities

- T002 can run alongside T001. T003 can run alongside T005 and T006.
- After T011, the test files test_l2_evaluation.py, test_l3_store.py, test_l4_replay.py and test_l5_agreement.py, and the Rust file e2e_reference.rs, can be written in parallel: T012–T023, with tasks in the same file running sequentially.
- T027 (README) can run alongside T024–T026.

```text
# After US1:
Task: "T015 [US3] C10, C11, FR-013 in test_l3_store.py"
Task: "T017 [US4] C13a, C13b, C13.history in test_l4_replay.py"
Task: "T020 [US5] Rust authoring in crates/behavior-models/tests/e2e_reference.rs"
```

## Implementation Strategy

1. **MVP (US1)**: the workflow, its expectations, the report machinery and the lifecycle run.
   Every other scenario then reports `failed` ("not run") until its story lands, so the report
   is honest from the start.
2. **US2 and US3**: the P1 safety and commit-contract evidence.
3. **US4 and US5**: replay, verification and the cross-path agreement.
4. **US6 and Polish**: gate, CI artifact, documentation and the final runs in both modes.

## Phase 10: Convergence

- [X] T030 Record, for every C19 outcome including `passed`, the verified module's identity (its BehaviorHash), the attestation hash, the verifier and solver versions, the profile (`checks`, `rlimit`) and the attestation's per-check results in the report entry, in models/examples/purchase_approval/test_l4_replay.py and models/examples/purchase_approval/report.py, per FR-025 (partial)
- [X] T031 Add step 5 (the duplicate-decision DENY) to the execution-conformance operations compared between Python and the released CLI in models/examples/purchase_approval/test_l5_agreement.py, per contracts/comparison.md §2 and contracts/reference-workflow.md §3 C16b (partial)
- [X] T032 Bring quick mode of scripts/conformance.sh within the plan's ~30 s budget (now ~42 s), for example by sharing the session walk of H across the prefix-based tests in models/examples/purchase_approval/test_l2_evaluation.py and test_l3_store.py, without weakening any check, per plan: Performance Goals (partial)
- [X] T033 Justify the binding change in python/behavior/module.py (items with parameters added before their scope closes under the command profile), which no 506 artifact requested, by recording it as a separate ecosystem bug fix in the PR description alongside its regression test, per plan: Constraints (unrequested)
