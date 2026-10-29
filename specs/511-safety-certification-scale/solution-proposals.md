# Safety certification scale: solution proposals

These proposals follow the [measured failure decomposition](root-cause-analysis.md).
They are not production implementation decisions. Core, the Safety v1 predicates,
complete inventories, policy scope and the mathematical guarantees remain protected.

## Decisions after grilling — 2026-10-22

The user's Q1–Q10 decisions are recorded in [spec.md](spec.md#clarifications) and take precedence
over the earlier proposal's representation-preservation preference. Preserve semantic guarantees
and exact module/policy binding; payload/chunk formats, concrete proof targets and verification
contracts may change with machine-checked correspondence, explicit review and versioning.

Investigate separation of proof construction and independent checking as a priority, without
choosing a witness format or algorithm yet. A producer may use unverified construction/search;
its correctness must not enter the checker-soundness assumption. A sound checker must establish
the exact claim and independently establish complete module/policy coverage for any accepted
untrusted witness. No arbitrary evidence-supplied code may execute. Mandatory axiom audit and
fresh kernel checking remain. Existing verified components are preferred for proof-authoritative
work, but proving the search algorithm itself is not required for soundness.

Assess soundness, coverage/completeness and computational scalability separately. Within the
second dimension, distinguish v1 language compatibility, mathematical completeness under the
declared contract and practical resource capacity. Preserve all supported v1 constructs; no
new Unsupported classifications or mandatory domain restrictions may hide scaling failures.

A256/A512 remain mandatory floors alongside mixed-model cases. Freeze separate total cold
certification and independent re-verification budgets before final evaluation. First use a fixed
x86_64-linux profile with installed pinned components, a fresh workspace and no prior request
results. Record preinstalled compiled dependencies and OS cache conditions; installation/download
is separate. A performance miss is not a mathematical counterexample. Historical artifacts remain
immutable under their original verification conditions; reproducible old code is not thereby sound.

## Mathematical assessment and existing verified algorithms — 2026-10-20

The directions are mathematically admissible **under the obligations below**;
they are not yet proved correct integrated remedies. The first investigation did
not complete a survey of existing verified algorithms. The follow-up source review
now identifies reusable results in the already pinned Lean 4.30.0 distribution;
see [verified-algorithms-research.md](verified-algorithms-research.md). No new proof
assistant, Mathlib dependency or toolchain upgrade is required to investigate them.

1. **Representation changes need checked semantic correspondence.** Factoring transparent
   definitions may preserve current values by definitional equality, but preserving that
   representation is not a prerequisite. A justified new format or target needs a checked
   interpretation establishing the same or stronger guarantee and exact module/policy binding.
   Under the old contract, flattened-byte equality alone does not establish nested-payload
   equality. A reviewed new contract may change that chunk representation; it must establish
   its own correct binding rather than pretend the old concrete proposition is unchanged.
   An unverified decoder must not become a hidden proof assumption.
2. **Sort and duplicate-check an auxiliary copy.** For original hash list H, a
   candidate witness Q must satisfy `Q.Perm H` and `Q.Nodup`; these imply the exact
   existing `H.Pairwise (· ≠ ·)` premise. Lean already has `List.mergeSort_perm`,
   `List.Perm.nodup_iff` and `List.nodup_iff_pairwise_ne`. A strict adjacent-order
   check additionally needs a proved transitive, irreflexive order and a bridge to
   global uniqueness. Non-strict sortedness alone accepts duplicates. Ordinary set
   equality is insufficient: it forgets multiplicities. Do not reorder Core actions,
   change their index bindings, assume cryptographic collision freedom or compare
   only truncated/unchecked encoded hashes.
3. **Indexed permissions need semantic equivalence.** Reusing a verified ordered
   set or map can help membership checks, but the adapter must preserve the exact
   `allowed` predicate for assignments, creation and removal, and the conjunction
   of all applicable policies. Taking the union of writers across restrictive
   policies would weaken ownership. Completeness of extracted inputs remains an
   explicit Rust/Core correspondence obligation.
4. **Retain proof authority and measure kernel work.** Standard-library algorithm
   correctness does not establish the cost of constructing, reducing or freshly
   checking this application's proof. Lean's sorting code has proved optimized
   runtime replacements; compiled runtime speed does not automatically carry over
   to kernel reduction. Check each reused theorem's assumptions and transitive
   axioms, prove the application-specific bridge, and measure the complete existing
   certificate/re-verification path. No native-evaluation axiom or cached unchecked
   result is authorized.

For proof-authoritative work, the preferred next comparison uses applicable verified library
components before custom sorting or membership checks. A fallible producer may use unproved
construction/search. A small checked adapter may still be needed on the verifier side. Library
soundness, adapter correctness, exact input correspondence and measured scalability are separate.

## Recommended direction

Address **data representation and concrete proof cost together**. A data-only
correction leaves the measured uniqueness proof bottleneck; an efficient proof
cannot help if its input fails to elaborate. Validate each intervention separately,
then validate the complete unchanged mathematical claim.

### 1. Bound data-construction depth and reduce identity-encoding overhead

**Evidence:** A256 reaches depth 4096 in `def data`. Removing only inline action
hashes removes the error; factoring the same hashes also removes it at the same
depth limit. Separately, the raw identity anchors generate 135,124 Nat literals and
2,112 leaf definitions before the proof.

**Candidate approaches:** factor large nested values, construct data through
bounded-depth expressions, or use a more compact exact byte representation with
kernel-checkable interpretation. Select through isolated measurements, including
the independent expected-identity audit and fresh replay. No encoding is selected
yet. Raw `nat_lit`, noncomputable declarations, and typed constructors are useful
controls, but the completed experiments do not establish a sufficient remedy.

**Contract:** retain exact inventory/policy semantic content and coverage of every action and
operation. Current `InstanceGuarantee` syntax and chunk boundaries may change under a reviewed,
versioned contract with checked preservation of the guarantee and exact semantic binding.
Hashes or producer claims alone cannot replace correspondence and completeness checks. Any
encoding/decoding or helper belongs to the explicit proof boundary and needs adversarial tests.

**Expected benefit:** remove an input-representation depth ceiling and reduce work
unrelated to the mathematical preservation argument. **Priority:** necessary
remediation candidate, with a demonstrated partial proof of concept for factoring.

### 2. Prove the same finite premises with better checked evidence

**Evidence:** A256's uniqueness obligation alone exceeds 60 seconds, while the
other well-formedness checks take about 1.3 seconds. The Boolean checker executes
to `true` in under a second, which does not itself prove the result. The present
uniqueness algorithm makes 32,640 pair comparisons; authorization makes another
32,896 identity comparisons.

**Candidate approaches:** machine-checked helper lemmas and reusable subproofs, or
verified uniqueness/membership evidence using ordered indices, sortedness or
permutation witnesses. Any faster algorithm must have a proved implication to the
same required concrete premise. Explore both proof construction overhead and
algorithmic work; do not assume one accounts for all cost.

**Contract:** retain the same or stronger mathematical guarantees and all necessary premises.
Final targets and audit roots may be reviewed/versioned with an explicit mapping covering the
baseline semantic obligations and new soundness/representation proofs. The kernel checks all
required concrete obligations, with transitive axiom auditing and fresh replay. A false required
premise blocks certification. No proof oracle, `sorryAx`, native-evaluation axiom or unchecked
accepted result is allowed. An untrusted producer's algorithm may remain unproved.

**Expected benefit:** reduce logically incidental evaluation/proof overhead and,
where justified, the asymptotic work of finite checks. **Priority:** necessary
remediation candidate. More proof work than data factoring, but directed at a
separately measured bottleneck. `decide +kernel` alone is not a validated remedy.

### 3. Examine the fresh-check dependency closure after the blocking costs

**Evidence:** fresh replay accounts for 120.253 seconds in the A1 positive control.
The pinned checker replays all imported and local declarations. This is one
measurement; no fixed-cost curve or dependency-level timing has been established.

**Candidate approach:** inspect whether narrower explicit proof imports reduce
unneeded dependency work while retaining all mathematical definitions and every
required replay. Measure before changing imports. Do not skip or cache away fresh
kernel checking.

**Contract:** fresh checking remains from a new environment with the pinned Lean
kernel and the complete actual proof dependency closure. No new logical axioms.

**Expected benefit:** reduce baseline certification latency. **Priority:** follow
the blocking data/proof investigation; it does not resolve the demonstrated depth
or uniqueness failures by itself.

## Approaches not justified by the evidence

- Raising recursion, heartbeat or wall budgets as the sole remedy. Extending
  observation time already exposed further failures without certifying A256.
- Removing identity payloads, action hashes, policies, actions or operations to
  make a case pass. Such removals are explicitly invalid attribution controls.
- Accepting a fast `#eval` result as mathematical evidence. It is diagnostic only.
  Pinned `native_decide` introduces an axiom outside the current whitelist.
  [Lean implementation](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Meta/Native.lean).
- Returning a cached certificate without the checking required by the existing
  trust contract, weakening the theorem or reporting resource exhaustion as proof.
- Redesigning Core, adding runtime Safety state, expanding LifecycleDependency's
  language or changing the application's domain model.

## Next acceptance experiment

First repeat the A256 failure/control set without overlapping heavy probes and
measure small increasing cases. Then compare candidate interventions independently
and in combination, preserving the admitted module, policy set and guarantees. The first meaningful remedy
milestone is the **whole** A256 module receiving a valid conditional certificate
and passing imported-bundle re-verification, followed by negative mutations and
the six-dimensional mixed-model scaling corpus. A passing diagnostic component
does not satisfy that milestone.

Choose a supported capacity from repeated data on a recorded resource profile;
do not turn the largest convenient passing case into an undocumented size ceiling.
Any required proof/checker version changes must follow the existing identity and
compatibility rules, with published 0.13.1 artifacts unchanged.
