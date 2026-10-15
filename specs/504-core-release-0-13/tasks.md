# Tasks: Core Release 0.13.0 alignment

**Input**: Design documents from `specs/504-core-release-0-13/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/versions.md, quickstart.md

**Tests**: required (constitution III, spec FR-009): every test task below is written and seen
**red against the 0.12.0 pin** before the pin moves. Record red/green evidence (command, counts,
log path, SHA-256) in `specs/504-core-release-0-13/implementation-log.md`.

**Environment for every command**: `cd ~/repos/behaviour-ir-504 && nix develop -c …` with
`CARGO_TARGET_DIR=$HOME/repos/behaviour-ir/target` (research R6). Never add a path/patch
override of `behavior-engine`. Never edit the Core repository (FR-008).

**Pin values** (research R1): version `0.13.0`, tag `v0.13.0`, commit
`15094972191aa0ae79705df0397c551a18745399`, CLI `behavior-0.13.0-x86_64-linux-musl` sha256
`b6e0d94a45110c0030a524683d6eb78d1fb266fd14329f596e430f0985a59ad0`, conformance
`behavior-conformance-0.13.0.tar.gz` sha256
`f7795af93736cb5402ca1be33bd1d0f19e25d0203fac68c5ad09391057f06bbb`.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [X] T001 Create `specs/504-core-release-0-13/implementation-log.md` (header: date, branch, base `main` `47b4231`, Core from 0.12.0 `b525da4` to 0.13.0 `1509497`) and record the baseline: run `scripts/gates.sh` on the unchanged worktree and log pass counts, so later failures are attributable to the upgrade.
- [X] T002 Confirm free disk ≥ 6 GB (`df -h /`) and that no other cargo build uses `~/repos/behaviour-ir/target`; note both in implementation-log.md. If < 6 GB, stop and report.

## Phase 2: Foundational (red tests that define the target)

- [X] T003 [P] Update `python/tests/test_core_alignment.py`: `CURRENT_CORE = "0.13.0"`; in the parametrized versions test set `records` to `["0.4","0.5","0.6","0.7","0.8"]` and `verifier` to `"0.9.0"` (contracts/versions.md); set `accepted_wire_ir` to `["0.4",…,"0.8"]` (Core 020, research R3); keep `read_records`, `command_stream`, `command_occurrence_domain` unchanged. Run it: red (pin still 0.12.0).
- [X] T004 [P] Update `python/tests/test_versions.py`: `behavior.__version__ == "0.13.0"` and `"binding": {"python": "0.13.0"}`; leave the `_check_versions` cases as they are. Run: red.
- [X] T005 [P] Update `python/tests/test_release_scripts.py` and `python/tests/test_release_dependency_guard.py` so the "current release" arguments are `0.13.0` / `v0.13.0` (and the skew message test names `0.12.0` and `0.13.0` where it now names `0.11.0` and `0.12.0`). Run: red.

**Checkpoint**: T003–T005 red with the expected messages; log them.

## Phase 3: User Story 1 - Use the 0.13.0 engine (Priority: P1) 🎯 MVP

**Goal**: build, package and report exactly Core 0.13.0.
**Independent Test**: quickstart steps 1–2 pass; `scripts/check-core-pin.sh` prints `OK (v0.13.0 1509497…)`.

- [X] T006 [US1] Update `core-release.json` (canonical one-line JSON, `behavior.core_pin.v1`) with the pin values above; first verify them again against `https://github.com/kallistoteles/behavior-ir-core/releases/download/v0.13.0/release-manifest.json` and `SHA256SUMS` (read-only `curl`).
- [X] T007 [US1] In `Cargo.toml` set `[workspace.package] version = "0.13.0"` and the `behavior-engine` `rev` to `15094972191aa0ae79705df0397c551a18745399`; run `cargo update -p behavior-engine --precise 15094972191aa0ae79705df0397c551a18745399` (or `cargo update -p behavior-engine`) so only Core crates and the workspace version change in `Cargo.lock`; check `git diff Cargo.lock` lists no unrelated upgrades.
- [X] T008 [US1] Run `scripts/check-core-pin.sh` → OK; run `scripts/fetch-core.sh` → both assets verified, `.core/0.13.0/` populated; run `scripts/tests/test_fetch_core.sh` (includes the checksum-mismatch refusal, spec US1 scenario 2).
- [X] T009 [US1] `cargo build --workspace` and `cargo clippy --all-targets -- -D warnings`. A compile error contradicts research R2 (identical public surface): stop, record it, do not patch around it.
- [X] T010 [US1] Set `"release": "0.13.0"` in `api/public-api.json` (no other change, contracts/versions.md) and rebuild the binding (`maturin develop` per the repo's normal flow).
- [X] T011 [US1] Run T003–T005 tests → green; run `python -m behavior._cli engine-info` and compare with contracts/versions.md; log both.

**Checkpoint**: US1 complete: pin, build, assets and version reporting agree on 0.13.0.

## Phase 4: User Story 2 - Command-profile actions that observe queries commit (Priority: P1)

**Goal**: regression for the 503 blocker (research R7), through engine and binding.
**Independent Test**: quickstart step 3.

- [X] T012 [P] [US2] Add to `crates/behavior-models/tests/core_assumptions.rs` a test module (same file style: `loc()`, `Builder` helpers) `command_profile_query_*`: a `SemanticProfile::CommandIntents` module with entities `Item { name: String }` and `Mark { item: Id<Item> }`, one command `Ping { item: Id<Item> }`, and action `mark(item: Entity(Item))` with precondition `¬any(select(Mark), m → m.item == item.id)` creating a `Mark`. Store: `genesis_v2_for(module, EvidencePolicyV2::from_json(<"none" policy>), seed)`, invocation via `Store::invoke` with a `behavior.invocation.v1` document. Cases: (a) first `mark` is allowed and `store.commit` succeeds, then state shows one `Mark`; (b) second `mark` on the same item is refused (no bundle) and history length is unchanged; (c) take the bundle from (a) on a fresh store, alter one query `result_hash` in its `read_facts`, commit → refused, head unchanged. Before T006–T007 the test must be red at (a) with `INVALID_CANDIDATE: invalid observed facts`; record that.
- [X] T013 [P] [US2] Add `python/tests/test_query_command_profile.py` with the same three cases through the binding: load the equivalent current wire document with `bh.BehaviorModule.from_wire_json`, `bh.Store.genesis_v2_for(module, seed, <none policy>)` (use the conformance fixture `governance-v2/evidence-policy-none.json`), `store.invoke(...)`, `store.commit(...)`. Red before the pin moves, green after.
- [X] T014 [US2] After US1: run T012 and T013 → green; log.

**Checkpoint**: 503 R13 is reproduced red at 0.12.0 and green at 0.13.0 without Approval.

## Phase 5: User Story 3 - Everything migrates to the 0.13.0 contract (Priority: P2)

**Goal**: tests, examples, skills and backends use the 0.13.0 contract directly; semantic
guarantees preserved, old call patterns not (spec "Scope decision", FR-006, FR-010–FR-012).
**Independent Test**: quickstart step 4; implementation-log.md lists each changed expectation.

- [X] T015 [US3] Run the full Rust suite and full pytest at 0.13.0 and classify every failure in implementation-log.md: `(a)` fresh record version/hash (Core 017, R4), `(b)` verifier 0.9.0 policy selection (R5), `(c)` corrected invalid-child-state outcome, `(e)` complete-snapshot requirement (Core 017, R9), `(f)` creation contract (Core 019, R10), `(g)` release metadata/docs (Polish), `(d)` unexplained → stop.
- [X] T016 [US3] Write the red tests for the new binding contract in `python/tests/test_evaluate.py`: `evaluate(...)` without `facts` is a `TypeError` (required keyword); with a complete universe it evaluates; with a universe missing a declared type it is `INVALID_INPUT` with reason `INVALID_STATE_SNAPSHOT`; and in `python/tests/test_intent.py`: `evaluate_intent(..., facts=<complete universe>)` evaluates (currently impossible: no `facts` parameter).
- [X] T017 [US3] Make `facts` a required keyword of `behavior.evaluate` and add a required `facts` keyword to `behavior.evaluate_intent` in `python/behavior/__init__.py`; forward it into the `ActionIntent` host document in `crates/behavior-py/src/lib.rs` (`evaluate_intent`, key `facts`, unchanged) and update `python/behavior/_engine.pyi`; rewrite both docstrings to state the 0.13.0 contract ("a complete parent snapshot: `universe` lists every declared entity type, empty types as `members: []`; bound `state` must agree; nothing is inferred"). No fallback. T016 → green.
- [X] T018 [US3] Migrate every direct `evaluate`/`evaluate_intent` call in `python/tests/` (test_state_machine, test_request_flow*, test_invocation, test_evaluate, test_queries, test_binding_equivalence, test_lifecycle, test_fixed_scale, test_constraints, test_authorize, test_exact_closure, test_boundary, test_cli_entry, test_intent and any other caller) to pass a complete universe built explicitly from the test's own entities; where several tests in a file share data, one local function per file builds it (tests only, not package API). Expected outcomes must stay equal on equal complete inputs; any outcome that changes is classified `(c)` with its Core reason or `(d)` → stop.
- [X] T019 [US3] Implement Core 019's creation contract in `DictBackend` (`python/tests/test_store_conformance.py`) and in `skills/behavior-application/examples/custom_backend.py`, mirroring Core `InMemoryBackend::create`: if a genesis is already held, refuse a different genesis, refuse differing position-0 versions/references or original state anchor, otherwise return without any write; first creation builds privately and assigns at the end. Update the expected conformance case count to the 0.13.0 list (45) by asserting the case names reported by Core, not a bare number. All conformance tests pass for every backend; the deliberately broken backends still fail their cases.
- [X] T020 [US3] Fix the remaining `(a)`/`(b)` failures (e.g. `test_command_documents.py`, `test_invocation.py` fixture expectations, verifier-0.9 policy fixtures per R5) with a one-line comment citing the Core feature; historical records keep their versions.
- [X] T021 [US3] In `python/tests/test_store_conformance.py` and wherever a store-level test exists, confirm the US3 replay guarantee: histories written under earlier fixtures replay with their recorded profile (`pytest -k "replay or legacy or migration"`).
- [X] T029 [US3] In `models/fixtures/compat.json` set both `core_release` entries to `{"commit": "15094972191aa0ae79705df0397c551a18745399", "version": "0.13.0"}`; `python3 scripts/check-models-compat.py` and `scripts/tests/test_models_compat.sh` pass; a changed pinned case output is `(d)` → stop.
- [X] T030 [US3] Remove obsolete code and docs made pointless by 0.13.0 (FR-012): search `python/behavior/`, `skills/`, `README.md` and `docs/` for statements or helpers that assume evaluation from bound state alone or a non-idempotent `create`, and delete or rewrite them; list each removal in implementation-log.md.
- [X] T031 [US3] Re-run the full Rust + Python suites → all green; log counts against the T001 baseline.

**Checkpoint**: every failure resolved by migration or justified; no shim, no fallback.

## Phase 6: Polish & Cross-Cutting

- [X] T022 [P] Update `README.md`: every `behavior-ir-core/blob/v0.12.0/` link → `v0.13.0`, the "bundles Core Release v0.12.0" sentence, and the example wheel file name `behavior-0.13.0-cp313-…`; check each linked path exists at tag `v0.13.0` (`git -C ~/repos/behavior-ir-core cat-file -e v0.13.0:<path>`).
- [X] T023 [P] Update `models/README.md` ARCHITECTURE link to `v0.13.0` (verify path at the tag).
- [X] T024 [P] Update `docs/versioning.md`: table rows Release/Core/Binding `0.13.0`, formats `records 0.4 … 0.8, verifier 0.9.0`; add a "Release 0.13.0" paragraph above the 0.12.0 one: minor bump because of fresh decision record 0.8 and verifier 0.9.0 (corrected outcomes for invalid child states), command-profile actions observing queries now commit (Core 023), fresh authenticated verification needs verifier-0.9 policies selected explicitly, historical records replay unchanged, Core 0.12.1 included; update the pinned core doc links to `v0.13.0`.
- [X] T025 [P] Update every skill example that evaluates directly to pass a complete universe (T018 rule), then update `skills/behavior-verification/SKILL.md`, `skills/behavior-application/SKILL.md`, `skills/behavior-authoring/SKILL.md`: front matter `release: 0.13.0`, "Behavior release 0.13.0", verifier `0.9.0` wording, Core links to `v0.13.0`; review the "semantic gaps in 0.12.0" / "precision debt (release 0.12.0)" sections and retitle to 0.13.0, removing only items Core 0.13.0 demonstrably closes (cite the Core feature). Run any skill/term check that `scripts/gates.sh` includes.
- [X] T026 Run `scripts/gates.sh` (full) → pass; log.
- [X] T027 Run `scripts/release-check.sh` → pass (clean install outside the checkout reports core 0.13.0 and commit `1509497…`); log.
- [X] T028 Finish implementation-log.md: red/green evidence, changed-expectation table with reasons, tested revisions (ecosystem commit, Core `1509497`), remaining limits; state that the Core repository was not modified and that 503 can now rebase (spec SC-004). Mark spec Status `Implemented`.

## Dependencies & Execution Order

- Phase 1 → Phase 2 → US1 (T006–T011) → US2 green (T014) and US3 (T015–T021, T029–T031) → Polish.
- US3 order: T016 → T017 → T018; T019, T020 independent; T029, T030 → T031.
- T012/T013 can be written during Phase 2 (they must be red before T006).
- US3 depends on US1 (needs the new pin); US2's green run depends on US1.
- T016–T018 depend on T015's classification; T022–T025 are independent of each other.

## Parallel Opportunities

- Phase 2: T003, T004, T005, and US2 tests T012, T013 together (different files).
- US3: T016, T017, T018 after T015 (different files).
- Polish: T022, T023, T024, T025 together.

## Implementation Strategy

- **MVP**: Phases 1–3 (US1): the ecosystem builds and reports Core 0.13.0.
- Then US2 (unblocks 503) and US3 (contract preserved), then docs and the full gates.
- Stop conditions: compile error (T009), unexplained changed output `(d)` (T015/T018/T029), disk < 6 GB (T002).
