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
