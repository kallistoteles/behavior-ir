# Data Model: Safety Models

All semantic normalization and validation is Rust-authoritative. Names below describe planned
types/contracts, not a new Core schema. Core entity values, execution and hashing remain Core's.

**State transparency (FR-038):** None of these Safety types adds persistent runtime state,
additional event streams or replay inputs. Policy/evidence retention supports retrospective
safety assurance only. Core IR and Store history alone reconstruct application state; no
Safety relationship or ownership registry exists. This applies to both Safety Model kinds.

## 1. Authoring declarations

### FieldOwnershipInput

| Field | Meaning / validation |
|-------|----------------------|
| author_id | Unique authoring reference; diagnostics/selection only, not permission by spelling. |
| entity | Existing entity handle/name resolved to its admitted declaration identity. |
| writers | Field-key → explicit action selector set. Each field must exist. Empty set forbids all assignments. |
| creators | Explicit action selector set permitted to create that entity. |
| removers | Explicit action selector set permitted to remove that entity. |

Omitted permissions normalize to empty sets, never unrestricted authority. Creation permits
initialization without writer permission but satisfies all applicable creator and safety checks.
Multiple policies intersect; an action cannot gain authority from another policy's permission.
Duplicate/ambiguous declarations or selectors are errors, never ignored.

### LifecycleDependencyInput

| Field | Meaning / validation |
|-------|----------------------|
| author_id | Unambiguous reference for diagnostics. |
| parent / child | Existing resolved entity declarations. |
| owner_field | Child's nonoptional Ref to this parent type, verified by type and Ref marker. |
| parent_state / child_state | Existing enum lifecycle fields. |
| prohibited / terminal | Valid enum member sets, normalized deterministically. |
| ownership | Applicable FieldOwnership declarations covering parent/child lifecycle fields, owner assignment, creation and removal. |

The owner field's effective assignment authority is empty. Existing generating Model ownership
restrictions remain conjunctive. Missing control premises cannot be replaced by an assumption
that authors will avoid unsafe actions. Existing lifecycle Models may further restrict writers;
they never exempt an operation from the Safety checks.

This v1 relationship is deliberately narrow. Optional, pool-or-parent and reassignable ownership
configurations return precise unsupported/configuration diagnostics; applications can retain such
schemas without claiming this particular certificate.

## 2. Resolved policies and identity

### ResolvedAction / ResolvedField

- Action: final admitted action hash and typed binding inventory. Author selectors/source labels
  are diagnostic mappings. Missing, ambiguous or stale references fail resolution.
- Field: admitted entity declaration hash plus declared field key and admitted type. Ref marker
  is recorded separately from normalized ID type.
- Enum member: resolved enum declaration identity/member and Core-produced literal hash where
  required. The checker validates membership rather than accepting arbitrary strings.

### SafetyPolicySet

Contains the normalized FieldOwnership and LifecycleDependency documents. Each new kind begins
with semantic version 1 and checker/lowerer version 1. Use the existing ModelHash tagged canonical
hashing infrastructure for its kind-specific document; no established Model document changes.

The complete set has a separately tagged identity over deterministically sorted member identities.
Reject ambiguous/duplicate policy declarations rather than silently dropping them. LD references
resolved ownership policies; these references must be acyclic and within this set. Policy hashes
do not include temporary paths, display labels or proof run durations.

The policy set does not generate actions. Resolved Core action hashes therefore create no
dependency cycle between policy identity and compiled Core behavior.

## 3. Complete module inventory

### ModuleData

| Field | Meaning |
|-------|---------|
| format / analysis_version | Versioned finite-data contract. |
| behavior_hash / core_release | Exact admitted module and immutable Core dependency identity. |
| declarations | Resolved types/field metadata needed for all operation footprints and policies. |
| policy_set | Complete normalized policies and their identity. |
| actions | Every admitted action, including handwritten/generated and property-irrelevant actions. |
| probes | Descriptors and Core-produced hashes for canonical expected expressions and State-role anchors. |
| inventory_hash | Tagged canonical identity of the complete finite inventory payload, excluding its own identity field. |

### ActionData

Contains action identity, typed parameter/binding references, complete mandatory precondition
roots and all assignment/create/remove operations. Keep Core fragment identities and positions
sufficient to compare this record independently with the admitted module. Do not omit actions
because Rust classified them irrelevant.

### Operation

| Kind | Data | Permission / dependency treatment |
|------|------|-----------------------------------|
| Assign | Action-local occurrence, target binding/type/field, RHS hash and supported literal descriptor | Always requires writer authority when protected, including no-ops. Relevant lifecycle fields consume one operation each. |
| Create | Occurrence, entity, fresh-ID expression, complete field initializer hashes/descriptors | Creator permission; one lifecycle operation including all initialization; each affected policy still checked. |
| Remove | Occurrence, target State binding/type | Remover permission; one lifecycle operation; Core result validity still required. |

Lean derives relevance/irrelevance from these operations and configured footprints. A shared
parent assignment is counted once across dependencies; two distinct assignments are two.
An operation with both parent/child roles is Unsupported under the selected pattern even if its
primitive count is one. FieldOwnership-only operations do not consume the LD count.

Unknown/unrepresented state mutation forms block certification. Additional effects are allowed
only with a checked frame argument for every dependency and all applicable ownership checks.
Read-only guards and command intents cannot substitute for or grant state-write authority.

## 4. Mathematical state and transition data

The formal state maps typed finite identities to the lifecycle/owner projections needed by
the property, with explicit membership and frame semantics. The definitions must retain enough
structure to prove effects preserve unchanged projections; they must not simply assume Q(S').

- R(S): every existing child has an existing referenced parent, terminal children included.
- P(S): every existing nonterminal child's parent has a lifecycle outside its prohibited set.
- Q(S): conjunction of R and P for every configured dependency.
- FO authority: each protected declared assignment/create/remove has the matching explicit
  action permission; abstract state changes have witnesses in those covered operations.

Abstract accepted transitions include defined effect execution, freshness, atomic resulting-state
Core-valid Ref integrity, canonical source guards, literal destinations and frame behavior.
The Core-to-abstract correspondence is a named trusted premise, not an axiom added to Lean to
assert that an arbitrary real Core action preserves Q.

The checked concrete proposition refers to this ModuleData and the exact policy set. Concrete
checking proves finite premises; generic theorems quantify over arbitrary finite states.

## 5. Analysis, proof evidence and certificate

### UncertifiedSafetyAnalysis

Produced after final admission and structural checks. Contains resolved policies, full inventory,
supported/unsupported obligation diagnostics and explicit lowered/uncertified state. It is not
a certificate and does not acquire one by serialization or hashing.

### ProofEvidence

Contains versioned theorem/predicate contract identity; generic and concrete theorem names/types;
exact encoded-data identity; proof source/import identities; checker/extractor/encoder/probe/auditor
identities; locked Lean/tool derivation identity; structured axiom sets and fresh replay results.
Time/resource/path logs live outside canonical semantic evidence.

### SafetyAssessment / SafetyCertificate

Assessment outcome is Certified under assumptions, Inconclusive, Unsupported or Failed. Only a
successful verified assessment can construct the certificate wrapper. Imported JSON is unverified
evidence until binding, inventory, theorem-type, axiom and replay checks succeed.

Certificates bind exact BehaviorHash, complete policy set, inventory and proof evidence. They
list correspondence assumptions and exclude genesis, migration, alternative-module execution
and physical host behavior. Initial validity is unestablished by a module certificate itself.

## 6. Exact-history state validation

### StateValidationRecord

| Field | Meaning |
|-------|---------|
| format / record_hash | Separate versioned canonical record and tagged identity. |
| behavior_hash / policy_set_hash | Exact incoming module and complete checked property set. |
| schema / state | Exact Core schema identity and StateId from the export. |
| source_history | Full HistoryRef, including Store identity, position, state and history-record anchor. |
| property_results | Original R/P results for every dependency, deterministic violations if any. |
| validity | Valid / Invalid / Unestablished, separate from module proof outcome. |
| provenance / assumptions | Acquisition through public export, backend completeness and validator correspondence. |

Use the complete live snapshot, not supplied subsets. Check typed IDs, enum membership,
duplicate/missing identities and exact metadata binding. A malformed/untrusted detached export
cannot establish an actual Store's validity. Validation at position k states only Q(S_k).

## 7. Deployment segment (documented host contract)

The host records one certificate identity, exact BehaviorHash, complete policy-set identity and
valid starting HistoryRef. All commits in the segment must use that admitted module under the
stated Core/host contract. This record is not a new Core authorization format.

Changing module/policies requires incoming-property validation at an enforced boundary. An
atomic or equivalent protocol prevents commits between validation and activation from invalidating
the anchor. StateId equality alone does not prove exact history continuity. Migration is a
separate boundary; a new valid anchor makes no claim about preceding history.
