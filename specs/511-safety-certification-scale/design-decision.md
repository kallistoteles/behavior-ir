# G1 decision: exact data, checked merge traces, original preservation contract

2026-10-22. Baseline: published Behavior 0.13.1, Core 0.15.0, Lean 4.30.0.
This selects integration work; it does not claim final performance acceptance or a library certificate.

## Evidence and alternatives

See [comparison](evidence/candidate-comparison.json) and [small controls](evidence/candidate-runs/prototype-controls.json).
The exact full A256 and A512 mathematical instances passed all twelve mapped baseline claims,
four bridge claims, intended-type/transitive-axiom audit and `leanchecker --fresh`.
Generic compiled imports were reused: these measurements are diagnostic stages, not cold chains.

- Direct verified merge-sort uniqueness exhausted simplification steps on A256 (~52.5 s).
- A verified inverse/index-witness checker exhausted 200,000 heartbeats on A256 (~47.1 s).
  These are implementation/resource failures, not false safety claims or counterexamples.
- A merge trace checked against original full keys proved A256 uniqueness in ~4.0 s, A512 ~8 s.
- Exact UTF-8 string anchors eliminate the original large decimal-byte constructor surface.
  Factored full keys retain every byte; raw Nat literals avoid unnecessary numeral elaboration.
- `noncomputable def` for proof-only concrete data suppresses runtime code generation without
  changing the definition's value, type, transparency or kernel checking. A512 data compiled in
  ~20 s, compared with ~34 s for the same raw-literal data with runtime compilation enabled.
- Explicit `prelude` imports narrow the actual fresh dependency closure; all original mathematical
  statements and proofs remain. Fresh replay fell from ~140 s to ~74–75 s in the measured prototypes.
  Required declarations are not skipped or cached.
- The combined A512 instance/data/audit/fresh steps took ~164 s. Generic compilation, public
  Core preparation, startup and persistence must still be measured in the final complete chain.

No new dependencies or permission lookup algorithm are justified. Retain original authority,
coordinate bounds, lifecycle link and lifecycle predicates. Quadratic residual work is measured,
not asserted to be linear. The profile's whole-chain 300 s budgets remain unchanged.

## Selected checked interpretation

Canonical inventory/policy UTF-8 bytes remain exact, represented as one interpreted chunk rather
than 64-byte literal chunks. Lean proves flatten preservation and string-byte injectivity. Rust's
extraction/canonical encoding remains trusted correspondence; those lemmas do not prove a JSON
parser or Core extractor correct. Full action keys are preserved, never replaced by small ranks.

The producer supplies a merge tree and left/right choices. Leaves consume the independently
prepared original action-key list in order. Replay must consume all input, and adjacent sorted
keys must be strictly increasing. Generic Lean proofs establish exact multiplicity-preserving
permutation and transfer strict-order uniqueness to the original keys, for *any* proposed trace.
The producer's sorting/construction algorithm is not a soundness assumption.

A checked assembly lemma reconstructs the original `checkModule` premise from this uniqueness
proof and the *unchanged* bounds, authority, lifecycle-link and lifecycle checks. The original
FieldOwnership and LifecycleDependency theorems then establish the baseline guarantee.

## Typed witness and execution boundary

Bundle v2 has exactly `format`, `module`, `inventory`, `policies`, `evidence`, `witness`.
The witness is a flat postorder tree:

```json
{"format":"behavior.safety_merge_trace.v1","nodes":[{"kind":"leaf"},{"kind":"leaf"},{"kind":"branch","left":0,"right":1,"choices":[false,true]}],"root":2}
```

An `empty` node is allowed for empty input. Every branch references earlier nodes. Root is last;
every other node has exactly one parent, preventing cycles, orphan nodes and exponential DAG
expansion. Unknown/missing keys, booleans as indices, nonboolean choices and malformed topology
are rejected. Request-specific bounds are derived from the complete input: at most `2*n+1` nodes
and `n*n+n` choices, with checked arithmetic; transport limits/exhaustion cannot certify.
The normal producer uses a balanced tree. Other valid trees are accepted within resource limits.
The flat transport has no recursive JSON or supplied expression/code grammar.

Only trusted installed Rust templates produce Lean source. No supplied Lean, olean, scripts,
plugins, prior-success flag or executable deserialization object is an input. The consumer freshly
re-admits the exact module, compares every operation and complete expected policies, and checks
the supplied trace. It does not reconstruct sorting merely to reproduce producer choices.
Different valid traces may have different evidence identities for the same mathematical claim.

## Versions and historical artifacts

- Ecosystem **0.14.0**: existing 0.x policy requires a minor for changed certification outcomes
  and evidence contracts. Core stays exactly 0.15.0; verifier stays 0.10.0.
- Safety checker **3**, statement contract **2**, evidence **v2**, bundle **v2**.
- FieldOwnership/LifecycleDependency policy meaning stays **1**. Complete policy semantic identity,
  Core BehaviorHash, ordinary Model receipts and runtime state/replay remain unchanged.
- New evidence hashes bind exact witness, theorem/checker/source/dependency identities and complete
  claim. Old evidence/fixtures/releases are immutable. This checker supports v2 only; historical v1
  is inspected with its original pinned implementation, subject to its soundness history.

The new trace soundness, exact-byte refinement and assembly obligations are audited in addition
to all twelve mapped baseline obligations. Exact final statement types and transitive axioms
(`propext`, `Classical.choice`, `Quot.sound` only) remain mandatory, followed by fresh replay.
No native oracle, proof hole or producer correctness premise is permitted.

## Trust and completion gates

Remaining trusted code: pinned Core semantics, Rust full-inventory extraction and model/probe
correspondence, encoding/expected wrapper generation, immutable build/toolchain integrity.
Lean proves the abstract checking-to-original-safety implication, not this entire implementation.
Genesis, migration safety, exact approved-module enforcement and fresh activation anchors remain
separate host obligations. No runtime Safety state or Core modification is introduced.

Positive and deliberately invalid small instances, alternate valid trees, truncated/extra merge
choices, full-key duplicates, identities, weakened statements and actual `sorryAx` controls passed.
Formal integration and final installed A256/A512/composed cold checks remain required. The final
candidate must satisfy the unchanged frozen profile and complete delivery gates before completion.
