# Investigation Plan: Scalable and Mathematically Sound Safety Certification

> **Historical investigation plan (2026-10-20):** This plan predates the revised
> remediation [specification](spec.md). Its original requirements are preserved in
> [investigation-spec.md](investigation-spec.md). It does not authorize a remedy or
> satisfy planning for the new acceptance requirements. Regenerate it with
> `speckit-plan` before producing implementation tasks.

**Feature**: `511-safety-certification-scale` | **Date**: 2026-10-19 | **Spec**: [spec.md](spec.md)

**Branch**: proposed `511-safety-certification-scale`; the working checkout remains on
`510-safety-models`. Setup resolves the feature through `.specify/feature.json`; it did not
create or switch a Git branch.

**Status**: empirical investigation started by the user's explicit instruction to skip the
grilling round and begin root-cause analysis and solution proposals. No production remedy is
selected or authorized by the experiments.

**Input**: `specs/511-safety-certification-scale/spec.md`.

## Summary

Investigate complete exact-module Safety certification on the published Behavior 0.13.1
baseline. Establish a neutral current-checker failure, resource/cost attribution, correspondence
assessment and empirically justified capacity before selecting a remedy. Preserve v1 semantics,
all proof-integrity obligations, explicit trusted boundaries and Core state transparency.

Initial source/artifact research is recorded in [research.md](research.md). Actual subsequent
reproductions, causal controls and proof-integrity checks are recorded in
[root-cause-analysis.md](root-cause-analysis.md), with ranked [solution proposals](solution-proposals.md).
The unchanged released A256 proof source reproduces recursion failure when observed past the
default wall budget. Data representation and finite proof reduction have separately measured
bottlenecks. No production remedy has been implemented or validated.

## Technical Context

**Language/Version**: existing authoritative Rust 1.98.1, edition 2024; Python 3.13 authoring;
locked Lean 4.30.0 for the release baseline. No new proof construction or algorithm selected.

**Primary Dependencies**: public pinned `behavior-engine` for Core 0.15.0; existing
`serde_json`, `thiserror` and `proptest`; the locked Nix proof-toolchain output. No new
crate/library/toolchain dependency is proposed.

**Storage**: immutable investigation inputs and separate observations in task-owned filesystem
outputs; no application Store state, new events or replay dependencies.

**Testing**: initial investigation executed 32 focused existing Python tests against the isolated
published wheel, an unmodified positive certificate with twelve audited roots and fresh replay,
four negative public-bundle controls and three actual Lean proof mutations. Failed attribution
harnesses are recorded and discarded. Full regression/release suites and a complete scaling
matrix remain outside this first empirical pass.

**Target Platform**: start with the released x86_64-linux wheel/proof profile and a fully recorded
reference environment. Hardware, resource budgets and cache conditions for the empirical capacity
claim remain measured investigation outputs. The existing aarch64-linux tool profile must not be
presented as measured capacity without corresponding evidence.

**Project Type**: existing Model-layer library and Python binding; scoped certification investigation.

**Performance Goals**: six-dimension characterization, an independently reproduced failing class,
explained costs and justified acceptance capacity established before remedy selection. No numeric
SLO, asymptotic promise or convenient passing action ceiling is invented here.

**Constraints**: exact complete module/policy coverage; existing expected-theorem checks,
transitive axiom allowlist and fresh kernel replay; no Core/Store/replay/Model semantic changes;
no caches or resource-limit increases accepted as a remedy without causal/validation evidence.

**Scale/Scope**: action count, operation count, ownership-policy count, dependency count, guard
count/complexity and proof-data size, including relevant interactions and supported generated
Model actions. Literal-discovery limits and Lean resource failures are distinct boundaries.

## Constitution Check

*Before planning research: PASS against the existing specification and read-only fact gathering.*

| Governing constraint | Planning assessment |
| --- | --- |
| I deterministic decisions | No AI/runtime decisions introduced; later cases must use fixed construction inputs. |
| II typed boundaries | Existing Rust authority and public Core admission remain authoritative. |
| III test first | Reproduction and valid negative/positive controls must precede any future correction; none implemented now. |
| IV reproducibility/replay | Pinned baseline, exact identities, immutable attempts and separate environmental measurements. |
| V auditability | Exact stages/diagnostics, measured-versus-configured values and missing evidence remain explicit. |
| VI simplicity | Existing crate, proof sources and gates; no new framework, release process or speculative remedy. |
| Core public surface/pin | Public pinned Core only; no private imports, copied semantics or committed overrides. |
| State transparency | No persistent Safety state or Core execution/persistence/replay changes. |
| Integrity/compatibility | No weakened propositions, unreviewed axioms, skipped checks or rewritten published records. |

*Investigation execution: existing constraints remain governing; production changes are outside
the current root-cause/proposal step.*
No constitution exception is requested.

## Current Execution Direction

The user explicitly ended the grilling round and requested root-cause analysis and solution
proposals. No answers to Q1–Q4 are inferred, and they no longer block investigation. Follow the
written specification, collect empirical evidence and present operational/trust tradeoffs with
any proposal rather than selecting them through another preliminary interview.

Current work: an isolated installation of the published 0.13.1 wheel, neutral public-API case
construction, unchanged library certification, controlled diagnostic probes and a reproducible
analysis report. Production certifier, proof theorems, public formats, Core and published
artifacts remain unchanged.

## Phase 0: Planning Research

Completed source/artifact inspection:

1. Release/checker/tool identities and historical-failure provenance.
2. Existing complete inventory, ownership/lifecycle and proof-data boundaries.
3. Required theorem types, twelve audited roots, permitted axioms and fresh checking.
4. Certification versus imported-bundle re-verification workflows.
5. Available phase measurements, unobserved counters and separate discovery limits.
6. Supported dimension-varying corpus/control seams and existing validation commands.

See [research.md](research.md). The original exact realistic input is unavailable; old recursion
failures have different/incomplete provenance. These observations do not establish the reported
checker-2 failure. Subsequent experiments now establish neutral checker-2 failures and separate
data-construction/proof-reduction causes. Actual heartbeat/depth high-water counters, repeated
cost curves, supported capacity and a fully validated remedy remain future outputs.

## Phase 1: Design and Contracts

Deferred as a full design deliverable. The requested investigation now produces runnable
experiments and a root-cause/proposal report directly. Later design artifacts may include:

- `data-model.md`: exact requests, independent coverage expectations, workload cases, run/stage
  observations, capacity profile and investigation/remediation assessments.
- `contracts/`: internal evidence and measurement contracts, including failure/no-authority
  semantics. Preserve existing public certificate/API meaning unless later justified findings
  require a separately reviewed change.
- `quickstart.md`: actual existing baseline/control entrypoints plus clearly labelled future
  empirical validation instructions; no invented implemented commands or passing measurements.

The design must organize the investigation and its evidence. It must not select a proof
representation, reduction algorithm, cache design or a limit increase as the solution.

## Project Structure

### Documentation (this feature)

```text
specs/511-safety-certification-scale/
├── spec.md
├── plan.md              # Current investigation status
├── research.md          # Initial source/artifact findings
├── root-cause-analysis.md
├── solution-proposals.md
├── experiments/         # Isolated research drivers, no certificate authority
├── evidence/            # Compact actual observations and controls
├── checklists/requirements.md
├── data-model.md        # Planned Phase 1 output; not generated yet
├── contracts/           # Planned Phase 1 output; not generated yet
└── quickstart.md        # Planned Phase 1 output; not generated yet
```

No `tasks.md` is produced by this command.

### Existing Source and Validation Touchpoints

```text
crates/behavior-models/
├── src/safety/          # Inventory, ownership/lifecycle, guard probes, proof data and certifier
├── proofs/safety/       # Existing mathematical statements, finite checks and integrity audit
└── tests/safety_*.rs    # Existing structural, correspondence, proof and regression controls
crates/behavior-py/src/safety.rs
python/behavior/safety.py
python/tests/test_safety_*.py
models/examples/safety_models/
scripts/{check-safety-proofs,gates,determinism-check,release-check}.sh
models/fixtures/{compat,safety-compat}.json
```

**Structure Decision**: use the established Model authority and validation seams. Investigation tooling stays within this feature's `experiments/` directory; any production
remediation touchpoints must follow empirical findings without a new general framework.

## Complexity Tracking

No constitution violations or exception requests. No new implementation architecture is selected.
