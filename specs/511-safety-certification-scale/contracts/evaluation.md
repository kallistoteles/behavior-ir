# Contract: Capacity and Whole-Chain Evaluation

## Freeze before final comparison

G0 produces `evidence/evaluation-profile.json` with actual finite values and a usage rationale.
It fixes acceptance conditions: profile revision/hash; CPU/virtualization and allocated cores,
affinity/effective quotas; memory/swap/process limits; OS/storage; approved Core/proof-toolchain
constraints; corpus/generator identities and semantic workload definitions; measurement method/version;
dependency-installation and OS-cache conditions; separate positive total cold certification and
independent-verification budgets; per-stage controls; repetition count and all accepted capacity cases.
It does not fix an unknown future candidate package, checker version or implementation-source hash.

Draft profiles are not acceptance profiles. Freeze/hash the manifest before final evaluation.
No placeholder timings or unexplained largest-passing ceilings qualify. Revisions retain prior
profiles/results and require explicit review plus complete final reevaluation. Published default
limits remain the comparison baseline, not the mathematical meaning of certification.

First binding platform is x86_64-linux. Retain other supported-platform correctness requirements;
make no unmeasured ARM performance guarantee. Recorded host inventory is not a guaranteed allocation.

## Build manifests and candidate-freeze gate

Every baseline, prototype and candidate observation separately identifies the exact build used.
Build manifests under `evidence/candidate-builds/` record artifact checksum, source-tree identity,
release/checker/statement/evidence versions, proof-source/dependency identities, actual Core/toolchain
derivations and preinstalled compiled dependencies. Record the exact per-build expected request
identities as well as the shared semantic workload definition. This permits explicit versioned
representation changes without substituting a different module or policy intention silently.

Each run references both the immutable acceptance-profile hash and its build-manifest hash.
Different implementation builds can be compared under one unchanged profile. A new build never
permits relaxed budgets, omitted cases or silently changed measurement conditions.

Before final evaluation, complete version transitions, all candidate implementation/packaging changes,
installed-package controls, mandatory test integration and measurement validation. Build/install the
candidate and freeze its exact manifest. Final runs and delivered-artifact verification use that
same artifact identity. Earlier smoke/prototype observations cannot substitute for final acceptance.

A later relevant package/source/proof/checker/dependency change creates a new build manifest and
requires complete final reevaluation under the existing acceptance conditions. Changing the approved
environment, corpus, budget or measurement method instead creates a reviewed profile revision,
retains previous results and requires comparable baseline observations plus final reevaluation.
Reproducible rebuilds must match the evaluated artifact; a mismatch cannot be silently accepted.

## Required test integration

The existing quality gates must collect the fast experiment-runner, generator and observation tests.
Explicitly exclude the real full-scale acceptance test from that fast invocation. A collection or
test failure propagates nonzero. This adds checks to the existing process, not a parallel workflow.

The real `experiments/tests/test_scale_acceptance.py` entry point drives the external runner against
the complete frozen corpus at G3. Its documented full invocation is separately mandatory; omitted
execution, missing cases, incomplete reports or budget misses fail capacity acceptance and prevent
feature completion. Fast fake-process tests never count as actual certification or capacity evidence.

## Cold and total cost

Installed pinned package/toolchain, new workspace, no previous request-result reuse. Evidence is
legitimate independent-checking input; prior acceptance is not authority. Record preinstalled
compiled dependencies and their trust conditions. Installation/download costs are separate. A new
workspace does not establish a cold OS page cache; describe actual controlled/observed/unknown state.

Use separate external producer and consumer processes/workspaces. Consumer has only original
expected module/policies and evidence; no producer work directory or compiled request proofs.
Measure process startup through final result/evidence persistence, including input parsing,
admission/extraction, construction or witness checks, type/axiom audits and fresh replay. Record
breakdowns without subtracting required work from total wall time.

Run heavy jobs sequentially, with no overlapping benchmark/gate/proof subprocesses. Retain inputs,
source hashes, exact commands, all successes/failures and failed/unrun stages. Measure CPU and
memory with explicit process-tree/direct-child/sampling scope. Configured heartbeats/depth are
ceilings, not observations; unavailable consumption counters must have a reason, not a fictitious zero.

Baseline limits are per process: 180 s wall, Lean heap 4096 MiB/200000 heartbeats/maxRecDepth4096,
-j1, address space 8192 MiB. Fresh checker receives address-space/wall controls but not the Lean
heap/heartbeat/depth options. Record actual stage-specific application; do not describe all limits
as whole-chain limits or as enforced on every stage alike.

## Corpus and repeats

- A256/A512: exact existing neutral authored-action references, one assignment/action, one FO
  policy and no LD. Required floors, not a maximum supported size.
- Six axes: actions, operation occurrences, FO policies, LD dependencies, guard count/complexity,
  encoded proof/input size. Freeze levels from pilots. Record coupled dimensions rather than
  claiming independent sweeps when input constraints couple them.
- Include mixed FO/LD, shared-parent dependencies and generated Model actions. Cover material
  action/policy/operation and dependency/guard interactions; no full Cartesian product is required.
- Keep every supported v1 construction as target coverage, including creator/remover permissions,
  canonical guard permutations, aliases and overlapping restrictions. No language expansion needed.
- Every accepted capacity case gets repeated cold pairs; at least three consecutive sequential
  successful pairs for each A256/A512 reference. Retain failures; do not rerun selectively until green.
- Each negative-control group has a valid passing control; include incomplete inventory/policy,
  unauthorized operations, wrong guards/bindings, duplicate/missing keys/witnesses and proof mutations.

## Assessment

Report mathematical checking, language/completeness and practical performance separately.
All cases in the frozen accepted envelope must certify and independently verify within their
separate budgets. A fully checked over-budget certificate may be valid while usability fails.
An interrupted necessary check cannot certify. Bad-witness rejection does not prove module unsafety.

Characterize construction/checking growth and causal contributors from repeated observations.
Do not infer kernel complexity from compiled execution or finite-run universal completeness.
Identical semantic inputs must reproduce evidence regardless of diagnostic timing/path noise.
Residual capacity/resource limits and measurement uncertainty remain explicit in the final report.

### Implemented measurement boundary

The original timing scope and profile bytes remain fixed: each role includes input staging,
process startup, independent preparation and semantic-result persistence. Observation-file
finalization is reported separately. The consumer directory now also has ordinary Unix search
permission denial on the producer directory while the trusted checking subprocess runs. This
correctness control does not change timing boundaries, counters, budgets or workload semantics.
The same-UID trusted process is not a sandbox for hostile executables; evidence never supplies
executable code. All role results remain subject to real Rust/Lean certificate authority.
The compact result now records checker/statement/source identities explicitly; these are
observations of the API result, never a replacement for its mandatory checks.

## Executed frozen profile

The complete real entrypoint passed60/60 pairs (120 roles), without prior request-result reuse.
All case/repeat reports and their1,440 tool-stage records were accepted by the independent collector.

- Profile SHA-256: `d89f382a170001623b16ee6ae95923819a6a8d42c9752484cae59070deed1272`.
- Candidate manifest SHA-256: `c6e392791cfcfe3b41e4fbfe34a59b5271ef3dbac7759b78f7152922a61d6ba8`.
- Wheel SHA-256: `810566f34559851fb6e725179a7cb066e64fae3a3a25d3eb85e524855de3902b`.
- Core 0.15.0 / verifier 0.10.0; Lean 4.30.0; checker 3 / statement 2 / evidence and bundle v2.

See [measurements](../scalability-results.md), [candidate](../evidence/candidate-builds/final-01.json)
and [complete assessment](../evidence/final-runs/assessment.json). The [quickstart](../quickstart.md)
contains the actual build/inspection/freeze/real-pytest/collector commands. Installation and
downloads are separate. OS page cache was uncontrolled; preinstalled pinned dependencies are
permitted. A fresh workspace is not a cold operating system. ARM execution was unavailable.

The recorded180 s per-tool and300 s per-role limits were retained. Unavailable consumed heartbeat
or recursion high-water measurements remain null, never equated to configured ceilings. No
future build/profile change inherits these observations silently.
