# Contract: Safety Evidence, State Validation and Deployment

## 1. Three independent claims

1. A module preserves the declared property under the listed assumptions.
2. An exact Store state satisfies that property.
3. The host permits only that module/policy segment's transitions from that state.

No individual artifact establishes all three. Safety Evidence is not a Core authorization.
Hashes establish content identity, not mathematical validity, provenance or approval.

### State transparency

Safety policies, theorem evidence and certificates are separate build artifacts. Neither
FieldOwnership nor LifecycleDependency adds persistent runtime state, a relationship/ownership
registry, an event stream, hidden execution behavior or a replay dependency. Ordinary Core IR
and Core Store history fully represent every application transition. Core state and behavior
replay require the relevant Core modules/BehaviorHashes; they must not read Safety declarations,
certificates or Lean. Missing safety artifacts may prevent reconstructing the historical safety
argument, never reconstructing application state. Retention of historical policy identities and
certificates is an assurance responsibility separate from Core persistence and replay.

## 2. Versioned artifacts

Use distinct tags and canonical documents:

| Artifact | Proposed format / hash domain |
|----------|-----------------------------|
| Individual Safety policy | Kind-specific canonical Model document / existing behavior.model.v1 tag |
| Complete policy set | behavior.safety_policy_set.v1 / same-named distinct domain |
| Extracted inventory | behavior.safety_inventory.v1 / same-named distinct domain |
| Assessment/certificate | behavior.safety_evidence.v1 / same-named distinct domain |
| State validation | behavior.safety_state_validation.v1 / same-named distinct domain |

Use public Core canonical/tagged hashing, not a second Core hashing implementation. Final tag
spellings are frozen with first v1 fixtures. Changing semantics/version/dependencies prevents
stale certificate reuse. Preserve existing Model receipt schemas and all established fixtures.
Each identity hashes its canonical payload excluding its own hash field; never introduce
self-referential inventory, policy-set, certificate or validation-record hashing.

Sets use canonical sorted arrays; typed values retain JSON types. Canonical semantic payloads
exclude wall-clock timestamps, durations, logs, source locations and temporary/absolute paths.
If recorded, such details reside in a separate diagnostic envelope. Actual pinned toolchain
derivation identity is a semantic input; record it canonically as an identity, not a mutable
executable search path. Different platform/tool builds are different reproduction inputs.
Compiled .olean bytes are transport products, not the stable mathematical statement identity.

## 3. Safety Evidence payload

The normative payload includes:

- Format, semantic/checker versions, outcome and typed deterministic diagnostics.
- Exact BehaviorHash and immutable Core release/dependency identity.
- Complete normalized policy set/member identities and policy-set hash.
- Full action inventory identity/list and covered mutation occurrences; no hidden partial scope.
- Every ownership, relevance/frame, guard/effect and preservation obligation/result.
- Generic/concrete theorem identities, versioned formal predicates/statements and proof-source
  dependency identities; exact concrete-data identity.
- Extractor/probe/encoder/auditor/checker identities and the selected locked Lean derivation.
- Structured transitive axiom results and fresh replay results for all required roots.
- Precisely named trusted correspondence assumptions and external exclusions.
- Initial validity explicitly unestablished by this module-only certificate; separate state
  records may establish a starting state for a deployment segment.

Outcome is Certified under assumptions, Inconclusive, Unsupported or Failed. Only the first
permits constructing a verified certificate, and only after all required checks succeed.
Known failed concrete premises cannot be hidden in an assumptions list. Initial-state/host
assumptions are external boundaries; missing module proof obligations are not optional ones.

A reproducible bundle contains admitted module transport, canonical policies/inventory,
fixed-schema proof input, pinned proof source manifest and assessment. Public Core serialization
may transport the module for re-admission; it must not become the expression-analysis interface.
Checker 2 additionally permits the release contract's non-authoritative literal-order discovery;
only Core-admitted exact-root probes and checked concrete premises can justify acceptance.

## 4. Evidence verification

Treat a loaded report as unverified data. Verify schema/types and canonical identities; admit
the supplied module through pinned Core; compare its actual BehaviorHash to the expected module;
resolve the exact policy set; independently re-extract complete actions/operations; compare all
inventory/proof-input identities; check expected theorem propositions and proof dependencies;
run the axiom audit and fresh replay. Regenerate concrete proof wrappers from validated data
and pinned supported proof sources; never execute untrusted bundle code or load its compiled
proof objects. Reject tampered, stale, incomplete or unsupported inputs.

This remains conditional on the trusted implementation/toolchain boundary. No signature or new
governance scheme is introduced. A host approves verified module/policy identities through its
existing authorization controls.

## 5. State validation payload and provenance

Acquire state through public Core `Store::export_seed_at(module, history)`. Use its complete
validated live seed, source HistoryRef, schema and StateId. Validate the original property in
Rust for every configured dependency, including terminal children's parent existence.

Record exact BehaviorHash, complete policy-set hash, Store identity, full HistoryRef including
position/history anchor, StateId, schema, per-property results, validator identity and provenance/
completeness assumptions. Return Valid, Invalid or Unestablished; these are state-validation
results, distinct from certificate outcomes. Incomplete/untrusted detached data never establishes
an actual Store state. Deterministic violations identify child/parent/policy without weakening
the original predicate.

A valid state at history k establishes Q(S_k), not validity of later states or previous history.
The validator is trusted Rust correspondence code, not automatically another Lean proof.

## 6. Exact-boundary host protocol

A documented simple protocol is one host synchronization authority covering **all** writers,
migrations and module activation:

1. Acquire its exclusive coordination boundary.
2. Select the incoming exact module and complete policy set and verify their certificate.
3. Capture current full HistoryRef H and export/validate that exact state under incoming policies.
4. Require a Valid record bound to H and those identities. Reconfirm exact current history if
   required by the backend/coordinator; a separate unlocked comparison is insufficient.
5. Atomically activate the incoming module/policy tuple while continuity from H is enforced.
6. Release coordination. All subsequent commits obey the activated tuple and Core commit rules.

Equivalent enforced protocols are allowed. The host must prevent unauthorized writers and
intervening commits from invalidating the anchor. Merely reading H, later comparing it, and
switching without coordination leaves a race. Checking StateId alone loses history continuity.
The reference stale-anchor control must include an intervening no-op history event.

This protocol is a host obligation/example, not a new Core locking, allowlist or governance API.
Core's hash-bound authorizations can participate, but its execution-policy allowlist is not a
declarative approved BehaviorHash set. The host/trusted authorizer must enforce eligibility.

## 7. Migration and history boundaries

Ordinary-transition certification excludes migrations/imported genesis and alternative-module
execution. Core target validity does not prove relational safety. Before resuming a certified
segment after migration, separately establish incoming-state validity or formally checked
preservation plus an enforced exact boundary. Feature 510 implements the state validator, not
a general migration proof system or seamless cross-module compatibility theorem.

Earlier valid segments do not prove incoming assumptions; a fresh valid segment does not
retroactively validate earlier states. Physical host behavior, scheduling, liveness and eventual
cleanup remain outside these certificates.

## 8. VM fixture scope

The acceptance fixture uses immutable required Ref[VM] ownership, existing parents for every
Resource, and non-Removed/non-Abandoned parents for nonterminal Resources. It is deliberately
restricted. Production domains may retain pool ownership or other arrangements; unsupported
Safety configurations receive precise diagnostics rather than a rewritten domain schema.
I-5b/I-6 remain unmapped pending authoritative definitions and implication/equivalence analysis.
No equivalence between live and nonterminal is assumed.
