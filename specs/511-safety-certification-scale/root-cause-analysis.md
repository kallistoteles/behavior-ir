# Safety certification scale: empirical investigation

> Historical investigation record (2026-10-19). Its observations and diagnostic-only status
> are preserved. Subsequent implementation and final acceptance are tracked separately in
> [design decision](design-decision.md), [scalability results](scalability-results.md) and
> [remediation assessment](remediation-assessment.md).

**Baseline:** published Behavior 0.13.1 wheel, Core 0.15.0, verifier 0.10.0,
Safety checker 2, pinned Lean 4.30.0. **Date:** 2026-10-19.

This is an investigation and proposal record. No production remedy, theorem,
certificate format, dependency or published compatibility record has changed.
Diagnostic Lean variants are not Safety Certificates.

## Reproduction and provenance

The published x86_64 Linux CPython ABI3 wheel was installed without dependencies
into a separate consumer virtual environment. Its SHA-256 is
`1e3bd82a54863cc4fa6d035e34891012c14b024bfbbd5cea3ea251e43dbf9a0d`.
The imported package path and `behavior.versions()` are recorded per attempt.
The toolchain is
`/nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0`.
The repository baseline is `21ba441299998ca0c6baf53a5178d1b591216b1f` (`v0.13.1`).

The neutral case has one ordinary integer field, N authored actions, one
assignment per action and one FieldOwnership policy authorizing every action.
The RHS constants differ so the actions have distinct semantic identities.
There are no lifecycle dependencies, queries or enum guards. An independently
constructed expected action/operation map must match the actual full inventory,
and structural analysis must report no issues before certification starts.

Run with the isolated wheel, from the repository root:

```bash
python specs/511-safety-certification-scale/experiments/certification_probe.py \
  --actions 256 --expect certified \
  --toolchain /nix/store/7m5ds60a41w276hsx3rybv76x3609l6k-lean4-4.30.0 \
  --output /tmp/behavior-511-reproduction-256
```

The command exits nonzero when the valid case cannot certify. Output paths must
be new; earlier attempts are never overwritten. Use the installed consumer
Python, not an editable development installation.

| Actions / assignments / FO policies | Unmodified library outcome | Failed stage | Total wall time |
| --- | --- | --- | --- |
| 256 / 256 / 1 | Inconclusive; no certificate | Concrete: wall-time limit (180 s) | 194.745 s |
| 512 / 512 / 1 | Inconclusive; no certificate | Concrete: wall-time limit (180 s) | 195.281 s |

Both attempts passed inventory comparison and all generic proof compilations.
Concrete, theorem-type audit, axiom audit and fresh replay did not complete.
These two attempts overlapped; their wall times are **not** a scaling curve or
an uncontended performance comparison.

This independently reproduces a practical inability to certify supported valid
modules. A subsequent unchanged-source observation also reproduces the recursion
diagnostic, as detailed below. The private application's exact failure remains
unavailable; equivalence to that particular input is not claimed.

## Hypotheses and discriminating controls

The following alternatives were stated before the attribution probes:

1. Generated data declarations consume substantial time before the concrete
   safety proof. Prediction: compiling the exact data prefix alone remains slow.
2. Reduction of the concrete Boolean checker dominates. Prediction: the prefix
   is fast and adding `checked` creates the delay; `decide +kernel` may distinguish
   elaborator reduction from kernel reduction of the same proposition.
3. Finite ownership/well-formedness checks grow poorly. Prediction: independently
   checking uniqueness, authority and lifecycle exposes different growth.
4. Resource boundaries interact. Prediction: measured failures differ by actual
   stage and input; a timeout cannot be reclassified as recursion exhaustion.

All probes reuse the already compiled generic imports and run in separate
directories. They record exact source hashes, command lines, wall budgets,
kernel-reported child CPU time and maximum RSS. They do not measure actual Lean
heartbeat consumption or recursion high-water depth. Cache temperature is
uncontrolled. No cold/warm or cross-platform capacity claim is made.

## Established findings

### 1. The unchanged source has multiple failure boundaries

The **byte-identical**, unmodified A256 `Concrete.lean` was observed with the same
Lean arguments and limits, allowing 360 seconds instead of the library's 180-second
wall budget. It finished unsuccessfully after 297.936 seconds:

```text
Concrete.lean:2120:4: error: maximum recursion depth has been reached
Concrete.lean:2132:55: error: (deterministic) timeout at `whnf`,
maximum number of heartbeats (200000) has been reached
```

Line 2120 is `def data`; line 2132 is `theorem checked`. The latter diagnostic
occurs after a failed data declaration and may be a cascading failure. It is not
treated as an independent heartbeat measurement for a valid checked instance.
The extended observation is **not a remedy or successful certification**. It
exposes errors hidden by the shorter wall budget. The archive verifies equality
of original and observed source hashes.

### 2. Inline action-hash data triggers the recursion failure

Remove only the identity anchors for diagnostic attribution, retaining the exact
action hashes, operations, ownership policies and all other checker inputs:

| Data-only control, no concrete theorem | Result | Wall seconds |
| --- | --- | ---: |
| Original prefix, all anchors | Still running when stopped at 60 s | ≥60 |
| Same prefix, generated defs noncomputable | Still running when stopped at 60 s | ≥60 |
| Same anchor bytes using raw `nat_lit` | Recursion error in `def data` | 137.261 |
| No identity anchors; otherwise unchanged data | Recursion error in `def data` | 23.228 |
| Above, remove only `actionHashes` | Pass | 1.829 |
| Above, remove only `actions` | Recursion error | 21.622 |
| Above, remove only `policies` | Recursion error | 24.864 |
| No identity anchors; **same hash values factored into named definitions** | Pass at depth 4096 | 23.316 |

The removal controls intentionally cannot support certification. Factoring the
same hash values preserves the logical data while changing its construction.
Together these controls locate the depth failure in the large inline nested hash
representation. It is not necessary to evaluate a lifecycle guard or preservation
theorem to trigger it. No exact internal Lean stack function or high-water depth
has been established.

Identity serialization also contributes substantial pre-proof work: 135,124
anchor literals and 2,112 leaf definitions precede the data record. Suppressing
code generation alone does not eliminate the observed ≥60-second prefix delay.
Replacing only anchor numerals with raw Nat syntax still leaves a 137-second
data failure. A combined explicit-constructor/factored-hash full-source experiment
still reaches its 180-second wall budget. These changes are **not validated
scalability fixes**.

### 3. Concrete proof reduction has an additional bottleneck

Use the successfully compiled, factored diagnostic data so each component probe
imports it without repeating the 23-second data construction. The identity payloads
are omitted in this diagnostic instance; `checkModule` never reads those fields.

| Separate obligation over the same data | Result | Wall seconds |
| --- | --- | ---: |
| Action-hash `Pairwise (· ≠ ·)` by `decide` | Still running at wall budget | ≥60 |
| All remaining `wellFormed` checks, with only uniqueness removed | Pass | 1.331 |
| `checkAuthority` | Pass | 15.091 |
| Lifecycle link and checker, empty FO-only lifecycle data | Pass | 0.526 |
| `#eval checkModule data` | `true` | 0.724 |

The native `#eval` is **diagnostic execution, not a proof or certificate**. It
shows that evaluating the finite Boolean predicate and constructing/checking its
logical proof have materially different costs here. Neither `#eval` nor
`native_decide` may replace the required kernel-checked proof under the current
trust contract.

The uniqueness component is a measured proof-reduction bottleneck. Its source
algorithm is quadratic, but these single-size measurements do not establish a
runtime asymptotic curve. Authority is another material contributor. Merely
switching the complete diagnostic proof to `decide +kernel` still times out at
60 seconds; the default tactic also times out. That comparison alone is not a fix.

### 4. The complete chain works for a small positive control

The unmodified published certifier issues a conditional certificate for A1 with
all six mandatory obligations passed, all twelve theorem roots audited, only the
three permitted logical axioms, and fresh kernel replay. Total time was 143.111
seconds; fresh replay accounted for 120.253 seconds. This is one overlapping run,
not an uncontended benchmark or a constant-cost scaling result.

Fresh replay uses the **same Lean kernel in a fresh environment**, not a separately
implemented verifier. The pinned checker replays all imported and local constants;
the proof dependency closure can therefore matter even for a small instance.
[Pinned checker implementation](https://github.com/leanprover/lean4/blob/v4.30.0/src/LeanChecker.lean#L36-L39).

### 5. Negative controls remain closed

- 32 existing authoring, ownership, lifecycle diagnostic and state-transparency
  tests pass against the isolated published wheel.
- Four tampered public bundles—wrong module identity, missing action, missing
  policy and forged proof-integrity record—are rejected without a certificate.
  These are bundle-validation controls, not substitutes for kernel mutation tests.
- Three actual Lean mutations are rejected: removing the writer permission makes
  `Concrete.checked` fail; `by sorry` compiles but Verify rejects transitive
  `sorryAx`; replacing the final certified proposition with `True` compiles but
  Verify rejects the wrong proposition.

The mutation auditor imports the **mutated** compiled module from its own output
directory. An earlier attempt imported the positive original through an incorrect
diagnostic search path; its harness failed and it is discarded. Production already
restricts `LEAN_PATH` to the correct proof workspace.

A separate early marker experiment lacked the required command import and is
also discarded. It does not establish a valid data-prefix 180-second measurement.
Both invalid attempts are listed in [environment.json](evidence/environment.json).

## Source-supported cost contributors

The exact 256-action Concrete file is 618,012 UTF-8 bytes. Its identity anchors
contain 115,988 inventory bytes and 19,136 policy bytes, represented as **135,124
Nat literals in 2,112 leaf definitions**, plus aggregate lists. This serialization
is compiled before the concrete theorem. [Anchor generation](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/src/safety/certify.rs#L358).

The proof checker also contains two distinct quadratic scans in this case:

- Action-hash uniqueness makes 256×255/2 = **32,640** pair comparisons.
- Authorization searches across the complete writer list require
  1+…+256 = **32,896** identity comparisons.

These counts follow from the finite algorithms and exact valid input. They
are **not measured stage runtimes** and do not identify the first bottleneck.
[Finite checks](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/proofs/safety/Check.lean#L19),
[authorization](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/proofs/safety/Data.lean#L33).

Ordinary numeral elaboration in pinned Lean creates type and instance
metavariables and an `OfNat.ofNat` application for each literal; `nat_lit` directly
creates a raw Nat literal. That provides a one-variable representation probe.
[Pinned Lean implementation](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Elab/BuiltinTerm.lean#L192-L217).

## Mathematical contract and trust boundary

The generic theorems quantify over arbitrary finite typed states; the concrete
checker establishes premises for the extracted admitted module. Runtime examples
and finite enumeration remain correspondence evidence, not universal proofs.

The production path requires exact expected theorem types, transitive auditing of
all twelve roots with only `propext`, `Classical.choice` and `Quot.sound`, and fresh
independent kernel replay. A diagnostic variant passing `lean` alone establishes
none of the remaining certification stages. [Audit](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/proofs/safety/Audit.lean#L15),
[mandatory stage sequence](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/src/safety/certify.rs#L683).

Rust's complete extraction, policy resolution, canonical guard correspondence and
encoding remain trusted implementation boundaries. Lean checks the supplied
instance; it cannot establish that an omitted Core action or undeclared intended
policy was supplied. Bundle verification independently re-admits/re-extracts and
compares the expected complete policy set. The expected policy set must itself
come from a trusted consumer input. [Bundle verification](https://github.com/kallistoteles/behavior-ir/blob/21ba441299998ca0c6baf53a5178d1b591216b1f/crates/behavior-models/src/safety/certify.rs#L782).

`checkModule` does not inspect the raw inventory/policy payloads. Those payloads
anchor identity in `InstanceGuarantee`, and the separately generated Verify
wrapper checks expected identities. Dropping payloads is permissible only as an
explicit diagnostic control; it would weaken identity correspondence as a remedy.

Certification continues to establish conditional ordinary-transition preservation.
Genesis validity, migrations, approved-module enforcement, exact activation
boundaries and snapshot completeness are separate obligations. No investigation
artifact adds application state, modifies Core IR or affects Core replay.

## Evidence

Compact records: [evidence/observations.json](evidence/observations.json).
Generators: [certification_probe.py](experiments/certification_probe.py),
[lean_stage_probe.py](experiments/lean_stage_probe.py).
Raw observations and exact generated files are retained under
`/tmp/behavior-511-investigation.zqro9o/`; source and observation hashes are archived.
Use the generators to reproduce outside this machine.

Further attribution measurements, proposal assessment and remaining work are
summarized in [solution-proposals.md](solution-proposals.md).

### What this investigation has not established

This is a supported neutral reproduction and causal decomposition, not completion
of Feature 511. Remaining work includes an uncontended repeated scaling matrix
over all six dimensions, mixed FieldOwnership/LifecycleDependency and generated
actions at scale, an empirically justified supported capacity, and a remedy that
passes the complete exact-module certification and independent bundle verification
chain. No production remedy has yet been validated. The full ecosystem/release
gates were not run for this investigation.

## Sequential remediation baseline — 2026-10-22 (T010)

A new external runner used the immutable installed published 0.13.1 wheel (SHA-256
`1e3bd82a54863cc4fa6d035e34891012c14b024bfbbd5cea3ea251e43dbf9a0d`), original public
case construction, separate fresh producer/consumer processes and sequential heavy runs.
Published Lean limits were unchanged. The external 600-second role cap is an observation limit,
not a passing acceptance budget. Raw reports are retained under
`target/safety-models/scale-511/baseline-6WFcsY/`; compact evidence is
[evidence/sequential-baseline.json](evidence/sequential-baseline.json).

| Case | Certification wall | Independent checking wall | Result |
| --- | ---: | ---: | --- |
| A1 | 164.79 s | 136.48 s | Both complete with certificate |
| A4 | 142.91 s | 142.30 s | Both complete with certificate |
| A256 | 194.73 s | NotRun | Concrete exhausted the published 180-second stage limit |
| A512 | 220.75 s | NotRun | Concrete exhausted the published 180-second stage limit |

A256's generated Concrete source is byte-identical to the original investigation's source:
SHA-256 `8a4ffbab0ae40d6c49817020a8d320b43c0d24866fd3373568bf9bb6b42ead2e`.
The earlier extended-observation recursion/data controls therefore apply to the same input;
these new ordinary-limit runs themselves establish exhaustion, not a safety counterexample.
Inventory and generic proofs passed; concrete proof, theorem/axiom audit and fresh replay did not
complete for the failing cases and cannot be counted as certified. The external observer retains
failed/unrun states even though the published library omitted its failed-stage resource file.

These runs isolate heavy feature jobs; OS file-cache conditions remain uncontrolled. Sampled
process-tree RSS may count shared pages repeatedly, and CPU scope is waited-child CPU. Neither
configured heartbeats/depth nor missing counters are actual consumption measurements. Environment,
driver hashes and visible cgroup ancestor controls are recorded in
[evidence/baseline-execution-environment.json](evidence/baseline-execution-environment.json).

G0 now freezes a separate evaluation profile: twenty public supported valid cases, six dimensions
and mixed/generated interactions; three cold pairs per accepted case; 300 seconds separately for
complete certification and complete independent checking. This agent-selected operational target
limits a build/deployment pair to ten minutes on the specified reference environment and is grounded
in the controlled small-case costs. It is not claimed as an already achieved capacity, and cannot
be relaxed after seeing a candidate fail. See [evidence/evaluation-profile.json](evidence/evaluation-profile.json).
No candidate proof algorithm or representation has been selected yet.
