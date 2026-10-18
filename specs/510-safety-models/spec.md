# Feature Specification: Safety Models — FieldOwnership and LifecycleDependency

**Feature Branch**: `510-safety-models`

**Created**: 2026-10-17

**Status**: Draft

**Input**: User description: "Feature 510 — Safety Models. Introduce FieldOwnership and
LifecycleDependency as reusable, deterministic Model-layer restrictions over existing domain
entities. Establish machine-checked generic safety theorems and check the exact compiled
module against their premises. Preserve Core semantics and existing Model identities. Passing
SMT guards or scenarios alone must never become a claim of universal system safety. Implement
FieldOwnership and its proof/correspondence milestone before LifecycleDependency."

**Repository**: `kallistoteles/behavior-ir`.

**Baseline**: Published Behavior 0.13.0 (`v0.13.0`, merged commit
`63f916cbd9d999270f476e33267200e0aacd8696`), exact Core 0.15.0 commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier 0.10.0.

**Dependencies**: Existing Model authority, composition/ownership, declaration identity,
receipts and compatibility rules (501–505, 507); existing 506/507 conformance and release 509.
Core 0.15.0 remains pinned throughout this feature. Missing public semantic information is a
feasibility blocker to report precisely, never permission to use private Core implementation.

## Clarifications

### Session 2026-10-17 — Grilling decisions Q1–Q17

The user confirmed the following contract and authorized implementation planning. These
decisions supersede the optional v1 choices in the original proposal.

| Decision | Agreed contract |
|----------|-----------------|
| Q1 | Reject every unauthorized assignment structurally, including unreachable and same-value assignments. |
| Q2 | Creator, writer and remover permissions are independent. Creation permits initialization, subject to every applicable creator and safety restriction. |
| Q3 | Require a nonoptional `Ref[Parent]` in the initial dependency pattern. Terminal children still require an existing parent; a prohibited lifecycle value is distinct from physical absence. |
| Q4 | The owner relationship is immutable throughout the child's lifetime. Reject every subsequent owner assignment, including same-value writes. |
| Q5 | Use separate, versioned, content-addressed Safety Evidence. Preserve existing Model receipt formats. Evidence identity is not proof validity. |
| Q6 | Use Lean 4.30.0 from the existing Nix lock; verify the actual selected derivation, not merely a version banner. No additional toolchain manager. |
| Q7 | Machine-check generic theorems and each concrete extracted module instantiation. Extraction completeness/correspondence remains a named trusted boundary until independently verified. |
| Q8 | Validate authored and existing Model-generated actions. Generate no new lifecycle capabilities and silently rewrite no actions. |
| Q9 | Permit at most one relevant lifecycle operation per action. Mechanically establish relevance and irrelevance from the complete module and configured properties. |
| Q10 | Permit only transitive logical axioms `propext`, `Classical.choice`, `Quot.sound`; structurally audit generic and concrete theorem dependencies and freshly replay proofs through the kernel. Reject holes, custom axioms and native-oracle assumptions. Version and review the actual predicates/statements. |
| Q11 | Support literal enum lifecycle values and documented canonical mandatory guards, checked for placement, bindings and effect correspondence. Unrecognized equivalent expressions are Unsupported. Demonstrate public-Core feasibility before dependent implementation. |
| Q12 | Supply a reusable Rust validator using public history-bound `SnapshotExport` data. Record exact history, StateId, schema, module and complete policy set separately from module preservation evidence. |
| Q13 | Include isolated parent creation, child removal and unreferenced-parent removal, subject to ownership, the theorem and Core's result-state Ref integrity. Combined relevant operations are Unsupported. |
| Q14 | Lowering and certification are distinct. Structural ownership violations remain build errors; successful lowering is uncertified until all required proof checks succeed. No Lean dependency during Store execution or ordinary existing-Model builds. |
| Q15 | Count actual operations once across the union of dependency footprints; discharge every affected dependency. Creation includes initialization; distinct assignments count separately. FieldOwnership-only writes require authorization but do not consume the dependency limit. A single operation affecting parent and child roles is not an exemption. |
| Q16 | A deployment segment fixes exact BehaviorHash and complete Safety Policy set. A switch requires validation under the incoming policy at the exact enforced history boundary, using atomic activation, exact-head comparison or an equivalent protocol. No intervening commit may invalidate the anchor. |
| Q17 | The VM/Resource fixture uses only the explicit predicates below. It is a restricted reference model, not a prescribed application schema. Pool ownership and other production arrangements are not prohibited by Behavior IR; unsupported safety configurations must be reported. |

For the selected Lean version, the bundled fresh replay executable is `leanchecker --fresh`.
It rechecks declarations with Lean's kernel; it is not an independent kernel implementation.
Fresh replay and the transitive axiom allowlist are separate mandatory controls.

The VM fixture requires immutable `Ref[VM]` ownership, an existing VM for terminal and
nonterminal Resources, and a VM outside Removed/Abandoned for every nonterminal Resource.
I-5b/I-6 remain explicitly unmapped until authoritative definitions and an equivalence or
implication analysis are supplied. `live` and `nonterminal` MUST NOT be assumed equivalent.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Protect state with auditable ownership (Priority: P1)

As a domain author, I declare which actions may write selected fields, create protected
entities or remove them. I can compose that policy with existing Models and know that every
action of the final module was checked, including generated actions and additional bindings.

**Why this priority**: Complete mutation coverage is the smallest useful safety pattern and
the foundation needed before claiming relational lifecycle preservation.

**Independent Test**: An ordinary protected field and a policy composed with StateMachine
accept authorized operations and reject all required unauthorized-operation controls. A
machine-checked ownership theorem and explicit correspondence evidence accompany the result.
No LifecycleDependency implementation is needed to demonstrate this milestone.

**Acceptance Scenarios**:

1. **Given** a field policy with explicit writer, creator and remover sets, **When** a final
   module passes explicit certification, **Then** conditional ownership certification
   identifies every checked action and each protected operation.
2. **Given** an unauthorized writer through a renamed parameter, a second binding of the same
   entity type, or another generated Model, **When** the module is checked, **Then**
   certification is rejected and the action and protected state are identified deterministically.
3. **Given** permission to write a field, **When** that action also creates or removes a
   protected entity without the corresponding lifecycle permission, **Then** certification
   is rejected; write permission grants no implicit lifecycle permission.
4. **Given** existing Model ownership and additional Safety policies, **When** their allowed
   sets overlap or disagree, **Then** every restriction applies; one policy cannot enlarge
   the authority granted by another.
5. **Given** an omitted mutation/action or weakened ownership premise, **When** the checker or
   proof is exercised by a deliberate mutation, **Then** the appropriate check fails; passing
   unrelated SMT checks cannot restore certification.

---

### User Story 2 - Preserve a parent/child lifecycle relationship (Priority: P1)

As a domain author, I describe existing parent and child types, their typed ownership link,
prohibited parent states and terminal child states. A supported lifecycle pattern prevents a
parent from becoming prohibited while it has nonterminal dependents, and prevents a child
from establishing or resuming an unsafe dependency.

**Why this priority**: This is the first relational guarantee justified by complete control
of state changes rather than requiring general relational operations in Core.

**Independent Test**: After the ownership milestone, a domain-neutral parent/child example
certifies under its stated premises. Actual Core transitions and independent property checks
show safe transitions succeed and unsafe or unsupported shapes cannot obtain certification.
Multiple dependencies sharing a parent can be demonstrated without the VM example.

**Acceptance Scenarios**:

1. **Given** a valid starting state with a nonterminal child, **When** an action would move its
   parent into a prohibited state, **Then** an accepted supported pattern makes that
   transition refuse under ordinary Core semantics, or the module fails certification.
2. **Given** a prohibited parent, **When** an action creates or reactivates a nonterminal
   child, **Then** the same safety condition is enforced; merely leaving ownership unchanged
   does not make reactivation safe.
3. **Given** a supported parent prohibition transition with all dependent children terminal,
   **When** its other declared conditions hold, **Then** safety analysis permits the module
   and adds no unjustified refusal to the supported lifecycle pattern.
4. **Given** a source-state guard that appears sufficient, **When** the same action also
   creates a nonterminal child or changes a relevant field so the child state is unsafe,
   **Then** certification fails or explicitly reports the shape unsupported.
5. **Given** two child types depending on one parent lifecycle, **When** either dependency
   has a nonterminal child, **Then** both policies compose and the parent transition cannot
   bypass the outstanding dependency.
6. **Given** an attempted ownership reassignment or an effect shape outside the declared v1
   pattern, **When** checked, **Then** the unsupported requirement is reported precisely;
   the author receives no weakened substitute for the requested guarantee.

---

### User Story 3 - Inspect what a safety certificate establishes (Priority: P1)

As a reviewer or application maintainer, I can distinguish a generic mathematical theorem,
a checked concrete instantiation and trusted implementation/deployment boundaries. I can
reproduce the evidence and see whether its assumptions hold for my Store and deployment.

**Why this priority**: A certificate that conceals extraction assumptions or unchecked genesis,
migration or alternative-module execution would misrepresent the guarantee.

**Independent Test**: Inspect reports for a supported module, an unsupported shape, an
inconclusive obligation and a failed premise. Reproduce semantic evidence from the same inputs
and confirm a policy change cannot reuse the old certification even if Core behavior is equal.

**Acceptance Scenarios**:

1. **Given** a supported module with every required proof/check obligation discharged,
   **When** evidence is produced, **Then** its status is "Certified under assumptions" and
   it identifies exact module/policy/theorem/tool identities, complete action coverage,
   obligations, assumptions and exclusions.
2. **Given** a generic theorem has been checked but extraction correspondence still depends
   on trusted code, **When** a report is read, **Then** that assumption is explicit and the
   report makes no end-to-end formal-verification claim.
3. **Given** an inconclusive, unsupported or failed required obligation, **When** certification
   is requested, **Then** its actual status and reason are retained and certification blocks.
4. **Given** identical Core behavior with different Safety policies, **When** evidence is
   compared, **Then** the policy/proof identities distinguish the results; a BehaviorHash
   alone cannot authenticate the safety policy.
5. **Given** reordered declarations or incidental labels that leave semantic inputs and Core
   identities unchanged, **When** evidence is regenerated, **Then** semantic identities and
   canonical safety evidence are identical; diagnostics cannot create semantic differences.

---

### User Story 4 - Establish the deployment's safety boundaries (Priority: P1)

As an application maintainer, I can establish initial validity against a complete state and
understand the additional controls needed when a migration or another Behavior module can
change the same Store. I can tell a conditional preservation claim from established safety
of a particular execution history.

**Why this priority**: Ordinary-transition preservation establishes reachable-state safety
only with a valid base state and complete control of the transitions actually committed.

**Independent Test**: Check valid, invalid and incomplete initial snapshots; a migration that
preserves Core validity but violates the declared relation; a same-schema alternative module;
and two conflicting candidates followed by re-evaluation. Reports and host guidance must
identify the exact boundary and required control in every case.

**Acceptance Scenarios**:

1. **Given** the state validator and a complete history-bound Core export, **When** its premise is
   recorded as established, **Then** it is bound to the exact complete, consistent state and
   checks the original property and relationship validity, including missing parent identities.
2. **Given** no initial-state evidence, **When** preservation certifies conditionally, **Then**
   initial validity is explicitly unestablished and no safety claim is made for a concrete
   Store history. A known invalid base state cannot be hidden as an unchecked assumption.
3. **Given** a migration or execution under another same-schema module, **When** a maintainer
   inspects the certificate, **Then** ordinary-transition coverage is not silently extended;
   separate validation/preservation and approved-module enforcement are required.
4. **Given** conflicting allowed candidates, **When** one commits and the other is retried,
   **Then** Core's commit conflict and re-evaluation rules apply; safety evidence grants no
   authority to force a stale candidate or bypass deployment controls.
5. **Given** a module or complete-policy-set switch, **When** a new deployment segment starts,
   **Then** its incoming property is validated at the exact enforced Store history boundary;
   an intervening commit invalidating that anchor prevents activation.

### Edge Cases

- Core rejects two State bindings resolving to the same typed identity. Exercise that refusal;
  still analyze every binding independently of its name. Input identities and query candidates
  can overlap bound entities and require the appropriate correspondence analysis.
- Display-name changes, duplicate semantic action identities, duplicate declarations and
  ambiguous policy references must not grant accidental authority; unsupported ambiguity blocks.
- Overlapping policies, empty allowed sets and an action authorized by one policy but forbidden
  by another must yield the intersection of restrictions.
- Creation initializes protected fields, removal destroys protected state, and a no-op-looking
  write still requires sound treatment under the declared ownership contract.
- A generated action can touch protected state through any of its bindings; a policy cannot
  rely on the originating Model's name as an exemption.
- A child may become nonterminal without changing its owner, or become terminal while a parent
  also changes. Every simultaneous effect must be justified or rejected as unsupported.
- A missing/nullable/wrongly typed owner and an absent parent must not make the safety property
  pass vacuously. Require a nonoptional Ref marker as well as the normalized parent ID type.
  Empty dependent sets must be handled according to the formal pattern.
- A parent prohibited-state transition and actual parent removal are distinct operations.
  Parent removal, child removal and surviving links require their own preservation treatment;
  unsupported removal shapes block certification.
- A guard can refer to one parent while an effect binds another; a matching-looking expression
  is insufficient without the required binding and semantic correspondence.
- Derived expressions, guards that can fail evaluation, unknown admitted effect forms and
  potentially aliasing mixed operations require sound coverage or an unsupported result.
- Multiple child types and multiple instances sharing lifecycle fields must preserve every
  dependency; support for one instance cannot mask another.
- A complete snapshot that is invalid differs from an incomplete or inconsistent snapshot;
  neither establishes initial validity.
- Changes to policy, compiled behavior, theorem, checker or proof dependencies must prevent
  stale evidence reuse. A required action omitted from the inventory must block certification.
- A target-valid migration or an authorization for one BehaviorHash is insufficient evidence
  that all commits use only certified modules or preserve the Model-layer relation. A state
  validated before an intervening commit is not an activation anchor for a new deployment segment.

## Requirements *(mandatory)*

### Functional Requirements

#### Scope, authority and compatibility

- **FR-001**: Provide FieldOwnership v1 and LifecycleDependency v1 in the existing Model
  architecture, over existing domain declarations. Do not require universal Parent, Child or
  Relationship entity abstractions.
- **FR-002**: Retain `crates/behavior-models` as the Rust authority for meaning, validation,
  normalization, composition, ownership and identity. Python MUST remain an authoring binding;
  evaluation, persistence and replay remain Core's semantics.
- **FR-003**: Use the existing public `behavior-engine` semantic interface and exact Core
  0.15.0 pin. Introduce no Core primitive, Core modification, private Core dependency or copied
  Core implementation. If public information is insufficient for complete analysis, identify
  exactly what is missing and block the affected capability; do not approximate it.
- **FR-004**: Check the complete final admitted module after resolving/lowering all behavioral
  and Safety Models. Bind evidence to that exact module; subsequent changes require rechecking.
- **FR-005**: Preserve StateMachine, RequestFlow, Approval and WorkQueue behavior and all
  established Model identities/reference mappings. Additional restrictions compose with their
  existing ownership checks and MUST NOT provide exemptions or broaden permissions.
- **FR-006**: Keep Safety policy and proof identities distinct from Core BehaviorHash. Follow
  existing ModelHash, semantic/lowerer-version and compatibility-fixture rules for new Model
  kinds; preserve established fixtures, receipt formats and immutability rules. Identify the
  complete canonical Safety Policy set as well as each individual policy.
- **FR-007**: Safety Models v1 validate authored and existing Model-generated capabilities
  within the supported pattern. They MUST NOT generate a new lifecycle capability family,
  silently rewrite actions, add hidden runtime behavior or reinterpret admitted Core operations.

**State-transparency invariant (FR-038):** Safety Models must be state-transparent. They MUST
NOT introduce persistent runtime state, additional event streams, replay dependencies or hidden
execution semantics. All application state transitions must be fully represented by ordinary
Core IR and Core Store history. Safety policies and proof certificates are build-time artifacts
only. Their absence may prevent safety certification, but MUST NOT prevent deterministic Core
replay. This applies equally to FieldOwnership and LifecycleDependency: neither stores a separate
ownership or relationship registry. Reconstructing application state and establishing the safety
guarantee of a historical deployment segment are separate operations; the latter may require
retained certificates and historical policy identities, the former does not.

#### FieldOwnership v1 (FO-001–FO-007)

- **FR-008**: Let an author select existing entity fields and explicitly authorize field
  writers, entity creators and entity removers as separate operation sets. Permission in one
  set MUST NOT implicitly grant permission in another; omitted/empty permissions cannot mean
  unrestricted authority. Creator permission governs initialization; writer permission governs
  assignments to existing entities. Reject unauthorized assignments even if unreachable or
  value-preserving. Creation still satisfies every applicable safety condition.
- **FR-009**: Inspect every admitted action and every relevant state-changing operation by
  resolved entity type, field and action identity, across all bindings and Model origins.
  Parameter/display names cannot be the basis of the ownership guarantee.
- **FR-010**: Account separately for field assignment, creation (including protected field
  initialization) and removal. Multiple restrictions MUST apply conjunctively, including when
  policies protect different fields of the same entity or overlap an existing lifecycle Model.
- **FR-011**: Reject unauthorized operations deterministically, identifying the responsible
  action and protected state. Reject unsupported/ambiguous policy configurations rather than
  weakening authority, including ambiguous selection among action identities.
- **FR-012**: Produce an exact ownership-policy record and complete checked-action inventory,
  with the operation coverage needed to audit both authorized mutations and claimed irrelevance.

#### LifecycleDependency v1 (LD-001–LD-008)

- **FR-013**: Let an author configure existing parent/child types, a typed child-to-parent
  relationship, resolved lifecycle fields, parent prohibited states, child terminal states and
  existing lifecycle Models or controlled transitions. Multiple instances MUST compose, including two child types sharing
  a parent lifecycle.
- **FR-014**: Define the lifecycle property precisely over arbitrary finite states. For each
  existing nonterminal child, its owner MUST resolve to an existing parent outside the prohibited
  states. Require an immutable nonoptional Ref to the parent; relationship validity requires
  parent existence for terminal children too. Reject every post-creation owner assignment,
  including same-value writes. Missing owners/parents cannot pass vacuously.
- **FR-015**: Enumerate every action that can affect relationship validity or lifecycle safety,
  including relevant field writes, creation, removal, reactivation and simultaneous effects.
  Establish exclusive control through FieldOwnership and existing lifecycle restrictions.
- **FR-016**: For supported actions, enforce that a parent enters a prohibited state only when
  all relevant dependent children satisfy the terminality condition. Enforce that child creation
  or reactivation cannot establish a nonterminal dependency on a prohibited or absent parent.
- **FR-017**: Check guard/effect relationships against actual typed Core transition semantics,
  including binding identity, possible aliasing and the complete resulting state. Source-state
  checks MUST NOT be assumed to survive other effects in the same action. Require literal enum
  lifecycle values and canonical guards present as actual mandatory preconditions with matching
  captures, bindings and effects; finding a guard hash somewhere in an expression is insufficient.
  Additional authored conditions may strengthen but never weaken the required guard.
- **FR-018**: Publish the restricted v1 pattern: immutable required Ref ownership; literal enum
  lifecycle values; at most one actual relevant operation per action across all dependency
  footprints. Creation counts once including initialization; distinct assignments count separately.
  Check every affected dependency. FieldOwnership-only writes do not consume this limit but
  remain authorized independently. Reject mixed parent/child roles even if represented as one
  operation. Support safe child creation/reactivation, parent lifecycle transitions, isolated
  parent creation, child removal and unreferenced-parent removal under Core's Ref integrity.
  Reject reassignment, optional links, combined relevant operations and unsupported guard/value
  forms precisely; do not obtain acceptance by denying every meaningful lifecycle operation.
- **FR-019**: Reject an unsafe instance or return Unsupported/Inconclusive for an unestablished
  shape. Produce property-specific evidence naming relevant actions, guard/effect relationships
  and every preservation obligation. Do not expand the supported language solely to fit examples.

#### Mathematical contract and machine-checked proofs

- **FR-020**: For each pattern, state the property, base-state premise and complete covered
  transition relation. Establish initial validity and preservation as distinct obligations;
  the inductive guarantee covers only histories satisfying both and the declared boundaries.
- **FR-021**: Machine-check at least **FO-1 (ownership completeness)** and **LD-1 (lifecycle
  preservation)** independently of the VM domain and for arbitrary finite states/cardinalities.
  Define the permitted operation patterns and their semantics sufficiently to prove these
  results; assuming the desired preservation conclusion as a premise is insufficient.
- **FR-022**: Use Lean 4.30.0 through the existing Nix lock, verifying the actual tool derivation.
  Audit transitive axiom dependencies structurally for every generic and concrete certified
  theorem; allow only `propext`, `Classical.choice`, `Quot.sound`. Require fresh kernel replay
  using the pinned `leanchecker --fresh`. Reject holes, custom axioms and native-oracle
  assumptions. Version/review predicate definitions and theorem statements, not just proof files.
  State the kernel/logical basis/imported dependencies; add no separate toolchain manager.
- **FR-023**: Establish concrete instantiation against the admitted typed module: derive the
  complete relevant mutation set, check ownership, establish required guards and effect
  relationships, and handle or reject simultaneous operations. A syntactic match is allowed
  only for forms whose required semantic correspondence is established; arbitrary equivalent
  expressions need not be supported in v1.
- **FR-024**: Specify correspondence across the admitted Core representation, the action/effect
  extractor, the theorem's abstract transition representation and reported evidence. Lean MUST
  check each concrete module instantiation, bound to exact BehaviorHash, complete policy set and
  full action inventory. Extraction completeness remains explicitly trusted until independently
  verified; a valid proof over an incomplete inventory is not certification. Identify every
  remaining unverified correspondence dependency; do not claim end-to-end formal verification.
- **FR-025**: Passing SMT checks, structural checks or finite scenarios MUST NOT substitute for
  the two generic theorems or missing concrete premises. Failed, unsupported and inconclusive
  required obligations MUST block certification; proof availability alone cannot certify an
  unchecked module.

#### Evidence and deployment boundaries

- **FR-026**: Produce deterministic, machine-readable safety evidence bound to exact
  BehaviorHash, canonical Safety configuration/identity, Model/lowerer and checker identity,
  theorem/proof-tool and proof-dependency versions, complete checked-action identities,
  obligation results, explicit assumptions and exclusions. Reproduce it from the exact module,
  source and supported dependency versions; retain established diagnostic/semantic distinctions.
  Use a separate versioned content-addressed artifact and preserve existing receipt formats.
  Hash identity never substitutes for proof validity or discharges a documented assumption.
- **FR-027**: Distinguish Certified under assumptions, Inconclusive, Unsupported and Failed.
  Certification requires all mandatory generic and concrete proof/check obligations discharged.
  Conditional external boundaries MUST be named precisely; a known failed premise cannot be
  hidden as an unestablished assumption or relabeled as certification.
- **FR-028**: Separately report whether initial validity has actually been established. When
  Core cannot express the relation directly, provide a reusable Rust validator using public
  `Store::export_seed_at`/`SnapshotExport`. Check the complete consistent state against the
  original property plus relationship validity. A separate validation record identifies exact
  Store/history anchor, StateId, schema, module and complete Safety Policy set. Retain backend
  completeness/provenance assumptions; partial snapshots and weaker approximations do not qualify.
- **FR-029**: Define ordinary-transition coverage to exclude genesis import and migrations.
  Extending a deployment claim across a migration requires separately checked preservation or
  validation of the complete resulting state. Core target validity alone MUST NOT extend it;
  implementing a general migration certifier is not required in v1.
- **FR-030**: Identify the approved-module boundary for Store commits. Investigate whether
  existing Core governance enforces an approved BehaviorHash set or only binds individual
  authorizations to a hash, and document the exact remaining host responsibility. Same-schema
  alternative modules MUST NOT inherit certification. Introduce no new governance mechanism
  without separate explicit approval. Define each deployment segment by one exact BehaviorHash
  and complete Safety Policy set. Switching either requires validation under the incoming policy
  at the exact enforced history boundary. Atomic activation, exact-head comparison or an equivalent
  host protocol MUST prevent intervening commits from invalidating the anchor. A new segment
  proves nothing about earlier states; independently certified modules alone do not justify a switch.
- **FR-031**: Safety evidence MUST NOT automatically act as Core governance authorization,
  authorize a Store commit, attest unchecked initial states/migrations, or claim physical host
  safety, scheduling, liveness or eventual cleanup.

#### Required acceptance and regression evidence

- **FR-032**: Require structural/mutation controls for unauthorized writers, renamed/aliased
  bindings, creation/removal, another Model's generated writer, missing/weakened lifecycle
  guards and a missing relevant action. Removing each essential abstract or concrete premise
  MUST make the corresponding proof/premise check fail appropriately.
- **FR-033**: Require actual-transition controls for child creation against a prohibited parent,
  reactivation without owner changes, unsafe parent removal, same-action parent prohibition and
  nonterminal child creation, source guards invalidated by simultaneous effects, and conflicting
  Store commits followed by re-evaluation. A restricted shape may be rejected as Unsupported;
  it MUST NOT receive certification or a misleading runtime-safety claim.
- **FR-034**: Exhaustively enumerate an explicitly documented bounded set of small typed states
  and supported action configurations. Independently check the original property before/after
  actual Core transitions and compare certification decisions. Vary identifiers, parameter
  names, declaration order and Model composition. State the enumeration bounds; these tests
  provide correspondence confidence, not the arbitrary-finite-state theorem.
- **FR-035**: Provide four domain-neutral acceptance examples: ordinary field ownership,
  ownership with StateMachine, a parent/child lifecycle dependency, and multiple dependencies
  sharing a parent. Include the supplied additional VM/Resource case: no VM in Removed or
  Abandoned may have a nonterminal owned resource. Record its precise predicates and map the I-5b/I-6 requirements
  only when authoritative definitions and an equivalence/implication analysis are available.
  The reference fixture has immutable required Ref[VM] ownership and existing parents even for
  terminal Resources. This is not a prescribed application schema: pool-owned resources and
  other production ownership arrangements need not adopt this fixture's restrictions. Report
  unsupported safety configurations precisely. Keep I-5b/I-6 unmapped and never equate `live`
  with `nonterminal` without justification. The generic theorem and first milestone MUST NOT
  rely on this example, an external application or VM-specific axioms.
- **FR-036**: Preserve the normal ecosystem gates, Core pin, established Model compatibility
  mappings and 506/507 conformance. Add proof/premise checks to the existing gates with the
  pinned proof prerequisites; missing or unsuccessful required proof checking blocks acceptance.
  Tests for checker changes follow the constitution's test-first rule.
- **FR-037**: Document public authoring, precise supported/excluded shapes, evidence statuses,
  trust/correspondence boundaries and host obligations with executable public examples. Record
  unsupported domain requirements precisely, without moving transition safety into hidden host
  rules or silently approximating them. Separate lowering from explicit build-time certification:
  ownership violations remain build errors; lowered modules awaiting or failing proof checks are
  uncertified. Certification requires Lean, fresh replay and every required obligation. Store
  execution and existing Models without Safety policies MUST NOT acquire a Lean dependency.

### Formal Scope and Guarantee

State-safety claims use induction over the complete covered ordinary transition relation T.
For LifecycleDependency, let R be the explicitly defined relationship-validity property and P
the lifecycle implication below. The induction must establish their conjunction Q = R and P:

- **Base**: Q(S0).
- **Preservation**: for every covered action a and arbitrary finite states S and S',
  Q(S) and T_a(S,S') imply Q(S'). Relationship validity must be preserved as well as lifecycle
  safety, or supplied by a separately justified semantic guarantee; it cannot disappear between
  induction steps.
- **Reachability**: every state reached from an established valid base by covered transitions
  satisfies Q, subject to the explicitly listed correspondence/deployment assumptions.

FO-1 establishes that protected field changes, creation and removal in each covered transition
have the corresponding policy authority, given complete sound mutation coverage. This structural
transition property is a premise for the state-safety induction. LD-1 establishes preservation
for a concrete restricted operation pattern. For every existing child, the intended implication P is:

`not terminal(child) => exists(owner(child)) and not prohibited(owner(child))`.

Relationship validity additionally requires every child's owner to exist, including terminal
children. Required Ref semantics provide existence under Core's complete result-state checks;
Safety policies separately forbid owner assignment after creation. Parent removal with surviving
references is refused by Core, independent of lifecycle values. Canonical supported patterns and
their public-Core correspondence require a feasibility gate before implementation. The result
is conditional certification of the declared
transition system, not unconditional safety of every module, deployment or physical system.

### Delivery Milestones and Planning Decisions

1. **FieldOwnership first**: define the structural pattern, machine-check FO-1, implement the
   minimal reusable coverage/policy analysis and its concrete correspondence controls, and
   demonstrate existing-Model composition. Pass positive, negative and premise-mutation evidence
   before implementing LifecycleDependency.
2. **LifecycleDependency second**: define and machine-check LD-1, then check the minimal supported
   relationship/guard/effect pattern. Both supported certification and deliberate unsafe/unsupported
   controls must pass. Do not broaden the expression language for extra examples.
3. **Evidence and integration**: complete reproducible reporting, versioned new-Model identity,
   existing gate integration and public guidance. Both patterns and all evidence boundaries are
   required for Feature 510 completion; the first milestone alone is not the complete feature.

Planning must implement the settled Q1–Q17 contract: Lean 4.30.0 plus fresh kernel replay and
axiom audits; per-module concrete proofs with explicit trusted extraction; conjunctive ownership;
validation of existing actions; separate versioned evidence; required immutable Ref ownership;
literal lifecycle values; and the single-relevant-operation profile. Select exact public API,
canonical guard construction, proof-data encoding and exhaustive-test bounds within that contract.
Any proposal for new governance needs separate approval. Public-Core API insufficiency must be
reported before dependent implementation proceeds; do not silently change the Core pin or trust
boundary. The formal tool/dependency choice must receive the constitution's normal justification.

### Non-goals

Core changes; general joins/grouping/recursion/bulk effects; arbitrary relational Safety
predicates; temporal liveness or eventual cleanup; physical execution guarantees; automatic
certification of unchecked genesis or migrations; replacement governance; a general Safety
framework designed ahead of these two patterns; external production-application integration.
E3 scoped cardinality and E4 cleanup obligations may inform later patterns and are not acceptance
requirements for this feature.

### Key Entities

- **Safety policy / Model**: canonical configuration of protected declarations, permitted
  operations or lifecycle dependencies, with an identity independent of Core behavior.
- **Protected operation**: a resolved assignment, creation or removal affecting protected state,
  with its responsible admitted action and binding relationships.
- **Action inventory**: the complete admitted action set and an auditable classification of
  relevant mutations, including justified irrelevance and unsupported forms.
- **Lifecycle dependency**: configured parent/child declarations, owner relationship, terminal
  and prohibited states, and the supported operation pattern that preserves their relation.
- **Generic theorem**: a machine-checked ownership or lifecycle statement over the declared
  abstract transition semantics and explicit premises, with pinned proof identity/dependencies.
- **Concrete instantiation**: the checked correspondence of one exact compiled module and policy
  to that theorem's premises, with trusted implementation boundaries identified.
- **Safety report / certificate**: reproducible evidence and status bound to module, policy,
  action inventory, obligations, theorem/tool versions, assumptions and exclusions.
- **Boundary evidence**: established/unestablished initial validity and separate migration/module
  authorization responsibilities; it cannot be inferred from ordinary-transition certification.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Both generic theorems and every certified concrete instantiation pass Lean 4.30.0,
  fresh kernel replay and the transitive axiom allowlist. No proof holes, custom axioms or native
  oracles are accepted. Removing each essential premise fails the appropriate proof obligation.
  Domain examples and finite enumeration are not the arbitrary-finite-state proof.
- **SC-002**: Every certified module accounts for 100% of admitted actions and every relevant
  mutation; all specified inventory, alias, unauthorized-operation and weakened-guard controls
  block certification. A theorem without a checked concrete instantiation certifies no module.
- **SC-003**: All four domain-neutral examples demonstrate supported use, including a permitted
  parent prohibition transition and safe child creation/reactivation, and the additional
  VM/Resource fixture checks its explicit predicates without claiming unmapped I-5b/I-6. Every required unsafe or excluded shape
  is refused or reported unsupported; no deliberately unsafe case obtains certification.
- **SC-004**: Within documented exhaustive small-state/configuration bounds, no certified action
  commits a transition violating the independent original property. Required supported examples
  certify; unsafe examples do not; safe shapes outside v1 may remain Unsupported. Publish bounds
  and profile expectations separately from the arbitrary-finite-state theorem's scope.
- **SC-005**: Repeating certification with identical semantic inputs produces byte-identical
  canonical evidence, including across nonsemantic ordering/name variations. Changed policy,
  compiled behavior or proof/checker dependencies cannot reuse the old certification identity.
- **SC-006**: Every report distinguishes all four required outcome classes and explicitly lists
  its trust assumptions/exclusions and initial-validity status. Every required non-success
  obligation blocks certification; no tested, structural or conditional result is mislabeled
  as unconditional system safety. Lowered is never confused with Certified. State-validation
  records are bound to exact history; segment activation refuses stale validation anchors under
  the documented host protocol.
- **SC-007**: All existing Model reference identities remain unchanged, the exact baseline Core
  pin is retained, and every mandatory ecosystem/compatibility/506/507 regression check passes.
- **SC-008**: FieldOwnership's theorem, concrete checker, composition and premise-mutation gate
  pass before LifecycleDependency implementation begins. Feature completion additionally requires
  both patterns, deterministic evidence and public guidance covering all deployment boundaries.
- **SC-009**: Histories produced with either Safety Model replay through ordinary Core using
  only Core modules and Store history, without reading Safety declarations, certificates or
  proof tools. State reconstruction and BehaviorHash-bound behavior replay remain deterministic;
  absence of safety evidence affects retrospective safety claims only.

## Assumptions

- Target users are domain authors, safety reviewers and application maintainers familiar with
  the existing Models and Store. Technical terms and mathematics are necessary domain language;
  the architecture/proof-tool constraints above are user requirements, not new implementation
  choices made by this specification.
- Ordinary transitions mean actions of the exact admitted module committed according to Core's
  existing contract. Genesis/import, migrations and alternative-module execution are separate
  boundaries, not silently included in the induction.
- v1 requires an immutable nonoptional Ref owner and rejects reassignment, computed lifecycle
  values, unrecognized guards and combined relevant operations. These explicit restrictions do
  not redefine application requirements or eliminate positive supported lifecycle use.
- Initial-state validity and approved-module enforcement are external premises until established
  by the documented mechanism. A conditional certificate reports their actual evidence status;
  it is not proof that a particular deployment satisfies them.
- I-5b/I-6 are unmapped external references. The explicit restricted fixture is the acceptance
  contract; claims of full external-invariant coverage require authoritative definitions and a
  formal equivalence/implication analysis. Pool-owned production resources need not be reshaped
  to fit the fixture. No equivalence between `live` and `nonterminal` is assumed.
- The proof toolchain, axiom policy, separate evidence, explicit certification and v1 restrictions
  are settled by Q1–Q17. Public API syntax, concrete proof-data representation, artifact schema
  and bounded enumeration design belong to planning. No correspondence assumption is proven
  merely by selecting that design.
- A proof checker, its trusted kernel/dependencies and Core's public semantics are prerequisites.
  Missing public semantic information blocks the affected feature work and is reported precisely.
- Established Model/Core semantics, compatibility fixtures, published tags, records and
  attestations remain immutable. Only new Safety Model identities and justified authoring or
  analysis capabilities belong to Feature 510.
