# Tasks: Safety Models — FieldOwnership and LifecycleDependency

**Branch**: `510-safety-models` | **Generated**: 2026-10-18

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [authoring contract](contracts/authoring-and-certification.md),
[proof contract](contracts/proof-contract.md), [evidence/deployment contract](contracts/evidence-and-deployment.md),
[quickstart.md](quickstart.md), and constitution 2.0.0.

**Tests**: Explicitly required by FR-032–FR-036, the accepted proposal and the constitution.
For every implementation change, write/review the associated controls and observe the intended
failure before making them pass. A missing tool is not a substitute for the intended red result.
Record red/green commands and results under `target/safety-models/`; no tests are executed by
generating this file. Deliberately invalid proofs must fail while their positive controls pass.

**Organization**: Four P1 stories, ordered by their actual dependencies. Shared proof/evidence
machinery needed for the FO milestone is delivered in US1; US3 completes its public verification
and reproducibility workflow. No dependency on unfinished US3 work may defer US1 proof checking.

## Format and Path Conventions

- Tasks use `- [ ] Tnnn [P?] [USn?] description with exact repository-relative paths`.
- `[P]` identifies independent file sets within a named eligible batch; it never bypasses a gate.
- Source files not yet present are planned additions. Existing files retain their public contracts.
- Runtime evidence/build products belong under `target/safety-models/`, not established fixtures.
- Preserve Core 0.15.0 commit `6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier 0.10.0,
  existing receipt formats and established compatibility records. Do not modify unrelated
  `.specify/`/`.agents/` changes, remove uncommitted work or introduce new Core semantics.
- No release, tag, publication, external production application or general proof framework task.

## Phase 1: Setup — Pinned Environment and Baseline

**Goal**: Establish reproducible prerequisites and preserve the review baseline.

- [X] T001 Record the existing worktree, pin metadata, established compatibility/receipt bytes and available disk space in `target/safety-models/baseline/manifest.json`, using `core-release.json`, `Cargo.toml`, `Cargo.lock`, `models/fixtures/compat.json` and the established reference builders; identify pre-existing user changes without rewriting or cleaning them.
- [X] T002 Write and observe failing tool-selection controls in `scripts/tests/test_safety_toolchain.sh` for the locked Lean package/output, actual derivation identity, wrong PATH executables and forbidden overrides; use controlled substitutes only to test rejection, never as successful proof evidence.
- [X] T003 Add locked `pkgs.lean4` 4.30.0 to the existing shell and expose its package identity in `flake.nix`; make T002 pass, verify both existing target derivations resolve, retain `flake.lock`, and use bundled `leanchecker` without elan, another toolchain manager or an additional proof library.

**Checkpoint**: Expected tool identities are available or explicitly blocked by environment
failure; the baseline and user work are preserved. No proof success is claimed yet.

## Phase 2: Foundational — Complete Finite Data Boundary

**Goal**: Supply only the shared typed inventory/identity boundary needed by both concrete patterns.
No LD implementation or generic framework belongs in this phase.

### Tests first

- [X] T004 [P] Write and observe failing typed-boundary tests in `crates/behavior-models/tests/safety_input.rs` for explicit lowered/uncertified state, well-formed action/field references, duplicate/ambiguous data rejection and typed errors; reject caller-supplied certification flags rather than promoting them to a certificate.
- [X] T005 [P] Write and observe failing complete-inventory tests in `crates/behavior-models/tests/safety_inventory.rs` covering every final handwritten/generated action, secondary/renamed bindings, assignment/create/remove occurrences, irrelevant actions, omitted-action/effect mutations and Core's State-alias refusal.
- [X] T006 [P] Write and observe failing canonical identity tests in `crates/behavior-models/tests/safety_identity.rs` for exact Core/module binding, deterministic order, JSON type preservation and the constraint “Tagged canonical identity of the complete finite inventory payload, excluding its own identity field.”

### Implementation

- [X] T007 Introduce the finite ModuleData/ActionData/Operation and uncertified-analysis types in `crates/behavior-models/src/safety/input.rs` and `crates/behavior-models/src/safety/mod.rs`, register the module in `crates/behavior-models/src/lib.rs`, and add typed errors in `crates/behavior-models/src/error.rs`; keep safety data separate from existing BuildOutput and receipts, with no LD semantics yet.
- [X] T008 Extract the full admitted action/parameter/precondition/assignment/create/remove inventory through public Core accessors in `crates/behavior-models/src/safety/inventory.rs`; add a separate comparison against the actual action set and operation occurrences, preserve unknown forms as blocking data/errors, and do not accept Rust-supplied relevance verdicts as proof inputs.
- [X] T009 Implement deterministic inventory encoding and tagged identity in `crates/behavior-models/src/safety/identity.rs`, using Core's public canonical/hash APIs, resolved entity declaration plus field key, sorted sets and self-hash exclusion; make T004–T006 pass without copying Core hash encodings or treating display/parameter spelling as authorization.

**Checkpoint**: Foundation tests pass and no mutation/action can silently disappear. This is
implementation correspondence evidence; extraction remains a named trusted boundary.

## Phase 3: US1 — Protect State with Auditable Ownership (P1, MVP)

**Goal**: Deliver usable FieldOwnership, its generic theorem, each concrete proof instance,
proof integrity and two neutral examples before starting LifecycleDependency.

**Independent test**: `fo_field` and `fo_state_machine` certify with complete inventories;
unauthorized/no-op/generated writers and unauthorized creators/removers fail. Every required
proof-integrity and omission control blocks certification. LD is not needed for this milestone.

### Tests first

- [X] T010 [P] [US1] Write and observe failing ordinary-field and actual-Core correspondence controls in `crates/behavior-models/tests/safety_ownership.rs`: reject unreachable/value-preserving unauthorized assignments, renamed/secondary-binding writers, malformed policies and unauthorized creation/removal; permit authorized creation initialization independently of writer permission and exercise all protected-field frame cases.
- [X] T011 [P] [US1] Write and observe failing composition controls in `crates/behavior-models/tests/safety_composition.rs` for intersecting Safety permissions and existing StateMachine/RequestFlow/Approval/WorkQueue ownership, including another Model's generated writer; compare admitted IR and legacy receipt bytes with the same module built without Safety policies; additionally enforce FR-038 in `python/tests/test_safety_transparency.py` by reopening/replaying exported Core IR/history with no policy/certificate access or Lean executable.
- [X] T012 [P] [US1] Write and observe failing positive-plus-mutation harnesses in `crates/behavior-models/tests/safety_proof_integrity.rs` and `crates/behavior-models/proofs/safety/tests/OwnershipMutations.lean` for missing essential FO premises, omitted actions/effects, wrong concrete theorem/data, proof holes, custom/native axioms, missing tools and resource exhaustion; require a valid control so blanket failure cannot pass the harness.
- [X] T013 [P] [US1] Write and observe failing public authoring tests in `python/tests/test_safety_authoring.py` for FieldOwnership/action selectors, thin Rust delegation, lowered versus certified outcomes, unchanged existing receipts and ordinary module execution without Lean; include malformed/default/empty permission cases.

### Implementation and formal proof

- [X] T014 [US1] Add FieldOwnership input and final-module selector resolution in `crates/behavior-models/src/safety/input.rs` and `crates/behavior-models/src/safety/resolve.rs`: enforce “Unique authoring reference; diagnostics/selection only, not permission by spelling.”, “Existing entity handle/name resolved to its admitted declaration identity.” and “Field-key → explicit action selector set. Each field must exist. Empty set forbids all assignments.”; resolve authored handles/exact names and generated `(model_id, local_action_id)` pairs, rejecting missing/stale/ambiguous selectors.
- [X] T015 [US1] Implement structural authority in `crates/behavior-models/src/safety/field_ownership.rs`: enforce “Explicit action selector set permitted to create that entity.” and “Explicit action selector set permitted to remove that entity.” independently of writers; omitted permissions are empty, all overlapping restrictions intersect, initialization uses creator authority, and unauthorized assignments fail regardless of reachability/value change with deterministic action/field diagnostics.
- [X] T016 [US1] Add the safety-aware composition entry point in `crates/behavior-models/src/kinds.rs` and `crates/behavior-models/src/lib.rs` after final admission and all existing ownership passes; preserve old `compose_models`, BuildOutput and receipt contracts, return separate uncertified analysis, reject structural violations, and prove Safety policies never rewrite the emitted admitted module.
- [X] T017 [US1] Add FieldOwnership's canonical resolved Model document, policy-set identity and version constants in `crates/behavior-models/src/safety/identity.rs` and `crates/behavior-models/src/safety/mod.rs`, honoring “semantic version 1 and checker/lowerer version 1”; resolve identities before hashing, exclude display/path/timing data, and leave established Model documents and fixtures unchanged.
- [X] T018 [US1] Define typed finite state, action inventories, assignment/create/remove and frame semantics in `crates/behavior-models/proofs/safety/Data.lean`; review/version the actual FO statement against `specs/510-safety-models/contracts/proof-contract.md`, with no LD theorem, domain-specific axiom or assumed ownership conclusion.
- [X] T019 [US1] Prove FO-1, assignment/create/remove authority and protected-change/frame witness lemmas in `crates/behavior-models/proofs/safety/FieldOwnership.lean` for arbitrary finite states; creation initialization must not require writer permission, and tests/SMT examples must not stand in for the proof.
- [X] T020 [US1] Implement the finite FO reflective checker and soundness theorem in `crates/behavior-models/proofs/safety/Check.lean`, visiting every action × policy and all operation occurrences, deriving permission/irrelevance checks, and yielding a concrete `checkModule data = true` proof plus derived theorem for the exact data rather than trusting an acceptance flag from Rust.
- [X] T021 [US1] Implement deterministic proof-data generation and retained/embedded FO proof sources in `crates/behavior-models/src/safety/certify.rs`; emit fixed identifiers/constructors and validated numeric byte/index data, bind BehaviorHash/policy set/full inventory in the checked record, and prevent authored names from becoming Lean code, imports or paths.
- [X] T022 [US1] Implement structured expected-theorem-type and transitive axiom auditing in `crates/behavior-models/proofs/safety/Audit.lean`, allowing only `propext`, `Classical.choice`, `Quot.sound` for generic and concrete roots; reject sorryAx/custom/native axioms and wrong/weakened theorem propositions without parsing human-readable `#print` output.
- [X] T023 [US1] Implement the pinned proof runner in `crates/behavior-models/src/safety/certify.rs`: verify expected Nix tool identities, use controlled sources/search paths, compile/kernel-check each concrete FO instance, run the structured audit and bundled `leanchecker --fresh`, and return noncertifying results on missing tools, wrong derivations, resource limits or any unsuccessful check; use no native-oracle fallback and launch no Lean at Store runtime.
- [X] T024 [US1] Implement the FO assessment/certificate boundary in `crates/behavior-models/src/safety/evidence.rs`, including the complete module/policy/inventory/proof identities, all obligations, exact assumptions/exclusions and initial validity unestablished; preserve all four assessment outcomes and construct a certificate only after T020–T023 and independent inventory comparison succeed.
- [X] T025 [US1] Expose FO safety-aware lowering and explicit Rust certification through `crates/behavior-py/src/safety.rs`, `crates/behavior-py/src/models.rs` and `crates/behavior-py/src/lib.rs`, preserving existing Model/receipt decoding and keeping loaded JSON or a caller-supplied status from constructing a verified certificate.
- [X] T026 [US1] Add FieldOwnership/GeneratedAction recording objects, `BehaviorModule(safety=...)`, uncertified analysis and `certify_safety` delegation in `python/behavior/safety.py`, `python/behavior/module.py`, `python/behavior/__init__.py` and `python/behavior/_engine.pyi`; make T013 pass without Python normalization/ownership semantics or a Lean requirement for ordinary no-policy builds.
- [X] T027 [US1] Add the two public-API positive/negative reference cases in `models/examples/safety_models/fo_field.py` and `models/examples/safety_models/fo_state_machine.py`, including authorized assignments/initialization/removal, denied extra writers and actual unchanged Core behavior under the added policies.
- [X] T028 [US1] Add `models/examples/safety_models/run.py` with the FO cases, `--output` and explicit `--certify`; absence of certification must report Lowered/uncertified, requested certification must fail nonzero on any required non-success, and a request with no Safety property must not issue a misleading certificate.
- [X] T029 [US1] Run the FO abstract/concrete premise and proof-integrity mutation matrix from `crates/behavior-models/tests/safety_proof_integrity.rs`, record each removed premise and violating witness in `target/safety-models/fo/mutations.json`, and verify every declared trusted boundary remains an assumption rather than an asserted proof.
- [X] T030 [US1] Run the independent actual-Core ownership/inventory/composition controls from `crates/behavior-models/tests/safety_ownership.rs`, `crates/behavior-models/tests/safety_inventory.rs` and `crates/behavior-models/tests/safety_composition.rs`; make name/order variations preserve decisions and require byte identity only when Core semantic identities are unchanged, recording results in `target/safety-models/fo/correspondence.json`.
- [X] T031 [US1] Execute and record the FieldOwnership milestone in `target/safety-models/fo/milestone.json`: both FO reference cases certify, generic/concrete roots pass audit/fresh replay, all mutation/correspondence controls pass, existing compatibility checks retain baseline bytes, and proof time/memory/tool identities are measured; stop dependent work on any unmet requirement.

**FO STOP GATE**: T031 must pass before any US2 task, including probe implementation. This
milestone is the MVP; it is not completion of Feature 510. Record a precise blocker rather
than changing the Core pin, weakening a premise or broadening the admitted expression language.

## Phase 4: US2 — Preserve Parent/Child Lifecycle Relationships (P1)

**Goal**: Certify the explicit restricted pattern, including every affected dependency and
isolated lifecycle operation, with canonical guards checked through public Core APIs.

**Independent test**: Neutral parent/child and shared-parent examples certify, execute useful
safe transitions and refuse unsafe ones. Missing/weakened guards, mixed operations, role
confusion, owner assignment and unsupported values cannot obtain certification. VM is additional.

### Public-Core feasibility first

- [X] T032 [US2] After T031, write and observe failing public-facade correspondence tests in `crates/behavior-models/tests/safety_correspondence.rs` for declaration/Ref identity reconstruction, expected derived-body hashes, State-role anchors, mandatory precondition roots, literal destinations and bound-parent owner initialization; include wrong binding/type/field/query/literal/role, nested/OR-weakened guards, input-ID overlap and explicit empty-membership forms.
- [X] T033 [US2] Implement only the canonical probe adapter in `crates/behavior-models/src/safety/guards.rs` using public Builder and admitted-module accessors, fresh declaration reconstruction and role equality; preserve all relevant declaration identities, compare whole precondition roots/RHSs/initializers, use no private variants, guard-Wire inspection, Debug/discriminant assumptions or copied Core hashes, and never emit scratch probes into the authored module.
- [X] T034 [US2] Run every required canonical form and adversarial case from `crates/behavior-models/tests/safety_correspondence.rs`, recording the exact public operations and identities in `target/safety-models/ld/public-core-preflight.json`; stop before T035–T054 if any supported guard/role/initializer cannot be checked through Core 0.15.0's public facade, and report the precise gap instead of silently extending trust or scope.

**PUBLIC-CORE STOP GATE**: T034 must pass before LD semantic/checker implementation.
Successful probe tests do not prove the Rust-to-Core correspondence itself.

### Tests first after preflight

- [X] T035 [P] [US2] Write and observe failing LD policy/operation controls in `crates/behavior-models/tests/safety_lifecycle.rs` for required Ref versus plain ID/optional owner, enum/control-policy validity, immutable owner including same-value writes after terminality, literal-only values, union operation counts, shared-parent obligations and mixed parent/child roles represented by a single operation.
- [X] T036 [P] [US2] Write and observe failing independent actual-Core state/transition and conflict controls in `crates/behavior-models/tests/safety_exhaustive.rs` and `crates/behavior-models/tests/safety_transitions.rs` for every supported operation, prohibited-parent creation/reactivation, terminal-child references, unsafe removals, source-guard invalidation, simultaneous effects, stale candidates and re-evaluation; the oracle must not call the Safety property checker. Extend `python/tests/test_safety_transparency.py` to LD histories: no Safety registry, event stream or proof dependency may enter Core state or behavior replay (FR-038).
- [X] T037 [P] [US2] Write and observe failing LD proof-premise controls in `crates/behavior-models/tests/safety_lifecycle_proofs.rs` and `crates/behavior-models/proofs/safety/tests/LifecycleMutations.lean`, with positive controls and violating witnesses for missing guard/binding/freshness/frame/Ref/one-operation premises, omitted actions and failure to check another affected dependency.
- [X] T038 [P] [US2] Write and observe failing Python LD recording/contract tests in `python/tests/test_safety_lifecycle.py` for explicit ownership references, unsupported owner arrangements/guards/values, unchanged generated IR and all five eventual case identifiers; no test may require a production pool-owned schema to adopt the VM fixture's ownership model.

### Implementation and formal proof

- [X] T039 [US2] Add LD input/resolution and versioned identity in `crates/behavior-models/src/safety/input.rs`, `crates/behavior-models/src/safety/lifecycle_dependency.rs` and `crates/behavior-models/src/safety/identity.rs`, enforcing “Unambiguous reference for diagnostics.”, “Existing resolved entity declarations.”, “Child's nonoptional Ref to this parent type, verified by type and Ref marker.”, “Existing enum lifecycle fields.”, “Valid enum member sets, normalized deterministically.” and “Applicable FieldOwnership declarations covering parent/child lifecycle fields, owner assignment, creation and removal.”; reject missing/cyclic/out-of-set policy references and keep effective owner writers empty.
- [X] T040 [US2] Derive typed dependency footprints and distinct operation occurrences in `crates/behavior-models/src/safety/lifecycle_dependency.rs` and `crates/behavior-models/src/safety/inventory.rs`: count creation once with initialization, each assignment separately including no-ops, shared operations once across policies, and FO-only writes outside the LD limit; retain every affected-policy obligation and reject mixed-role bypasses.
- [X] T041 [US2] Connect the demonstrated probes to actual operation/guard relationships in `crates/behavior-models/src/safety/guards.rs` and `crates/behavior-models/src/safety/lifecycle_dependency.rs`: require literal lifecycle hashes, correct State-parent captures and owner equality/initializer, actual mandatory roots and checked frame irrelevance for other effects; additional authored conditions may strengthen but never weaken the canonical guard.
- [X] T042 [US2] Extend the abstract state/transition contract from `crates/behavior-models/proofs/safety/Data.lean` in `crates/behavior-models/proofs/safety/LifecycleData.lean` with R, P and their conjunction across all policies, immutable required owners, freshness and complete resulting-state Ref validity; define the selected operation semantics and projection frames without assuming Q(S') or embedding VM-specific facts.
- [X] T043 [US2] Prove LD-1 and the configured-policy conjunction in `crates/behavior-models/proofs/safety/LifecycleDependency.lean` for arbitrary finite states, covering prohibited/permitted parent updates, terminal/nonterminal child updates, child/parent creation and isolated child/parent removal; preserve parent existence for terminal children and retain initial validity as a separate premise.
- [X] T044 [US2] Extend `crates/behavior-models/proofs/safety/Check.lean` with `LifecycleCheck.lean` and `LifecycleSequence.lean` to derive all action × dependency footprints, operation counts, canonical guard/effect premises and irrelevant-effect frames from the complete concrete data, require every affected dependency and FO restriction, and prove accepted instances satisfy LD-1's premises; reject Unsupported shapes rather than approximating them.
- [X] T045 [US2] Extend concrete proof generation/root-type manifests and mandatory auditing/replay in `crates/behavior-models/src/safety/certify.rs`, `crates/behavior-models/src/safety/evidence.rs` and `crates/behavior-models/proofs/safety/Audit.lean` so each LD certificate checks both generic theorems and the exact composed module/policy-set instance, with no placeholder or Rust-only acceptance path.
- [X] T046 [US2] Expose LD recording/resolution through `crates/behavior-py/src/safety.rs`, `python/behavior/safety.py`, `python/behavior/__init__.py` and `python/behavior/_engine.pyi`; make T038 pass while leaving validation, normalization and operation analysis in Rust and preserving all existing Model APIs.
- [X] T047 [US2] Implement the neutral parent/child reference in `models/examples/safety_models/ld_parent_child.py`, exercising safe child creation/reactivation, parent status changes, isolated parent creation, child removal and unreferenced-parent removal through public Core Store APIs, with canonical authored guards and explicit useful-success assertions.
- [X] T048 [US2] Implement `models/examples/safety_models/ld_shared_parent.py` with two child types and one parent lifecycle; show the parent operation counts once but needs both dependencies' guards, and ensure either active dependent prevents prohibition while safe terminal/empty cases succeed.
- [X] T049 [US2] Implement `models/examples/safety_models/ld_vm_resource.py` using immutable required Ref[VM], an existing VM even for terminal Resources and non-Removed/non-Abandoned VMs for nonterminal Resources; explicitly permit a terminal Resource referencing an existing Removed VM while rejecting physical deletion with surviving references, label this a restricted fixture, keep I-5b/I-6 unmapped, and infer neither live/nonterminal equivalence nor constraints on pool-owned production resources.
- [X] T050 [US2] Extend `models/examples/safety_models/run.py` to all five planned cases and `--case all`, emitting every case's actual assessment and failing requested certification if any required case is absent/noncertifying; keep lowering-only output explicitly uncertified and add no second semantic implementation.
- [X] T051 [US2] Run the independent enumeration in `crates/behavior-models/tests/safety_exhaustive.rs` with 0–2 parents, 0–2 children per child type, one/two child types, two parent/child lifecycle categories, all valid owner mappings and representative ID/name/order variants; cover every supported operation plus targeted mixed cases and publish exact bounds/counts in `target/safety-models/ld/coverage.json`, allowing safe Unsupported cases but zero unsafe certified commits.
- [X] T052 [US2] Execute the LD abstract/concrete proof-premise and inventory/guard mutation controls from `crates/behavior-models/tests/safety_lifecycle_proofs.rs` and `crates/behavior-models/tests/safety_lifecycle.rs`, recording each violating witness/check result in `target/safety-models/ld/mutations.json`; ensure omissions and weakened guards block certification rather than becoming documented optional assumptions.
- [X] T053 [US2] Execute actual commit/conflict/re-evaluation and full-result-state controls from `crates/behavior-models/tests/safety_transitions.rs`, including a same-action parent prohibition/nonterminal-child creation rejected by certification, terminal-child Ref blocking removal and stale candidate refusal followed by changed evaluation; record outcomes in `target/safety-models/ld/transitions.json` without changing Core semantics.
- [X] T054 [US2] Record the LD milestone in `target/safety-models/ld/milestone.json` only after all three LD cases, both generic proofs, every concrete instance, axiom/fresh replay checks, positive witnesses, exhaustive/mutation controls and FO regressions pass; retain exact correspondence assumptions and stop on any missing supported operation or unmet obligation.

**Checkpoint**: US2 is independently demonstrable using its neutral cases and required shared
US1 machinery. Five fixture cases do not imply production-domain invariant coverage.

## Phase 5: US3 — Inspect and Reproduce Safety Evidence (P1)

**Goal**: Complete the public sidecar verification, outcome and reproducibility workflow without
deferring any proof integrity already required by US1/US2.

**Independent test**: Reproduce and verify a valid bundle; reject tampered/stale/incomplete
bundles. Demonstrate all four assessment outcomes and changed policy identity with unchanged
Core behavior. Every report names extraction/probe/encoder assumptions and external boundaries.

### Tests first

- [X] T055 [P] [US3] Write and observe failing evidence/outcome contract tests in `crates/behavior-models/tests/safety_evidence.rs` for complete obligation/identity fields, all four outcomes, unverified-import versus verified-certificate types, lowered separation, initial validity unestablished, exact theorem/version bindings and absence of unconditional end-to-end claims.
- [X] T056 [P] [US3] Write and observe failing imported-bundle controls in `python/tests/test_safety_evidence.py` for edited module/policy set/inventory/proof inputs, omitted operations, forged certified status, missing/mismatched tools, wrong theorem statement and supplied Lean/.olean code; untrusted proof objects must never be executed as verification input.
- [X] T057 [P] [US3] Write and observe failing reproducibility/resource-packaging tests in `crates/behavior-models/tests/safety_reproducibility.rs` and `scripts/tests/test_safety_package.sh` for canonical ordering/types, own-hash exclusion, path/log/timing independence, changed proof/checker/tool identities, source-injection strings, empty-working-directory certification and installed proof-resource/tool selection without a repository checkout; ordinary installed execution must work without Lean.

### Implementation and integration

- [X] T058 [US3] Complete the versioned canonical payloads and distinct hash domains in `crates/behavior-models/src/safety/evidence.rs` and `crates/behavior-models/src/safety/identity.rs`, including every normative evidence field, full policy set, actual proof/type/source/tool identities, per-obligation results and explicit assumptions; keep diagnostic envelopes and compiled proof bytes out of semantic identity and exclude each document's own hash field.
- [X] T059 [US3] Implement bundle verification in `crates/behavior-models/src/safety/certify.rs`: re-admit transported Core IR, resolve policies, independently re-extract and compare the full inventory, regenerate fixed proof wrappers from validated data and pinned supported sources, check the exact propositions/axioms and freshly replay; reject stale or tampered bundles without trusting JSON status, arbitrary source or cached compiled proof objects.
- [X] T060 [US3] Finish retained/embedded proof-resource and source-manifest handling in `crates/behavior-models/src/safety/certify.rs`, with packaging declarations in `crates/behavior-models/Cargo.toml` only if needed; add a minimal public-API FO certification/no-Lean ordinary-execution smoke in `release/safety_smoke.py` through the existing `scripts/release-check.sh` package path, using explicit verified tool configuration outside its existing timed ordinary smoke; make T057 pass without repository-path assumptions, runtime downloads, another proof library, a new Cargo dependency or another release process.
- [X] T061 [US3] Expose readonly evidence/certificate wrappers and `verify_safety_evidence` delegation in `crates/behavior-py/src/safety.rs`, `python/behavior/safety.py` and `python/behavior/_engine.pyi`, preserving typed Rust failures and explicit initial-state/host boundaries; make T056 pass without a Python verifier or automatic governance authorization.
- [X] T062 [US3] Write and observe failing isolated-repository compatibility controls in `scripts/tests/test_models_safety_compat.sh` for new Safety tuples/current-built cases, unchanged legacy fixtures, accepted merge-base history and repeated branch-local version changes; new mappings must not bypass established supersession/coverage rules.
- [X] T063 [US3] Extend `scripts/check-models-compat.py` for new sidecar tuples and add only new v1 mappings in `models/fixtures/safety-compat.json`, checking ModelHash/policy-set/BehaviorHash/semantic-inventory identities with existing version/history rules; keep `models/fixtures/compat.json` and established builders byte-identical, verify proof dependency bindings separately, and baseline no paths/timings/compiled proof bytes.
- [X] T064 [US3] Add repeated canonical Safety output checks to the existing `scripts/determinism-check.sh`, using the Rust-authoritative reference runner and identical pinned reproduction inputs; make T055–T057 pass and verify same-Core/different-policy evidence differs while genuinely nonsemantic changes preserve bytes.
- [X] T065 [US3] Run and record US3's four outcomes, bundle re-verification, tamper controls, compatibility and repeated-output checks in `target/safety-models/evidence/milestone.json`; enumerate passed proof results separately from trusted code, genesis/migration/authorization assumptions and Unsupported limitations, with no known failed obligation hidden as an assumption.

**Checkpoint**: Verified artifacts are reproducible and auditable; hashing/importing a file is
not certification and changes to any bound proof/module/policy identity prevent stale reuse.

## Phase 6: US4 — Establish Exact State and Deployment Boundaries (P1)

**Goal**: Validate the original property through the existing public snapshot export and document
an enforced exact-history activation protocol, without introducing Core governance.

**Independent test**: Valid/invalid/untrusted complete-state inputs are distinguished; terminal
children require existing parents. Stale switch anchors, including unchanged-StateId history
advances, are refused by the reference host protocol. Migration/alternative-module execution
does not inherit an ordinary-transition certificate.

### Tests first

- [X] T066 [P] [US4] Write and observe failing Rust validator/record tests in `crates/behavior-models/tests/safety_snapshot.rs` for all dependency predicates, terminal-child parent existence, typed IDs/enum membership, duplicate/missing rows, incomplete/detached exports and changed history/schema/state/module/policy metadata; use an independent original-property oracle rather than the implementation under test.
- [X] T067 [P] [US4] Write and observe failing host-boundary controls in `crates/behavior-models/tests/safety_deployment.rs` for incoming-policy invalidity, stale full-HistoryRef activation, a real Core command-only/no-state-change history advance, same-schema alternative modules, target-valid but relationally unsafe migration, and commit conflicts followed by re-evaluation; no test may assume Core has a BehaviorHash allowlist.
- [X] T068 [P] [US4] Write and observe failing public state-validation tests in `python/tests/test_safety_snapshot.py` for exact public export acquisition, separate Valid/Invalid/Unestablished results, complete policy sets, forged detached JSON and certificate/state-record separation; do not add another Store enumeration path.

### Implementation and integration

- [X] T069 [US4] Implement the reusable original R/P validator in `crates/behavior-models/src/safety/snapshot.rs` over Core's public complete SnapshotExport values, checking every configured dependency, typed IDs/enums and existing parents even for terminal children; classify malformed/incomplete/untrusted input without claiming an actual Store is valid, retaining backend/provenance/validator correspondence assumptions.
- [X] T070 [US4] Add separate state-validation records and canonical hashing in `crates/behavior-models/src/safety/evidence.rs`, enforcing “Exact incoming module and complete checked property set.”, “Exact Core schema identity and StateId from the export.”, “Full HistoryRef, including Store identity, position, state and history-record anchor.” and “Valid / Invalid / Unestablished, separate from module proof outcome.”; exclude the record's own identity field and claim only validity at that anchor.
- [X] T071 [US4] Add the Rust binding adapter in `crates/behavior-py/src/safety.rs` and `crates/behavior-py/src/lib.rs` to call existing public `Store::export_seed_at` for the requested HistoryRef and pass its complete product to T069, preserving provenance and all metadata without reimplementing Store reading or accepting a caller's certification/provenance flag.
- [X] T072 [US4] Expose `validate_safety_snapshot` and readonly validation records in `python/behavior/safety.py`, `python/behavior/__init__.py` and `python/behavior/_engine.pyi`, reusing `python/behavior/store.py`'s existing Store wrapper; make T068 pass with no Python property checker, implicit module activation or Lean invocation during Store operations.
- [X] T073 [US4] Add the reference host coordination example in `models/examples/safety_models/deployment.py`, fixing one BehaviorHash plus complete policy set per segment and coordinating all writers/migrations/activation across incoming validation and exact-head activation; demonstrate that unlocked check-then-switch and StateId-only checks are insufficient, and identify this as a host protocol rather than new Core governance.
- [X] T074 [US4] Execute the migration/alternative-module/stale-anchor controls from `crates/behavior-models/tests/safety_deployment.rs` and compare against `models/examples/safety_models/deployment.py`; require fresh incoming-policy validation at the enforced boundary, record no retroactive history claim, and leave general migration certification/seamless switching out of scope.
- [X] T075 [US4] Extend `models/examples/safety_models/run.py` to emit separate exact-history state-validation results for the LD reference cases alongside module assessments; include missing/invalid-base demonstrations, make initial validity explicit, and never merge host approval or snapshot validity into a preservation proof status.
- [X] T076 [US4] Run and record the state/deployment milestone in `target/safety-models/state/milestone.json`, covering T066–T075 and the full source-history metadata, original predicates, known-invalid/unknown distinctions, exact enforced switch boundary and all backend/host trust obligations; no earlier/later state may inherit a single snapshot's validity.

**Checkpoint**: Module preservation, state validity and host authorization remain independently
checkable. An exact validated anchor and enforced continuity are required for each new segment.

## Phase 7: Polish and Cross-Cutting Acceptance

**Goal**: Integrate required checks into the existing process and close documentation/acceptance
without expanding the supported language, changing Core or publishing a release.

- [X] T077 Write and observe failing mandatory-gate controls in `scripts/tests/test_safety_proofs.sh` for missing/wrong tools, early Rust/Lean failure, omitted required cases, Unsupported/Inconclusive/Failed assessments and failed concrete roots; require nonzero status and explicit failure/unrun diagnostics rather than a skipped or falsely successful proof gate.
- [X] T078 Implement executable `scripts/check-safety-proofs.sh` and add its named mandatory gate to `scripts/gates.sh` after binding build prerequisites, exercising both generic theorems, every required concrete case, structured axiom audits and fresh replay; make T077 pass, reject active overrides, preserve existing gates and keep proof work outside the 507 quick timer.
- [X] T079 [P] Document public Safety authoring, supported operation/guard forms, explicit certification and receipt/version compatibility in `models/README.md` and `README.md`; use concise public-API examples and describe the actual predicate/theorem trust boundary without claiming all-SMT or end-to-end verification.
- [X] T080 [P] Document the five reference cases, full evidence classes, exact snapshot/activation protocol and unsupported limits in `models/examples/safety_models/README.md`, linking the deployment example; keep I-5b/I-6 unmapped, live/nonterminal distinct and mandatory VM ownership confined to the restricted fixture.
- [X] T081 Run `nix develop -c env BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh` and explicit complete 506/507 conformance through `scripts/conformance.sh`, recording commands/statuses/tool identities in `target/safety-models/final/gates.json`; preserve all existing compatibility identities and mandatory runtime budgets, and diagnose failures without weakening gates or pins.
- [X] T082 After T081 succeeds, run `nix develop -c scripts/release-check.sh --skip-gates` with its installed `release/safety_smoke.py` check plus public API controls in `python/tests/test_safety_evidence.py`, recording outputs in `target/safety-models/final/package.json`; use a fresh temporary distribution, verify empty-directory certification/proof resources and no-Lean ordinary execution, reuse T081's recorded gates rather than rerunning them, and publish/tag nothing.
- [X] T083 Execute the focused positive/negative validation sequence in `specs/510-safety-models/quickstart.md`, reuse already captured identical full-gate runs where applicable, and update that guide to actual working commands/results; verify lowering-only/missing-tool/tampered-bundle cases and reproducibility without adding a second runner or unsupported capabilities.
- [X] T084 Review the final diff against `target/safety-models/baseline/manifest.json`, `core-release.json`, `models/fixtures/compat.json` and established receipt builders; verify unchanged Core pin/old identities/user work and record remaining proof-cost, environment and correspondence limits in `target/safety-models/final/review.json` without deleting unrelated data or concealing blocking findings.
- [X] T085 Record the FR-001–FR-038, SC-001–SC-009 and Q1–Q17 acceptance matrix in `specs/510-safety-models/checklists/implementation.md`, linking actual generic/concrete proof roots, mutation/coverage manifests, five cases, state/activation evidence and final gate outputs; mark complete only discharged work and retain precise assumptions/unsupported limits with no full I-5b/I-6 or unconditional system-safety claim.

## Dependencies and Execution Order

### Phase graph

```text
Setup T001–T003
  → Foundation T004–T009
  → US1 T010–T031 → FO STOP GATE
  → US2 preflight T032–T034 → PUBLIC-CORE STOP GATE
  → US2 pattern/proofs T035–T054 → LD gate
  → US3 T055–T065
  → US4 T066–T076
  → Integration/acceptance T077–T085
```

US1 is independently deliverable after the foundation. US2 requires US1's established ownership
and proof machinery. US3 completes evidence verification for both supported kinds; its minimal
producer/integrity prerequisites are already in US1. US4 requires the resolved property set and
evidence identities. Independent testing means each story has its own acceptance criteria using
completed prerequisites, not that these mathematical dependencies can be bypassed.

### Critical task dependencies

| Task(s) | Required predecessor(s) / reason |
|---------|---------------------------------|
| T003 | T002 observed red; T001 baseline preserved. |
| T007–T009 | T004–T006 reviewed and observed failing; finish before US1. |
| T014–T017 | T010–T013 red controls; selector resolution precedes policy checking/composition. |
| T019 → T020 | T018 state/effect definitions; generic soundness precedes concrete instantiation. |
| T021–T024 | T019/T020 theorem contract plus T012 controls; T022 audit and T023 fresh replay precede any certificate. |
| T025–T028 | Rust lowering/certification boundary complete before Python delegation/reference runner. |
| T029–T031 | All FO production code and positive controls; no dependency on US2 or unfinished US3. |
| T032–T034 | Successful T031; no LD implementation before successful T034. |
| T039–T046 | T035–T038 red; guard adapter already demonstrated; T042/T043 semantics/soundness before T044/T045 concrete proof acceptance. |
| T047–T054 | T046 public API and T045 certification; all examples/mutations/actual-Core enumeration before LD milestone. |
| T058–T061 | T055–T057 red; existing proof obligations remain mandatory throughout. |
| T063 | T062 red; never alter established manifest/builders to obtain green. |
| T064/T065 | T058–T063 complete and both Models supported. |
| T069–T073 | T066–T068 red; exact snapshot predicate/record before binding and host example. |
| T074–T076 | T073 enforced protocol and T069–T072 metadata/provenance binding. |
| T078 | T077 red; all five cases, bindings and proof roots exist. |
| T081–T085 | Required features/gates/docs complete; unsuccessful mandatory checks block completion. |

Within a test batch, implement no production change until the relevant controls are reviewed
and observed failing. Run tests again after each corresponding implementation step. Earlier
task numbers may be revisited only to fix defects, never to mark an unmet gate successful.

### Stop conditions

- FO proof/correspondence incomplete: stop before T032.
- Public Core cannot support a required pattern: stop before T035 and report the exact facade gap.
- Missing tool, false premise, unknown/unsupported operation, wrong theorem or failed integrity:
  no certificate; no conditional label may hide the failed obligation.
- Resource exhaustion: Inconclusive with actual diagnostics; do not silently relax proof integrity.
- Environment/disk trouble: record and handle separately; remove no uncommitted/unrelated work.

## Parallel Opportunities and Examples

Only the following marked batches are eligible after all their preceding phase/gate prerequisites
pass. They use different files; shared source integration and theorem construction are serialized.

| Stage | Parallel task batch | Prerequisite |
|-------|---------------------|--------------|
| Foundation | T004, T005, T006 | Setup complete. |
| US1 | T010, T011, T012, T013 | Foundation complete; no LD work. |
| US2 | T035, T036, T037, T038 | FO and public-Core preflight gates passed. |
| US3 | T055, T056, T057 | LD gate passed. |
| US4 | T066, T067, T068 | Evidence workflow complete. |
| Documentation | T079, T080 | Required gate integration complete. |

### US1 example

```text
After T009, write T010's ownership controls in safety_ownership.rs alongside
T011's composition controls in safety_composition.rs and T012's proof controls.
Wait for their intended failures before implementing the corresponding code.
```

### US2 example

```text
After T034, write T035's policy/operation controls and T036's independent
actual-Core enumeration while T037 writes separate proof-premise controls.
Do not start any of these before the public-Core stop gate passes.
```

### US3 example

```text
After T054, write T055's Rust evidence contract, T056's Python import/tamper
controls and T057's separate reproducibility/resource tests in parallel.
Serialize edits to shared evidence.rs/certify.rs during implementation.
```

### US4 example

```text
After T065, write T066's snapshot predicates, T067's host-boundary controls
and T068's Python adapter tests in separate files before implementing the validator.
```

These examples identify opportunities, not authorization to spawn agents or overlap gates.

## Requirement Coverage

| Requirement group | Primary tasks |
|-------------------|---------------|
| FR-001–FR-004 authority/public Core/final module | T007–T009, T016, T025–T026, T032–T034, T046 |
| FR-005–FR-007 compatibility/identity/no generated behavior | T011, T016–T017, T039, T041, T062–T064, T084 |
| FR-008–FR-012 FieldOwnership | T010–T020, T027–T031 |
| FR-013–FR-016 relationship/control/guards | T035–T044, T047–T048, T069 |
| FR-017–FR-019 result-state/profile/fail closed | T032–T045, T051–T054 |
| FR-020–FR-025 generic/concrete proofs and correspondence | T012, T018–T024, T029–T034, T037, T042–T045, T051–T054 |
| FR-026–FR-027 separate deterministic evidence/outcomes | T024, T055–T065 |
| FR-028 exact state validation | T066, T068–T072, T075–T076 |
| FR-029–FR-031 migration/module/host boundaries | T067, T073–T076, T080 |
| FR-032–FR-034 adversarial, actual-Core and exhaustive tests | T005, T010–T013, T029–T038, T051–T054 |
| FR-035 four neutral + restricted VM acceptance | T027–T028, T047–T050, T080 |
| FR-036 existing gates/proof integration | T031, T062–T065, T077–T078, T081–T084 |
| FR-037 public docs, explicit certification, no runtime Lean | T023–T028, T046, T061, T071–T073, T079–T083 |
| SC-001–SC-002 generic/concrete integrity and coverage | T018–T024, T029–T031, T042–T045, T052–T054, T078 |
| SC-003–SC-004 usable cases and independent finite coverage | T027–T028, T047–T054 |
| SC-005–SC-006 reproducible truthful evidence/state boundaries | T055–T076 |
| FR-038 / SC-009 state-transparent Core replay | T011, T036, T079–T081, T085 |
| SC-007–SC-008 regression and FO-first acceptance | T031, T034, T054, T081–T085 |
| Q1–Q5 structural rules, Ref, immutability, sidecars | T014–T017, T024, T035, T039–T045, T058 |
| Q6–Q10 toolchain, concrete proof, validator-only, single op, integrity | T002–T003, T018–T024, T032–T045 |
| Q11–Q13 canonical forms, exact validator, isolated lifecycle operations | T032–T045, T047, T066–T072 |
| Q14–Q16 explicit certification, union counts, exact switch | T016, T023–T026, T040, T044, T067–T076 |
| Q17 restricted fixture and unmapped domain labels | T049, T080, T085 |

## Implementation Strategy

### MVP first

Complete T001–T031 and stop to validate FieldOwnership independently. Its theorem, concrete
checking, audit/replay and two public examples are part of this MVP, not optional follow-up work.
Do not describe the MVP as completion of Feature 510.

### Incremental delivery

1. Establish pinned tools and a complete inventory boundary.
2. Deliver and prove FieldOwnership; pass the FO gate.
3. Demonstrate public-Core canonical correspondence; stop on an actual API gap.
4. Deliver/prove the restricted LifecycleDependency profile and independent actual-Core controls.
5. Complete reproducible bundle verification and new semantic compatibility mappings.
6. Deliver exact-state validation and host-boundary examples.
7. Integrate existing gates, verify packaging/docs, and close only evidenced acceptance items.

Do not turn safe rejected shapes, polymorphic ownership, E3/E4, unprovided I-5b/I-6 definitions,
new governance or performance polish into automatic scope additions. Preserve explicit unsupported
limits and name every remaining trusted assumption. Implementation work begins only when invoked.

## Task Summary

| Phase / story | Tasks | Count |
|---------------|-------|-------|
| Setup | T001–T003 | 3 |
| Foundation | T004–T009 | 6 |
| US1 — FieldOwnership MVP | T010–T031 | 22 |
| US2 — LifecycleDependency incl. preflight | T032–T054 | 23 |
| US3 — Evidence | T055–T065 | 11 |
| US4 — State/deployment boundaries | T066–T076 | 11 |
| Cross-cutting acceptance | T077–T085 | 9 |
| **Total** | **T001–T085** | **85** |

There are **19 [P] tasks** in six eligible batches. All tasks start unchecked; their presence
does not assert implementation, proof success, runtime validation or passing gates.

## Release-Readiness Extension — Execution Scope

**Generated**: 2026-10-18. **Input**: US5–US7, FR-039–FR-046, SC-010–SC-014;
[current plan](plan.md#current-plan--release-readiness-2026-10-18),
[research §10](research.md#10-release-readiness-research-2026-10-18), [data model §8](data-model.md#8-release-readiness-additions),
[release contract](contracts/release-readiness.md) and [quickstart §7](quickstart.md#7-release-readiness-validation).

The 85 preceding tasks/markers and their summaries are preserved as the original implementation
record. The additional scope is T086–T113 below; its markers and acceptance matrix now record
actual completion. Previous completion/convergence does not discharge it. No existing phase is
restarted. Tests are explicitly required by FR-045, FR-046
and Constitution III. Write/review controls and observe their intended failure before each fix;
record red/green commands, exit codes and actual results under `target/safety-models/release-readiness/`.
Missing tools, compile mistakes in a negative control and blanket refusal are not intended red
results or positive proof evidence. Generating this extension executes no tests.

Retain the exact Core 0.15.0 pin, verifier 0.10.0, Lean 4.30.0 and existing runtime budgets.
The public serializer exception is limited to untrusted string-array candidate discovery in
one private helper; actual mandatory-root equality and concrete proofs remain mandatory.
No Core modification, expression-equivalence framework, runtime state, authoring capability,
production-application dependency, tag creation/push or publication is authorized here.
Preserve unrelated Spec Kit/Skills work and uncommitted user data.

## Phase 8: Release Setup — Preserve the Implemented Baseline

**Goal**: Establish the exact candidate and protected inputs without repeating project setup.

- [X] T086 Record current source/pin/tool identities, checker version, release metadata, existing compatibility/receipt bytes, unrelated user changes and available build space in `target/safety-models/release-readiness/baseline.json`; retain an authentic current checker-1 bundle from `target/safety-models/proof-gate/` for stale-evidence controls, or produce that baseline with the unchanged implementation if unavailable; use task-owned build/temp storage and remove no unrelated data.

## Phase 9: Release Foundation — Public-Core Feasibility Stop Gate

**Goal**: Demonstrate bounded candidate discovery and exact-root construction before dependent
recognition changes. This phase supplies the shared boundary, not a replacement Core parser.

- [X] T087 Write/review and observe failing public-facade preflight controls in `crates/behavior-models/tests/safety_correspondence.rs` and scoped exception controls in `scripts/tests/test_check_public_surface.sh`: reconstruct both direct and nested-query roots in the actual member order, retain duplicates, reject wrong type/binding/placement and hint-only acceptance, refuse each exhausted discovery budget, and keep serialization outside the named helper and private Core imports rejected; require valid positive controls.
- [X] T088 Implement private `crates/behavior-models/src/safety/membership_orders.rs`, register it in `crates/behavior-models/src/safety/mod.rs`, add only the scratch ordered-probe support needed in `guards.rs`, and scope the named exception in `scripts/check-public-surface.sh`; enforce “exact distinct-member set, retained order/repetitions, deterministic vector deduplication, bounded total work” without interpreting expression keys, with cumulative limits of 16 MiB output, 1,000,000 JSON nodes, depth 128, 4,096 distinct vectors, 4,096 values/vector and 4,096 probe constructions/analysis; check bytes before decoding and counters before further work, report `SAFETY_GUARD_SCAN_EXHAUSTED` on exhaustion, never truncate to success, and make T087 pass without integrating production certification yet.
- [X] T089 Execute the public-Core preflight from `crates/behavior-models/tests/safety_correspondence.rs` and `scripts/tests/test_check_public_surface.sh`, record actual commands, roots, counts/resource observations and source/gate boundaries in `target/safety-models/release-readiness/preflight.json`, and stop dependent work if any supported row fails; justify any measured default-budget tuning without changing proof limits or runtime budgets, using no private variants, Wire semantic classification, copied hashing, factorial search or changed pin.

**PUBLIC-CORE STOP GATE**: T089 must pass before T090–T113 production work. Source inspection
alone is not a passed preflight. Discovery influences availability; a hint cannot grant authority.

## Phase 10: US5 — Certify Supported Membership Orders (P1, Release MVP)

**Goal**: Correct the reported C2 recognition defect in both existing guard families and retain
exact admitted identities and the complete mathematical certification boundary.

**Independent test**: All 32 distinct permutations of two-, three- and four-member sets in each
family certify against their own exact modules; duplicate/larger-set controls and adversarial
guards preserve the required set/placement/bindings. Missing premises never produce a certificate.

### Tests first

- [X] T090 [P] [US5] Write/review and observe the reported C2 failure in new `crates/behavior-models/tests/safety_membership.rs`, with minimal public-Core fixtures local to that test: exhaust the 32 permutations per direct-parent and nested-child-query guard family, include repeated literals and seeded permutations of 8/16/32 members, reject missing/extra members, wrong enum/field/binding/polarity and weakened Boolean placement, and compare actual authored hashes, runtime membership results and safety-free IR/receipts independently; do not depend on DeclarativeHost internals.
- [X] T091 [P] [US5] Write/review and observe failing concrete-membership, identity and proof-integrity controls in `crates/behavior-models/tests/safety_lifecycle_proofs.rs`, `crates/behavior-models/tests/safety_reproducibility.rs`, `crates/behavior-models/tests/safety_evidence.rs` and `crates/behavior-models/proofs/safety/tests/CategoryMembership.lean`: alter ordered-member/set witnesses, roots, captures or dependency linkage, remove essential premises, and attempt stale checker/source evidence; require positive theorem/instance controls, unchanged existing theorem conclusions and no certificate from omitted/corrupt hints or forged match data.
- [X] T092 [P] [US5] Write/review and observe failing public SDK controls in new `python/tests/test_safety_membership.py` for reversed/duplicate direct and query guards, wrong-context refusal, exact BehaviorHash/evidence binding, Inconclusive resource refusal and ordinary no-Lean lowering/replay; assert thin Rust delegation and no Python guard normalization or module rewriting.

### Implementation and evidence

- [X] T093 [US5] Integrate candidate matching in `crates/behavior-models/src/safety/guards.rs` and `crates/behavior-models/src/safety/lifecycle_dependency.rs`: construct typed `MatchedMembershipGuard` data only after full Core probe admission and actual mandatory-root equality, retain “Actual action/root hash, dependency identity, guard family, enum/field/binding coordinates, ordered vector and required policy set.”, choose the lowest matching root hash deterministically, preserve empty-set literal conventions and duplicate multiplicity, and share authoritative matched results with the concrete encoder; keep full action/effect coverage and reject unsupported forms without rewriting Core.
- [X] T094 [US5] Encode “Raw ordered member bytes and the corresponding policy set in finite proof input; kernel-checked membership equivalence/set agreement alongside existing root/role/operation premises.” in `crates/behavior-models/src/safety/proof_data.rs` and `crates/behavior-models/proofs/safety/LifecycleCheck.lean` / `crates/behavior-models/proofs/safety/Check.lean`; update `crates/behavior-models/src/safety/certify.rs`, `crates/behavior-models/src/safety/evidence.rs`, `crates/behavior-models/proofs/safety/Audit.lean` and required-root manifests so analysis and concrete checking use actual matched roots, bind the helper in implementation-source identity, and classify scan exhaustion as Inconclusive through analysis/proof preparation before the US5 milestone; retain every generic/concrete type, axiom and fresh-replay obligation with no caller-deserializable match authority.
- [X] T095 [US5] After T091's version/stale-data controls are red, bump `SAFETY_CHECKER_VERSION` to 2 in `crates/behavior-models/src/safety/mod.rs`, extend lowerer-version/history controls in `scripts/tests/test_models_safety_compat.sh`, and add only new checker-2 tuples to `models/fixtures/safety-compat.json` through `scripts/check-models-compat.py --safety --write`; preserve policy semantic version 1, the meaning/format of existing evidence and receipts, every old tuple and `models/fixtures/compat.json`, and make stale checker/source bundles noncertifying rather than reusing their success flags.
- [X] T096 [US5] Run T090–T095's positive/negative public and Rust controls plus the existing generic/concrete proof-integrity suites, certify all required small permutations and full representatives at sizes 8/16/32, repeat canonical evidence for identical inputs and mutate every new witness binding; record exact case/seed/count bounds, positive certificates, negative outcomes, audit/fresh-replay results and unchanged authored IR/receipts in `target/safety-models/release-readiness/membership.json`, making T090–T092 pass with zero deliberately insufficient certified cases.

**MVP STOP GATE**: US5 is independently usable only after T096 passes, including concrete
proofs and the new checker identity. It is not completion of release readiness or permission
to publish. Do not defer a missing proof to later documentation or label it an assumption.

## Phase 11: US6 — Report the Actual Failed Lifecycle Prerequisite (P1)

**Goal**: Make the three parent prerequisites individually actionable without changing the
accepted lifecycle pattern or concealing resource/structural failures.

**Independent test**: Each isolated cause yields its exact stable code and dependency/action
context; combined and multiple-parent cases follow precedence, fixing one defect leaves other
defects visible, and resource exhaustion remains Inconclusive without certification.

### Tests first

- [X] T097 [P] [US6] Write/review and observe failing Rust diagnostic/assessment controls in `crates/behavior-models/tests/safety_lifecycle.rs` and `crates/behavior-models/tests/safety_evidence.rs` for the three exact release-contract codes, action/dependency/entity/field context, combined-failure precedence, a later valid parent candidate, association on one parent plus guard on another, correction of one cause while another remains, terminal-assignment exemption, terminal-creation association, and Unsupported versus Inconclusive versus structural/identity failure outcomes.
- [X] T098 [P] [US6] Write/review and observe failing Python controls in `python/tests/test_safety_lifecycle.py` and `python/tests/test_safety_evidence.py` for deterministic structured issues in both Lowered analysis and failed certification assessments, exact codes/context and correction behavior; reject tests that merely match the old combined Debug/message text or treat a new code as a certificate.

### Implementation and evidence

- [X] T099 [US6] Add optional dependency ModelHash context to `crates/behavior-models/src/safety/input.rs` and implement staged all-parent checking in `crates/behavior-models/src/safety/lifecycle_dependency.rs`: C is every suitable State parent, A every correctly associated member of C, G every member of A with the required mandatory guard; emit the earliest established empty-stage diagnostic per action/dependency/operation, require association and guard on the same candidate, sort by action/dependency identity and operation occurrence, retain existing terminal-update/creation distinctions, and populate “Stable code, action, optional dependency ModelHash, entity, field and detail.” without adding runtime data.
- [X] T100 [US6] Replace substring-dependent classification for the three prerequisite codes with explicit Unsupported handling in `crates/behavior-models/src/safety/certify.rs`, preserve `SAFETY_GUARD_SCAN_EXHAUSTED` as Inconclusive through analysis/proof preparation, and expose original typed issues in `crates/behavior-models/src/safety/evidence.rs` assessment diagnostics with thin forwarding through `crates/behavior-py/src/safety.rs` / `python/behavior/_engine.pyi` only as needed; preserve canonical/diagnostic separation, ownership build errors, existing Failed semantics and the no-certificate rule.
- [X] T101 [US6] Make T097–T098 pass and record exact isolated, combined, wrong-binding, later-valid-candidate and correction outcomes from the Rust/Python suites in `target/safety-models/release-readiness/diagnostics.json`; compare complete serialized issue order across repeats and confirm all three new codes remain Unsupported while resource exhaustion remains Inconclusive, with no downstream cause asserted when its prerequisites were not evaluable.

## Phase 12: US7 — Explain and Demonstrate the Release Contract (P1)

**Goal**: Give authors the complete lifecycle obligations and an independently recorded Core/
Safety comparison, then prepare consistent candidate metadata using the existing release path.

**Independent test**: The six-case matrix has two actual conditional certificates and four
actual Unsupported refusals, separately records nonempty query-sensitive Core checks, and fails
closed on omitted rows/tools/proofs. Documentation covers every v1 operation, including unchanged-
owner nonterminal transitions. Final US7 acceptance additionally requires Phase 13's full release run.

### Tests first

- [X] T102 [P] [US7] Write/review and observe failing public matrix/authoring controls in new `python/tests/test_safety_guard_contract.py` for `--guard-contract --certify`, all six variant identities, exact modules/profiles/check subjects/attestations and Safety evidence, a nonempty query-sensitive Core postcondition inventory, actual supported/excluded results and Planned → Created / Terminal → Active examples; ensure a genuine Core Inconclusive result is preserved independently and no live DeclarativeHost application or assumed SMT outcome is required.
- [X] T103 [P] [US7] Write/review and observe failing mandatory-report/runner controls in `scripts/tests/test_safety_proofs.sh` and `python/tests/test_gate_runner.py` for omitted matrix rows, empty Core checks, absent tools, early comparison failure, an unexpected negative-case certificate and supported-case proof failure; require nonzero gate status plus explicit failure/unrun reporting while retaining the existing five required certificates, ordinary lowering behavior and named gate ordering.

### Fixture, reporting and documentation

- [X] T104 [US7] Add `models/examples/safety_models/guard_contract.py` using existing public APIs for one minimal parent-prohibition/query fixture and six variants (canonical, permuted, equivalent unsupported, absent guard, wrong parent, insufficient OR), with a relevant local postcondition and child-side unchanged-owner examples; preserve the original five reference builders and the restricted VM contract, and make the fixture controls in T102 pass without introducing generated lifecycle capabilities or another semantic checker.
- [X] T105 [US7] Extend `models/examples/safety_models/run.py` with explicit `--guard-contract --certify --output PATH`, require certification for that mode, call public Core verification and Rust Safety certification independently, and write complete `guard-contract.json` rows containing “Variant, exact BehaviorHash, intended local Core property/profile, canonical attestation and individual outcomes, Safety outcome/certificate presence/diagnostics/evidence identity.”; record actual solver outcomes without promoting Inconclusive or substituting local proofs for LD induction, and make errors/missing checks/rows nonzero with unrun rows preserved.
- [X] T106 [US7] Integrate the six-case comparison into the existing mandatory path in `scripts/check-safety-proofs.sh` and `models/examples/safety_models/run.py`, extend aggregate expectations in `scripts/tests/test_safety_proofs.sh` / `python/tests/test_gate_runner.py`, and make T103 pass; require the original five certificates plus a complete valid comparison, distinguish expected Unsupported negative controls from failed supported obligations, and keep the work outside the 507 quick timer without another gate/release process.
- [X] T107 [P] [US7] Update `README.md`, `models/README.md` and `models/examples/safety_models/README.md` with executable public references, all v1 lifecycle-operation obligations, both nonterminal example transitions even with unchanged ownership, parent State/owner association/canonical whole-root structure, duplicate/order behavior, diagnostic codes/precedence and the independently observed verification matrix; retain genesis/migration/extraction/host assumptions, state transparency, VM-fixture scope and unmapped I-5b/I-6, and recommend a shared authoring form only if actually demonstrated.
- [X] T108 [P] [US7] Recheck published versions and the planned 0.13.1 policy rationale, then align `Cargo.toml`, workspace package entries in `Cargo.lock`, `api/public-api.json`, `docs/versioning.md`, new `release/notes/0.13.1.md` and the version declarations in `skills/behavior-authoring/SKILL.md`, `skills/behavior-verification/SKILL.md` and `skills/behavior-application/SKILL.md` where existing checks require it; preserve the Core pin, lock dependency graph, established API/format entries and published notes, keep Skills edits to required metadata accuracy, and reassess rather than reuse an already published version or silently pick another contract.
- [X] T109 [US7] Execute the new public mode, mandatory proof/report controls and essential documentation examples from T102–T108, record all six independent result pairs and observed recommended-form limitations in `target/safety-models/release-readiness/contract.json`, and verify candidate metadata consistency with existing API/Skill checks; leave the original reference identities and previous evidence untouched and report current obligations honestly before full release validation.

## Phase 13: Corrected-Candidate Acceptance and Handoff

**Goal**: Establish readiness of this exact corrected candidate through all existing required
checks, preserving evidence classes and user work. This phase prepares a release; it publishes none.

- [X] T110 Run `nix develop -c scripts/release-check.sh` with full required gates on the corrected candidate, capturing command/source/checker/tool identities, exit status, raw logs and installed-package/artifact checks in `target/safety-models/release-readiness/release.json`; require full properties and opt-in release controls, current Rust/Python tests, generic/concrete proofs and audits/fresh replay, the new six-case gate, packaged proof resources, clean consumer installation, Core-only replay and reproducibility; diagnose failures without weakening gates/budgets/pins or substituting a prior `--skip-gates` result.
- [X] T111 Execute `scripts/conformance.sh 506 complete` and `scripts/conformance.sh 507 complete` in the pinned Nix environment and record fresh reports under `target/safety-models/release-readiness/`; verify all required cases/obligations, the full release run's unchanged quick budget, canonical evidence repetition, stale-checker rejection and final public package/API expectations, making no universal proof claim from finite conformance results.
- [X] T112 Review the candidate and new evidence against `target/safety-models/release-readiness/baseline.json`, `core-release.json`, both compatibility manifests and established reference/receipt builders; record baseline protection, exact module/policy/proof bindings, observed remaining nonblocking limitations, disk/build environment and independently classified Core/Safety results in `target/safety-models/release-readiness/review.json`, preserving all unrelated Spec Kit/Skills work and user ignore rules with no tag, publication or hidden approval mechanism.
- [X] T113 After T110–T112 succeed, append the US5–US7 / FR-039–FR-046 / SC-010–SC-014 acceptance evidence to `specs/510-safety-models/checklists/implementation.md` and update `specs/510-safety-models/quickstart.md` §7 plus `specs/510-safety-models/release-readiness.md` to actual commands/results, linking all fresh milestones and retaining historical scope labels; distinguish valid proofs, concrete checks, tests and host/correspondence assumptions, mark only discharged tasks complete, and hand off separate post-merge publication through the existing process.

## Release Dependencies and Execution Order

```text
T001–T085 original implementation (preserved)
  → T086 baseline
  → T087 red preflight controls → T088 helper → T089 PUBLIC-CORE STOP GATE
  → US5: T090/T091/T092 red → T093 matching → T094 concrete proof → T095 versions → T096 MVP gate
  → US6: T097/T098 red → T099 diagnostics → T100 assessment mapping → T101 story gate
  → US7: T102/T103 red → T104 fixture → T105 report → T106 mandatory integration
           → T107/T108 documentation + metadata → T109 contract gate
  → T110 full release check → T111 complete conformance → T112 review → T113 acceptance
```

All three extension stories are P1. Their independent checks use the already implemented
FO/LD authority; US7 needs both corrected recognition and diagnostics. US6 edits shared files
after the US5 milestone, so serialize its integration. A test-first batch is complete only
when its relevant intended failures have been observed; task numbering never waives that gate.

| Dependency | Reason |
| --- | --- |
| T088 after T087 | The scoped public-surface exception and bounded helper need rejection controls first. |
| T090–T096 after T089 | No production guard integration before actual public-Core feasibility. |
| T093/T094 after T090–T092 | Actual-root/witness and thin-binding controls precede fixes. |
| T095 before T096 | Changed certification inputs must use checker 2 before the MVP certifies. |
| T099/T100 after T097/T098 | Diagnostic shape, precedence and outcome classes must fail correctly first. |
| T104–T106 after T102/T103 | Public comparison and mandatory-gate behavior require observed red controls. |
| T107/T108 after T106 | Document demonstrated behavior and finalize versions before release checks. |
| T110 after T109 | Validate the complete corrected candidate, including public metadata/docs. |
| T113 after T110–T112 | Previous successful runs and unresolved mandatory failures cannot establish current completion. |

### Release parallel opportunities

| Story | Eligible batch | Prerequisite / disjoint files |
| --- | --- | --- |
| US5 | T090, T091, T092 | T089 passed; separate Rust membership, proof/evidence and Python test files. |
| US6 | T097, T098 | T096 passed; Rust diagnostic tests and Python contract tests are separate. |
| US7 | T102, T103 | T101 passed; public comparison tests and shell/gate aggregation tests are separate. |
| US7 | T107, T108 | T106 passed; public authoring docs and package/version/Skill metadata use different files. |

**Examples**: US5 can write membership controls alongside the proof-mutation and Python controls;
US6 can write Rust precedence controls alongside Python serialized-diagnostic controls; US7 can
write matrix acceptance controls alongside gate failure controls. In each batch, finish reviewing
and observing relevant failures before changing production code. Shared `guards.rs`,
`lifecycle_dependency.rs`, `certify.rs`, `evidence.rs`, proof data and runner integration remain
serialized. These are work-splitting opportunities, not authorization to spawn agents.

## Release Coverage and Implementation Strategy

| Intent | Tasks |
| --- | --- |
| US5/AC1–AC2; FR-039–FR-041; SC-010, SC-014 | T087–T096, T110–T112 |
| US6/AC1–AC2; FR-042; SC-011 | T097–T101, T110 |
| US7/AC1; FR-043; SC-012 | T102, T104, T107, T109 |
| FR-044 six independent mechanisms/cases | T102–T106, T109–T110 |
| FR-045 targeted, permutation and negative evidence | T087, T090–T092, T096–T098, T101–T103, T109–T111 |
| US7/AC2; FR-046; SC-013 | T095, T106, T108–T113 |
| Original proof, inventory, state/deployment and state-transparency contracts | T091, T094, T096, T100, T110–T113 |

**MVP**: Finish T086–T096 and stop to assess US5 independently, including real concrete
certificates and negative controls. Then deliver US6 and US7, and finish exact-candidate release
acceptance. No new functionality, proof caching, external application review or performance
project is added to make these checks green. Unsupported domain arrangements remain explicit.

### Release task summary

| Phase / story | New tasks | Count |
| --- | --- | --- |
| Setup | T086 | 1 |
| Public-Core foundation | T087–T089 | 3 |
| US5 — Membership orders | T090–T096 | 7 |
| US6 — Diagnostics | T097–T101 | 5 |
| US7 — Authoring/comparison/candidate | T102–T109 | 8 |
| Final acceptance | T110–T113 | 4 |
| **Release-readiness extension** | **T086–T113** | **28** |
| **Whole task file** | **T001–T113** | **113** |

The extension has **9 [P] tasks** in four eligible batches. At generation, all 28 new tasks
were unchecked and claimed no implementation or proof success. Execution evidence is now
recorded in [the acceptance matrix](checklists/implementation.md#release-readiness-extension-acceptance-2026-10-18)
and the task markers above; publication remains a separate explicit post-merge action.
