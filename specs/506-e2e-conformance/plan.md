# Implementation Plan: End-to-End Behaviour Conformance

**Branch**: `506-e2e-conformance` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/506-e2e-conformance/spec.md`

## Summary

One reference application, a purchase approval workflow, is built from the existing public
model kinds:
- a RequestFlow `purchase_request` with an embedded StateMachine;
- a standalone StateMachine `budget`;
- an Approval `purchase_request_approval` over the flow's request entity, whose eligibility
  reads the actor's role through a query.

The application is run through the whole lifecycle on the pinned Core 0.13.0, using only the
public Python interface, the released Core command-line binary and the Rust model authority:
author → lower → admit → evaluate → commit → replay → verify.

A scenario-tagged test suite checks the 21 scenarios (C01–C19, with C13a/b and C16a/b) against
expected outcomes that are stated independently in a reference fixture. A small reporting layer
turns the outcomes into a canonical JSON conformance report with six statuses: passed, failed,
inconclusive, unverifiable, unsupported, blocked.

Nothing new enters Core, the model kinds or the Store protocol. Every check calls an existing
public operation. Where one is missing, the check reports UNSUPPORTED with the owning layer.

## Technical Context

**Language/Version**: Python 3.13 (reference application, suite, report), Rust 2024 (the
independent Rust authoring of C16a), Bash (orchestration).

**Primary Dependencies**:
- the `behavior` Python package: `BehaviorModule`, `RequestFlow`, `StateMachine`, `Approval`,
  `Store`, `InMemoryBackend`, `evaluate`, `invoke`, `replay_invocation`, `replay_data`,
  `replay_behavior`, `run_conformance`, `verify`;
- the released Core CLI bundled at `python/behavior/_bin/behavior` (`eval`, `invoke`,
  `invoke-replay`, `admit`, `hashes`, `version`);
- the `behavior-models` crate with `behavior-engine` from Core 0.13.0;
- pytest.

No new dependency.

**Storage**: `InMemoryBackend` only. The report and the C16a identity file are written to
`target/conformance/` and are not committed.

**Testing**: pytest (`models/examples/purchase_approval/`), `cargo test -p behavior-models`
(`tests/e2e_reference.rs`), and `scripts/conformance.sh` (quick and full modes) through
`scripts/gates.sh` and CI.

**Target Platform**: Linux (Nix dev shell, CI, `x86_64-linux-musl` CLI).

**Project Type**: Library ecosystem. The feature adds a reference application and a
conformance suite.

**Performance Goals**: The quick mode stays within the existing local gate budget, at most
about 30 s added. The full mode (static verification, full property depth) runs in CI.

**Constraints**:
- no Core change, no new model kind, Store protocol or serialization format (FR-024);
- no second evaluator or verifier (P1, FR-025);
- no consumer-side snapshot completion (P6);
- deterministic, with no clock, randomness, working directory or network dependence (FR-019).

**Scale/Scope**: One workflow with 4 users, 2 requests and about 12 committed transitions per
reference history. There are 21 scenarios, a reference fixture with expected outcomes, one
Rust test file, a report writer, and one orchestration script.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core | Pass. Every decision is Core's. The suite only calls public operations and compares their results. |
| II. Validated output | N/A. There is no AI. CLI output is parsed as canonical JSON and checked, never trusted as free text. |
| III. Test-first | Pass. This feature is mostly tests. Each scenario is written against its fixture expectation first, and the report writer and readiness mechanism get self-tests before use. |
| IV. Reproducibility | Pass. Fixed seeds, explicit `commit_time` and context, canonical JSON, sorted iteration. The report is a pure function of the outcomes and versions. |
| V. Explicit state and auditability | Pass. The canonical report records every check, its status, reason and owning layer. |
| VI. Simplicity | Pass. One app, a pytest marker plus a conftest hook for the report, one script. No framework. |
| Technology constraints | Pass. fmt, clippy `-D warnings` for the Rust test, typed errors, mypy-clean Python. |
| Bindings and packaging | Pass. Python implements no semantics. The CLI is the pinned release's binary, verified by `scripts/fetch-core.sh`. |

No violations.

**Post-design re-check**: Pass. The design introduces no semantic code. The expected outcomes
are data, and the invariant oracles are independent predicates over read results, stated in
the fixture, not reimplementations of Core rules (research R6).

## Project Structure

### Documentation (this feature)

```text
specs/506-e2e-conformance/
├── plan.md
├── research.md            # R1–R12
├── data-model.md          # workflow domain, scenario, outcome, report, readiness
├── quickstart.md
├── implementation-log.md  # /speckit-implement
├── contracts/
│   ├── reference-workflow.md     # the workflow and its independently stated expectations
│   ├── conformance-report.md     # the report document and status rules
│   └── comparison.md             # authoring and execution normalization contracts
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
models/examples/purchase_approval/
├── __init__.py
├── workflow.py            # authoring: entities, enums, RequestFlow, StateMachine, Approval, module()
├── run.py                 # executable lifecycle (FR-001, quickstart); prints each step's outcome
├── expected.json          # independently stated expectations per scenario (FR-023)
├── harness.py             # fresh store, step runner, HISTORY, snapshot, VO-003/VO-004 oracles
├── conftest.py            # scenario marker, status outcomes, report hook, readiness registry
├── test_l1_model.py       # C01, VO-005 and VO-001 negative controls
├── test_lifecycle.py      # C03–C06 along history H, invariants after each commit
├── test_l2_evaluation.py  # C07, C08, C09, C09.input, C12
├── test_l3_store.py       # C10, C11, atomic publication, C17, backend conformance
├── test_l4_replay.py      # C13a, C13b, C13.history, C14.*, C19
└── test_l5_agreement.py   # C02, C15, C16a (Python half), C16b, C18

crates/behavior-models/tests/e2e_reference.rs   # C16a Rust half: independent Rust authoring
scripts/conformance.sh                           # quick | full; runs both halves, writes the report
scripts/tests/test_conformance_report.sh         # self-tests of the status rules
.github/workflows/ecosystem-ci.yml               # full mode, report uploaded as an artifact
```

**Structure Decision**: The reference application sits beside the existing
`models/examples/state_machine` example, so `scripts/gates.sh`'s `pytest … models` already
collects it. The Rust half of C16a is an ordinary `behavior-models` integration test.
`scripts/conformance.sh` sequences both halves and writes the report, and `gates.sh` calls it
in quick mode. No production package code changes.

## Complexity Tracking

No constitution violations.
