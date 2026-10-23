# Implementation Plan: Scalable Safety Certification and Independent Proof Checking

**Feature**: `511-safety-certification-scale` | **Date**: 2026-10-22 | **Spec**: [spec.md](spec.md)

**Checkout**: currently `510-safety-models`; Spec Kit resolves this feature through
`.specify/feature.json`. No branch switch or production implementation is implied.
The earlier plan is preserved in [investigation-plan.md](investigation-plan.md).

## Summary

Remedy the measured data-construction and finite-proof bottlenecks while retaining complete
exact-module/policy coverage, supported Safety v1 language, conditional guarantees and proof
integrity. Separate authoritative request preparation, fallible proof production and independent
checking at the existing Rust certification seam. Investigate construction/checking separation
before selecting an algorithm or witness format. Establish checker soundness for arbitrary
proposed witnesses without assuming producer correctness.

Early implementation tasks must produce controlled measurements, a frozen evaluation profile and
a justified mathematical design. Later production tasks depend on these gates. No complete remedy,
witness format, numeric budget or new proof representation has already been validated.

## Technical Context

**Language/Version**: Rust 1.98.1, edition 2024; Python 3.13 bindings; locked Lean 4.30.0 via Nix.

**Primary Dependencies**: public `behavior-engine`, Core 0.15.0 at
`6b93898a9375113f7a8946f23ab5535319b6f5d6`; verifier 0.10.0; existing Rust dependencies
and pinned Lean Init/Std. Baseline Behavior 0.13.1, Safety checker 2, policy semantics 1.
No new external dependency is selected; additions require the FR-009 review.

**Storage**: build-time evidence and task-owned experiment files only; no runtime Safety state.

**Testing**: test-first Rust/Python correspondence tests, Lean mutations, installed-consumer
verification, controlled scaling experiments and existing compatibility/determinism/package gates
plus complete 506/507 conformance. No such suite was run during this planning step.

**Target Platform**: first binding performance profile x86_64-linux; preserve supported
aarch64-linux correctness/reproducibility without unmeasured performance promises.

**Project Type**: existing Model-layer Rust library and thin Python bindings.

**Performance Goals**: A256 and A512 each complete certification and independent verification
in three consecutive sequential cold pairs within separate whole-chain budgets. Six scaling axes
and mixed/generated cases must meet a pre-frozen capacity profile. Numeric budgets and sweep
levels are required G0 experimental outputs based on usage and controlled pilots, not invented
planning constants or post hoc passing ceilings.

**Constraints**: no Core/policy/runtime semantic changes, private Core imports, weakened theorems,
new domain restrictions, omitted coverage, proof holes, result-cache substitution or skipped
fresh replay. Published artifacts and established Model receipts remain unchanged.

**Scale/Scope**: actions, operation occurrences, FO policies, LD dependencies, guard
count/complexity and encoded size, with material interactions. A256/A512 are floors, not limits.

## Constitution Check

**Before research: PASS for proposed scope. After design: PASS by design review.**
These are planning checks, not claims that implementation gates have passed.

| Principle | Required evidence |
| --- | --- |
| I deterministic authority | Fixed Rust/Lean checks decide acceptance; producer approval is not authority. |
| II typed boundaries | Bounded typed Rust input; reject code and unsupported contracts. |
| III test first | Observe red regression/mutation controls before production corrections. |
| IV reproducibility/replay | Pinned requests/tools; deterministic semantic evidence independent of timings. |
| V auditability | Complete inventories, trust boundaries, failed/unrun stages and actual limit scopes. |
| VI simplicity | Existing crate/API/proof runner; justify added dependencies and custom proof authority. |
| Core public surface/pin | Public pinned Core only; no overrides in required gates. |
| State transparency | No change to ordinary IR, receipts, state, history or replay requirements. |
| Integrity/delivery | Intended types, allowed axioms, fresh replay and existing gates retained. |

No exception is requested. Newly discovered conflicts block the affected gate until resolved.

## Phase 0: Research Outcomes

[research.md](research.md) consolidates decisions and alternatives, supported by notes on
[checker boundaries](research/checker-boundary.md),
[interfaces/versioning](research/interfaces-versioning.md) and [evaluation](research/evaluation.md).
Prior actual measurements remain in [root-cause-analysis.md](root-cause-analysis.md); reusable
library results are in [verified-algorithms-research.md](verified-algorithms-research.md).

The baseline verifier reconstructs exact inventory and expected policies independently but then
repeats certification. Preserve that independent boundary. Baseline `rfl` identity equalities do
not prove Rust extraction/encoding corresponds to Core semantics. New checked interpretations
must not conceal the remaining trusted code. Data-depth, uniqueness reduction and fresh dependency
costs need separate and combined measurements; no candidate remedy is already established.

No user clarification remains pending. Numeric profiles, representation selection, exact wire
grammar and version transitions are explicit experimental deliverables with exit criteria below.

## Phase 1: Design and Contracts

### Responsibilities

1. Prepare the expected request independently from admitted Core and the caller's complete intended
   policy set. Preserve full identities, action coordinates and operation occurrences.
2. Produce proposed proof data using fallible construction. Its correctness is not a checking
   assumption and its output has no certificate authority.
3. Independently check the exact claim, complete coverage/binding, intended theorem types,
   transitive axioms and fresh kernel replay. Generate fixed wrappers from validated data;
   never execute evidence-supplied code.
4. Issue an opaque certificate only after all required checks pass. Keep operational budget
   acceptance distinct; interrupted checking cannot issue a certificate.
5. Measure complete construction and checking chains in separate cold processes/workspaces.

These responsibilities stay behind existing APIs; they do not mandate a new module hierarchy or
public witness API. See [data-model.md](data-model.md),
[certification contract](contracts/certification-and-evidence.md),
[proof contract](contracts/proof-correspondence.md),
[evaluation contract](contracts/evaluation.md) and [quickstart.md](quickstart.md).

### G0 — Reproduce, instrument and freeze evaluation

- Retain published artifacts/previous attempts. Sequentially reproduce the released full failure,
  isolate root versus cascading errors and validate paired controls.
- Add an external cold harness with complete failure reports, exact applied limits and separate
  construction/checking processes. Validate tiny valid/invalid cases before costly pilots.
- Freeze a justified x86_64-linux profile, exact corpus/generator identities, six axes/interactions,
  finite per-stage limits and separate total cold budgets before final candidate comparison.
  Record installation/cache conditions, process-tree accounting and unavailable counters.
- Hash the profile; retain all pilot results. Explicit reviewed revisions preserve earlier results
  and require complete final reevaluation. No silent budget relaxation.
- Freeze conditions, workload definitions and method separately from build identities. Each baseline,
  prototype or candidate attempt references a separate exact build manifest, so new implementations
  can be compared under the same budget/environment profile.

**Exit**: runnable baseline, reliable measurement harness, independently expected inventories,
and a profile/corpus with actual finite values and usage rationale. No remedy selected yet.

### G1 — Compare mathematical designs and select a checked contract

- Inventory necessary semantic obligations and map all twelve baseline audit obligations.
- Compare changed representation and finite-proof checking independently, then together.
  Prioritize construction/checking separation; compare data-only witness checking with a verified
  direct checker or justified hybrid. No format is selected in advance.
- Assess pinned verified permutation/uniqueness/membership results first. Prove necessary adapters:
  full-key/multiplicity coverage, strict uniqueness, index preservation and conjunctive per-role
  permissions. Discharge comparator laws; avoid circular distinctness arguments.
- Prototype machine-checked checking/refinement with valid and adversarial controls, exact types,
  allowed axioms and fresh replay. Measure whole consumer cost; compiled Boolean speed is insufficient.
- Record chosen interpretation, bounded data grammar if applicable, soundness/refinement proofs,
  independent coverage, trust table, source identities, audit mapping and contract/checker/release
  version transitions. New dependencies require an explicit FR-009 review.
- Apply the selected ecosystem version metadata before candidate builds. Before integrating changed
  checking contracts, observe red version/identity controls and apply their approved numeric versions.

**Exit**: justified reviewed design with checked bridges, nonvacuous controls and evidence for
proceeding to frozen-capacity evaluation. If no candidate is sound/feasible, record the obstacle
and continue investigation; do not weaken guarantees or reach into private Core APIs.

### G2 — Integrate complete certification and independent checking

- Test first; implement the selected contract at existing Rust request/production/checking seams.
  Preserve authoring APIs and opaque authority unless the decision justifies a versioned change.
  Python remains a binding.
- Exact encoding/index controls and independently valid/invalid proposed-data tests must fail as
  intended before their corresponding encoding or checker implementation. Story grouping cannot
  postpone those tests until after the code they drive.
- The verifier reconstructs expected module/inventory/policies itself. Missing operations, policy
  subsets, stale identities and producer flags cannot satisfy obligations.
- Check all selected generic/concrete proofs, intended types, mapped baseline obligations and new
  soundness/refinement roots, allowed transitive axioms and fresh replay.
- Enforce bounded data parsing and code rejection. When witnesses are used, accept a valid
  independently constructed witness and reject fabricated/omitted/duplicated/index-mismatched data.
- Finish installed-package coverage and mandatory test integration before final evaluation. Add
  fast experiment tests to existing gates with the real scale acceptance file explicitly excluded.
  Document that real acceptance entry point as the separate mandatory G3 corpus run, failing on
  omitted execution, missing cases, incomplete reports or failed budgets.

**Exit**: complete A256/A512 certification and independent verification without weakened premises;
foundational integrity/adversarial controls pass. Budget/capacity acceptance still requires G3.
A larger limit or passing isolated component is insufficient.

### G3 — Validate soundness, coverage and capacity separately

- Entry gate: after version metadata, implementation, packaging, gate integration and measurement
  validation, build/install the final candidate and freeze its artifact/source/proof/checker manifest.
  Each final report binds that manifest plus the unchanged acceptance-profile hash. A later relevant
  build change requires complete final reevaluation; a changed measurement method/environment/budget
  requires an explicit profile revision with comparable baseline observations.
- Run the frozen corpus sequentially, including three cold pairs per mandatory reference,
  mixed FO/LD, shared dependencies and generated actions; cover all axes and interactions.
- Preserve supported v1 regression constructions under unchanged assumptions/profile. Do not
  relabel difficult cases Unsupported or require new domain restrictions.
- Run paired inventory/policy/guard/identity/proof mutations and independently valid controls.
- Assess complete construction/checking budgets separately. Explain measured growth and residual
  boundaries; finite tests do not prove universal completeness or an asymptotic bound. Identical
  full requests/configurations must reproduce semantic evidence.

**Exit**: separate assessments satisfy SC-001–006 and SC-009–011. A budget miss fails usability
when the proof completes; interruption of a required check prevents a certificate.

### G4 — Preserve delivery contracts and finish assessment

- Exercise installed-wheel certification/independent verification outside the checkout, packaged
  sources, incompatible historical-contract behavior and Core-only replay.
- Check delivered/rebuilt artifact identities against the frozen evaluated candidate. Do not apply
  late checker/version/package changes without a new candidate and complete final reevaluation.
- Preserve fixtures/receipts; document explicit version transitions and supported contracts.
  Reproducible historical acceptance does not establish an old verifier's soundness.
- Reuse existing proof/package/gates/release-check/reproducibility and complete 506/507 checks.
  No publication/tag push or external application evaluation is authorized here.
- Record causal remediation, formal results, construction/checking costs, residual trusted code,
  conditional assumptions and measured capacity in the final assessment.

**Exit**: SC-007/008 and remaining acceptance evidence complete. An unfrozen profile, missing
checks, weakened statement or unexplained practical ceiling cannot pass.

## Project Structure

### Documentation

```text
specs/511-safety-certification-scale/
  spec.md, plan.md, research.md, data-model.md, quickstart.md
  contracts/{certification-and-evidence,proof-correspondence,evaluation}.md
  research/{checker-boundary,interfaces-versioning,evaluation}.md
  investigation-plan.md, initial-source-research.md, root-cause-analysis.md
  solution-proposals.md, verified-algorithms-research.md
  experiments/                       # Scoped drivers, fast tests and mandatory scale acceptance
  evidence/                          # Observations, frozen acceptance profile and build manifests
  tasks.md                           # speckit-tasks output
```

### Source and validation seams

```text
crates/behavior-models/src/safety/
  certify.rs, evidence.rs, inventory.rs, resolve.rs, proof_data.rs
  field_ownership.rs, lifecycle_dependency.rs, guards.rs, identity.rs, mod.rs
crates/behavior-models/proofs/safety/  # Generic checks, audit and justified new bridges
crates/behavior-models/tests/safety_*.rs
crates/behavior-py/src/safety.rs
python/behavior/safety.py
python/tests/test_safety_*.py
models/examples/safety_models/
models/fixtures/{compat,safety-compat}.json
release/safety_smoke.py
scripts/{check-safety-proofs,gates,release-check,determinism-check}.sh
scripts/tests/test_safety_scale.sh    # Planned gate-collection/exit-status regression controls
```

**Structure decision**: deepen existing boundaries; no new crate, general framework, service,
runtime subsystem or parallel release workflow. Exact helper files follow the selected design.

## Requirement Traceability

| Requirements | Gate / contract |
| --- | --- |
| FR-001–004, 025–027 | G1/G2; certification/proof contracts |
| FR-005–010 | G1/G2; checked representation and verified adapters |
| FR-011–013 | G1/G2/G4; intended types, audit mapping, fresh integrity |
| FR-014–018, 028–029 | G0/G3; frozen evaluation/whole-chain observations |
| FR-019–021 | G0/G2/G3; paired controls, reports and determinism |
| FR-022–024, 030 | G2/G3/G4; language compatibility, transparency and installed gates |
| SC-001–006, 009–011 | G3; complete corpus and separate assurance assessments |
| SC-007–008 | G4; delivery gates and final assessment |

## Complexity Tracking

No constitution violation. Alternatives are bounded experiments, not approved added architecture.
Justify custom proof authority/new dependencies in G1; an untrusted producer does not require an
unnecessary formal proof of its search algorithm.
