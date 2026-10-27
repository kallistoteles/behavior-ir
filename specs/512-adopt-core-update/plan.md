# Implementation Plan: Adopt Core Update

**Branch**: `512-adopt-core-update` | **Date**: 2026-10-27 | **Spec**: [spec.md](spec.md)

**Checkout**: `/home/kalle/repos/behavior-ir-512`, isolated from the existing dirty 510/511 checkout and based on published Behavior 0.14.0 (`1cba9e16ff73f927ae08e70a7bc74a6ca52355e4`).

## Summary

Prepare baseline evidence and focused adoption controls, then adopt one immutable Core release containing 029/030. Reuse the public binding, compatibility gates, Safety proof system and established release pipeline. No binding redesign or new runtime semantics are indicated by research.

**External prerequisite (resolved):** initial inspection on 2026-10-27 found only Core v0.15.0. The separately authorized Core publication completed at 07:44:59 UTC: [Core 0.16.0](https://github.com/kallistoteles/behavior-ir-core/releases/tag/v0.16.0), commit `63a8c164523275e06ac1d7dc5330db4a899c9d99`, verifier 0.11.0. Published assets, independent build equality and all six downloaded-CLI proof cases are verified in [selected release evidence](evidence/core-release.json). T007 is complete; pin adoption starts at T008. Publication was separately authorized rather than inferred from Feature512 implementation.

## Technical Context

**Language/Version**: Rust 1.98.1/edition 2024, Python >=3.13, existing shell tooling.

**Primary Dependencies**: Existing public `behavior-engine`; ecosystem pin now selects published Core 0.16.0/verifier 0.11.0 at the exact commit above. T008–T010 are complete; final acceptance is recorded in the evidence ledger. Lean 4.30.0 from the locked Nix toolchain; existing proof contract/checker 3, statement 2 and evidence v2 remain unchanged unless a demonstrated adaptation requires explicit version review.

**Storage**: Existing Core Store and native InMemoryBackend; deterministic JSON evidence under this feature. No new runtime state, backend capability or replay dependency.

**Testing**: pytest/public Python workflow; existing Rust, proof-integrity, complete 506/507 conformance, compatibility and release checks. Observe meaningful red tests before fixes.

**Target Platform**: Existing supported distribution; first growth comparison on the recorded x86_64-linux environment. Other platforms receive no unmeasured performance claim.

**Project Type**: Library/binding and bundled release adoption.

**Performance Goals**: Retain one Store and measure complete evaluate/commit windows at populations 64, 256 and 1024, with three repetitions and eight fresh creations per repetition. Measure baseline before setting final thresholds; freeze the acceptance profile before any candidate measurement. Separate setup, evaluation, commit, serialized output sizes and replay. A thousand-item boundary test is separate from the measurement workload's larger limit.

**Constraints**: Exact published Core pin in required checks. No local override, Core implementation dependency or private source copy. Preserve historical baselines. Available disk is about 1GB: reuse the already installed, checksummed 0.14.0 baseline; avoid speculative builds and deletion of unrelated work.

**Scale/Scope**: Six positive removal/profile combinations, targeted negatives, a bounded growth comparison, existing Models and evidence. All 16 FRs/7 SCs remain mandatory for completed adoption.

## Constitution Check

| Principle / constraint | Required behavior | Pre/post-design result |
|---|---|---|
| I–II | Deterministic fixtures and typed public boundaries; no AI runtime | PASS |
| III | Record baseline refusals and meaningful red regression before fixes | PASS by execution sequence |
| IV | Record exact wheel/source/pin/environment; preserve replay and historical bytes | PASS |
| V | Separate semantic, verification, Safety and performance evidence | PASS |
| VI | Existing binding/report passthrough; small fixture/probe only | PASS |
| Bindings/Packaging | Released public Core only; exact dependency graph and bundled assets | PASS; new release is an external execution gate |
| Workflow | Existing gates and release process; no publication in this invocation | PASS |

No exception is requested. Do not cross the released-Core gate using a merged branch.

## Project Structure

```text
specs/512-adopt-core-update/
  spec.md, plan.md, research.md, data-model.md, quickstart.md, tasks.md
  contracts/adoption.md
  probes/adoption_cases.py, growth.py
  evidence/                         # exact baseline and later candidate records
python/tests/test_core_update_adoption.py
python/tests/test_pinned_identity_baseline.py
scripts/tests/test_pinned_identity_baseline.sh
core-release.json, Cargo.toml, Cargo.lock
models/fixtures/compat.json, models/fixtures/safety-compat.json
skills/behavior-{authoring,verification,application}/
README.md, docs/versioning.md, release/notes/0.15.0.md
```

Existing files may be changed only for demonstrated adoption requirements. No Core sources are imported into the ecosystem.

## Phase 0 — Research

[research.md](research.md) records release availability, the persistent Store/report boundaries, required guarded mixed forms, evidence identities and a necessary baseline gate normalization. Two read-only research agents examined public consumer contracts and evidence/release integration. No unknown product decision remains.

## Phase 1 — Design and execution boundaries

1. Isolate the feature; identify and checksum the published 0.14.0 wheel and installed environment. Capture semantic reference snapshots and current gap009 proof outcomes.
2. Author neutral public Python fixtures and run the six required support assertions red on verifier 0.10. Record independent positive runtime/negative controls. Build a small growth probe and freeze a baseline-derived profile; do not expand this into a benchmark framework.
3. Correct the historical receipt comparison to ignore only top-level `core_release` metadata. Prove actual semantic differences, nested fields and missing receipts still fail. Never modify the original baseline.
4. **Release gate:** verify a published Core release containing 029/030 and its manifest/assets. Until then, leave pin/version/Skills claims unchanged and dependent tasks incomplete.
5. Adopt exact release metadata, build through the released dependency graph and run the public regression matrix and frozen growth comparison.
6. Compare semantic snapshots separately from intentionally changed verification artifacts. Add new-Core compatibility tuples without superseding historical tuples. Safety inventory hashes intentionally bind Core; classify those as dependency-bound evidence, not Model semantic drift.
7. Refresh current Core/Safety evidence and independently check it, retaining stale-context refusals and archival meaning.
8. Update the three Skills and release documentation. Run all existing required gates, complete 506/507 and release checks, then record readiness. No tag or distribution is published.

## Requirement traceability

| Scope | Execution coverage |
|---|---|
| FR-001/002; SC-001; US1 | Published release gate, exact pin/assets, clean installed identities |
| FR-003/004; SC-002; US2.1/2 | Six public authored fixtures, actual reference checks, unsafe/unsupported controls |
| FR-005/006; SC-004/005; US2.3/4 | Frozen growth profile, whole consumer cycles, boundary and fallback tests |
| FR-007/008; SC-003; US3.1 | Baseline gate normalization, semantic snapshots and new-Core tuples |
| FR-009–011; SC-006; US3.2/3 | Fresh/archival Core evidence, dependency-bound Safety evidence and independent checks |
| FR-012/013; US4.1 | Three Skills/examples, precise limitations and versioned documentation |
| FR-014–016; SC-007; US4.2 | Existing complete conformance, release/gates, exact tested artifacts and findings ledger |

## Complexity Tracking

No constitutional exception. A small probe and a narrow receipt-comparison correction are necessary to validate adoption; new proof algorithms, caching frameworks, governance mechanisms and release pipelines are excluded.
