# Research: Safety Models

**Date**: 2026-10-17. **Baseline**: Behavior 0.13.0 / exact Core 0.15.0.
Read-only repository/source investigation and confirmed decisions Q1–Q17. No tools installed,
proofs compiled, probes executed or runtime tests run during planning.

## 1. Composition and authority

**Decision**: Keep Safety policies separate from generating Models. Add a Rust safety-aware
composition entry point sharing the existing lowering path; preserve the old entry point and
BuildOutput/receipt representation. Run Safety analysis on the exact final admitted module
after existing ownership checks. Python adds a `safety=` input and delegates to Rust.

**Rationale**: Existing admission, ownership and receipt generation are already correctly
ordered. The generating-owner table enforces a single owner; putting intersecting Safety
restrictions there would conflate different contracts. Existing Python receipt decoding also
rejects unknown fields, so extending old receipt JSON would break consumers.

**Alternatives considered**: a new crate/framework; inserting Safety into generating ownership;
adding fields to old receipts; a second Python semantic checker. All add unnecessary authority
or compatibility risks.

Sources: `crates/behavior-models/src/kinds.rs:532–556`, `composition.rs:610–695`,
`source_map.rs:174–184`, `python/behavior/models.py:1–5,80–93`.

## 2. Complete mutation extraction and FieldOwnership

**Decision**: Extract every action, parameter, assignment, creation and removal through public
admitted-module accessors. Resolve assignment/removal targets through typed parameter bindings;
resolve creations by entity declaration. Check explicit creator/writer/remover sets independently.
Reject unauthorized assignments without evaluating reachability or RHS equivalence.

**Rationale**: The facade-returned semantic objects expose the required inventory, and existing
StateMachine/Approval checks already inspect every final action. Creation carries its complete
initializer separately from ordinary assignments. Field identity is the resolved entity
declaration plus declared field key; no new Core field-identity primitive is needed.

**Alternatives considered**: parameter-name authorization, trusting generating Model origin,
examining only writes listed by the author, treating initialization as ordinary assignment.
These fail the structural or complete-module contract.

Sources: `crates/behavior-models/src/composition.rs:669–695`, `approval/ownership.rs:10–90`,
`identity.rs:51–68`; [pinned Core semantic module](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/crates/behavior-core/src/semantic/module.rs).

**Correction from source investigation**: Core refuses two State bindings with the same typed
identity (`STATE_ALIAS_NOT_ALLOWED`). Tests must exercise that refusal and renamed/secondary
bindings; they must not invent successful State alias execution. Input IDs and query members
can still overlap bound identities. See pinned Core `docs/verification.md:217–218` and
`specs/012-unified-invocation-model/spec.md:387–390`.

## 3. Public expression gap and chosen canonical profile

**Decision**: Use public Core construction and admitted expected-expression hashes, never
private semantic variants or classification of serialized guard ASTs. Recognize entire required
precondition roots, not arbitrary descendants. Require literal enum lifecycle destinations.

**Rationale**: The public facade does not export ExprKind/QueryKind. `children()` and opaque
hashes alone cannot independently recover expression meaning. The same gap was documented
in `specs/501-model-lowering-state-machine/research.md:38–44`.

The candidate profile uses bound State parents on child-side operations:

- Parent becomes prohibited: a mandatory canonical negated `any` over children whose owner is
  that parent and whose lifecycle is nonterminal, for each dependency.
- Child becomes nonterminal: mandatory owner-equality and permitted-parent-state conditions,
  using an additional bound State parent.
- Child creation: owner initializer exactly that bound parent's ID; a literal lifecycle value;
  permitted-parent-state guard when nonterminal.
- Terminal child updates and permitted parent updates require literal classification; existing
  safety plus the single-operation restriction provides the remaining preservation argument.
- Isolated parent creation, child removal and parent removal have explicit theorem cases using
  freshness, frame semantics and Core result-state reference integrity.

**Probe construction proposal**: Reconstruct declarations in a scratch Builder using public
enum/nominal/entity accessors, preserving normalized Ref markers and comparing declaration
identities. Admit expected guards/literals/owner-ID expressions as derived bodies with the same
captured entity binding names/types. Read their Core-produced expression hashes. The authored
module is never modified or replaced by this probe.

Parameter roles need a separate check: a supplied Context/Input Entity is not trusted Store
state. ParamRole has no facade enum path, but returned values implement equality. Compare
with an admitted public State-role anchor (for updates, the known State assignment target;
for creation, an auxiliary scratch action built with public Role::State). No Debug parsing,
discriminant assumptions or private imports.

**Alternatives considered**: arbitrary SMT equivalence, wire-AST parsing, private imports,
copying Core hashing, generating replacement actions, scalar-ID owner initialization and
additional canonical query families. They are unnecessary or outside the selected v1 surface.

**Feasibility status**: Chosen design, not demonstrated capability. Mandatory preflight must
show declaration identity preservation, probe/action hash agreement, role checks and wrong-root/
binding/owner controls. `Builder::for_module().finish()` drops original declaration lists and
is not suitable for this construction; use a fresh reconstructed declaration context. Fail
precisely if the public route is insufficient. Empty membership sets need explicit canonical
true/false forms, not an assumption that an empty `in` expression admits.

Sources: pinned Core `crates/behavior-engine/src/lib.rs:101–114`,
`crates/behavior-core/src/semantic/module.rs:43–85,144–152,264–343,359–386`,
`builder.rs:264–277,433–577,699–855,1206–1222`;
[public hashing contract](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/specs/001-verifiable-behavior-ir/contracts/hashing.md);
existing `crates/behavior-models/tests/core_assumptions.rs:211–225,267–274`.

## 4. Proof and concrete instantiation

**Decision**: One closed ModuleData per exact admitted module; a small reflective Lean checker;
generic FO/LD soundness; a concrete kernel-reduced proof of `checkModule data = true` and its
derived theorem for every certificate. Lean derives relevance from all typed operations and
policy footprints, rather than trusting Rust's relevance verdicts.

**Rationale**: This separates finite premise checking from arbitrary-finite-state mathematics.
FO defines actual abstract effect/frame semantics. LD proves each restricted transition case
and their conjunction across policies. Neither theorem may assume its desired conclusion.

**Trusted boundary**: Rust faithfully extracts Core operations, bindings and precondition roots;
Core-built probe descriptors actually denote the claimed predicates; Core semantic hash
equality preserves that meaning; the data encoder and Rust snapshot predicate correspond to
their declared contracts. Lean proves the abstract model, not the correctness of those programs.
Independent inventory comparisons and mutation/actual-Core tests increase confidence but do
not remove this boundary. Hashes beside a theorem are insufficient: the concrete record must
occur in its checked proposition, and verification must match it to the actual module.

**Alternatives considered**: generic theorem only; Rust-only concrete checking; generated
per-action proof programs; a full Core interpreter in Lean; end-to-end verification of Rust.
The first two do not meet Q7; the others exceed the two-pattern feature.

Use fixed generated declaration names and validated numeric data/byte literals. Authored names
cannot change Lean imports, commands, notation or theorem statements. Start with bundled Lean
libraries; no Mathlib or external proof dependency is currently justified.

## 5. Toolchain and proof integrity

**Decision**: Add pkgs.lean4 to the existing Nix shell and expose that locked package for tool
identity resolution. Use Lean 4.30.0 and its bundled `leanchecker --fresh`; no lock update,
elan or additional manager. Use a structured `Lean.collectAxioms` audit on generic and concrete
roots with allowlist `propext`, `Classical.choice`, `Quot.sound`.

**Rationale**: The current lock's nixpkgs revision already pins this Lean source/version/hash.
The pinned version includes the fresh checker. Fresh replay accepts axioms, so it does not
replace the transitive audit. Native evaluation can introduce extra axioms; kernel reduction
for finite checks avoids that additional trust. Audit expected theorem types as well as names.

Verify actual selected Nix derivation/executable identities; a banner is insufficient. Build
in a controlled directory/search path from the pinned proof sources, rather than loading
arbitrary caller-provided Lean source or .olean files. Fresh replay uses Lean's kernel, not a
second independent implementation. Toolchain/build environment integrity remains trusted.

**Alternatives considered**: latest/unpinned Lean, banner-only checking, compiling without an
axiom audit, separately pinning the older lean4checker package, native_decide for convenience.
They weaken reproducibility/integrity or add an unnecessary dependency.

Sources:

- [Locked nixpkgs recipe](https://github.com/NixOS/nixpkgs/blob/6774f7bc253789b113a4f39285dc0fa100abeacc/pkgs/by-name/le/lean4/package.nix)
- [Pinned fresh checker](https://github.com/leanprover/lean4/blob/v4.30.0/src/LeanChecker.lean)
- [Pinned structural axiom collector](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Util/CollectAxioms.lean)
- [Pinned replay](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Replay.lean)
- [Pinned kernel/native decision paths](https://github.com/leanprover/lean4/blob/v4.30.0/src/Lean/Elab/Tactic/Decide.lean)
- [Proof-validation trust distinctions](https://lean-lang.org/doc/reference/latest/ValidatingProofs/)

## 6. Ref integrity and exact state validation

**Decision**: Require immutable nonoptional Ref[Parent]. Validate the exact property in Rust
using Core's public complete `SnapshotExport`, acquired at a specified HistoryRef. Bind the
validation record to Store/history, schema, StateId, module and complete policy set.

**Rationale**: Ref is ID plus existence constraint, independent of lifecycle values. Core checks
the whole resulting reference graph, so terminal children still block physical parent removal.
It does not make references immutable. The public export already supplies complete validated
live entities, including source history, schema, StateId and typed seed values. Its public seed
rows contain JSON values, an intended state interface distinct from guard-AST inspection.

Rust checks typed ID/enum values, parent existence for all children and permitted-parent state
for nonterminal children. Store::create establishes seed validity; genesis_v2_for alone does
not. An arbitrary detached JSON object does not inherit export provenance/completeness.

**Alternatives considered**: plain ID, optional/polymorphic owner, a second Store enumeration
mechanism, trusting Ref to enforce immutability, an initial-state statement without a validator.
They weaken the selected pattern or duplicate an existing public API.

Sources: pinned Core `specs/006-entity-lifecycle/spec.md:249–331`,
`specs/013-durable-command-intents/contracts/engine-api.md:35–50`,
`crates/behavior-store/src/documents.rs:123–141,474–480`, `store.rs:91–193,562–595,985–992`;
existing Python `python/behavior/store.py:232`.

## 7. Deployment authorization and switch boundary

**Decision**: No new governance mechanism. Document host enforcement of one certified
BehaviorHash and complete policy set per segment, beginning at an exact validated history.
Switching either requires incoming-property validation and an atomic/exact-head-enforced
activation boundary. A history advance with unchanged StateId still cannot be ignored.

**Rationale**: Core governance binds individual authorizations to BehaviorHash/candidate/Store/
history/context. Its policy allowlist is of execution-policy hashes, not an approved BehaviorHash
set. Trusted authorizers/hosts can enforce eligibility, but a hash-bound authorization alone
does not establish that every future commit uses only certified behavior.

**Alternatives considered**: treating Safety Evidence as governance authority, accepting any
certified module without checking incoming assumptions, checking only StateId, or an unenforced
read-head/check-head/switch sequence. They do not establish continuity at the boundary.

Sources: pinned Core `specs/013-durable-command-intents/contracts/governance.md:49–73,105–132`,
`docs/governance.md:5–17`, `docs/persistence.md:398–401`.

## 8. Identities, compatibility and evidence

**Decision**: Resolve authored action selectors and generated `(model_id, local_action_id)`
selectors after final admission. Hash canonical resolved policies using existing ModelHash
infrastructure with new kind/version documents. Use separate domain tags for policy-set,
inventory, certificate and state-validation identities. Retain old receipt schemas/bytes.

New Safety mappings live in `models/fixtures/safety-compat.json`, checked by the existing
compatibility gate. Existing `compat.json` and builders are not rewritten. Add semantic
determinism to the existing script and proof checks to the existing gate list.

**Rationale**: Safety emits no Core IR, so action-hash policy references have no circularity.
The complete module/inventory is bound in evidence, not hidden in a source selector. A policy
change can change its identity without changing Core behavior. Actual toolchain/proof inputs
also affect certification identity; logs, temporary paths and timings do not.

**Alternatives considered**: selector strings as authority, existing-receipt extension, an
independent second compatibility/release process, evidence identity as proof validity.

Sources: `crates/behavior-models/src/{ids,identity,source_map}.rs`,
`scripts/check-models-compat.py:129–215`, `scripts/gates.sh`, `scripts/determinism-check.sh`,
`.github/workflows/ecosystem-ci.yml:43`, `scripts/release-check.sh:33`.

## 9. Acceptance and coverage

**Decision**: Four neutral cases plus the explicit restricted VM fixture. Use exhaustive small
states and actual Core evaluation/commit for correspondence, with bounds specified in plan.md.
Rejection of safe shapes outside the profile is acceptable; certification of an unsafe supported
transition is not. Require useful accepted transition witnesses to avoid vacuous deny-all success.

The VM fixture does not prescribe mandatory VM ownership for production domains. Pool ownership
may be outside this safety pattern without being forbidden by Behavior IR. I-5b/I-6 remain
unmapped; no equivalence of live/nonterminal or additional domain rule is inferred.

## Research Completion and Evidence Still Required

All product choices are resolved and a concrete public-API design is selected. The following
are mandatory implementation gates, not established results: FO frame/correspondence checks;
canonical-probe/State-role feasibility; practical concrete kernel reduction/replay; exhaustive
Core correspondence; snapshot predicate/provenance controls; unchanged existing compatibility.
Failure blocks affected work and cannot be repaired by broadening the trusted claim silently.
