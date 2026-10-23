# Contract: Certification and Untrusted Evidence

## Existing public seams

Retain `certify_safety(module, *, output, toolchain=None, limits=None)`,
`load_safety_bundle(bundle)` and
`verify_safety_evidence(module, bundle, *, output, toolchain=None, limits=None)` in
[Python](../../../python/behavior/safety.py), backed by authoritative Rust in
[certify.rs](../../../crates/behavior-models/src/safety/certify.rs).
Snapshot validation remains a separate exact-history operation.

Loading transport returns unverified data. Only a completed checking chain creates the opaque
certificate. Preserve existing authoring semantics and thin bindings. Any necessary public change
must be justified/versioned in G1 before implementation.

## Inputs and independent checking

- Consumer supplies its independently expected admitted module and complete policy set. A
  BehaviorHash cannot reveal an external undeclared policy intention.
- Re-admit transported module data through public Core and compare exact module identity.
- Independently derive full action/operation inventory and canonical policies; compare against
  consumer expectations and all transported claims. Include authored/generated actions and all
  mutation occurrences, not only a producer-selected relevant subset.
- Check proposed proof data against those independent inputs. Producer flags, inventories,
  prior certificates and result caches cannot substitute for any required check.
- Regenerate trusted proof/audit wrappers from validated typed data; check exact statements,
  allowed transitive axioms and fresh pinned-kernel replay before authority is issued.

The producer may be wrong or unverified. The independent verifier must establish the same full
claim without assuming its correctness. A valid witness from another producer must not be rejected
merely because of origin. Invalid-witness rejection does not establish module unsafety.

## Data-only boundary and selection gate

Baseline bundle v1 has exactly `format`, `module`, `inventory`, `policies`, `evidence`.
Do not add optional witness keys silently. G1 selects and versions a replacement grammar only if
justified; this contract does not prescribe its encoding, container or sorting algorithm.

For any selected grammar, define typed fields, duplicate/unknown-field treatment, sizes/depth/count
bounds, full-key representation, reference/index validation and contract/version dispatch. Reject
malformed/unsupported data deterministically and before executable use. Diagnostic resource limits
must not turn previously supported valid v1 language into a hidden reduced certification language.

Evidence cannot supply arbitrary Lean source, scripts, plugins, executable deserialization hooks or
compiled code for execution. Fixed trusted checking templates may embed only validated data using
unambiguous encodings. Test injection and forged-success attempts with an execution sentinel.

## Results and versioning

All required stages must succeed for an opaque certificate. Record Failed/Unsupported/Inconclusive
according to existing semantics, distinguishing invalid premises, unsupported shape, exhaustion and
established counterexamples. Unrun obligations remain explicit. A performance miss is separate from
mathematical validity; interruption of a necessary check prevents certificate issuance.

Bind evidence to exact module, complete policy identity, statement/proof/checker/source/toolchain
identities and checked obligations. Changes require a reviewed mapping/version decision under
[versioning policy](../../../docs/versioning.md). Policy semantics need not change merely because
proof representation changes. Established Model receipts and published fixtures remain unchanged.

Declare supported contracts explicitly. Incompatible historical evidence may be rejected without
claiming a counterexample. Preserve historical evidence and original pinned verification conditions;
reproducibility is not evidence that an old verifier is sound. No automatic legacy tool execution or
publication is authorized.

## Contract acceptance

Paired tests must cover missing action/operation/policy, stale identity, generated actions, differing
valid witnesses, dropped/duplicated/fabricated witness entries, malformed/incompatible data, code
injection and previous-success substitution. A failed producer, loader or partial proof cannot mint
certificate authority. All public API checks must also work in a clean installed wheel outside the
source checkout with packaged trusted proof sources.

## G1 selected contract (2026-10-22)

[The selection record](../design-decision.md) specifies checker 3, statement 2, evidence/bundle v2
and the flat `behavior.safety_merge_trace.v1` grammar. Bundle v2 adds exactly one `witness` field;
v1 remains immutable and is not silently extended. This checker declares v2 support only.
The witness is untrusted postorder merge-tree data; original full action keys and all policies
come from independent preparation. Root/edge/choice bounds and exact schema validation precede
trusted source generation. Supplied artifacts cannot redefine expected inputs or execute code.

All twelve mapped obligations plus trace-soundness/UTF-8/assembly bridges remain mandatory.
Different accepted trees bind distinct concrete proof/evidence identities; they need not match the
producer's canonical tree. Failed witness checking proves neither module unsafety nor the absence
of another valid witness. No producer approval or precompiled theorem substitutes for checking.
