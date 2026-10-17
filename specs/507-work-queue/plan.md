# Implementation Plan: WorkQueue Model

**Branch**: `507-work-queue` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/507-work-queue/spec.md` (clarified 2026-10-17,
Q1–Q37)

## Summary

WorkQueue is a new model kind in `behavior-models`:

```text
L_WQ(Q) = Compose( Decls(Q), Inv(Q), Defs(Q), Enqueue(Q), L_SM(SM(Q)), Reads(Q) )
  Decls = { <E>Status, E }      Inv = { ValidWork }      Defs = { terminal }
  SM(Q) = (E, status, {READY}, {claim, renew, complete, retry, fail, recover, exhaust, cancel})
  Reads = { ready(now), expired(now) }   (identity-only projections)
```

Planning results:

1. **Every planning gate except one passed by execution** (research R1). A hand-written module
   shaped like the lowering was:
   - admitted;
   - correct at runtime: stale, late and current completions, the identity-only read,
     `GENESIS_INVALID` and `ENTITY_ID_ALREADY_USED`;
   - `verified` by the pinned verifier, with invariant preservation proven for every action, the
     generation postconditions proven, and claim overflow proven impossible.
2. **One facade gap: effects on other entities from transitions** (R3). StateMachine
   transitions bind only their subject. **Decided: D1(a)** (user, 2026-10-17): optional
   transition state bindings as a general model-layer capability. The decision has requirements
   1–8 (R3):
   - the StateMachine ownership exemption is narrowed to the generated status effect through
     the subject;
   - a binding never grants authority;
   - Core is unchanged.

   **Versioning (R13)** uses language levels. StateMachine 3 is the extended language. A
   machine without bindings stays version 2, with an unchanged canonical document and
   mappings. RequestFlow follows the same rule. New tuples are added, nothing is superseded,
   and every established tuple stays byte-identical.
3. **The invariant and the two reads** are added by WorkQueue in P4 through the public builder
   (R2, as implemented; not new common contribution kinds).
4. **Evidence is first-class**: a per-VO dependency record in the report (R10), and the
   reviewable arguments in `contracts/safety-arguments.md`, each premise tied to a check and a
   negative control.
5. **The 506 conformance infrastructure is shared and parameterized** (R9). The 506 report is
   byte-identical after the change.

## Technical Context

**Language/Version**: Rust stable (edition 2024, pinned toolchain); Python 3.13 binding.

**Primary Dependencies**:
- `behavior-engine` (Core 0.13.0, `1509497`), `serde_json`, `thiserror`;
- dev only: `proptest`;
- binding: `pyo3`.

No new dependency.

**Storage**: N/A for the model. Tests use Core's public `Store` with `InMemoryBackend`, plus
the backend conformance suite in full mode.

**Testing**:
- `cargo test -p behavior-models`: resolve, lowering goldens, ownership, identity, the Rust
  reference authoring, and property tests over random capability sequences;
- pytest: the end-to-end Store tests, the 507 conformance suite and the negative controls;
- `models compat`;
- the determinism check.

**Target Platform / Project Type**: unchanged (a library plus a Python extension).

**Performance Goals**:
- 507 quick conformance ≤ 35 s with the pinned development extension and Rust targets already
  built. This explicitly replaces the original ≤ 30 s goal after final review: the full command
  measured 30.15 s after shared-history optimizations, and earlier runs measured 32–34 s.
  Profiling attributes most time to Core evaluation/commit and Store creation. The 35 s limit
  retains all checks, 8 quick histories and 200 full histories, and allows modest runtime
  variation. `scripts/conformance.sh 507 quick` enforces it after writing the report; a slow run
  fails the gate. Both default and full `scripts/gates.sh` runs execute the quick check; full
  mode then additionally runs 507 full. Cold compilation and development-environment setup are prerequisites, not
  part of the intended quick benchmark;
- property suites ≥ 1,000 cases (Rust) and seeded Python sequences ≥ 200.

**Constraints**:
- no Core change;
- `behavior-engine` only;
- no Wire JSON as an interface;
- deterministic outputs;
- 501, 502 and 503 pinned hashes unchanged, including after D1(a);
- the 506 report unchanged.

**Scale/Scope**:
- about 8 new Rust files (`work_queue/`);
- the StateMachine binding extension;
- two contribution kinds;
- one Python module;
- the shared conformance package;
- the 507 example suite;
- fixtures.

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core | No AI. Lowering is a pure function of the input; every decision is Core's. ✅ |
| II. Validated outputs | Binding input is untrusted: typed resolution and typed errors (R5). ✅ |
| III. Test-First | Each phase starts with failing tests, including the regression on every pinned case before D1(a) and the 506 report regression before R9. Property tests cover attempts, fencing and terminality. ✅ |
| IV. Reproducibility | Canonical orders (fields, parameters, capabilities, definitions, reads); the compat tuple; time comes only from recorded context. ✅ |
| V. Explicit state, auditability | The lifecycle is an explicit enum and machine. Receipts map fragments to Core items. Per-VO evidence records. ✅ |
| VI. Simplicity | No new crate or dependency. New common abstractions: transition bindings (D1(a), required by FR-030) and two contribution kinds (required by Q17 and Q19). The report generalization removes duplication instead of copying 506. ✅ |
| Bindings: no core semantics | Python records input. Guards, names, ordering and validation are all Rust's. ✅ |
| `behavior-engine` only | `add_invariant`, `read_projection` and `add_read` are on the public builder. ✅ |
| Gates = `scripts/gates.sh` | The same gates; `conformance` runs 506 and 507 in quick mode; compat gains `work_queue`. ✅ |

**Result: PASS** at design. Implementation deviations and their review are recorded in
Complexity Tracking below; the historical test-first deviation cannot be undone retrospectively.

## Planning gates (spec, Planning gates)

| # | Gate | Result | Evidence |
|---|---|---|---|
| 1 | generated WorkItem and WorkStatus | PASS | R1 (admitted), R2 (contribution path of 503) |
| 2 | creation and embedded StateMachine (self-transition, multi-source) | PASS | R1: `renew` and `cancel` admitted and correct |
| 3 | optional owner and deadline, `is_some`-guarded ordering | PASS | R1 runtime, SMT preservation proven |
| 4 | exact attempt increment and bound | PASS | R1: claim postcondition and `evaluation_error` proven |
| 5 | generation-guarded capabilities | PASS | R1: stale DENY, postconditions proven |
| 6 | local invariant and full-module ownership | PASS | invariant: R1; ownership: R4 (new pass, model layer) |
| 7 | the two reads, identity-only | PASS | R1: `[{"id": "d1"}]`, `now` recorded as a captured fact |
| 8 | command intents and effects on other bound entities | **PASS (D1(a) decided)** | enqueue: RequestFlow precedent; transitions: facade gap (R3) closed in the model layer, no Core change |
| 9 | deterministic ModelHash and receipts | PASS | R6 (502/503 pattern) |
| 10 | Store conformance, replay, verification | PASS | R1 (Store, verifier); 506 infrastructure (R9) |

Gate 8 is classified as a **missing public facade capability** in the model authority. It is
not a missing Core semantic capability: Core actions take any number of state parameters.

## Project Structure

### Documentation (this feature)

```text
specs/507-work-queue/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/
│   ├── work-queue-contract.md   # meaning, authoring, invocation, guarantees, scenarios → checks
│   └── safety-arguments.md      # premises P1–P7, SA-MONO, SA-FENCE, SA-TERM, SA-OWN, SA-EXCL, migrations, genesis
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code

```text
crates/behavior-models/src/
├── composition.rs        # MODIFIED: Contribution::{Invariant, Read} (kind-neutral); final_pass exemption narrowed to the subject parameter (D1(a))
├── kinds.rs              # MODIFIED: AnyModel::WorkQueue; Needs/Contrib; compose dispatch; receipts
├── error.rs              # MODIFIED: R5 codes
├── lib.rs                # MODIFIED: WORK_QUEUE_* versions, re-exports
├── state_machine/
│   ├── input.rs          # MODIFIED: TransitionInput.state (optional bindings)
│   ├── resolve.rs        # MODIFIED: binding resolution (unknown entity, duplicate name)
│   ├── lower.rs          # MODIFIED: binding params after the subject, sorted by name
│   └── (canonical form)  # MODIFIED: `state` only when non-empty
├── request_flow/…        # MODIFIED: transitions accept `state` (pass-through to the machine)
└── work_queue/           # NEW
    ├── mod.rs
    ├── input.rs          # WorkQueueInput, CapabilityInput
    ├── resolve.rs        # names, payload, time type, M, capabilities, every R5 error
    ├── decls.rs          # enum, entity, invariant, terminal, reads
    ├── lower.rs          # enqueue creation; the machine's transitions with generated fragments
    ├── ownership.rs      # R4 table
    └── identity.rs       # canonical document, receipt, source map
crates/behavior-models/tests/
│   work_queue_resolve.rs, work_queue_lower.rs, work_queue_ownership.rs,
│   work_queue_identity.rs, work_queue_laws.rs (proptest), work_queue_reference.rs (W20),
│   state_machine_bindings.rs (D1(a), incl. pinned-case regression)
crates/behavior-py/src/models.rs   # MODIFIED: work queue input, capability tracer, handles; transition `state`
python/behavior/work_queue.py      # NEW: WorkQueue API
python/behavior/models.py, request_flow.py   # MODIFIED: transition `state=`
python/behavior/__init__.py        # MODIFIED: export WorkQueue
python/tests/test_work_queue*.py   # NEW: unit and Store-level tests
python/tests/test_state_machine_bindings.py  # NEW
models/conformance/                # NEW: shared report package (moved from purchase_approval/report.py, kind-neutral)
models/examples/purchase_approval/report.py, conftest.py  # MODIFIED: use the shared package (report bytes unchanged)
models/examples/work_queue/        # NEW: workflow.py, harness.py, report.py, conftest.py, run.py,
                                   #      expected.json, test_w*.py, test_obligations.py, test_negative_controls.py
models/fixtures/reference_cases.py, compat.json   # MODIFIED: wq_* cases, `work_queue` tuple
scripts/conformance.sh             # MODIFIED: `<feature> [mode]`
scripts/gates.sh                   # MODIFIED: conformance 506 quick + 507 quick
scripts/tests/test_conformance_report.sh  # MODIFIED: feature argument, obligation status rule
api/public-api.json, models/README.md     # MODIFIED: WorkQueue section, transition bindings
```

**Structure Decision**: a module inside the existing crate, like `approval/`. WorkQueue
vocabulary stays out of kind-neutral files; the neutrality test is extended to it.

## Implementation phases (tests first in each)

0. **Regression nets.** Do this before any change:
   - the pinned-case regression over every 501, 502 and 503 manifest case;
   - the 506 report byte regression.
1. **D1(a) transition bindings** (R3 requirements 1–8, R13):
   - StateMachine `TransitionInput.state`, its resolution and lowering;
   - the v3 canonical format with the 505 identities of the bindings; the version is chosen
     by content (2 without bindings);
   - RequestFlow transitions pass bindings through, with the same version rule;
   - the narrowed ownership exemption; `TRANSITION_WRITES_OWNED_FIELD` through any parameter
     of the subject type;
   - the Python `state=` on transitions;
   - the compat gate accepts coexisting 2 and 3 tuples; new `sm_bind_*` and `rf_bind_*` cases;
   - tests:
     - bindings work, and aliasing is left to Core;
     - a subject-typed binding cannot write the owned field;
     - other models' ownership catches writes through bindings;
     - authoring order does not affect the ModelHash;
     - **every pinned case is byte-identical**: stop on any difference.
2. **Contribution kinds**: kind-neutral invariant and read contributions in `composition.rs`,
   ordered after declarations and definitions.
3. **WorkQueue resolve**: input, names, payload, time type, M, capabilities, every R5 error.
4. **Declarations**: enum, entity, `ValidWork`, `terminal`, the two reads. Golden tests of the
   Core bodies.
5. **Lowering**:
   - enqueue;
   - the eight transitions with their generated fragments, in the order of data-model §3;
   - IR tests for premises P1, P3–P7.
6. **Ownership**: the R4 table, `AUTHORED_WORK_EFFECT`, `WORK_ITEM_BINDING`, and two queues
   (W25).
7. **Identity**: canonical document, receipt, source map; W19 cases.
8. **Python**: the `WorkQueue` API and tracer; Store tests per capability (W01–W15); W21–W24.
9. **Rust reference authoring** (W20) and property tests (SA-MONO and SA-FENCE over random
   sequences).
10. **Shared conformance package**: move with a byte-identical 506 report; `conformance.sh
    <feature>`; gates.
11. **507 suite**:
    - the reference workflow and `run.py`;
    - scenarios W01–W25;
    - the `obligations` section;
    - the negative controls, including the two migration controls;
    - quick, full and complete modes.
12. **Compat, docs and gates**: the `work_queue` tuple, README, public API listing, the terms
    check, and full gates in both modes.

## Test plan (spec → tests)

| Spec item | Test | Kind |
|---|---|---|
| FR-001, FR-002, FR-018, FR-028 | golden IR of declarations, invariant, reads, actions | Rust |
| FR-006, FR-021, FR-022, R5 | each build error | Rust + Py |
| FR-008–FR-017 | per-capability Store tests from constructed states (W01–W15) | Py |
| FR-019, FR-020, W16, W22, W25 | each forbidden writer, binding, collision; two queues | Rust + Py |
| FR-023 | `is_some` present in every generated ordering (IR); absent lease denies | Rust + Py |
| FR-024, W23a, W23b | lost-ack resubmission and re-evaluation | Py |
| FR-025, W04, VO-008 | two claims from one head | Py |
| FR-026, VO-003, VO-005 | SMT postconditions (full); SA-MONO, SA-FENCE premises; property sequences | Rust + Py |
| FR-030, W24 | complete with a bound `Document` effect and a command | Py |
| FR-032, FR-033, SC-005 | the `obligations` section; a VO with one failing dependency is not passed | Py + script self-test |
| FR-034, W17, W18 | replay of the reference history; tampered context | Py |
| FR-035 | verifier per check; a forced Core limitation recorded as `blocked` | Py |
| FR-036, SC-007 | NC-writer, NC-generation, NC-expiry, NC-bound, NC-mig-reset, NC-mig-reactivate | Py |
| FR-037, SC-008 | 507 report separate; 506 report byte-identical | script |
| FR-038, W19 | compat tuple; hash changes | Rust + gate |
| W20 | Python vs Rust authoring | Rust + Py |
| W21 | incomplete snapshot | Py |

## Risks

- **D1(a) touches StateMachine.** It is mitigated by the phase 0 baseline and by language
  levels (R13): v2 content never takes the v3 path. If any established identity changes, stop
  and investigate; never update golden files to pass.
- **Compat gate cases must map to one tuple.** Coexisting 2 and 3 tuples are supported (R13).
  Reference cases keep one language level each.
- **Verifier time.** The probe verified in seconds. The full module adds more actions; it runs
  only in full mode, as C19 does in 506.
- **Report generalization.** It is mitigated by the byte regression of the 506 report.
- **Rust authoring of `ValidWork` and guards.** W20 needs the Rust reference to build the same
  bodies. The pattern of 506's `e2e_reference.rs` applies.

## Complexity Tracking

| Deviation | Reason and review evidence | Alternative considered |
|---|---|---|
| Constitution III: T018, T020, T022 and T035 tests followed the first compiling implementation | A process violation, recorded in implementation-log.md. Disabling WorkQueue ownership made all four tests targeting that pass fail. The two survivors cover StateMachine ownership and positive composition. The separate StateMachine fault also made its negative test fail. This provides regression evidence but does not claim the original test-first workflow was followed. Final-review bug fixes start with reproducing tests. | Rewriting the same code to simulate a prior red phase would not repair the historical process. Keep the deviation explicit and retain the mutation evidence. |
| P4 generation instead of new invariant/read Contribution variants | WorkQueue alone produces these items. Public Core builders still generate ordinary per-entity invariants and reads; final admission and identity cover them. No Core semantics change. | Two general contribution variants would introduce an unused interface for other kinds. |
| WorkQueue callable keywords after the positional subject | Up to five generated parameters make positional fragments hard to read. The Python contract documents names and enqueue's lack of a subject; existing model conventions remain positional. Unknown options are rejected. | Changing the established kinds would expand scope and break authoring contracts. |
