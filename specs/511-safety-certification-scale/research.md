# Planning Research: Scalable Safety Certification

**Date**: 2026-10-22. Source inspection and consolidation of prior experiments only; no new
proofs, tests or benchmarks were executed. The earlier source research is preserved in
[initial-source-research.md](initial-source-research.md).

## Decision — Separate request preparation, production and checking

**Decision:** use the current Rust certifier/consumer boundary, preserving independent admission,
inventory comparison and the complete caller-expected policies. Prioritize investigating a
fallible producer and sound independent checker. Choose no witness format yet.

**Rationale:** the current consumer already reconstructs the expected input but repeats the whole
certification process. Producer correctness need not be a soundness assumption when the checker
establishes the full claim for arbitrary supplied data. Coverage must come from the exact original
request, never from the producer's claimed inventory. See [checker research](research/checker-boundary.md).

**Alternatives considered:** optimizing repeated proof construction alone remains a comparison;
trusting a prior approval or incomplete producer inventory is unacceptable. A separate service,
crate or public authoring API is unnecessary without experimental evidence.

## Decision — Compare data and proof bottlenecks together

**Decision:** controlled pilots compare representation changes and finite-proof improvements
separately, then together. Preserve mathematical guarantees and semantic identity, not old chunks.

**Rationale:** unchanged A256 eventually failed recursion in data construction; factored diagnostic
data isolated another expensive uniqueness proof. Native Boolean success is not proof, and a data-only
control is not a complete remedy. Prior overlapping observations cannot establish a growth curve.
See [causal report](root-cause-analysis.md) and [proposals](solution-proposals.md).

**Alternatives considered:** raising limits alone, dropping anchors/operations or counting a fast
component as full certification cannot satisfy the specification. A new representation is allowed,
but its checked interpretation and versioned target must retain all required semantics.

## Decision — Reuse proved components with explicit bridges

**Decision:** assess pinned Init/Std results first for proof-authoritative ordering, uniqueness and
permission work. A fallible producer may use unproved search. Select components only after checking
premises, bridge obligations, full kernel cost and dependency impact.

**Rationale:** available permutation, nodup and membership results can reduce custom proof work;
they do not automatically prove an adjacent-order adapter, multiplicity preservation, correct
comparators, index correspondence or conjunctive permissions. Runtime optimization is not kernel
reduction speed. See [verified algorithm research](verified-algorithms-research.md).

**Alternatives considered:** custom algorithms require a mathematical or measured justification.
New external libraries require the FR-009 trust/reproducibility/distribution decision; neither their
necessity nor the sufficiency of current libraries is assumed.

## Decision — Prove the new checking interpretation without hiding trusted code

**Decision:** require checker soundness for arbitrary witnesses and checked refinement of changed
logical representations, plus all baseline safety obligations. Version audit mappings if targets
change. Preserve exact types, allowed transitive axioms and fresh pinned-kernel replay.

**Rationale:** baseline identity equalities do not prove JSON-to-Core semantic correspondence.
Rust admission/extraction/encoding and canonical guard probes remain explicit trusted boundaries
unless separately proved. A checksum identifies evidence; it is not a correctness proof.

**Alternatives considered:** preserving exact baseline root names is unnecessary when a reviewed
mapping preserves every obligation. Removing an obligation, assuming producer correctness or adding
an unreviewed decoder assumption is unacceptable. See [proof contract](contracts/proof-correspondence.md).

## Decision — Freeze operational acceptance from controlled pilots

**Decision:** G0 must establish a fixed x86_64-linux environment, exact six-axis/mixed corpus and
separate finite whole-chain cold budgets before final remedy comparison. Record usage rationale,
actual resource-limit scope, installation/cache conditions and missing measurements.
Keep those conditions in the profile and exact package/checker/source identities in separate
per-build manifests. Final measurements require a completed, frozen candidate artifact; late
relevant build changes require complete final reevaluation under the same conditions.

**Rationale:** per-process 180-second limits are not total budgets. Fresh replay has different
limits from Lean elaboration. Prior timing overlap and missing counters prevent justified numeric
capacity claims now. A256/A512 are floors; all accepted profile cases must pass. See
[evaluation research](research/evaluation.md) and [evaluation contract](contracts/evaluation.md).

**Alternatives considered:** invented timing constants, a passing action ceiling chosen after final
runs or omitted slow cases would make acceptance circular. A profile revision must remain explicit,
retain old results and trigger full reevaluation. Installation need not be repeated for each cold run.
Freezing an unknown future package identity at G0 would prevent candidate comparison; silently
applying checker/package versions after final measurement would invalidate its provenance.

## Decision — Preserve package/API boundaries and version changed contracts

**Decision:** retain current entry points and opaque certificate authority. Any selected new data
contract must be explicitly versioned; v1's strict five-key bundle cannot silently acquire a witness
field. Independently reject malformed/incompatible data without executing package-provided code.
Observe version/identity and encoding/checker red controls before their corresponding changes.
Apply versions and packaging changes before freezing the candidate for final evaluation.

**Rationale:** current sources, theorem/checker/toolchain dependencies and proof format are
identity-bound. New representation does not require new policy semantics, but may require checker,
statement, evidence and ecosystem release changes. Follow existing 0.x versioning; no release
number or publication is selected here. See [interface research](research/interfaces-versioning.md).

**Alternatives considered:** universal historical dispatch, automatic legacy-tool downloads and a
parallel release process are unnecessary. Preserve old evidence and pinned historical conditions;
reproducibility alone does not show that an old verifier remains sound.

## Remaining experimental outputs, not unanswered user decisions

G0 produces actual reference budgets/capacity and complete baseline observations. G1 produces
selected representation/checking relation, checked bridge prototypes, grammar, audit mapping and
version decision. These gates are executable work in [plan.md](plan.md), not unresolved interview
questions. No production remedy, profile capacity or passing final acceptance is claimed yet.
