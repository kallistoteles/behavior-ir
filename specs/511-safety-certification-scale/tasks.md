# Tasks: Scalable Safety Certification and Independent Proof Checking

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/) and [quickstart.md](quickstart.md).
**Date**: 2026-10-22. Revised after cross-artifact analysis. All checkboxes describe future work;
prior investigation is evidence, not completed remediation. Paths are relative to repository root.

**Tests**: the specification and constitution require test first. Write/extend the relevant tests,
observe their intended red failure and record it before changing implementation. Validate each
mutation's actual imports and pair it with a valid control; broken harnesses are invalid evidence.

**Organization**: P1 stories execute US3 -> US2 -> US1. Exact encoding tests and identity transitions
belong to US2; package readiness belongs to the complete US1 chain. P2 US4 evaluates the frozen
candidate, then US5 checks final compatibility/delivery without changing that candidate silently.

## Format and scope

`- [ ] Tnnn [P?] [USn?] Action with exact file paths`

`[P]` permits independent file authoring after prerequisites, never overlapping heavy proof,
benchmark or gate runs. Unmarked tasks are sequential in their phase. No new Core semantics,
runtime state, Safety language, governance or cached proof authority. No external application tests.
The checkout remains `510-safety-models`; the logical feature is `511-safety-certification-scale`.
Preserve unrelated/uncommitted work. No merge, release/tag publication or new feature is authorized.


## Phase 1: Setup — preserve baseline provenance

**Goal**: preserve released artifacts and establish independently expected regression inputs.

- [X] T001 Record the starting source commit, installed Behavior 0.13.1 wheel checksum, Core 0.15.0/public dependency pin, verifier 0.10.0, checker 2, policy semantics 1 and Lean 4.30.0 derivation identities in `specs/511-safety-certification-scale/evidence/remediation-baseline.json`; retain existing evidence unchanged and reject Core overrides before required checks.
- [X] T002 [P] Define the valid v1 regression inventory and independently expected action/operation/policy counts, including five Safety references, creators/removers, aliases, canonical guard permutations, generated actions and shared dependencies, in `specs/511-safety-certification-scale/evidence/v1-regression-corpus.json`; distinguish existing cases from new planned constructions and do not copy private application code.
- [X] T003 [P] Record the execution/provenance convention and fixture/hash inventory for existing experiments in `specs/511-safety-certification-scale/experiments/README.md`; use task-owned fresh outputs, preserve earlier observations and identify invalid historical controls without deleting them.

**Checkpoint**: baseline provenance and expected regression inventory recorded; no passing tests implied.

## Phase 2: Foundational — reproduce, freeze conditions and select (G0/G1)

**Goal**: a trustworthy experiment and justified design before production changes. G0 freezes
conditions, not a future implementation identity. G1 selects the checked contract and version policy.

- [X] T004 Write failing harness tests in `specs/511-safety-certification-scale/experiments/tests/test_cold_runner.py` for distinct producer/consumer processes and workspaces, whole-chain timing, absence of producer work products at the consumer, failures/timeouts, persisted failed/unrun stages and no prior-result substitution; use a deterministic fake process boundary for fast runner tests without treating it as proof evidence.
- [X] T005 Implement the external sequential installed-consumer runner in `specs/511-safety-certification-scale/experiments/cold_runner.py` to satisfy T004; include startup, input parsing/admission/extraction, construction/checking, type/axiom audit, fresh replay and final persistence in totals; record exact commands, CPU/memory accounting scope, cache/dependency conditions and absent heartbeat/depth counters explicitly. Bind each baseline/candidate attempt to a separate exact build manifest and the shared evaluation-profile identity; never infer a candidate build from the profile.
- [X] T006 Write failing generator tests in `specs/511-safety-certification-scale/experiments/tests/test_workloads.py` for exact A256/A512 requests, independently expected occurrence counts, six dimension values, FO/LD composition, shared-parent dependencies and generated actions; detect coupled dimensions and accidental duplicate action identities.
- [X] T007 Implement domain-neutral workload construction in `specs/511-safety-certification-scale/experiments/workloads.py`, reusing `experiments/certification_probe.py` under the feature directory and public authoring APIs; preserve exact A256/A512, create valid one-axis/mixed cases within supported v1 semantics and derive expected counts independently of the Safety extractor.
- [X] T008 Write failing early-exit/resource-accounting tests in `crates/behavior-models/tests/safety_proof_integrity.rs` for Concrete/audit/fresh timeouts and missing tools: failed and unrun obligations persist, no certificate is issued and configured limits are not reported as measured consumption or as applied to stages that did not receive them.
- [X] T009 Correct stage/report instrumentation only as needed in `crates/behavior-models/src/safety/certify.rs` for T008, keeping operational measurements out of semantic identity and recording the changed implementation-source identity separately from the immutable published baseline; expose actual stage-specific applied controls and measurement limitations, retaining failure records even when child execution exits early.
- [X] T010 Reproduce released full-chain A256/A512 failures sequentially and run controlled small pilots using `specs/511-safety-certification-scale/experiments/cold_runner.py`; retain all attempts in `specs/511-safety-certification-scale/evidence/baseline-runs/` and update `root-cause-analysis.md` in the feature directory with root/cascading diagnostic distinctions and data/proof/fresh cost attribution.
- [X] T011 Freeze `specs/511-safety-certification-scale/evidence/evaluation-profile.json` after pilots with actual x86_64-linux allocation/limits, exact six-axis/mixed corpus identities, finite separate total cold certification/checking budgets, repetitions and intended-use rationale; preserve published defaults as baseline controls, hash the manifest before final comparison and reject unknown mandatory values. This is G0's exit gate; do not invent timings or choose the largest passing case as capacity. Freeze only acceptance conditions and baseline semantic workload definitions; package/checker/source identities belong in per-build manifests, with candidate-specific expected request identities recorded separately.
- [X] T012 Inventory all twelve baseline audited semantic obligations, permitted axioms, trusted Core/extraction/encoding/probe boundaries and necessary checking/refinement premises in `specs/511-safety-certification-scale/proof-obligations.md`; define what candidates must prove without fixing chunk layout, root names, format or algorithm.
- [X] T013 Compare isolated representation and finite-check prototypes, separately and together, under `specs/511-safety-certification-scale/experiments/candidates/` with results in `specs/511-safety-certification-scale/evidence/candidate-runs/`; prioritize construction/checking separation, compare witness/direct/hybrid approaches, reuse pinned proved components where applicable and measure actual proof/type/audit/fresh costs with positive and adversarial controls rather than native execution alone.
- [X] T014 Complete the G1 selection gate in `specs/511-safety-certification-scale/design-decision.md`: choose only the empirically justified representation/checking route, define its interpretation and bounded grammar if needed, checked bridge prototypes, independent coverage, complete audit mapping, source identities, trust/dependency review and approved checker/statement/evidence version transitions and the ecosystem release-version decision before integrating the changed contracts; update `contracts/` and `data-model.md` in the feature directory to that decision. If no sound feasible design exists, document the exact obstacle and continue investigation; do not start production integration or weaken the contract.
- [X] T015 Record and apply the ecosystem version decision under `docs/versioning.md` and the existing metadata in `Cargo.toml` as warranted by changed verifier outcomes/formats; preserve `pyproject.toml`'s dynamic version derivation from that single authority, document supported contracts/pinned historical conditions and use the existing release metadata conventions without choosing a patch by assumption or publishing. Complete metadata changes now, before building any final evaluation package; this task does not publish or claim mathematical checks have passed.

**G0 exit**: T011 has finite justified budgets/corpus/conditions. **G1 exit**: T014 selects the justified contract; T015 applies its ecosystem version policy before candidate builds. No arbitrary witness format/dependency is preselected. Profile revisions retain earlier results and require explicit review and reevaluation.

## Phase 3: US3 — Reuse verified mathematics without overstating assurance (P1)

**Goal**: checked generic foundations and integrity for the selected route.
**Independent test**: exact generic/bridge statements pass; actual holes, forbidden axioms,
missing premises and weakened types fail with valid parent controls.

- [X] T016 [P] [US3] Add proof-premise/law negative controls with valid parents in `crates/behavior-models/proofs/safety/tests/OwnershipMutations.lean` and `crates/behavior-models/proofs/safety/tests/LifecycleMutations.lean`; cover the selected representation/checker, full-key duplicates, incomplete coverage and applicable comparator/permission premises without assuming a particular sorting witness.
- [X] T017 [P] [US3] Extend expected-type/transitive-axiom/fresh-replay controls in `crates/behavior-models/tests/safety_proof_integrity.rs` for selected new generic roots, forbidden custom/native axioms, `sorryAx`, weakened conclusions and missing audit rows; prove the actual mutated declaration is imported.
- [X] T018 [US3] Complete the verified-component assessment in `specs/511-safety-certification-scale/verified-algorithms-research.md`: document exact pinned results/laws, missing bridges, import costs and why selected custom work or new dependencies are justified; keep the fallible producer outside proof-authoritative algorithm requirements.
- [X] T019 [US3] Implement the selected machine-checked representation interpretation/refinement in `crates/behavior-models/proofs/safety/Check.lean`, reusing the finite definitions in `crates/behavior-models/proofs/safety/Data.lean`; preserve intended guarantees and exact semantic coordinates without requiring old payload chunks or pretending Rust-to-Core extraction is proved.
- [X] T020 [US3] Prove selected uniqueness/permission adapters in `crates/behavior-models/proofs/safety/Check.lean` and `crates/behavior-models/proofs/safety/FieldOwnership.lean`: original full-key multiplicity coverage and duplicate rejection, index preservation and conjunction of all per-role restrictions; discharge actual library laws and do not use distinctness-assuming theorems circularly.
- [X] T021 [US3] Prove the selected checking implication for arbitrary proposed data in `crates/behavior-models/proofs/safety/Check.lean`, reusing `crates/behavior-models/proofs/safety/LifecycleSequence.lean` for complete FO/LD preservation; no correctness assumption about search/production may appear, and the conclusion must include the mapped baseline guarantee rather than merely witness validity.
- [X] T022 [US3] Integrate intended-type and transitive axiom checks for every mapped baseline and new checker/refinement obligation in `crates/behavior-models/proofs/safety/Audit.lean`; permit only `propext`, `Classical.choice`, `Quot.sound` and document the versioned obligation-to-root mapping in `specs/511-safety-certification-scale/proof-obligations.md`.
- [X] T023 [US3] Integrate selected generic sources and the complete actual dependency closure into `crates/behavior-models/src/safety/certify.rs`; retain fresh pinned-kernel replay and measure narrower imports only if G1 evidence warrants them, without skipping or caching required checks.
- [X] T024 [US3] Run generic theorem/adapter and actual integrity mutations through `crates/behavior-models/tests/safety_proof_integrity.rs` and `crates/behavior-models/tests/safety_lifecycle_proofs.rs`; save positive and rejected-mutant evidence in `specs/511-safety-certification-scale/evidence/proof-foundation/`, including actual import/tool identities and no unreviewed axiom dependencies.
- [X] T025 [US3] Update `specs/511-safety-certification-scale/contracts/proof-correspondence.md` with the integrated theorem statements, proved bridge coverage and precise remaining trusted Rust/Core/probe/encoding boundaries; distinguish kernel acceptance from statement adequacy and independent fresh replay from a separate kernel implementation.

**Checkpoint**: generic/bridge proofs and actual integrity controls pass. They do not alone certify a module; producer search correctness is not an assumption.

## Phase 4: US2 — Trust mathematical equivalence and independent checking (P1)

**Goal**: exact-request checking independent of producer completeness or executable input.
**Independent test**: independently valid data passes; omission, identity, key/index and injection
controls fail. Encoding and version-transition tests are explicitly before their implementation.

- [X] T026 [P] [US2] Add independent expected-request coverage tests in `crates/behavior-models/tests/safety_evidence.rs` for omitted/duplicated authored or generated actions, assignment/create/remove occurrences, policy subsets and stale module/policy identities; include same-value writes and one-denying overlapping policy controls with valid counterparts. Include red tests for G1-approved contract/checker version and implementation-identity transitions before applying them.
- [X] T027 [P] [US2] Add typed-transport/injection tests in `python/tests/test_safety_evidence.py` for malformed/bounded data, undeclared contract versions, forged success/prior acceptance and supplied source/code/executable objects; an execution sentinel must establish that rejection executes no arbitrary supplied code and loading grants no certificate authority.
- [X] T028 [P] [US2] Write and observe failing acceptance/rejection tests in `crates/behavior-models/tests/safety_correspondence.rs` for independently constructed valid proposed data and paired dropped/duplicated/equal-length-nonpermutation/index-mismatched variants; where the selected checker is direct, exercise its adversarial data boundary without inventing an unnecessary witness format.
- [X] T029 [P] [US2] Write and observe failing encoding/coordinate and exact intended-claim regression controls in `crates/behavior-models/tests/safety_input.rs` for the G1 representation at larger sizes, full keys and all operation/policy bindings; altered chunking may differ while semantic correspondence and exact request identity remain mandatory.
- [X] T030 [US2] After the preceding identity/contract tests have failed as intended, apply only G1-approved checker/statement/evidence/source identity transitions in `crates/behavior-models/src/safety/mod.rs`, `crates/behavior-models/src/safety/identity.rs` and `crates/behavior-models/src/safety/evidence.rs`; preserve policy meaning and established Model identities/receipt formats, and do not edit `models/fixtures/compat.json` or `models/fixtures/safety-compat.json` to erase a regression. Apply numeric contract/checker transitions before the changed encoder/verifier is integrated; final source identities are captured from the completed candidate build.
- [X] T031 [US2] Separate authoritative expected-request preparation from fallible construction at `crates/behavior-models/src/safety/certify.rs`, using `crates/behavior-models/src/safety/inventory.rs` and `crates/behavior-models/src/safety/resolve.rs`; independently re-admit through public Core and compare the full expected policy set rather than trusting transported inventories.
- [X] T032 [US2] Implement only the selected bounded typed evidence/proof-data contract in `crates/behavior-models/src/safety/evidence.rs`, including explicit supported-version dispatch, exact identities and opaque certificate authority; baseline bundle v1's strict five-key schema must not silently gain a witness field.
- [X] T033 [US2] Implement the independent checking path in `crates/behavior-models/src/safety/certify.rs` against the expected request and selected generic soundness theorem; validate any proposed data independently, exclude producer correctness/prior success from authority and avoid unnecessary reconstruction work only where G1 establishes correctness.
- [X] T034 [US2] Integrate selected semantic input/identity encoding in `crates/behavior-models/src/safety/proof_data.rs` and its fixed expected wrappers in `crates/behavior-models/src/safety/certify.rs`; preserve action/operation/policy coordinates and bind the exact admitted request without assuming hashes alone prove decoding/extraction correspondence.
- [X] T035 [US2] Generate and check complete concrete obligations, intended final types, all mapped audit rows and fresh replay before certificate issuance in `crates/behavior-models/src/safety/certify.rs`; missing, unsupported or inconclusive necessary checks cannot produce a partial certificate.
- [X] T036 [US2] Preserve distinct invalid-premise, unsupported-shape, resource-exhaustion, bad-witness and established-counterexample diagnostics in `crates/behavior-models/src/safety/evidence.rs` and `crates/behavior-models/src/safety/certify.rs`; record unrun obligations and keep completed mathematical validity distinct from failed total-budget acceptance.
- [X] T037 [US2] Keep thin installed-consumer access through `crates/behavior-py/src/safety.rs` and `python/behavior/safety.py`, preserving public entry points unless G1 explicitly versions a necessary change; Python must not acquire a second safety semantics or public certificate constructor.
- [X] T038 [US2] Extend actual concrete mutation checks in `specs/511-safety-certification-scale/experiments/proof_integrity_controls.py` for wrong identities, omitted obligations, altered representation and weakened final statements under the selected contract; validate imports and retain passing controls plus rejected proof-hole/custom/native-axiom cases.
- [X] T039 [US2] Run separate-process installed-consumer checking with `specs/511-safety-certification-scale/experiments/cold_runner.py` on small valid/invalid cases while denying access to producer work directories/results; retain complete inventory/policy/proof checks in `specs/511-safety-certification-scale/evidence/independent-checking/`.
- [X] T040 [US2] Finalize the implemented input/result, data-only execution, independent coverage, supported-version and producer/checker responsibilities in `specs/511-safety-certification-scale/contracts/certification-and-evidence.md`; state that rejecting one witness proves neither unsafety nor absence of another valid witness.

**Checkpoint**: complete exact-request checking, encoding, contract versions and mandatory integrity are integrated after their red controls. No final scale acceptance yet.

## Phase 5: US1 — Certify supported modules beyond the observed failure (P1)

**Goal**: a complete installed certification/checking chain for A256/A512.
**Independent test**: full exact requests receive certificates and exported evidence passes checking
in a separate consumer. Installed-package and required-test integration precede final measurements.

- [X] T041 [P] [US1] Add complete-chain regression expectations for A256/A512 in `specs/511-safety-certification-scale/experiments/tests/test_scale_acceptance.py`, invoking the external runner with exact frozen requests and requiring actual opaque certificate and consumer success; retain released failures as red evidence and never replace formal execution with mocks in this acceptance test.
- [X] T042 [US1] Implement the selected scalable data-construction path in `crates/behavior-models/src/safety/proof_data.rs` and `crates/behavior-models/src/safety/certify.rs`; remove demonstrated incidental depth/encoding work without omitting identities, operations, policies or required checked interpretation.
- [X] T043 [US1] Implement selected fallible proof-data construction in `crates/behavior-models/src/safety/certify.rs`, keeping search/construction separate from independent acceptance; preserve deterministic relevant producer inputs and do not require an unnecessary formal proof of this untrusted algorithm.
- [X] T044 [US1] Integrate the full production chain in `crates/behavior-models/src/safety/certify.rs` with generic sources, concrete instances, exact-type/axiom checks, fresh replay and evidence persistence; keep the public producer and independent-checker paths consistent without trusting each other's previous success.
- [X] T045 [US1] Extend installed-wheel coverage in `release/safety_smoke.py` for the selected independent-checking contract, packaged trusted sources, exact expected policies and no supplied-code execution outside the checkout; retain ordinary Core execution/replay with Lean absent and no access to producer proof workspaces.
- [X] T046 [US1] Write and observe failing gate-collection/exit-status controls in `scripts/tests/test_safety_scale.sh`: the existing gates must execute fast tests under `specs/511-safety-certification-scale/experiments/tests/`, a failing fast test must propagate nonzero, the real `test_scale_acceptance.py` must be excluded from fast collection, and missing full-scale cases/reports must not be accepted. Separate fast harness tests from the real mandatory expensive corpus run; fake subprocess controls never count as proof evidence.
- [X] T047 [US1] After the preceding gate-collection tests fail as intended, integrate fast runner/generator/observation tests into the existing pytest gate in `scripts/gates.sh`, explicitly excluding `specs/511-safety-certification-scale/experiments/tests/test_scale_acceptance.py` from that fast invocation; update `scripts/check-safety-proofs.sh`, `scripts/tests/test_safety_proofs.sh` and `scripts/tests/test_safety_package.sh` only as needed for all mapped obligations. Document the mandatory full-scale invocation of `specs/511-safety-certification-scale/experiments/tests/test_scale_acceptance.py`, driving the external `specs/511-safety-certification-scale/experiments/cold_runner.py`, separately in `specs/511-safety-certification-scale/quickstart.md`; missing cases, omitted execution, incomplete reports and failed tests must fail the corresponding gate. Keep full-scale evaluation mandatory at G3 without creating a parallel release workflow.
- [X] T048 [US1] Execute complete A256 and A512 certification/check pairs using `specs/511-safety-certification-scale/experiments/cold_runner.py` under the frozen profile and save all stages in `specs/511-safety-certification-scale/evidence/scale-smoke/`; a larger limit alone or a passing data/uniqueness component does not satisfy this task.
- [X] T049 [US1] Execute paired large-case omission/identity/unauthorized-operation controls through `specs/511-safety-certification-scale/experiments/bundle_controls.py`, adapted only to the selected contract; record results in `specs/511-safety-certification-scale/evidence/scale-controls/` and verify no optimization loses full-request coverage.
- [X] T050 [US1] Record whole-chain success, exact-request identities, all required checked obligations and remaining capacity work in `specs/511-safety-certification-scale/remediation-assessment.md`; link producer/checker results separately and do not declare the feature complete before US4/US5.

**G2 / MVP checkpoint**: US3 + US2 + US1 deliver a complete checked remedy for both mandatory references, with installed-package checking and test wiring. Full three-pair cold/mixed acceptance remains US4.

## Phase 6: US4 — Know capacity, costs and failure boundaries (P2)

**Goal**: repeated measured capacity under fixed conditions using an exact frozen candidate.
**Independent test**: every frozen accepted case certifies/checks within separate whole-chain
budgets; failed/unrun obligations remain explicit and negative controls never certify the request.

- [X] T051 [P] [US4] Extend fault-path tests in `specs/511-safety-certification-scale/experiments/tests/test_cold_runner.py` for interrupted stages, over-budget completed valid proofs, missing counters, incomplete reports and post hoc profile/corpus changes; assert nonzero operational failure without relabeling valid completed proof as a counterexample.
- [X] T052 [P] [US4] Add deterministic semantic-vs-observation tests in `crates/behavior-models/tests/safety_reproducibility.rs` for identical request/config/witness inputs, different valid witness artifact identities where applicable, incidental paths/timings and irrelevant representation size; exact BehaviorHash changes must still remain distinguished.
- [X] T053 [US4] Complete whole-chain reporting and profile validation in `specs/511-safety-certification-scale/experiments/cold_runner.py` for T051, preserving actual per-stage control scope, process-tree measurement method, startup/persistence costs and installation/cache declarations; distinguish draft from frozen profiles and never count configuration as measured consumption. Preserve the frozen measurement contract; any material method change requires an explicit profile revision and renewed comparable baseline observations before final evaluation.
- [X] T054 [US4] Extend actual measurement/case aggregation in `specs/511-safety-certification-scale/experiments/collect_observations.py` to retain all failures, repeats, exact input identities, six dimension values and producer/checker totals independently; reject incomplete accepted-case reports and keep timings out of semantic evidence. Add and observe failing aggregation/report-completeness tests in `specs/511-safety-certification-scale/experiments/tests/test_collect_observations.py` before changing the collector.
- [X] T055 [US4] Build/install the final candidate and freeze its exact wheel checksum, source-tree identity, release/checker/statement/evidence versions, proof sources and dependency/toolchain identities in `specs/511-safety-certification-scale/evidence/candidate-builds/`; require completed version, packaging and gate-integration work and a validated frozen measurement method before any final run. Bind the manifest to `evidence/evaluation-profile.json` in the feature directory without changing its budgets. Any later relevant package/source/proof/checker/toolchain change requires a new build manifest and complete final reevaluation; changes to acceptance conditions require a separately reviewed profile revision.
- [X] T056 [US4] Execute the mandatory real `specs/511-safety-certification-scale/experiments/tests/test_scale_acceptance.py` entry point, driving `specs/511-safety-certification-scale/experiments/cold_runner.py` over the frozen six-axis corpus and material interactions sequentially, saving every repeated construction/checking attempt in `specs/511-safety-certification-scale/evidence/final-runs/`; include mixed FO/LD, multiple/shared dependencies, generated actions and all accepted profile cases without removing slow failures. Use only the candidate artifact fixed by the immediately preceding candidate-freeze gate and bind every report to both profile and build-manifest identities.
- [X] T057 [US4] Verify three consecutive sequential cold certification/check pairs for each A256/A512 against their separately frozen total budgets in `specs/511-safety-certification-scale/evidence/final-runs/`; record every failed attempt and require a complete new evaluation sequence after a candidate-build/profile revision rather than selecting isolated passing runs.
- [X] T058 [US4] Run all paired duplicate/key-law/permission/guard/binding/coverage/identity/proof mutations against the frozen evaluation requests with `specs/511-safety-certification-scale/experiments/bundle_controls.py` and `specs/511-safety-certification-scale/experiments/proof_integrity_controls.py`; save valid/invalid outcomes in `specs/511-safety-certification-scale/evidence/final-controls/` and reject bad imported controls as invalid evidence.
- [X] T059 [US4] Repeat identical complete accepted requests/configurations and selected equivalent-premise representations using `specs/511-safety-certification-scale/experiments/cold_runner.py`; compare canonical semantic evidence with `crates/behavior-models/tests/safety_reproducibility.rs` and retain results in `specs/511-safety-certification-scale/evidence/reproducibility/` without assuming all different valid witnesses share one artifact identity.
- [X] T060 [US4] Analyze six-axis construction/checking growth, data/proof/audit/fresh/dependency costs, causal improvements and measured residual limits in `specs/511-safety-certification-scale/scalability-results.md`; distinguish necessary mathematics from implementation overhead and avoid unsupported asymptotic or ARM performance claims.
- [X] T061 [US4] Assess every frozen accepted case against separate complete-chain budgets in `specs/511-safety-certification-scale/remediation-assessment.md`; report proof validity, supported-language coverage, proved completeness conditions and empirical practical capacity separately, with explicit uncertainty and no unexplained passing ceiling.
- [X] T062 [US4] Update `specs/511-safety-certification-scale/contracts/evaluation.md` and `specs/511-safety-certification-scale/quickstart.md` with the actual reproducible harness commands, fixed environment/corpus identities, measured limitations and cold definition; do not rewrite prior profiles or describe an installed dependency cache as a cold OS.

**G3 checkpoint**: all accepted frozen cases meet both budgets; A256/A512 each pass three consecutive sequential cold pairs. Six-axis/mixed observations, controls and the exact evaluated-build identity are complete.

## Phase 7: US5 — Preserve applications and delivery contracts (P2)

**Goal**: unchanged supported v1 language, historical records, Core state and replay.
**Independent test**: regression, installed package, platform and existing delivery gates pass for
the evaluated build. Relevant late changes force reevaluation before completion.

- [X] T063 [P] [US5] Extend state-transparency tests in `python/tests/test_safety_transparency.py` for ordinary IR, BehaviorHash, Model receipts, transition results and Core Store replay with/without policies and without proof tools/artifacts; preserve separate snapshot/activation/genesis/migration obligations rather than implicitly certifying them.
- [X] T064 [P] [US5] Add explicit supported/incompatible historical-contract tests in `crates/behavior-models/tests/safety_evidence.rs` and version/identity tests in `crates/behavior-models/tests/safety_identity.rs`; reject stale/mismatched artifacts without rewriting them, claiming unsafety or automatically executing legacy verifier components.
- [X] T065 [US5] Evaluate the complete valid v1 regression corpus from `specs/511-safety-certification-scale/evidence/v1-regression-corpus.json` under unchanged assumptions and the frozen profile, saving results in `specs/511-safety-certification-scale/evidence/v1-regression/`; no formerly supported construct may become Unsupported or require a new domain restriction to make scaling pass.
- [X] T066 [US5] Run available supported-platform correctness/reproducibility/package checks through the existing `scripts/gates.sh` and toolchain profiles in `crates/behavior-models/proofs/toolchain.json`; save actual platform/tool identities in `specs/511-safety-certification-scale/evidence/platform-checks/` and explicitly report missing platform execution rather than inventing ARM performance evidence.
- [X] T067 [US5] Run `scripts/gates.sh` with `BEHAVIOR_FULL_PROPERTIES=1`, `scripts/conformance.sh 506 complete`, `scripts/conformance.sh 507 complete` and `scripts/release-check.sh` with existing reproducibility/artifact/installed-package checks; preserve logs and artifact identities in `specs/511-safety-certification-scale/evidence/delivery-gates/`, sequentially outside benchmark runs and with no Core override or tag publication. Verify the delivered wheel/source/checker/proof identities match the frozen evaluated build; any mismatch invalidates its final acceptance evidence and requires reevaluation against the same acceptance conditions (or an explicitly revised profile), never silent substitution.
- [X] T068 [US5] Record unchanged established fixtures/receipts, actual supported-platform results, declared evidence contracts and separate initial-state/migration/module-authorization/activation obligations in `specs/511-safety-certification-scale/remediation-assessment.md`; distinguish immutable historical reproducibility from continuing trust in an older implementation.

**G4 checkpoint**: required compatibility/platform/package/delivery checks pass and delivered artifact identities match the evaluated candidate. Missing mandatory execution or changed candidate identities cannot be waived by recording a limitation.

## Phase 8: Cross-cutting documentation and final acceptance

**Goal**: precise consumer guidance and traceable final acceptance without substituting artifacts.

- [X] T069 [P] Update consumer-facing Safety guarantees, mathematical/trusted boundaries, actual cold costs/capacity and versioned evidence guidance in `models/examples/safety_models/README.md` and `docs/versioning.md`; keep unsupported shapes, bad witnesses, exhaustion and safety violations distinct and never present INCONCLUSIVE as proven. Documentation-only updates cannot silently substitute a rebuilt artifact for the frozen evaluated package; apply the candidate-freeze mismatch rule when package contents change.
- [X] T070 [P] Complete the FR-001–030 / SC-001–011 evidence matrix in `specs/511-safety-certification-scale/acceptance-evidence.md`, linking exact formal roots, controls, complete profiles/runs and delivery logs; label proven claims, structural/implementation checks, empirical measurements and host assumptions separately.
- [X] T071 Execute the documented existing/new validation steps in `specs/511-safety-certification-scale/quickstart.md` from an installed pinned package and fresh output locations; record results in `specs/511-safety-certification-scale/evidence/quickstart-validation/` and correct inaccurate commands without weakening required checks.
- [X] T072 Finish `specs/511-safety-certification-scale/remediation-assessment.md` and update `specs/511-safety-certification-scale/tasks.md` only for actually completed work; audit scope/immutability, all gates, exact complete coverage, soundness/completeness/cost distinctions and remaining nonblocking limitations. Do not call the feature complete with an unresolved frozen-case failure, missing proof obligation, unexplained ceiling or unmeasured mandatory gate.

**Completion**: all acceptance evidence is complete for the exact delivered candidate; no publication is implied.

## Dependencies and Execution Order

```text
Setup T001–T003
  -> G0/G1 + release-version decision T004–T015
  -> US3: checked mathematics T016–T025
  -> US2: red controls -> identities/encoding/checker T026–T040
  -> US1: complete installed chain + mandatory test wiring T041–T050
  -> US4: measurement readiness -> candidate freeze -> final runs T051–T062
  -> US5: unchanged-candidate delivery validation T063–T068
  -> Final acceptance T069–T072
```

- T028 and T029 are red controls before T030 and T031–T035. In particular,
  T034 cannot begin until the encoding/index tests in T029 have failed as intended.
- Version metadata T015 precedes candidate builds; numeric checking/statement/evidence transitions
  T030 precede changed contract integration. Final source/package identities come from the build,
  not speculative hashes fixed before implementation.
- Installed-package coverage T045, red gate controls T046 and gate wiring T047
  precede smoke runs T048. Measurement work T053–T054 then precedes the exact candidate
  freeze T055; no final evaluation T056–T059 may run before that freeze.
- T067 compares delivered artifacts with T055's manifest. Changed wheel/proof/checker/source
  identities invalidate the earlier final acceptance and require a new candidate plus complete final
  runs. Changed budgets/corpus/environment/method require a reviewed profile revision and comparable
  baseline observations; both old profiles and runs are retained.
- Fast tests under the feature's `experiments/tests/` must be collected by existing gates after
  T047. The actual expensive full-scale run T056 and report validation T057–T061 are
  separately mandatory G3 checks, with explicit failure for missing cases/reports or omitted execution.
- T057 may assess already recorded protocol-compliant repeats from T056; it need not duplicate
  heavy execution. Reuse observations, never prior acceptance instead of required proof checking.
- Every implementation correction follows its applicable red controls. No two heavy measurements,
  proofs or gates overlap. No agent spawning is required by these scheduling examples.

## Parallel Opportunities by Story

| Story | Independent file-authoring examples | Serial boundary |
| --- | --- | --- |
| US3 | T016 Lean controls and T017 Rust integrity controls | Proofs/audits follow tests; actual proof runs are sequential. |
| US2 | T026, T027, T028, T029 in distinct test files | All applicable controls precede changed identities/encoding/checking. |
| US1 | T041 acceptance-test authoring and preparation of installed-package test expectations | Package, gate wiring and actual scale runs follow chain integration. |
| US4 | T051 harness controls and T052 semantic reproducibility tests | Freeze the completed candidate before final sequential measurements. |
| US5 | T063 transparency and T064 historical/identity tests | Delivery gates may not silently modify the evaluated artifact. |

T002/T003 and T069/T070 also touch different files at their phase checkpoints.
`[P]` never bypasses a dependency or permits simultaneous heavy executions.

## Implementation Strategy and Stop Conditions

1. Complete G0/G1 investigation and profile/contract decisions before production integration;
   no additional problem-definition interview is needed. Choose representation from evidence.
2. Deliver US3 -> US2 -> US1 as the smallest meaningful remedy: complete certification and
   independent checking for both references. A passing component or raised limit is insufficient.
3. Complete the measured US4 envelope and US5 compatibility/package requirements before calling
   Feature 511 done. Report soundness, language/completeness and computational capacity separately.
4. If public Core information is insufficient, document the exact missing data and stop the affected
   route; no private Core access, language expansion, new domain restriction or weaker theorem.
5. Preserve all failures, published records and unrelated work. A candidate/profile revision cannot
   hide omissions, failed obligations, Unsupported relabeling or an unexplained practical ceiling.
6. No merge, tag push or publication. Delivery checks verify the same candidate that passed final
   evaluation; rebuild differences trigger the explicit reevaluation rule.

## Requirement Coverage

| Requirement | Tasks / acceptance evidence |
| --- | --- |
| FR-001 | T002, T012, T026, T031, T034, T035 |
| FR-002 | T065 |
| FR-003 | T032, T034, T055 |
| FR-004 | T025, T063, T068 |
| FR-005 | T019, T034, T029 |
| FR-006 | T020, T029 |
| FR-007 | T020, T028 |
| FR-008 | T020, T026 |
| FR-009 | T013, T014, T018 |
| FR-010 | T021, T035 |
| FR-011 | T022, T023, T035, T047 |
| FR-012 | T016, T017, T022, T024 |
| FR-013 | T012, T025, T060 |
| FR-014 | T041, T048, T056, T057 |
| FR-015 | T011, T055, T056, T057 |
| FR-016 | T006, T007, T056, T060 |
| FR-017 | T005, T053, T054, T060 |
| FR-018 | T003, T005, T010, T054 |
| FR-019 | T008, T009, T036, T051, T053 |
| FR-020 | T016, T026, T038, T049, T058 |
| FR-021 | T052, T059 |
| FR-022 | T063, T045 |
| FR-023 | T014, T064, T030, T015, T055, T067 |
| FR-024 | T046, T047, T066, T067 |
| FR-025 | T012, T013, T014, T043 |
| FR-026 | T021, T026, T031, T033, T028 |
| FR-027 | T027, T032, T035 |
| FR-028 | T004, T005, T011, T053, T055 |
| FR-029 | T011, T066 |
| FR-030 | T002, T065, T068 |
| SC-001 | T048, T056, T057 |
| SC-002 | T026, T031, T034 |
| SC-003 | T018, T021, T025 |
| SC-004 | T038, T049, T058 |
| SC-005 | T056, T060 |
| SC-006 | T052, T059 |
| SC-007 | T063, T065, T045, T047, T066, T067, T071 |
| SC-008 | T050, T060, T061, T068, T069, T070, T072 |
| SC-009 | T021, T033, T028 |
| SC-010 | T011, T055, T056, T061, T062 |
| SC-011 | T002, T065, T068, T070 |
