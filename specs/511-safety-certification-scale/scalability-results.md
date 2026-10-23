# Scalability results

**Capacity profile passed:** all twenty frozen cases, three cold pairs each, on the exact
installed candidate. Mathematical claims, trusted correspondence and empirical capacity remain
separate. Final control/delivery reconciliation is tracked in the remediation assessment.

## What was necessary, and what was incidental

Certification needs a complete inventory, correct ownership and lifecycle premises, exact claim
binding and checked generic/concrete proofs. It does not logically require decimal byte-list
syntax, a particular payload chunk layout, pairwise uniqueness evaluation, runtime compilation of
proof-only data, or every dependency of Lean's broad default imports.

The released failure has several distinct contributors; `maxRecDepth=4096` is not a root-cause
explanation by itself. Exact published A256 input was reproduced, then data elaboration, uniqueness,
authority, audit and fresh replay were isolated. Large constructor/elaboration surfaces and the
original pairwise full-key uniqueness computation imposed avoidable work. Narrowing imports also
reduced the actual dependency closure freshly checked by the kernel. The original ordinary
transition theorems and their twelve claims were retained.

See [root-cause analysis](root-cause-analysis.md), [candidate comparison](evidence/candidate-comparison.json)
and [design decision](design-decision.md). Diagnostic prototype times reused compiled generic
imports and are not complete cold-chain acceptance measurements.

## Checked representation and algorithm

Exact canonical UTF-8 payloads replace large literal constructor surfaces. Lean proves flattening
and injectivity of the interpreted bytes. Full action hashes remain authoritative; small ranks
never replace their uniqueness obligation. A producer proposes a finite merge trace. Lean checks
that replay consumes every original key exactly once and yields strictly increasing keys, then
transfers uniqueness through its proved permutation relation. A checked assembly lemma restores
the original `checkModule` premise. Producer correctness is not an assumption.

The existing pinned Lean library supplies verified permutation, ordering and string-byte facts.
No new library, oracle, native-evaluation axiom or permission-lookup algorithm was needed. Direct
merge-sort reduction and inverse/index witnesses were evaluated and rejected on measured resource
cost, rather than accepted from their native execution complexity.

For a balanced trace, the producer emits a logarithmic number of merge levels and proportional
choices per level. Arbitrary accepted traces may be skewed; request-derived bounds also constrain
resource use. This structural observation is not a theorem about Lean elaboration or kernel time.
The unchanged authority checker still nests actions, policies, operations and permission-list
membership. Growing action and writer-list counts together retains quadratic membership work in
the simple FO action sweep. No linear whole-certification complexity claim is made.

## Measurement interpretation

The frozen profile varies action count, operations, policy count, lifecycle dependencies, guards
and payload size, plus mixed/shared-parent/generated-action interactions. Some dimensions are
coupled by supported input constraints; these are not independent factorial sweeps. Every accepted
case requires three complete certification/independent-checking pairs, measured separately.

Each role includes process startup, input staging, fresh admission/extraction, generic and concrete
proofs, exact theorem-type and axiom audits, fresh kernel replay and result persistence. Tool-stage
records provide breakdowns but are not subtracted from total time. Installed pinned dependencies
are present; OS page cache is uncontrolled. No previous request result substitutes for a check.
CPU is waited-child time; memory is sampled process-tree RSS/VmSize with stated limitations.
Consumed heartbeat and recursion high-water measurements are unavailable.

## Final measurements

The real acceptance entrypoint completed **60/60 pairs (120 roles)** in 9,476.77 seconds. The
independent collector accepted all expected requests, artifact/profile identities, complete proof
obligations and **1,440 actual tool-stage records**. Every role completed within its unchanged
300-second budget; the published per-stage limits were also retained.

- [Frozen candidate](evidence/candidate-builds/final-01.json), [original profile](evidence/evaluation-profile.json).
- [Complete assessment](evidence/final-runs/assessment.json), [aggregated observations](evidence/final-runs/scaling-summary.json).
- [Mandatory three-pair floor records](evidence/final-runs/mandatory-floors.json).
- Artifact: `behavior-0.14.0-cp313-abi3-manylinux_2_28_x86_64.whl`, SHA-256
  `810566f34559851fb6e725179a7cb066e64fae3a3a25d3eb85e524855de3902b`.

Times below are seconds: **median [minimum–maximum]**, three independently constructed/checked
cold pairs per case. This is descriptive measurement, not a probabilistic performance guarantee.

| Case | Actions / operations / FO / LD | Inventory bytes | Certification | Independent checking |
| --- | --- | ---: | ---: | ---: |
| A1 | 1 / 1 / 1 / 0 | 983 | 67.14 [66.59–67.44] | 67.45 [66.95–70.00] |
| A4 | 4 / 4 / 1 / 0 | 2,336 | 66.80 [66.18–67.03] | 68.39 [68.34–69.61] |
| A16 | 16 / 16 / 1 / 0 | 7,748 | 68.80 [67.94–69.96] | 67.75 [67.48–67.99] |
| A64 | 64 / 64 / 1 / 0 | 29,396 | 72.10 [71.11–75.15] | 72.48 [71.62–73.05] |
| A256 | 256 / 256 / 1 / 0 | 115,988 | 101.71 [100.94–106.61] | 103.10 [101.86–103.49] |
| A512 | 512 / 512 / 1 / 0 | 231,444 | 188.52 [183.96–195.66] | 186.32 [180.83–188.61] |
| O4 | 16 / 64 / 1 / 0 | 19,550 | 68.95 [67.61–69.72] | 69.34 [69.07–69.88] |
| O8 | 16 / 128 / 1 / 0 | 35,286 | 69.60 [69.06–69.84] | 69.67 [68.72–70.62] |
| P2 | 16 / 128 / 2 / 0 | 35,286 | 70.26 [69.28–70.88] | 69.18 [68.82–69.96] |
| P4 | 16 / 128 / 4 / 0 | 35,286 | 69.14 [69.04–70.38] | 70.94 [68.56–72.97] |
| P8 | 16 / 128 / 8 / 0 | 35,286 | 69.51 [69.17–70.06] | 70.50 [69.77–71.08] |
| D1 | 8 / 8 / 2 / 1 | 4,827 | 70.41 [70.29–71.50] | 71.44 [71.39–71.55] |
| D2 | 12 / 12 / 3 / 2 | 7,336 | 72.89 [72.26–73.39] | 74.11 [72.58–74.89] |
| D4 | 20 / 20 / 5 / 4 | 12,354 | 79.93 [79.49–80.86] | 81.66 [80.39–82.76] |
| G1 | 8 / 8 / 2 / 1 | 4,827 | 70.22 [69.43–70.46] | 70.03 [69.97–71.79] |
| G3 | 8 / 8 / 2 / 1 | 4,827 | 69.97 [69.83–70.54] | 72.96 [70.67–73.83] |
| B32 | 16 / 16 / 1 / 0 | 8,804 | 68.45 [67.86–70.61] | 68.33 [67.99–68.43] |
| B128 | 16 / 16 / 1 / 0 | 11,972 | 69.15 [67.76–69.45] | 68.19 [67.69–68.79] |
| M2 | 17 / 17 / 5 / 2 | 10,144 | 74.10 [73.77–74.66] | 74.68 [74.31–76.32] |
| M4 | 37 / 37 / 7 / 4 | 20,604 | 84.30 [83.25–85.75] | 85.40 [84.45–87.25] |

Maximum whole-role times were **195.66 s certification** and **188.61 s checking**. Maximum sampled
process-tree RSS was approximately **2.49 GiB**; this is not an exact or shared-page-adjusted peak.
All twenty requests reproduced identical canonical semantic evidence across their three pairs
and between producer and consumer. Different valid witnesses need not identify one artifact.

### Measured contributors

Certification component medians, seconds. Components have independent medians and therefore need
not sum exactly to the median total. Concrete combines data elaboration and proof; the separate
data/proof attribution comes from explicitly diagnostic prototype observations, not invented
subtimings for these final runs.

| Case | Generic sources | Concrete data + proof | Type/axiom audit | Fresh replay | Outside tool intervals |
| --- | ---: | ---: | ---: | ---: | ---: |
| A1 | 13.84 | 0.77 | 1.82 | 50.11 | 0.55 |
| A64 | 13.52 | 5.13 | 1.82 | 51.09 | 0.61 |
| A256 | 13.58 | 29.97 | 1.90 | 55.61 | 0.79 |
| A512 | 13.68 | 96.84 | 2.05 | 74.40 | 1.10 |
| D4 | 13.38 | 13.39 | 1.88 | 50.71 | 0.64 |
| M4 | 13.49 | 16.41 | 1.82 | 51.63 | 0.68 |

The remaining interval includes startup, admission/extraction, input/output preparation, tool
selection and result persistence. It is included in the whole-chain budget. Fresh replay uses the
same pinned Lean kernel in a new checking environment, not a separately implemented kernel.

### Growth and causal interpretation

1. **Actions.** A1–A512 hold one operation per action and one FO policy; total operations and the
   writer list grow with action count. Concrete medians grow from 0.77 s to 96.84 s; whole-chain
   medians from 67.14 s to 188.52 s. Generic compilation and fresh replay form a large baseline.
   The unchanged authority scan retains quadratic membership work. Twofold action growth from
   A256 to A512 increases concrete time about 3.23 times, but this finite curve is not a theorem
   about kernel asymptotics. Full pipeline growth is neither asserted linear nor unbounded.
2. **Operations.** At sixteen actions, A16/O4/O8 increase operation occurrences from 16 to 64 to
   128. Certification medians are 68.80/68.95/69.60 s; concrete work 1.62/1.84/2.37 s. Complete
   operation inventories remain checked; constant proof infrastructure dominates these sizes.
3. **Policies.** O8/P2/P4/P8 retain 128 operations while partitioning protected fields among
   1/2/4/8 FO policies. Certification medians remain approximately 69–70 s, while concrete work
   rises from 2.37 to 2.71 s. This is measured small-policy capacity, not a general constant-cost
   policy lookup claim. Ordinary Core behavior is unchanged by the policy-set variations.
4. **Dependencies.** D1/D2/D4 grow 1/2/4 shared-parent dependencies. Actions, operations, ownership
   controls and guards necessarily grow too; this is a coupled sweep. Concrete medians are
   3.62/6.36/13.39 s and certification 70.41/72.89/79.93 s. Every affected dependency is checked.
5. **Guards.** G1/D1/G3 use one/two/three selected enum values. Concrete medians are
   3.28/3.62/3.90 s; total medians overlap near 70 s. Three repeats do not establish an exact
   guard-complexity law. The separate 82-instance v1 regression covers larger membership sets;
   it is correctness evidence, not this frozen performance sweep.
6. **Encoded data.** B32/B128 add unused schema fields while retaining sixteen actions and their
   effects. Certification medians remain 68.45/69.15 s. Compact exact string anchors avoid the
   old constructor surface without deleting bytes or changing relevant semantics.
7. **Interactions.** M2/M4 include generated StateMachine actions, separate FO writes and
   two/four shared dependencies. Certification medians are 74.10/84.30 s and checking
   74.68/85.40 s. All actual operations are inventoried, including those mechanically irrelevant
   to LD; all applicable FO and LD obligations remain mandatory.

The [sequential published baseline](evidence/sequential-baseline.json) failed to complete A256/A512
concrete checking under the original 180-second stage wall limit. Its uncompleted chains are not
valid denominators for a claimed full-chain speedup. The final candidate completes those exact
semantic request classes without increasing resource limits. Isolated source controls establish
the representation/uniqueness/import-closure causes; final measurements establish practical
capacity of this complete chosen solution. They do not allocate all improvement to one change.

## Reproducibility and limits

Re-run the summary with `python evidence/final-runs/summarize.py` from the feature directory,
or use the absolute script path from elsewhere.
The immutable profile specifies x86_64-linux, Xeon X5670 KVM, eight available CPUs and the actual
memory/kernel conditions. No competing heavy task was run during the sequential sweep. Pinned
installed dependencies and OS page cache are distinct: prior request results were not reused,
while page cache was uncontrolled. Installation/download time is separate.

Finite tested capacity is not universal completeness or an action-count language cap. A256/A512
are minimum references. Larger modules can exhaust existing stage/heap/recursion limits; that
produces Inconclusive, not a counterexample or new Unsupported language rule. Permission-list
work and dependency replay remain measurable residual costs. No ARM performance has been measured.

Rust extraction, semantic probes and exact encoding remain trusted correspondence. Initial validity,
migrations, approved module/policy enforcement and activation continuity remain separate obligations.
See [remaining limitations](remaining-limitations.md) and [assessment](remediation-assessment.md).
