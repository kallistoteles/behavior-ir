# Evaluation design: complete construction and independent checking

**Status:** planning research, 2026-10-22. Source and environment inspection only;
no new proof, benchmark or test was run for this document. Numerical performance
budgets and practical capacity have not yet been established. The steps explicitly
marked **planned** are implementation work, not existing commands or passing gates.

## 1. Whole-chain acceptance, separate from soundness

**Decision:** freeze acceptance conditions before final remedy comparison: capacity
corpus/generator and semantic workload definitions, approved proof-toolchain constraints,
environment, distinct construction/checking budgets, per-process limits and measurement
procedure. Exact installed wheel, source/checker versions and per-build request identities
belong in separate build manifests referenced by every run. The freeze is a required
implementation gate, not a reason to invent passing timings or a future build identity.

**Rationale:** A256 and A512 are known valid regression floors, but their measured
baseline attempts overlap and cannot justify a cost curve. Their release failures
are Concrete-stage 180-second timeouts. A1's complete 143.111-second observation
includes 120.253 seconds fresh replay, also overlapping. Neither defines an
uncontended performance promise. [Measured provenance and limitations](../root-cause-analysis.md).

**Alternatives considered:** setting budgets from these noisy observations would
give false precision; increasing budgets after seeing final failures would make
acceptance circular; testing only Concrete would omit a material replay cost.

The **planned freeze procedure** is:

1. Characterize baseline costs using sequential controlled pilots, record the
   intended deployment/build usage and available reference resources, and retain
   all successes and failures. Pilot sizes need not all fit release defaults.
2. Select the full capacity corpus and separate total wall budgets for cold
   construction and cold independent checking. Include A256/A512 and the mixed
   cases below. Record the operational rationale and measured baseline evidence.
   Do not choose an upper bound solely because it is the largest passing case.
3. Commit/hash the manifest before final evaluation. Bind its corpus generator,
   fixture hashes, environment and measurement version. Freeze repeated-run count
   as at least three sequential cold construction/check pairs per accepted case.
4. Finish version transitions, implementation, packaging and mandatory test wiring;
   validate the measurement method, then build/install and freeze the exact final
   candidate manifest. Reports reference both this build and the acceptance profile.
5. Evaluate the remedy without silently excluding slow or failed cases. Any
   changed envelope/profile is an explicit reviewed revision with old results
   retained and the complete final comparison repeated. The feature does not pass
   while budgets or the capacity envelope remain unfrozen. Relevant later artifact,
   proof/checker/source changes require a new candidate manifest and complete final
   reevaluation. Delivery checks must match the artifact actually evaluated.

Report two axes: completed mathematical checks and performance acceptance. A
completed valid proof beyond the budget is a performance failure, not a safety
counterexample. A timeout interrupting a required check is uncertified, with the
failed and unrun stages explicit. [Requirements FR-014–019, FR-028–030 and
SC-001/005/010](../spec.md).

## 2. Cold construction and independent checking

**Decision:** use separate fresh workspaces and separate consumer processes for
construction and checking. Install the pinned wheel/toolchain beforehand. Do not
reuse any earlier request's accepted result. Supply only the immutable evidence
package and an independently reconstructed expected module and complete policies
to the checking process.

**Rationale:** this implements Q6/Q7/Q10 without conflating independence with a
different kernel implementation. The current public verifier re-admits the
transported module, re-extracts policies/inventory and calls `certify` again. That
is a valid baseline, but it currently repeats proof construction; a future witness
checker may avoid that work only after its soundness and coverage are established.
[Current verification implementation](../../../crates/behavior-models/src/safety/certify.rs#L895),
[public Python boundary](../../../python/behavior/safety.py#L120).

**Alternatives considered:** reusing producer analysis as the expected consumer
inventory trusts the claim being checked; treating the producer's success flag as
an input skips proof checking; requiring fresh downloads confuses installation
with certification cost. Preinstalled compiled dependencies may be part of a
declared profile, but do not replace mandatory auditing or fresh kernel replay.

**Planned timing boundaries:** define the input artifact boundary identically for
all candidates. Measure each operation externally from consumer-process startup,
including reading/validating its supplied module/policies, admission/extraction,
evidence parsing, witness checking or construction, intended-type/axiom audits,
fresh replay and final result/evidence persistence. Record authoring/generation,
installation/download and process startup breakdown separately, without excluding
in-process required work from the whole-chain total. Producer build directories,
compiled request proofs and acceptance flags are inaccessible to checking.

Record OS file-cache conditions as observed/controlled/unknown. A new workspace
is not a cold OS cache; do not drop host caches as an unstated prerequisite. Record
the location and provenance of precompiled dependencies. Warm or result-reuse
experiments are separately labelled and cannot replace the cold gate.

## 3. First reference profile and actual limit scope

**Decision:** x86_64-linux is the first binding performance platform. Freeze a
specific available host/resource allocation before final measurements; recorded
inventory alone is not a reservation or a demonstrated capacity profile.

**Observed facts:** the investigation archive identifies Linux 6.18.38/glibc2.42,
Intel Xeon X5670 at 2.93 GHz, the published 0.13.1 wheel hash and Lean 4.30.0 Nix
derivation. Current source-only host inspection reports KVM, eight online vCPUs,
affinity 0–7, and 24,542,280 KiB total system memory. Available memory fluctuates;
it is not a fixed allocation. The archive has an empty `cgroup_limits` object;
the presumed `/sys/fs/cgroup/cpu.max` path is absent in this session. Neither is
evidence that there is no effective resource restriction.
[Archived environment](../evidence/environment.json).

**Planned profile fields:** CPU model/virtualization and allocated CPUs, affinity,
effective cgroup/scheduler quota, memory allocation and headroom, swap policy,
OS/kernel, storage, package/toolchain derivation identities, process limits,
thread counts, dependency-cache provenance and OS-cache observation. Resolve
effective controls using actual mount/cgroup paths. Reserve sequential heavy
runs; note competing load, throttling and any host migration. ARM retains existing
correctness/compatibility checks without an unmeasured performance guarantee.

The published process profile is precisely:

| Control | Actual baseline scope |
| --- | --- |
| `wall_seconds = 180` | Each launched stage process separately; no total request budget |
| `max_heartbeats = 200000` | Lean elaboration invocations via `-DmaxHeartbeats`; not an observed consumed count |
| `memory_mb = 4096` | Lean `-M4096` heap setting, not the total process RSS/address-space cap |
| `address_space_mb = 8192` | Bash `ulimit -v` for both Lean and fresh checker |
| `-DmaxRecDepth=4096`, `-j1` | Lean compilation/audit invocations |
| `leanchecker --fresh Concrete` | Fresh checker receives no corresponding Lean heartbeat/heap/depth flags |

[Default limits](../../../crates/behavior-models/src/safety/certify.rs#L190),
[process launcher and memory sampling](../../../crates/behavior-models/src/safety/certify.rs#L404),
[stage arguments](../../../crates/behavior-models/src/safety/certify.rs#L664),
[fresh invocation](../../../crates/behavior-models/src/safety/certify.rs#L737).

The baseline samples direct-child VmRSS/VmSize every 20 ms, not a kernel exact
whole-tree peak. Its wall-time failure returns before the normal stage-resource
record is written. **Planned instrumentation** must retain failed-stage records,
monotonic total wall time, CPU, peak memory with clearly stated direct-child/tree
scope, exact commands and effective limits. Record unmeasured actual heartbeat and
recursion depth as unavailable, never as zero or their configured ceilings.

## 4. Six-dimensional domain-neutral corpus

**Decision:** retain A256/A512 byte-identifiable baseline requests, add independent
axis sweeps and targeted interactions. Freeze concrete sweep levels from pilots
before final comparison; no numerical general capacity claim is justified yet.

| Axis | Planned valid cases and controls on confounding |
| --- | --- |
| Actions | FO-only distinct actions with one assignment each; A256 and A512 mandatory. Keep protected field/policy count fixed. Add generated-action inventory coverage separately. |
| Operations | Fixed authored action count, vary independent field assignments per action under FO; inventory occurrence counts are independently expected. Mixed LD cases keep at most one relevant lifecycle operation and vary mechanically irrelevant extra writes. |
| FO policies | Vary partitioned fields and overlapping restrictive policies; hold field/operation count fixed where possible. Use conjunctive permissions with distinct creator/writer/remover cases. Report when an input constraint couples dimensions. |
| LD dependencies | Replicated neutral parent/child pairs and several child types sharing one parent; every affected dependency checked. A parent transition remains one actual operation even with many guard obligations. |
| Guards | Supported canonical forms only: vary required dependency guards, terminal/prohibited literal-set sizes and supported membership permutations. Distinguish recognized bound/enum complexity from unsupported arbitrary equivalent expressions; never broaden v1 to manufacture a benchmark. |
| Encoded size | Fixed action/policy/operation counts with longer source labels or unrelated declaration data; separately measure transport, inventory/policy bytes, generated proof source/data and witness size. Names can change BehaviorHash, so each request retains its own exact identity; compare safety premises, not hash equality. |

Material interactions include action × FO-policy count/permission density,
action × operations, LD count × guards/shared parents, and generated Models plus
FO/LD in the same admitted module. Construct generated actions that retain the
existing one-relevant-operation rule; do not turn safe-by-construction expectations
into a waiver for the full inventory. Retain five existing Safety reference cases,
membership-order cases, creators/removers and all previously certified valid v1
constructions as regression sources. [Existing reference runner](../../../models/examples/safety_models/run.py),
[generated StateMachine fixture](../../../models/examples/safety_models/fo_state_machine.py),
[shared dependencies](../../../models/examples/safety_models/ld_shared_parent.py).

**Alternatives considered:** a full Cartesian product is unnecessarily expensive;
FO-only action growth cannot establish mixed-model capacity; blindly cloning
identical actions can create duplicate identities rather than grow a valid case.
Use one-axis controls plus justified interaction cases, log all actual dimensions,
and independently compare intended occurrences against admitted extraction.

## 5. Positive, negative and mutation acceptance

**Decision:** every negative group has a valid paired control and asserts the
relevant expected request. No mutation result substitutes for a positive full
certificate and independent check.

- Uniqueness/representation: duplicate full key, dropped/duplicated witness entry,
  equal-length non-permutation, truncated key/wrong ordering law, modified payload
  and stale identity. Include a different independently constructed *valid* witness.
- Permissions/coverage: unauthorized assignment including aliases and same-value
  writes, creators/removers separately, generated unauthorized action, one denied
  overlapping policy, missing action/operation and omitted expected policy.
- Lifecycle: removed guard, wrong binding/owner association, insufficient OR or
  polarity, same-action conflicting relevant effects; preserve explicit supported
  versus unsupported classification rather than calling all rejection unsafe.
- Proof integrity: real `sorryAx`, forbidden/custom/native axiom, weakened final
  theorem or wrong identity, missing audit root, tool mismatch, interrupted stage.
  Audit must import the actual mutated module; positive-parent import is invalid.
- Untrusted transport: supplied source/code/compiled executable artifact, malformed
  or incompatible contract, forged success flags and prior-result substitution.
  Inputs must be rejected before arbitrary package code could execute.
- State transparency: compare ordinary IR, BehaviorHash, established Model receipts,
  Core transitions and replay with/without policies; separate state-anchor/genesis
  and deployment authorization claims from preservation certificates.

[Existing public bundle controls](../experiments/bundle_controls.py) are four
small-A1 transport mutations, not kernel negative proofs. [Existing proof controls](../experiments/proof_integrity_controls.py)
cover writer removal, a proof hole and a weakened statement. Extend these controls
for the selected checked representation without claiming they already exercise a
new witness format. [Public evidence tests](../../../python/tests/test_safety_evidence.py)
already include code rejection and expected complete-policy reconstruction.

## 6. Existing commands versus planned additions

**Existing runnable investigation driver:**

```bash
python specs/511-safety-certification-scale/experiments/certification_probe.py \
  --actions 256 --operations 1 --policies 1 --expect certified --verify \
  --toolchain "$BEHAVIOR_SAFETY_TOOLCHAIN" --output /tmp/behavior-511-A256-new
```

Use a new output path and isolated installed consumer Python. Replacing 256 with
512 gives the other required baseline. This FO driver supports actions,
operations/action, partitioned policies (`policies <= operations`) and label
padding. `--verify` uses the public verifier but does not provide a fresh external
consumer process or complete independent resource measurement. It is not the final
cold-capacity gate. [Driver](../experiments/certification_probe.py).

**Existing complete-reference/integration commands:** `scripts/check-safety-proofs.sh
--output PATH` runs five certification references, Rust preflight and the six-case
Core/Safety guard matrix, not a six-axis scale sweep or independent bundle timing.
`scripts/gates.sh` already includes that gate plus 506/507 and other ecosystem
checks. Reuse this integration rather than a parallel release process.
[Proof gate](../../../scripts/check-safety-proofs.sh),
[ecosystem gate](../../../scripts/gates.sh).

**Planned additions:** corpus manifest/generators for LD/generated interactions;
external sequential installed-consumer runner with separate fresh checking process;
profile/resource capture and fail-safe incomplete reports; budget-freeze manifest;
construction/checking growth report and independent witness mutations. Command
names and schema details belong to implementation planning/contracts, and no
claim is made that these additions already execute or pass.
