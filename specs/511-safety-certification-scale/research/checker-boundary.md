# Checker boundary research — Feature 511

Source-only assessment, 2026-10-22. This records existing facts and planning decisions/candidates;
it does not establish that a replacement checker or witness format already works. No tests,
proofs or production changes were performed for this assessment.

## Decision 1 — Keep the original request authoritative

**Decision:** The independent verifier receives the admitted module and complete expected policy
set separately from the untrusted evidence. Re-admit transported Core data with Core's public
admission API; derive and compare the complete inventory and canonical policies itself. A producer
may suggest data but cannot define which actions or policies count.

**Rationale / existing facts:**

- `Inventory::extract` traverses public `Module::entities/actions`, entity fields and reference
  markers, action parameters and pre/postconditions, and every assignment/create/remove occurrence.
  The representation keeps operation-kind occurrence indices, target entity, initializer/value
  hashes and full action identities. Validation rejects duplicate action hashes and incomplete
  initializer inventories.
- `Inventory::verify_module` independently traverses the admitted public accessors and checks
  counts, declarations, fields, types, references, parameters, root hashes and every operation
  occurrence. This is a separate Rust comparison, not a formal proof of extraction.
- Current `prepare_bundle` already re-admits transport with `behavior_engine::admit`, checks the
  expected BehaviorHash, validates transported inventory against the module, reconstructs a fresh
  analysis using `resolve::imported`, and compares the exact canonical policy records with the
  caller's expected analysis. `imported` performs fresh extraction and resolves canonical policy
  declarations, including dependency-to-ownership references.

Sources: `crates/behavior-models/src/safety/inventory.rs:242`, `:573`;
`crates/behavior-models/src/safety/certify.rs:779`;
`crates/behavior-models/src/safety/resolve.rs:121`.

**Alternatives:** Trusting bundle inventories, a producer success flag or a policy subset is
insufficient. No verifier can discover an undeclared external policy intention from a module's
BehaviorHash alone; the complete expected policy set is necessary request input.

## Decision 2 — Separate constructing evidence from checking it at the existing consumer seam

**Decision:** Investigate a producer/checker split around `verify_safety_evidence`, retaining its
independent request preparation and replacing repeated construction only if a checked alternative
establishes the same complete claim. Keep existing public authoring and Core execution unchanged.

**Rationale / existing facts:** Current `verify_safety_evidence` calls `prepare_bundle` and then
`certify(module, &fresh, ...)`, regenerating trusted wrappers and repeating the entire proof chain.
It compares freshly generated semantic evidence with the supplied record. The consumer does not
execute supplied Lean source or load supplied compiled proof objects. This is independent
verification but currently not a distinct lower-cost witness-checking path.

`certify_checked` is the execution seam: inventory comparison and structural ownership check;
generic source compilation; concrete proof compilation; regenerated expected-type/axiom wrapper;
and `leanchecker --fresh Concrete`. A certificate is issued only when the chain succeeds.

Sources: `crates/behavior-models/src/safety/certify.rs:569`, `:618`, `:900`.

**Candidates to measure, not selected formats:**

1. A data-only auxiliary ordering/permutation witness, independently checked against full original
   action keys. A producer may use unverified sorting/search. The checker must prove coverage with
   multiplicity, duplicate rejection and the original uniqueness premise; set equality alone fails.
2. A verified direct uniqueness/permission algorithm over a more suitable representation, with
   no external witness. This is the necessary comparison against an unnecessary witness format.
3. A hybrid: producer-supplied finite certificates for expensive premises; the verifier reconstructs
   all authoritative module/policy data and checks them with fixed trusted proof templates.

Benchmark producer cost and complete consumer cost separately. No `native_decide`, accepted
producer result or unchecked executable container lookup may replace kernel authority.

## Decision 3 — Make representation refinement an explicit theorem obligation

**Decision:** Permit a different finite encoding/chunking and concrete theorem target. Define its
interpretation into the authoritative logical input, prove the checker sound for arbitrary witness
data, and preserve the versioned exact-module/policy binding. Do not make today's nested payload
structure an invariant of the new implementation.

**Rationale / existing facts:** `Safety.ModuleData` contains action hashes, operations, policies,
lifecycle input and identity anchors. `checkModule` checks well-formedness, conjunctive authority,
lifecycle link and lifecycle premises. `module_preserves` and `lifecycle_module_preserves` derive
the generic guarantees from that finite check. `InstanceGuarantee` additionally asserts equal
BehaviorHash/inventory/policy bytes and **nested** inventory/policy payload equality.

The Rust encoder currently builds both numeric semantic input and canonical byte anchors.
The concrete proof establishes anchor equalities by `rfl`; it does not parse those JSON bytes in
Lean or prove that Rust extracted Core's semantics correctly. Replacing an anchor by a digest
alone would therefore not automatically establish semantic correspondence. A new representation
needs a checked interpretation/refinement to the intended finite model; trusted extraction and
encoding must still be reported separately wherever not formally verified.

Sources: `crates/behavior-models/proofs/safety/Check.lean:6`, `:43`, `:66`, `:87`, `:101`;
`crates/behavior-models/src/safety/certify.rs:259`, `:337`;
`crates/behavior-models/src/safety/proof_data.rs:1`.

**Required abstract bridge (planned, not an existing theorem):**

`Checked(requestData, witness) = true -> PreservedGuarantee(requestData)`.

The conclusion includes ownership and conditional lifecycle preservation, complete action and
policy coverage, and exact semantic identity; it is not merely witness validity. It must hold
for arbitrary witnesses independently of the producer's algorithm. Representation decoding and
indexing must preserve entity/field/action coordinates and operation occurrences. If the new
contract proves the old finite predicate, an equivalence or implication bridge can reuse current
soundness theorems. If its target changes, map every baseline semantic obligation explicitly.

**Alternatives:** Preserve current `ModuleData` as an interpretation target; or introduce a
versioned logical input plus a proved bridge to current guarantees. Retaining old chunks merely
to reuse `rfl`, weakening uniqueness, or trusting a decoder by omission are not acceptable.

## Decision 4 — Preserve the exact proof-integrity contract, versioning changed roots

**Decision:** The verifier generates fixed wrappers from validated data, checks expected theorem
types, audits transitive axioms and performs fresh pinned-kernel replay. A new checker theorem and
representation theorem are additional proof authority, not replacements for omitted guarantees.

**Rationale / existing facts:** `Audit.lean` uses `isDefEq` against regenerated expected types and
`collectAxioms`, permitting only `propext`, `Classical.choice`, `Quot.sound`. The Rust runner checks
the exact twelve audit rows and performs fresh replay before issuing evidence. Existing bundle
verification checks exact proof source/checker/dependency identities, not a generic historical
version dispatcher.

Sources: `crates/behavior-models/proofs/safety/Audit.lean:8`;
`crates/behavior-models/src/safety/certify.rs:696`, `:851`.

**Alternatives:** Keep all old root names where the same propositions remain; or version the
contract and record an obligation-by-obligation root mapping. Neither requires the new verifier
to accept every historical format. Keep published artifacts immutable and identify unsupported
contracts explicitly. Do not infer trust in an old implementation from reproducibility alone.

## Decision 5 — Retain the current explicitly trusted correspondence boundary

**Decision:** Distinguish independent Rust checks from machine-checked mathematics throughout
reports. Do not imply that producer/checker separation itself proves extraction correctness.

**Rationale / existing facts:** Lifecycle analysis counts relevant operations across the union of
dependencies and checks each affected dependency. The finite Lean model independently checks links,
classifies operations and derives preservation under `ProbeSemantics`, Core integrity/freshness
and runtime-binding assumptions. Rust canonical probes reconstruct declarations via public Core
builders, check matching declaration identities and field/reference types, derive canonical root
hashes and require actual mandatory precondition roots. Proof encoding includes operation/guard
bindings and membership evidence. These paths are explicit trusted correspondence code, not
formally verified Core extractors.

Sources: `crates/behavior-models/src/safety/lifecycle_dependency.rs:225`, `:313`;
`crates/behavior-models/src/safety/guards.rs:196`, `:234`, `:330`;
`crates/behavior-models/src/safety/proof_data.rs:1`;
`crates/behavior-models/proofs/safety/Check.lean:75`.

The current evidence explicitly lists Core semantics/backend, extraction, encoding, semantic
hash identity, pinned proof infrastructure and (for lifecycle) canonical probes. Initial validity,
migrations and approved deployment segments remain separate obligations. New helpers should
reuse these public Core seams and preserve state transparency, not introduce runtime state.

Source: `crates/behavior-models/src/safety/evidence.rs:100`.

**Alternatives:** A formally verified Core-to-abstract extraction would strengthen assurance but
is not already present. A new proof-producing algorithm can be untrusted; the independently
implemented request traversal, canonical policy resolution, encoding and exact-type wrapper
remain trusted unless separately proved. The final trust table must say exactly which changed
representation bridges are machine-checked and which Rust/Core correspondence remains assumed.

## Planned discriminating controls

- Omit an action, an operation occurrence, a policy or a generated action from producer data:
  independent request preparation must reject or reconstruct it before proof acceptance.
- Supply a valid witness built by another producer: acceptance must depend on validity, not origin.
- Duplicate/drop/reorder auxiliary keys, truncate hash comparisons or mismatch action indices:
  check the original complete-key premise and permission relationships, with valid controls.
- Change a module/policy identity, weaken a theorem statement, add `sorryAx`, or supply executable
  evidence text: no accepted certificate and no execution of supplied code.
- Preserve all existing v1 valid constructions; do not hide slow cases as newly Unsupported.
- Measure exact reconstruction/parsing, finite checks, audit and fresh replay as part of the cold
  consumer total. A quicker producer is not evidence of quicker independent verification.
