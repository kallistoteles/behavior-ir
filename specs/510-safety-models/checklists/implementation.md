# Feature 510 implementation evidence

This is an implementation acceptance matrix, not a modification of the reviewer-owned requirements checklist.
**Status: complete. All required local gates and package/API controls passed.** Certification remains conditional on the explicit trust boundaries below.

## Evidence index

- [FO milestone](../../../target/safety-models/fo/milestone.json): completed before LifecycleDependency.
- [Public-Core feasibility](../../../target/safety-models/ld/public-core-preflight.json).
- [LD milestone](../../../target/safety-models/ld/milestone.json), [bounded coverage](../../../target/safety-models/ld/coverage.json), [mutations](../../../target/safety-models/ld/mutations.json).
- [Evidence milestone](../../../target/safety-models/evidence/milestone.json).
- [State/deployment milestone](../../../target/safety-models/state/milestone.json).
- [Final integrated gates](../../../target/safety-models/final/gates.json): 306 Rust tests; 621 Python/model tests, 5 existing release-only skips; full 506/507; required quick budget 31.11s / 35s.
- [Final proof gate](../../../target/safety-models/proof-gate/summary.json): five conditional certificates, eleven audited roots each.
- [Installed-package validation](../../../target/safety-models/final/package.json), [public API rerun](../../../target/safety-models/final/package-api-controls.json), [quickstart negatives](../../../target/safety-models/final/quickstart-controls.json), [baseline review](../../../target/safety-models/final/review.json).

The generated evidence is local build output. Reproduce it using [quickstart](../quickstart.md); no checked-in JSON status confers certificate authority.

## Functional requirements

| Requirement | Implementation and evidence | Boundary / acceptance |
| --- | --- | --- |
| FR-001 | Rust safety inputs/resolution; five public reference modules | Two concrete patterns, no universal domain entities |
| FR-002 | Rust safety authority; thin PyO3/Python adapters | Core alone executes/stores/replays |
| FR-003 | Public-Core preflight and public-surface gate | Exact Core 0.15.0 pin; pin and protected-file review passed |
| FR-004 | Complete Inventory extraction, independent verify_module, concrete anchors | Rust extraction completeness remains trusted |
| FR-005 | safety_composition/ownership tests; legacy models compatibility gate | Old-fixture byte review passed |
| FR-006 | Separate normalized policy/complete-set hashes; safety-compat.json | Old Model receipts/compat.json preserved |
| FR-007 | Canonical probes inspect mandatory roots; no action rewriting | No generated lifecycle API |
| FR-008 | FO structural permission tests | Unauthorized unreachable/no-op assignments rejected |
| FR-009 | Full inventory; alias and generated-writer controls | Resolved entity/field/action identity |
| FR-010 | Creator/writer/remover controls and actual transitions | Creation initialization independent of assignment permission |
| FR-011 | Typed deterministic SafetyError diagnostics | Responsible action/entity/field identified |
| FR-012 | Analysis and evidence inventories plus normalized ownership policy | Lowered is explicitly uncertified |
| FR-013 | LD typed inputs, required Ref, immutable owner controls | No optional/plain-ID/reassignment pattern |
| FR-014 | LifecycleData.lean R/P definitions; original-property validator | Arbitrary finite theorem; terminal child parent existence |
| FR-015 | Union operation footprints from all actual mutations | No manually maintained relevant-action allowlist |
| FR-016 | Canonical no-nonterminal-child guard and actual Core transition tests | Every affected dependency checked |
| FR-017 | Role/binding/root correspondence controls and classified sequence theorem | Guard correspondence to Core remains explicit trust |
| FR-018 | Operation-count/mixed-role/literal guard controls | Conservative v1 rejects safe unsupported simultaneous effects |
| FR-019 | Unsupported/Failed/Inconclusive reports and negative controls | No required non-success obligation certifies |
| FR-020 | Separate inductive transition certificate and exact state record | Host authorization and valid base are independent premises |
| FR-021 | Safety.FieldOwnership.sound; Safety.LifecycleDependency.sound | Versioned generic definitions and finite-state quantification |
| FR-022 | Pinned Lean 4.30.0, Audit.lean collectAxioms/type checks, fresh checker | Allowed logical axioms only; same Lean kernel in fresh process |
| FR-023 | Check/LifecycleCheck/LifecycleSequence and Concrete.checked | Typed raw operations/probes checked against theorem premises |
| FR-024 | Concrete.certified with independent exact anchors/full inventory | Extraction/probe/encoding correspondence remains trusted |
| FR-025 | Premise mutation tests and mandatory proof-gate controls | Tests/structural arguments do not substitute for Lean proofs |
| FR-026 | Canonical semantic evidence, dependency/source identities and reproducibility tests | Diagnostics/resource logs excluded from semantic identity |
| FR-027 | Four outcomes; missing tools/exhaustion/unsupported/tamper controls | Conditional certification, never unconditional system safety |
| FR-028 | Public export adapter, original R/P validator, exact-history opaque record | Backend completeness/provenance and validator correspondence trusted |
| FR-029 | Actual Core-valid relationally unsafe migration test | Fresh incoming-state validation; no general migration proof |
| FR-030 | Alternative-module/policy-switch/command-only stale-anchor controls; host example | All writers/migrations/activation must obey external coordination |
| FR-031 | Certificate vs validation vs authorization types/docs | No governance authorization, physical safety or liveness claim |
| FR-032 | Ownership/inventory/correspondence/proof-integrity mutation suites | Unauthorized aliases/generated writes and omitted inventory rejected |
| FR-033 | Actual transitions/deployment tests and unsafe mixed source-guard example | Conflicts require re-evaluation; Ref prevents surviving-reference deletion |
| FR-034 | safety_exhaustive independent Core enumeration; published coverage manifest | 3,924 constructed / 1,904 valid states; 38,006 attempts; 30,218 commits; zero unsafe |
| FR-035 | Five reference cases including shared parent and restricted VM/Resource | I-5b/I-6 unmapped; live != nonterminal; pool-owned domains unrestricted |
| FR-036 | Existing gates plus named Safety proofs gate and package smoke | Full ecosystem/506/507/package checks passed |
| FR-037 | README, models README, reference README, quickstart, deployment.py | Public API; lowering distinct from explicit proof checking |
| FR-038 | Five policy-free Core replay controls with Safety hooks forbidden | No persistent Safety state/events/registry/replay dependency |

## Success criteria

| Criterion | Evidence / status |
| --- | --- |
| SC-001 | Generic/concrete roots, axiom audit, fresh replay and broken-premise controls; final gate passed |
| SC-002 | Full action/effect inventory plus independent admitted-module comparison and omission controls |
| SC-003 | Five useful reference histories; all v1 unsupported cases fail closed |
| SC-004 | Independent bounded actual-Core enumeration: zero unsafe certified commits (bounds in coverage manifest) |
| SC-005 | Repeated canonical bytes across paths/logs and display-policy label/order variants |
| SC-006 | Four assessment outcomes; exact-history state records; locked incoming-policy host boundary |
| SC-007 | Old compatibility/pin regression and complete 506/507: full integrated execution passed |
| SC-008 | FO milestone precedes public-Core/LD milestone; both patterns and explicit assumptions documented |
| SC-009 | Core-only replay for all five histories; policies/certificates/Lean never consulted |

## Decisions Q1–Q17

| Decision | Implemented contract |
| --- | --- |
| Q1 | Structural unauthorized assignments rejected, including no-op/unreachable |
| Q2 | Creator, writer, remover permissions independent |
| Q3 | Required Ref parent existence, including terminal children |
| Q4 | Owner immutable throughout lifetime |
| Q5 | Separate versioned content-addressed evidence; hashes are identity, not proof |
| Q6 | Lean 4.30.0 from locked Nix, no Lake/elan toolchain manager |
| Q7 | Generic and concrete checking; extraction completeness explicitly trusted |
| Q8 | Validate existing actions; no new generated lifecycle family |
| Q9 | One relevant operation across complete union of dependencies |
| Q10 | Transitive allowed-axiom/type audit and fresh kernel replay |
| Q11 | Canonical whole mandatory roots with actual State roles/bindings |
| Q12 | Original-property validator over public exact-history Core export |
| Q13 | Isolated parent creation, child removal and unreferenced parent removal supported |
| Q14 | Lowered vs explicit Certified; no Lean during Store execution |
| Q15 | Union counting once, every affected dependency, mixed roles rejected |
| Q16 | Incoming exact module + complete policies + coordinated fresh full-history activation |
| Q17 | Restricted VM/Resource predicates only, no inferred domain-invariant mapping |

## Explicit remaining boundaries

- Rust extraction, canonical probes, finite-data encoding and snapshot evaluator are trusted correspondence, not end-to-end verified code.
- Core semantic contracts, hash collision resistance, complete consistent backends and immutable pinned tool/build infrastructure remain assumptions.
- Initial validity, migration safety and approved-module activation/continuity are separate obligations. No certificate grants Core authorization.
- Host example is one coordinator; uncoordinated/multi-process backend writers require equivalent externally enforced synchronization.
- Conservative supported syntax/operation limits and proof cost are deliberate v1 limits; no temporal liveness, physical execution or arbitrary relational verification.
- Published Behavior 0.13.0 artifacts are unchanged. Feature 510 is unreleased and publishes/tags nothing.

## Integration findings resolved before the successful full run

The first complete attempt failed on four stale gate-harness expectations and one missing API
manifest update. The mandatory proof gate is now stubbed/ordered in the disposable harness,
its failure explicitly stops the chain, and the API manifest adds exactly nine exports while
preserving all existing names/kinds/formats. Eleven focused integration tests passed, followed
by a complete successful rerun of the unchanged mandatory gate chain. The failed attempt is
retained as `final/gates-attempt1.json` / `.log`; it is not reported as a successful gate.

The five skipped Python tests are existing opt-in release-script tests controlled by
`BEHAVIOR_RELEASE_TESTS=1`; no Safety test or proof obligation was skipped. The separately
required fresh installed-package/reproducibility checks passed through `release-check.sh`.

The implementation and source remain unreleased. The release-check wheel is an unpublished
local validation artifact; no existing tag, published record or attestation was changed.

## Final acceptance

All 85 implementation tasks are discharged. The final targeted public API rerun passed all
11 tests, with JUnit results in `final/package-api-controls.xml`. The baseline review passed:
78 protected files unchanged; only the intentional `.gitignore` target-symlink handling differs.
All old API names/kinds/formats are retained, with nine additive exports. Core remains 0.15.0,
verifier 0.10.0; established Model fixture/receipt builders and unrelated Spec Kit/Skills work
are unchanged. The remaining boundaries above are explicit assumptions/unsupported v1 scope,
not failed proof obligations. No mandatory check remains failed or unrun.

## Release-readiness extension acceptance (2026-10-18)

**Status: T086–T113 implemented and locally validated for candidate 0.13.1.** The preceding
matrix/counts record the original T001–T085 scope. The evidence below establishes US5–US7 /
FR-039–FR-046 / SC-010–SC-014; earlier runs do not discharge these corrections. The original
[proof-gate summary](../../../target/safety-models/release-readiness/historical-proof-gate-summary.json)
is archived separately because the shared proof-gate output path has now been regenerated.

| Requirement / story | Current implementation and actual acceptance evidence |
| --- | --- |
| US5; FR-039; SC-010 | Bounded order hints plus whole typed mandatory-root reconstruction; [82 current exact-module certificates](../../../target/safety-models/release-readiness/membership-current.json) cover both families, all 2/3/4-member permutations, duplicates and 8/16/32-member representatives; malformed contexts do not certify |
| FR-040; SC-014 | [Public-Core preflight](../../../target/safety-models/release-readiness/preflight.json), scoped serializer exception and rejection controls; cumulative byte/node/depth/vector/probe budgets fail closed; no factorial recognition or Core rewriting |
| FR-041 | Selected actual-root/ordered-vector witnesses flow into strengthened concrete Lean premises; twelve audited roots per certificate, only permitted logical axioms, fresh kernel replay; reordered/forged/missing proof data controls fail |
| US6; FR-042; SC-011 | [Diagnostics](../../../target/safety-models/release-readiness/diagnostics.json): distinct parent binding, owner association and parent guard codes, complete candidate staging and stable context/order; terminal assignment/creation and resource outcomes remain distinct |
| US7; FR-043; SC-012 | Public authoring docs and tested unchanged-owner Planned → Created / Terminal → Active examples explain required State parent, exact association and mandatory permitted-parent root |
| FR-044 | [Current six-case comparison](../../../target/safety-models/release-readiness/comparison-current.json) records nonempty query-sensitive local checks, exact action/subject/module identities and independent Safety results; unexpected/missing rows fail the mandatory gate |
| FR-045 | Existing ownership/inventory/snapshot/replay controls plus new adversarial cases passed; [actual-Core finite enumeration](../../../target/safety-models/release-readiness/exhaustive-acceptance.json) has 38,006 attempts, 30,218 commits and zero unsafe results within its explicit bounds |
| FR-046; SC-013 | [Full release-check exit 0](../../../target/safety-models/release-readiness/release.json), all 19 required gates, [complete 506/507](../../../target/safety-models/release-readiness/conformance-complete.json), clean installed-package proof resources/certification/reverification and matching repeated wheel builds |

### Fresh evidence index and counts

- [Baseline](../../../target/safety-models/release-readiness/baseline.json) and [exact trial-source journal](../../../target/safety-models/release-readiness/release-source.json).
- [Membership milestone](../../../target/safety-models/release-readiness/membership.json), [current full-run instances](../../../target/safety-models/release-readiness/membership-current.json), [current identical repeats](../../../target/safety-models/release-readiness/membership-repetition-current.json), [diagnostic milestone](../../../target/safety-models/release-readiness/diagnostics.json) and [authoring/contract milestone](../../../target/safety-models/release-readiness/contract.json).
- [Full gates](../../../target/safety-models/release-readiness/gates-current.json): **320 Rust tests; 662 Python/model tests; no Python/model skips**. Full properties and opt-in release controls ran.
- [Current mandatory proof gate](../../../target/safety-models/release-readiness/mandatory-proof-gate-current.json): **five conditional reference certificates and all six comparison rows**, complete action inventories, twelve audited roots and fresh replay.
- [Complete conformance](../../../target/safety-models/release-readiness/conformance-complete.json): **506: 39 checks; 507: 61 checks and 12 obligations**, all passed. [Quick runtime](../../../target/safety-models/release-readiness/507-quick-budget.json): **32.56s / unchanged 35s**; quick coverage remains explicitly incomplete.
- [Installed package](../../../target/safety-models/release-readiness/safety-package-current.json): Core-only execution/replay, opaque certificate and independently reverified bundle; **23 Skill examples and 49 API/drift/equivalence checks** passed through release-check.
- [Unpublished trial artifacts/checksums](../../../target/safety-models/release-readiness/artifacts/SHA256SUMS): two additional builds produce identical wheel bytes. [Final review](../../../target/safety-models/release-readiness/review.json) records **32 protected files, 31 non-Safety Rust sources and 25 Python production/binding files unchanged**, all old compatibility tuples/receipt builders preserved and **22 pinned receipts unchanged**.

### Evidence classes and remaining nonblocking limits

Generic and concrete Lean proofs, transitive permitted-axiom/type audits and fresh replay are
machine-checked evidence. Whole-module extraction/probe/encoding checks are implemented in Rust
and remain a trusted correspondence boundary. Exhaustive small-state tests and conformance are
finite implementation evidence. WorkQueue's twelve passed obligations retain 5 contract, 7 SMT,
5 structural and 13 test dependencies, with named assumptions for nine obligations; they are not
twelve universal SMT proofs.

Genesis validity, migration safety, complete/provenanced snapshots and host authorization of the
exact module/complete policy set from a fresh enforced activation anchor remain separate obligations.
The canonical v1 language, immutable required Ref, mixed-operation exclusions and restricted
VM/Resource fixture remain explicit. No persistent Safety state, event stream or replay dependency
was introduced. No production-domain ownership model or inferred I-5b/I-6 mapping is prescribed.

The local Core verifier still reports Inconclusive for the permuted and equivalent-outside-profile
query fixtures. The supported permutation independently certifies under Safety's assumptions;
the excluded equivalent form remains Unsupported. No Inconclusive result is relabeled as proven.

Fresh proof checking is costly (the Core enumeration took 4,858.61s; the membership group
1,884.78s). All mandatory checks remain intact. Build/temp/cache work uses the data volume;
generated caches were removed or losslessly archived with byte verification. Root capacity still
needs observation; no uncommitted user work was removed. The failed formatting/disk attempts and
the pre-pytest release-test version correction remain explicit in the run/source journals.

Behavior **0.13.1**, Core **0.15.0**, verifier **0.10.0**, Lean **4.30.0**, checker/lowerer **2**
and policy semantics **1** agree. Consumer Skills changed only required version metadata.
All 113 tasks are discharged by the original and extension evidence; no mandatory check remains
failed or unrun. These ignored local artifacts are reproducible through [quickstart §7](../quickstart.md#7-release-readiness-validation).
The candidate remains uncommitted/unpublished; release publication must rebuild the approved
merged source through the existing separate explicit process. Existing published records are immutable.
