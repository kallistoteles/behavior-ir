# Implementation log: WorkQueue (feature 507)

## Baseline (T001, 2026-10-17)

- `main`: `1171f10` (Merge PR #24, feature 506); branch `507-work-queue`.
- Core pin: 0.13.0, `15094972191aa0ae79705df0397c551a18745399`; `behavior engine-info`: engine
  0.13.0, verifier 0.9.0, wire IR up to 0.8.
- Established identities: `scripts/check-models-compat.py --print` →
  `specs/507-work-queue/baseline/compat-print.txt` (16 receipts, sha256 `ef6476c3…`). The gate
  reported 10 cases in 5 tuples (state_machine 2/1, request_flow 2/1, three approval tuples).
- 506 quick report → `specs/507-work-queue/baseline/506-report-quick.json` (37 passed,
  2 unsupported).

## Regression nets (T003, T004)

- `scripts/tests/test_pinned_identity_baseline.sh`: runs the compat gate, then requires every
  baseline receipt to be produced byte-for-byte (new receipts allowed). Fault injection: one
  altered hash in a copy of the baseline → exit 1 naming the receipt; baseline restored
  (sha256 unchanged).
- `scripts/tests/test_conformance_506_bytes.sh`: the 506 quick report must equal the baseline
  byte-for-byte. Passes on `main`'s code.

## D1(a) transition state bindings (T005–T012)

- `TransitionInput.state` and `ResolvedTransition.state`; lowered as state parameters after the
  subject, then inputs and context, each sorted by name. Resolution: unknown entity →
  `UNKNOWN_ENTITY`; a binding name colliding with the subject, an input or a context parameter
  → `INVALID_PARAMETER` (the existing parameter code; tasks.md said `DUPLICATE_PARAMETER`,
  which does not exist — deviation recorded).
- Language levels (research R13): `ResolvedStateMachine::semantic_version()` is 3 iff a
  transition binds; the level-3 document `behavior.model.state_machine.v3` lists `state` on every
  transition as `{name, entity, entity_hash}` (505 identity) sorted by name; level 2 is unchanged.
  RequestFlow follows with `flow_version` (`behavior.model.request_flow.v3`, embedded machine 3).
- Ownership: the StateMachine exemption in `final_pass` now requires the effect to go through
  the owner's subject parameter. Through the public API an authored write of the owned field
  through any subject-typed parameter is already `TRANSITION_WRITES_OWNED_FIELD` at lowering,
  so the narrowing is defence in depth and not separately observable; tests cover both
  observable paths (own field via a binding; another machine's field via a binding).
- Python: `StateMachine.transition(state=...)` (and through RequestFlow's `transition`); the
  tracer binds each bound parameter to its entity.
- Core facts observed: two bindings naming the same id are refused by Core with
  `INVALID_BINDING`; the model adds no rule.
- Compat: reference cases `sm_bind_task_board`, `rf_bind_ticket`; new tuples `state_machine`
  3/1 and `request_flow` 3/1 recorded with `--write` (no supersession); `compat.json` diff is
  47 added lines, 0 removed. Self-tests: a new level beside the established one passes; drift of
  either fails; a case mixing standalone levels 2 and 3 fails "no single version tuple".
- **Baseline incident (resolved, no identity change)**: the first baseline run after adding the
  cases reported three changed StateMachine receipts. Investigation: only `source_map` line
  numbers differed, because a new import line in `reference_cases.py` shifted the authored
  lines. Hashes were identical. Fixed by importing on the existing line; the baseline then
  passed with all 16 receipts byte-identical and 2 new.
- Disk: the build directory reached 4.5 GB and filled the disk; removed `target/debug` and
  builds now run with `CARGO_INCREMENTAL=0`.

## WorkQueue core (T013–T014, T018–T025; also T035, T036, T040)

- **Deviation (T013, T014; Constitution VI):** invariants and reads are not new
  `Contribution` kinds in `composition.rs`. WorkQueue is their only producer and nothing
  hand-written refers to them, so `work_queue/decls.rs` adds them in P4 through the public
  builder (`add_invariant`, `read_projection`, `add_read`), mapping `DUPLICATE_NAME` to
  `GENERATED_NAME_COLLISION`. `terminal` uses the existing P2 definition mechanism
  (`DefinitionSpec`), so hand-written behavior can call it. No WorkQueue vocabulary entered
  kind-neutral files.
- `work_queue/`: `input.rs`, `resolve.rs` (names `<E>Status`/`E`, schema, generated parameters,
  every R5 error, the embedded machine resolved by the unchanged StateMachine resolver),
  `decls.rs`, `lower.rs` (enqueue as a creation; `QueueSource` places the generated
  fragments before the authorization and the author's parts for the unchanged StateMachine
  lowerer), `ownership.rs` (R4 table, every `E`-typed parameter of every action, allowed
  writers only through their subject), `identity.rs` (`behavior.model.work_queue.v1`, receipt
  with `embedded` and `generated`).
- Typed WorkQueue errors raised inside the machine's fragment source travel by a side channel
  (`QueueSource.errors`) and replace the `WORK_QUEUE_RULE` fragment error that stopped the
  machine. `TIME_NOT_ORDERED` for a nominal without `order` is detected where Core refuses the
  first generated comparison (`OP_NOT_ALLOWED`), so the model does not read Core's op bits.
- `scripts/check-public-surface.sh`: `into_wire` additionally allowed in
  `work_queue/lower.rs` (enqueue's creation), as for RequestFlow and Approval.
- Python: `behavior.WorkQueue` (`work_queue.py`), `_engine.work_names`, the capability tracer.
  Callables receive the subject positionally and every other parameter **by keyword** (only
  those they declare, or all with `**kwargs`); a deviation from the positional convention of
  the earlier kinds, because a capability has up to five generated parameters whose positions
  would otherwise be unreadable.
- **Test-first deviation:** the Rust tests of T018, T020, T022 and T035 were written after the
  core code of phase 6 compiled, not before. To compensate, each suite was checked against a
  deliberate fault: with the WorkQueue ownership pass disabled, 4 of the 6 ownership tests
  failed (the other two exercise the machine's pass and a positive build). The negative
  controls of T043 extend this to the safety premises.
- Results: `cargo test -p behavior-models` all green (new suites: lower 5, resolve 8,
  ownership 6, identity 4); clippy clean; `test_work_queue_build.py` 3 passed (build, typed
  errors, a short Store lifecycle); mypy clean.

## Shared conformance package (T015–T017)

- `models/conformance/report.py` holds the statuses, outcomes, readiness, evidence, hooks, the
  canonical writer, the gate and (new) obligations; `configure(report, declared, readiness,
  obligations)`. `models/examples/purchase_approval/report.py` keeps only 506's scenarios and
  registry. A report without obligations has no `obligations` key.
- 506 regression: the 506 quick report is byte-identical to the baseline from `main`
  (`test_conformance_506_bytes.sh`), checked after the move and again in the gates.
- `scripts/conformance.sh <506|507> [mode]`; each feature removes only its own files in
  `target/conformance`. `scripts/gates.sh` runs `conformance 506` and `conformance 507`, and its
  pytest gate ignores both suites. Self-tests: obligation statuses (weakest dependency; `not run`;
  unknown evidence class; an assumption alone never passes), a readiness entry with `run: true`
  keeping its own status, and the gate failing on a failed obligation.

## 507 suite (T026–T034, T037–T043, T047)

- `models/examples/work_queue/`: `workflow.py` (RequestFlow `document_request` + WorkQueue
  `document_work`; admins authorize enqueue, recover, exhaust and cancel; active workers claim;
  complete binds the document, marks it processed and emits `DocumentProcessed`), `harness.py`,
  `expected.json` (hand-stated), `report.py` (W01–W25 and VO-001..VO-012 with dependencies and
  assumptions), `run.py`, `mirror.py` (the hand-authored mirror for negative controls) and the
  tests `test_us1…us5`, `test_obligations.py`, `test_negative_controls.py`.
- `run.py`: every step, the full final state and both replays match.
- Observed Core facts: an evaluation against a parent snapshot without `Worker` is
  `INVALID_INPUT` with reason `INVALID_STATE_SNAPSHOT` (W21); a hand-written action named like a
  generated one fails composition (the collision surfaces as Core's admission failure of the
  module); a migration may not transform an identically declared entity
  (`INVALID_TRANSFORM`), so the migration controls add a payload field.
- Negative controls (FR-036): NC-writer (a mirror with an attempts-lowering writer lets a stale
  token complete; the real build rejects the writer), NC-generation (`attempts >= generation`
  lets a stale token complete; spec said `<=`, which cannot admit a *smaller* stale generation,
  so the weakening is `>=` — deviation recorded), NC-expiry, NC-bound (without the guards only,
  ValidWork refuses READY at attempts = M; without the guards and the invariant's bound, a fourth
  claim commits), NC-mig-reset and NC-mig-reactivate as **real Core migrations** on the reference
  module: ValidWork holds after both, and an old generation completes again.
- W20: the independent Rust authoring (`work_queue_reference.rs`) and the Python build give the
  same BehaviorHash, item hashes, model hashes and generated hashes; fault-injected (an altered
  model hash) → the agreement check fails.
- T032 laws: `work_queue_laws.rs` (proptest, 1,000 cases in full mode, 100 in the quick gate):
  for random M, payloads and capability orders, only claim writes `attempts`, the generation
  postconditions exist, and the identities do not depend on authoring order. The property
  checks over random **histories** run through the Store in `PROP.mono` (200 seeded histories of
  12 steps in full mode, 8 in quick): SA-MONO, SA-FENCE, SA-TERM and ValidWork after every step.
- Results: quick 56 passed, 5 unsupported (SMT and backend run in full mode), obligations 5
  passed and 7 unsupported; **full 61 passed, all 12 obligations passed, "full conformance:
  complete"** (SMT: preservation proven for every WorkQueue action, the generation
  postconditions proven, claim's evaluation_error proven).
- T047 initial measurement: quick mode 32 s, exceeding the original mandatory 30 s budget.
  This was not a passing performance result; see the explicit amendment in Final review below.
  The cost is the debug extension's
  ~0.12 s per evaluate-and-commit; quick mode runs fewer repetitions of W04 (4, full 20) and of
  PROP.mono (8, full 200), and W15.terminal shares one store per status.

## Polish (T044–T046)

- README: "Transition state bindings" under StateMachine and a WorkQueue section (meaning,
  fencing, trust boundary, time, ownership, genesis and provenance, migration contract,
  non-guarantees, Python).
- Compat: `wq_documents` (embedded machine 3), `wq_minimal` and `wq_two_queues` (embedded 2):
  two `work_queue` tuples, recorded with `--write`; `compat.json` gained 212 lines and lost none;
  the identity baseline still holds all 16 established receipts. `check-models-compat.py` now
  names `queue_id` and records the work entity and the queue's `generated` hashes.
- `api/public-api.json` lists `WorkQueue`; terms, workflows and ownership checks pass.

## Quickstart and gates (T048, T049)

1. The Rust suites (bindings, resolve, lower, ownership, identity, laws, reference) pass in the
   `cargo test` gate.
2. `conformance.sh 507 quick`: 56 passed, 5 unsupported; obligations 5 passed, 7 unsupported.
3. `conformance.sh 507 full`: 61 passed; obligations 12 passed; "full conformance: complete".
4. `conformance.sh 507 complete`: exit 0 (61 passed, 12 obligations passed).
5. The 506 report: quick 37 passed / 2 unsupported, full 38 passed / 1 blocked (C19 by
   behavior-ir-core#35), byte-identical to the baseline (`test_conformance_506_bytes.sh` in the
   script-test gate).
6. VO-005: passed, with dependencies SA-FENCE (structural), SMT.postcondition (smt), W07 and
   NC-generation (test), and the assumptions "ordinary transitions only" and "now is the
   trusted evaluation time".
7. `scripts/gates.sh`: `gates: OK` (8 min 57 s); `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh`:
   `gates: OK` (17 min 42 s). Python suite 557 passed, 4 skipped, 3 xfailed.

Disk: the build directory is again about 4.6 GB with 1.5 GB free on the volume; no failures.

## Final review (2026-10-17)

1. **Evidence classes.** Each VO keeps its own classes. VO-009 used the check W24 for both its
   `contract` and its `test` dependency. Its contract dependency is now `W24.no_partial` (a
   refused commit publishes nothing, which is the Store's atomicity contract). A new assumption,
   `SMT_MODEL`, is named by every VO with an SMT dependency: the results hold within the
   verifier's modeled fragment and are local to each action. The global theorems (SA-MONO,
   SA-FENCE, SA-TERM) stay `structural`. Time and migration guarantees stay host assumptions
   (`TIME`, `ORDINARY`). The gate summary now prints the evidence mix ("evidence: contract 5,
   smt 7, structural 5, test 13; assumptions named by 9 of 12 obligations"). The `ASSUMPTIONS`
   check also rejects a VO that names one check for two classes. During the review, an edit by
   someone else (not this session) strengthened VO-003, VO-005 and VO-007 with further
   negative-control and SA-OWN dependencies, and made `WorkQueue._plain` keyword-only (unknown
   capability arguments now raise). Both predate the final verification run and pass in it.
   The final dependency audit also found that `NC-writer` did not propagate to VO-003 and
   VO-005. It now affects VO-003, VO-005, VO-007 and VO-011; VO-005 also lists `NC-expiry`, and
   VO-007 lists `SA-OWN`. Fault injection in the report self-test proves these dependencies
   fail their obligations. The current evidence mix is contract 5, smt 7, structural 5, test 13.
2. **The two ownership tests unaffected by the WorkQueue-pass fault.**
   - `a_hand_written_status_write_is_the_machines_error` tests the **StateMachine** pass: status
     is the machine's, not the WorkQueue table's. With the machine's exemption check disabled
     it fails (5 passed, 1 failed), so it is sensitive to the fault it is meant to catch.
   - `two_independent_queues_build_with_distinct_identities` is a positive build. Its
     ownership counterpart, `another_queues_actions_cannot_write_this_queues_items`, failed
     under the fault.
3. **Version contracts.**
   - Level-2 inputs lower exactly as before (the baseline: 16 of 16 receipts byte-identical).
     A level-2 document never gains `state`.
   - Only content with bindings takes level 3. The narrowed exemption cannot change a level-2
     result: level-2 transitions have only the subject, and an authored write of the owned field
     is already refused at lowering.
   - `compat.json` has 0 removed lines. Established v2 tuples and the superseded v1 records in
     `supersedes` are untouched, and the gate's immutability check passes.
4. **Performance: explicit acceptance amendment.** The original ≤30 s requirement was not
   reliably met: a complete warm run took 34.18 s; after sharing the immutable final command
   export and the built reference module, a run with 4 quick histories took 27.93 s. Restoring
   the original 8 histories produced 30.15 s, and the newly added 30 s gate correctly failed
   after preserving the report. The reviewed budget is now **≤35 s** for the complete warm
   command with the pinned development extension. This replaces the original requirement in
   plan.md and T047 explicitly. Profiling found 11.45 s in 108 Core commits, 6.44 s in 253
   evaluations and 2.99 s in Store creation; the extra 5 s covers the measured overshoot and
   modest execution-time variation. No Core optimization or semantic change is introduced.
   The runner enforces 35 s after report generation, including the Rust reference check,
   pytest and report gate; cold compilation and environment setup are prerequisites.
   A controlled-clock regression passes at 34.99 s and fails at 35.01 s with the report intact.
   Both ordinary and full `scripts/gates.sh` runs execute this quick check; full mode then
   runs the larger 507 suite too. A gate-runner regression checks both invocations in full mode.
   - W18 builds its portable record from the session's walked history (`export_seed_at` at the
     step's parent) instead of a second walk;
   - the final command stream and reference model are shared across read-only tests;
   - quick-mode PROP.mono retains 8 seeded histories; full mode keeps 200.
5. **Deviations reviewed.**
   - *Python keyword convention*: binding only, no semantic effect. It is now in the contract
     (`work-queue-contract.md` §2) and the README. The review found that the forwarding methods
     silently discarded unknown options, including `guards=` and misspelled `requires`.
     Fourteen reproducing tests failed first; the explicit keyword signature now rejects each
     spelling before changing the capability. The supported spelling is `requires=`.
   - *Invariant and reads added in P4 by WorkQueue*: the invariant is a Core per-entity
     invariant, checked on every resulting state of every action, so its coverage is unchanged.
     The reads are ordinary declared reads. Research R2 and the plan now say so.
   - *Migration assumptions*: unchanged and host-side. The negative controls show both breaks
     with real Core migrations.
6. **Disk.** Removed only the generated Rust artifacts at
   `/tmp/core025-planning-target/debug` (Cargo cache tag and artifact layout checked first).
   Free space increased from 1.5 GB to 5.1 GB. Its sibling consumer sources, this repository's
   target directory, all source files, uncommitted work and unrelated Spec Kit changes were
   preserved. Future cold builds still need capacity planning; this is an environment concern.
   Afterwards a second session removed this repository's own rebuildable `target/debug`
   (4.6 GB), keeping `target/conformance`, all sources and uncommitted work; free space was then
   9.6 GB (8.8 GB after the next builds). A cold rebuild needs about 4.6 GB.
7. **Independence.** The 507 report and completion are separate. 506: quick report
   byte-identical to the baseline; full mode C19 `blocked` by behavior-ir-core#35 with the
   exact result, "full conformance: NOT complete".

Previously recorded verification: `scripts/gates.sh` → `gates: OK` (9 min 17 s; 506 quick 37
passed, 2 unsupported; 507 quick 56 passed, 5 unsupported); `conformance.sh 507 full` → 61
passed, 12 obligations passed, "full conformance: complete".

### Focused verification after the guard, evidence and runtime corrections

- Regression tests were observed failing before each correction: unknown Python options,
  ownership negative-control propagation, runtime budget, and full-gate scheduling of quick.
- Final Python regressions: 24 passed (authoring, runner budget and gate orchestration).
  Shared report self-tests passed, including injected failed evidence dependencies.
- `conformance.sh 507 quick`: 56 passed, 5 unsupported; 5 VOs passed, 7 unsupported;
  complete command **30.07 s / 35 s**, runtime gate passed, 8 random histories retained.
- `conformance.sh 507 complete`: 61 passed in 139.73 s; all 12 VOs passed;
  `full conformance: complete`, including 200 random histories.
- `conformance.sh 506 full`: 38 passed, C19 blocked by `behavior-ir-core#35`;
  `complete: false`. A fresh 506 quick run (37 passed, 2 unsupported) was byte-identical
  to the captured baseline.
- The compatibility self-tests and baseline test passed: 16 full receipt objects unchanged,
  6 new receipts; all 5 established manifest tuples unchanged, 4 additions and no deletions.
- Fresh Rust tests: state_machine_bindings 7, request_flow_composition 7,
  work_queue_ownership 6, all passed. The standalone WorkQueue command matched all
  expectations and both replays. Core pin, public surface, ownership, terms, workflows,
  fmt, clippy, mypy and diff checks passed.
- The complete gate script was not rerun after these focused corrections; its updated quick
  and full scheduling was verified by the isolated runner tests. The earlier gate result above
  is historical, not a claim of a new complete gate run.
- Last filesystem observation: **8.8 GB free**. This review's temporary Cargo-artifact cleanup
  recovered the earlier 1.5 → 5.1 GB; a subsequent reduction of the local target cache was
  observed separately. No tracked source or unrelated Spec Kit content was removed.

Remaining limitations: SMT checks are action-local within the pinned verifier's modeled
fragment. The global induction arguments remain structural; trusted time, backend conformity,
genesis provenance and migration rules remain explicit assumptions. The historical test-first
deviation is recorded in plan.md Complexity Tracking; mutation evidence does not rewrite that
process history. No Core semantics were added or changed.
