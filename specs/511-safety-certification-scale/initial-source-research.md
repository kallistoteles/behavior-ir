# Research: Safety Certification Scale

**Scope:** initial planning research by source/artifact inspection. No tests, benchmarks or proof
runs were executed for this initial document. The subsequent empirical investigation is recorded
in [root-cause-analysis.md](root-cause-analysis.md), with measured reproductions, discarded
controls and [solution proposals](solution-proposals.md). Historical limitations stated below
describe the evidence available before those experiments.

## 1. Baseline and provenance

**Decision:** start the investigation from immutable Behavior 0.13.1, commit
`21ba441299998ca0c6baf53a5178d1b591216b1f`, Core 0.15.0 commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier 0.10.0, Safety checker 2,
policy semantics 1 and locked Lean 4.30.0. Preserve released objects and use isolated task-owned
experiment outputs. Record a distinct source identity for every later instrumented/candidate build.

**Rationale:** earlier checker versions and unrecorded commands cannot establish reproduction
on the release baseline. Host caches and source changes must not be mistaken for algorithmic effects.

**Alternatives considered:** treating historical output as a release reproduction; changing
resource limits before observing the original behavior; using the current dirty worktree as
an unidentified baseline. None gives the necessary attribution.

Sources: [release pin](../../core-release.json), [toolchain](../../crates/behavior-models/proofs/toolchain.json),
[release notes](../../release/notes/0.13.1.md), [existing triage evidence](../../docs/upstream-feedback-0.13.1.md).

## 2. Available recursion failures do not establish the reported checker-2 failure

**Decision:** retain historical files as leads, with their provenance limitations, and obtain
an independently repeatable neutral checker-2 failure before asserting the reported cause.

Two local files under `target/safety-models/ld/proof-control/proof-1331851-0/` and
`proof-1332417-0/` report maximum recursion depth in `Concrete.lean:3:4`. Their generated inputs
are each 75,141 UTF-8 bytes (74,941 characters); recorded phase durations are 27.422s / 26.138s, with sampled RSS
864,628 / 866,592 KiB. Adjacent top-level evidence belongs to checker 1, eight actions/eight
operations, and a later successful run; a later generated input is 50,243 UTF-8 bytes. Exact failing
command lines are absent. These facts do not establish the current recursion setting, original
realistic input, causal mechanism or released checker-2 outcome.

**Rationale:** shared directory names and a matching diagnostic are not provenance. A missing
original input requires an explicit link between the reported class and neutral reproduction;
it does not justify inventing an exact original reproduction or requesting private repo access.

**Alternatives considered:** absence of any recursion logs (factually wrong); equating those
logs to the new report (unsupported); declaring the report disproved without a current reproduction.

Local evidence is not a published attestation. Archive copies and hashes before any experiment;
never overwrite earlier attempts or treat a later success file as evidence about an earlier failure.

## 3. What the current certification path actually does

**Decision:** measure distinct stages and both available public workflows without selecting a
replacement proof representation or reduction strategy.

Regular certification independently compares its inventory against public admitted-module
accessors using `Inventory::verify_module`. It does not freshly call `Inventory::extract` at
that point. It checks structural ownership, prepares finite proof data and identity anchors,
compiles eight bundled generic modules, compiles generated concrete proof input, checks an
expected-type/transitive-axiom wrapper and performs `leanchecker --fresh Concrete`.

The generated concrete proof uses `by decide`. Canonical inventory and policy payloads are
anchored as 64-byte chunks. These are representation facts to investigate, not diagnosed causes
or a requirement to preserve or replace this representation.

Bundle verification additionally re-admits transported Core, reconstructs expected policies,
freshly extracts inventory, checks the full module/policy/report identities and invokes the
certifier with regenerated trusted sources. Supplied source/.olean files and success flags do
not establish authority. It then compares resulting semantic evidence.

**Rationale:** an improvement can affect extraction, encoding, elaboration, auditing or fresh
replay differently. Measuring only the failed process misses preparation and consumer-side work.

**Alternatives considered:** timing only `Concrete`; treating Rust preflight as proof authority;
using transported executable proof files; inferring a complexity bound from the current loops.

Sources: [inventory](../../crates/behavior-models/src/safety/inventory.rs),
[certifier](../../crates/behavior-models/src/safety/certify.rs),
[imported policy reconstruction](../../crates/behavior-models/src/safety/resolve.rs),
[proof encoding](../../crates/behavior-models/src/safety/proof_data.rs),
[concrete check](../../crates/behavior-models/proofs/safety/Check.lean).

## 4. Integrity and the remaining correspondence boundary

**Decision:** preserve the existing proof-integrity contract for all acceptance certificates.
The exact guarantee and every relevant premise must remain reviewable. No theorem weakening,
unreviewed axiom or missing required check can be hidden behind a successful reduction.

The pinned contract checks exact theorem propositions, twelve audited roots, transitive axioms
limited to `propext`, `Classical.choice`, `Quot.sound`, and independent fresh kernel replay.
Six obligations are mandatory: inventory comparison, generic proof, concrete proof, expected
theorem types, axiom audit and fresh replay. Lifecycle preservation additionally applies when
configured. Structural ownership rejection can happen before any Safety assessment exists.

Lean establishes results about the encoded mathematical instance. Rust extraction, public Core
probe/hash correspondence and data encoding remain trusted implementation boundaries; the
separate inventory comparison is executable correspondence evidence. Core/backend contracts,
pinned proof infrastructure, initial validity and host deployment/migration obligations remain
explicit. No source inspection here proves those implementations correct.

**Rationale:** complete inventory/policy identities and independent mutation controls are necessary
in addition to kernel acceptance; a proof over incomplete or differently interpreted data does
not establish the requested guarantee.

**Alternatives considered:** treating an evidence digest as validity, accepting a subset without
comparison to the expected policy set, or claiming end-to-end formal verification from Lean success.

Sources: [proof contract](../510-safety-models/contracts/proof-contract.md),
[evidence classes](../../crates/behavior-models/src/safety/evidence.rs),
[structured audit](../../crates/behavior-models/proofs/safety/Audit.lean).

## 5. Existing resource data and its limits

**Decision:** record configured budgets, observed counters and measurement limitations as
separate fields; do not infer unexhausted resources from a recursion exception alone.

Current defaults: 200,000 heartbeats, Lean `-M4096`, virtual-address cap 8,192 MiB and wall cap
180s per process. Lean elaboration uses `-j1`, bounded reduction depth and hardcoded
`maxRecDepth=4096`. The fresh checker is invoked separately with `--fresh Concrete`; it is not
passed that Lean elaboration option. Current resource JSON omits the recursion setting.

The runner records monotonic elapsed milliseconds and 20ms sampled direct-child VmRSS/VmSize.
It does not record actual heartbeat usage, recursion high-water, CPU time, exact peak memory,
whole-tree resource use or total public-call wall time. Its wall-time kill path currently returns
before persisting the normal resource file. Investigation evidence must retain timeout/kill/unrun
observations even when that file is absent, without presupposing a production change.

The one-field installed-wheel local record sums 133.795s for certification and 132.187s for fresh
bundle verification; fresh replay alone takes 113.996s / 112.946s. The local sampled per-process
RSS maximum is 1,530,896 KiB. Release CI records 118.832s for a combined installed-package step.
Different measurement scopes and unrecorded hardware/cache conditions prevent a controlled
performance comparison or cold/warm conclusion.

A separate membership-discovery boundary is also present: 16 MiB serialized output, one million
JSON nodes, depth 128 and 4,096 vector/value/probe limits. These are distinct from Lean recursion;
a scaling failure must identify which boundary was reached. No blanket limit increase is selected.

**Alternatives considered:** calling the first pass cold; calling independent re-verification a
cache hit; reporting configured memory as observed peak; attributing CI differences to hardware.

Sources: [runner and limits](../../crates/behavior-models/src/safety/certify.rs),
[discovery budgets](../../crates/behavior-models/src/safety/membership_orders.rs),
[measurement scopes](../../docs/upstream-feedback-0.13.1.md#d2--document-certification-cost-without-inventing-a-coldwarm-comparison).

## 6. Feasible observation without changing the proof contract

**Decision:** begin with existing phase records plus an external end-to-end observation. Where
needed, use separately identified diagnostic probes on isolated copies, and validate conclusions
against unchanged baseline behavior. Instrumented artifacts cannot masquerade as release certificates.

The pinned Lean source documents `IO.getNumHeartbeats` / `withHeartbeats`. Internal heartbeat
units differ by a factor of 1,000 from user-option units, and scopes can reset baselines. A value
without its unit and scope is not comparable. Current recursion depth/limit are available in CoreM;
there is no established ready-made run-wide high-water measurement in the inspected interfaces.
A counter that cannot be observed honestly remains unavailable; the corresponding causal hypothesis
cannot be ruled out merely because the normal resource file omits it.

**Rationale:** diagnostics may help locate necessary computation without changing the theorem,
accepted axioms or fresh-check requirements. Probe overhead and its changed source identity belong
in the observation, not hidden in final acceptance timing.

**Alternatives considered:** treating all counters as already measured; introducing a new proof
oracle; disabling fresh checking; selecting a new proof strategy before causal experiments.

Pinned first-party sources:
[heartbeat units/scopes](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Util/Heartbeats.lean),
[CoreM resource scope](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/CoreM.lean),
[recursion-limit machinery](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Exception.lean).

## 7. Empirical outputs remain to be established

- An independently repeated checker-2 failing class linked honestly to the original report.
- Causal explanation, six-dimension growth measurements and relevant interactions.
- Completeness/correspondence assessment at scale with positive and mutation controls.
- A justified reference environment and capacity profile frozen before remedy selection.
- A remedy comparison and, only subsequently, complete corrective validation.

These are the investigation's deliverables, not unresolved tool/architecture selections to fill
with guesses. The user subsequently instructed the agent to skip grilling and begin empirical root-cause
analysis and solution proposals. See the experiment driver and forthcoming analysis report;
no unconfirmed grilling recommendation is treated as accepted.
