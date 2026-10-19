# Implementation Plan: Safety Models

**Branch**: `510-safety-models` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature 510 and confirmed grilling decisions Q1–Q17.

## Current Plan — Release Readiness (2026-10-18)

**Scope**: US5–US7, FR-039–FR-046, SC-010–SC-014. The implementation below already exists;
its original design remains recorded after this section. Extend it rather than repeat T001–T085.
The next tasks pass must preserve those tasks and append new work starting at T086.

### Technical decisions

- Keep Rust 1.98.1, Python 3.13, Lean 4.30.0, Core 0.15.0 and verifier 0.10.0. Add no dependency,
  crate, authoring capability or runtime state. Existing proof/Store boundaries remain in force.
- Add a small private `safety/membership_orders.rs` helper. It uses Core's public serializer to
  discover ordered string-array candidates, without interpreting serialized expression operators,
  types, bindings or Boolean structure. Only a reconstructed, Core-admitted **whole mandatory
  root** with the right typed declarations/captures can establish a guard match.
- Preserve candidate order and repetitions; compare its distinct members with the configured
  set. Core membership ignores duplicates at evaluation but hashes their order and multiplicity.
  Keep the existing explicit true/false convention for empty policy sets.
- Share validated match results between `lifecycle_dependency.rs` and `proof_data.rs`.
  Record actual authored roots and ordered member witnesses; Lean checks exact set agreement
  and the existing placement/binding/effect premises. Preserve the FO/LD theorem meanings.
- Split diagnostics using all parent candidates: binding → owner association → parent guard.
  Association and guard must hold for the same candidate. Preserve terminal-update exemptions
  and the distinct terminal-creation binding/association requirements.
- Bump Safety checker/lowerer version to **2**; keep policy semantic versions **1**, Core verifier
  **0.10.0**, statement meaning and existing receipt formats. Source/input identities change and
  stale evidence remains unusable. Add new sidecar tuples; preserve existing mappings.
- Target package **0.13.1**: published 0.13.0 contains no Safety API; this release first adds it.
  Corrections change no previously published Safety outcomes. Record this rationale in versioning
  docs/release notes; recheck published versions before implementation finalization/publication.

**Revised architecture decision**: The earlier blanket serializer prohibition is narrowed only
for non-authoritative order discovery in this helper. Public surface enforcement gets one named
exception plus rejection controls; all other Model wire-analysis restrictions remain enforced.
No serialized expression can justify acceptance by itself. The exact-root check and concrete
proof remain mandatory. This is a design revision, not permission for a Model-side Core parser.

### Constitution Check — Before Research and After Design

| Constraint | Before | After design |
| --- | --- | --- |
| I deterministic decisions | PASS | Fixed traversal, sorted candidate/root selection, no AI path |
| II typed boundaries | PASS | Strict data-only candidates; Core admission establishes types/meaning |
| III test first | PASS | Reproduce C2 and isolated diagnostic failures before fixes |
| IV reproducibility | PASS | Actual identities preserved; diagnostics/budgets separate from semantics |
| V auditability | PASS | Exact roots/member witnesses, structured dependency diagnostics, separate outcomes |
| VI simplicity | PASS | One helper and existing authority, proof runner, examples and release process |
| Public Core / authority | PASS with preflight | Named serializer exception discovers candidates only; no private imports or semantic parser |
| Proof integrity / compatibility | PASS | Existing obligations retained; checker version 2, immutable old mappings |
| State transparency | PASS | No Store/replay changes or runtime proof dependency |

PASS describes the design, not executed validation. No constitution exception is proposed.
The helper's discovery affects availability; trusted Core probe/hash correspondence still
justifies the recognized expression. It does not establish end-to-end compiler verification.

### Implementation Stages

1. **Reproduce and prove feasibility (FR-039–FR-041).** Add minimal C2 and direct/query membership
   controls to `tests/safety_membership.rs` and `tests/safety_correspondence.rs`. Observe the
   ordering failure, then demonstrate candidate discovery plus whole-root reconstruction,
   duplicate handling, wrong-context rejection and resource refusal through public Core only.
   No dependent recognition implementation proceeds if this preflight fails.
2. **Integrate recognition and concrete premises (FR-040, FR-045).** Add the helper; adapt
   `guards.rs`, `lifecycle_dependency.rs`, `proof_data.rs`, implementation-source identity in
   `certify.rs`, and the finite checker in `LifecycleCheck.lean`/`Check.lean` as needed. Add a
   small membership-equivalence lemma to the audited proof closure; retain every existing
   theorem conclusion and integrity requirement. Missing/corrupt hints cannot authorize a root.
3. **Fix diagnostics (FR-042).** Add optional structured dependency identity to `SafetyError`;
   preserve action/entity/field context and deterministic precedence. Explicitly map the three
   new codes to Unsupported. Expose the underlying issues in analysis and assessment diagnostics;
   do not require authors to parse Debug text. Resource limits remain Inconclusive.
4. **Document and compare (FR-043–FR-044).** Add a minimal local fixture in
   `models/examples/safety_models/guard_contract.py` and extend the existing `run.py` with an
   explicit `--guard-contract --certify` mode. Record six independent Core/Safety result pairs;
   include this mode in the mandatory proof gate. Update public docs and lifecycle examples.
5. **Version and validate (FR-046).** Update checker identity/sidecar mappings and package
   metadata to the selected version. Run the full existing release check with its required
   gates, complete 506/507 and installed proof resources. Record new acceptance evidence under
   `target/safety-models/release-readiness/`. No tag creation/push or publication during implementation.

### Validation and Resource Contract

- Exhaust all 32 permutations for distinct two-, three- and four-member sets in each existing
  membership guard family (direct parent guard and nested child-query guard). Require useful
  positive certification, exact authored-root binding and all adversarial controls. Use seeded
  permutation/property checks for sets of 8, 16 and 32 members; include full certification
  representatives at each larger size. Retain existing five references and all old regressions.
- Scan existing input vectors; never generate permutations in production recognition. Cache
  discovery per admitted module and required set. Bound total work, not only each loop.
  Initial internal budgets: 16 MiB serialized output, 1,000,000 visited JSON nodes, depth 128,
  4,096 distinct candidate vectors, 4,096 values per vector and 4,096 probe constructions per
  analysis. Byte cap is checked before JSON decoding; Core serializer allocation itself remains
  part of the trusted bounded-process/build environment. Check counters before further work.
  Exceeding a budget produces explicit Inconclusive diagnostics and blocks certification;
  never truncate and certify a partial scan. These are conservative implementation defaults,
  not a new runtime/performance promise; measure them in preflight and document any tuning.
- For duplicate literals, require exact set equality while reconstructing their actual sequence.
  Extra unrelated arrays cannot broaden acceptance; omitted hints may cause refusal only.
- Keep all existing proof limits and the 507 quick budget. Run the new formal/matrix work outside
  that timer. No proof caching, new solver framework or performance project is required.

### Artifacts and Handoff

Research decisions are appended to [research.md](research.md); data additions to
[data-model.md](data-model.md). The [release contract](contracts/release-readiness.md) defines
diagnostics, recognition, comparison results and acceptance, and [quickstart.md](quickstart.md)
contains the future validation sequence. Original design sections follow as historical context;
the dated release decisions above supersede their sorted-only recognition/blanket serializer
restriction and initial checker version. Planning ran no probes, tests, proofs or release builds.

## Summary

Add FieldOwnership v1 and LifecycleDependency v1 to the existing Rust Model authority.
They constrain the complete final admitted module without changing Core IR, generating new
lifecycle capabilities or changing existing Model receipts. Python records author inputs and
delegates resolution, analysis, identities, certification and snapshot checks to Rust.

FieldOwnership is the first milestone: generic theorem, concrete checking and correspondence
controls must pass before LifecycleDependency implementation. The latter first requires a
public-Core canonical-guard feasibility gate. This plan does not claim that gate has run.

Use Lean 4.30.0 from the existing Nix lock. A small reflective checker derives obligations
from a closed extracted module inventory. Generic soundness quantifies over arbitrary finite
states; every certificate also needs kernel-checked acceptance of its concrete inventory.
Audit transitive axioms and freshly replay generic and concrete proofs with `leanchecker --fresh`.

Separate module-preservation evidence from exact-history state validation and host authorization.
Extraction, canonical-probe correspondence and the Rust snapshot predicate remain explicitly
trusted implementation boundaries. No unconditional system-safety claim follows.

State transparency is an absolute constraint (FR-038). Safety policies and evidence stay outside
Core Store state, event streams and replay inputs; neither pattern maintains a relationship or
ownership registry. Core-only replay must work with exported IR/history and no Safety policies,
certificates or Lean. Historical safety assurance separately requires retained evidence and
policy identities. Extend the existing composition/transition regressions to enforce this for
both patterns without introducing a new runtime subsystem.

## Technical Context

**Language/Version**: Rust 1.98.1 / edition 2024; Python >=3.13; Lean 4.30.0.

**Primary Dependencies**: Existing public `behavior-engine`, exact Core 0.15.0 commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier 0.10.0; existing serde_json, thiserror,
PyO3 and proptest; locked `pkgs.lean4` and bundled libraries. No external proof library,
private Core dependency, separate toolchain manager, new crate or runtime tool dependency.

**Storage**: Existing Core Store and public `SnapshotExport`; canonical evidence files in a
caller-selected directory; generated proof products under `target/`. No second Store reader.

**Testing**: Existing Rust/proptest, pytest, shell, compatibility/determinism and 506/507 checks;
new proof/premise mutations and exhaustive actual-Core small-state controls. No tests run here.

**Target Platform**: Existing locked Nix targets x86_64-linux and aarch64-linux. Explicit
certification requires the pinned toolchain; ordinary authoring/execution does not.

**Project Type**: Rust library and Python authoring binding with proof sources and reference cases.

**Performance Goals**: Measure concrete kernel reduction/replay time and memory at the FO
milestone; configure explicit resource limits, returning Inconclusive on exhaustion. No new
numeric performance claim. Preserve the existing 507 quick budget; run proofs outside its timer.

**Constraints**: Exact Core pin; old hashes/receipts/contracts unchanged; immutable nonoptional
Ref; literal lifecycle values; canonical mandatory guards; one relevant operation across all
dependency footprints; no runtime Lean, governance mechanism or second release process.

**Scale/Scope**: Two Safety Models; four neutral cases and one restricted VM fixture. Generic
proofs cover arbitrary finite states; concrete inventories are finite and subject to explicit
resource limits. Test bounds are separately recorded, never promoted to universal proofs.

## Constitution Check

*Assessed before research and after design. PASS describes the design, not executed checks.*

| Principle / constraint | Before research | After design / enforcement |
|------------------------|-----------------|----------------------------|
| I deterministic decisions | PASS | Rust analysis and Lean proof checking; no AI decision path. |
| II typed boundaries | PASS | Typed policy/evidence/data parsing; names cannot become Lean syntax. |
| III test first | PASS | Each implementation stage begins with failing controls; no code/tests added here. |
| IV reproducibility | PASS | Ordered canonical data, pinned sources/tools, isolated builds; paths/timings diagnostic. |
| V auditability | PASS | Lowered, certified, state-validated and authorized are distinct claims. |
| VI simplicity | PASS | Two concrete patterns, one reflective checker, existing crate/gates. |
| Rust constraints | PASS | Existing pinned tools/errors/unsafe prohibition retained. |
| Public Core only | PASS with feasibility gate | Public accessors and Core-built probes; stop on facade gaps. |
| Model/Core authority | PASS | Existing lowering and ownership precede Safety analysis; no IR rewrite. |
| Compatibility | PASS | Established fixture/receipt bytes unchanged; new Safety mappings only. |

Lean is the sole new tool dependency: the user requires generic and concrete machine-checked
proofs. SMT scenarios or handwritten arguments cannot substitute. The locked package avoids
another resolver. Start with bundled libraries; a general proof/plugin framework is excluded.

## Project Structure

### Documentation (this feature)

```text
specs/510-safety-models/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── contracts/
    ├── authoring-and-certification.md
    ├── proof-contract.md
    └── evidence-and-deployment.md
```

`tasks.md` contains the original implemented work. Its release extension belongs to the
subsequent speckit-tasks step, not this command.

### Source Code (planned)

```text
crates/behavior-models/
├── src/safety/
│   ├── mod.rs, input.rs, resolve.rs, inventory.rs
│   ├── field_ownership.rs, lifecycle_dependency.rs, guards.rs
│   └── identity.rs, evidence.rs, certify.rs, snapshot.rs
├── proofs/safety/
│   └── Data.lean, FieldOwnership.lean, LifecycleDependency.lean, Check.lean, Audit.lean
└── tests/safety_*.rs
crates/behavior-py/src/                 # existing adapter
python/behavior/safety.py
python/behavior/{module,__init__}.py
python/behavior/_engine.pyi
python/tests/test_safety_*.py
models/examples/safety_models/          # five reference cases and one runner
models/fixtures/safety-compat.json      # new mappings; old manifest unchanged
scripts/check-safety-proofs.sh
scripts/{gates,determinism-check}.sh
scripts/check-models-compat.py
scripts/tests/test_safety_*.sh
flake.nix                              # existing locked Lean package; no lock update
```

**Structure Decision**: Keep semantics/proofs in the existing Rust authority. Retain/embed the
exact proof sources for installed certification. Python and the example runner delegate to it.
Small neighboring files may be combined without changing these contracts or adding a crate.

## Original Implementation Stages and Stop Gates

### A. FieldOwnership first

1. Start with failing complete-inventory, aliases/bindings, no-op writer, create/remove,
   generated-writer, malformed-policy and intersecting-restriction controls.
2. Define assignment/create/remove and frame semantics. Prove FO-1 and change-witness lemmas;
   assuming the desired ownership property as a premise does not meet this milestone.
3. Encode the full concrete inventory with fixed syntax and data-only bytes/indices. Lean
   derives permission/relevance checks and proves that exact instance accepted.
4. Add structured axiom auditing and fresh replay for generic and concrete roots. Independently
   compare the emitted action/operation inventory with the admitted module.
5. Demonstrate ordinary field ownership, StateMachine composition, omission/proof mutations,
   deterministic evidence and unchanged existing references. Measure proof costs.

**FO gate**: All required positive/negative, proof-integrity and correspondence controls pass;
the trust boundary is recorded. Only then start B/C. No passing result is asserted by this plan.

### B. Public-Core feasibility before LD implementation

Demonstrate declaration-preserving expected-expression probes, actual mandatory-precondition
root matching, literal/owner initializer matching and public State-role comparison. Include
wrong captures, weakened/nested guards and supplied Input/Context Entity values.

**Gate**: All required supported patterns in the proof contract have positive/adversarial
evidence through Core 0.15.0's facade. Stop affected work if this requires private variants,
serialized guard AST analysis, copied Core hashing or a changed pin. Report the exact missing
operation. Tests establish implementation confidence; probe correspondence remains trusted.

### C. LifecycleDependency

Prove LD-1 for the restricted operation cases and full result-state Ref semantics. Extend the
checker to derive union footprints, operation counts, immutable ownership, required guard/value
correspondences and every affected dependency's obligations. Mixed parent/child roles without
a preservation rule are Unsupported, including when represented by one Core operation.

**LD gate**: Generic/concrete proofs, useful positive transition witnesses, independent actual-Core
enumeration and all negative/mutation controls pass. No domain-specific axiom or weakened property.

### D. State validation, deployment and integration

Implement the exact Rust property validator over public complete snapshot exports. Record exact
history/schema/StateId/module/policy set separately. Demonstrate invalid snapshots and stale
activation anchors, including an intervening history event with unchanged StateId. Describe an
enforced host switch protocol; implement no new Core governance.

Integrate Python authoring, five reference cases, new identity mappings, documentation and proof
checks into existing gates. Run existing compatibility, determinism, complete 506/507 and normal
ecosystem checks during implementation; preserve all budgets and established records.

## Validation Design

- Structural controls cover unauthorized writers through every binding, same-value assignments,
  creation/removal, generated writers, omitted actions/effects and overlapping authority.
- Proof controls remove each essential abstract/concrete premise; introduce holes, native/custom
  axioms, wrong theorem statements, stale policy/module/data identities and incomplete inventories.
- Actual-Core controls cover child creation/reactivation, parent prohibition/removal, conflicts,
  invalidated source guards, State-alias refusal, overlapping IDs and shared-parent dependencies.
- Initial exhaustive bounds: 0–2 parents, 0–2 children per child type, one/two child types,
  two parent and two child lifecycle categories, all reference-valid owner mappings plus invalid
  snapshot controls. Enumerate each supported operation class and representative ID/name/order
  changes. Add targeted mixed-role and multiple-operation configurations. Publish exact counts
  and limits in a coverage manifest; safe Unsupported cases are not false proof failures.
- Repeat canonical output with identical pinned inputs; mutate every bound identity; re-extract
  inventory and verify the actual concrete theorem type when verifying an imported evidence bundle.
- Preserve every established receipt/hash/reference entry. Add only new Safety mappings through
  the existing compatibility gate; extend existing determinism checks for the new sidecars.

Finite bounds are test coverage, not theorem limits. Adjustments must publish their actual
coverage and preserve every required adversarial category.

## Complexity Tracking

No constitution violations proposed. The proof tool/checker is justified above. Public-Core
feasibility, kernel reduction costs and unverified correspondence are risks, not gate waivers.

## Planning Completion

Product decisions Q1–Q17 are resolved; research and design are linked below. Required feasibility
checks remain implementation work and have not run. No Core changes, tool installation,
runtime tests, commits or publication are part of this step. No extension hooks are registered.

- [Research](research.md)
- [Data model](data-model.md)
- [Authoring and certification](contracts/authoring-and-certification.md)
- [Proof contract](contracts/proof-contract.md)
- [Evidence and deployment](contracts/evidence-and-deployment.md)
- [Validation quickstart](quickstart.md)
