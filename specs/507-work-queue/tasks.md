---

description: "Tasks for feature 507: WorkQueue model"
---

# Tasks: WorkQueue Model

**Input**: Design documents from `specs/507-work-queue/`

**Prerequisites**:
- plan.md, spec.md (Q1–Q37), research.md (R1–R13);
- data-model.md;
- contracts/work-queue-contract.md, contracts/safety-arguments.md;
- quickstart.md.

**Tests**: required. Constitution III is non-negotiable, and the spec makes tests, negative
controls and evidence part of the feature (FR-026, FR-032, FR-033, FR-036).
- Every implementation task is preceded by its failing test.
- Scenario tests carry `@pytest.mark.scenario("<W-id>", level=<n>)` and read their expectation
  from `models/examples/work_queue/expected.json`, which is written by hand.

**Organization**: setup, the foundational phases (regression nets, D1(a) bindings, contribution
kinds, shared conformance package, WorkQueue core), then the user stories US1–US5 of spec.md,
then polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5

Paths are relative to `~/repos/behaviour-ir`. Commands run inside `nix develop`, after
`scripts/fetch-core.sh` and `maturin develop`. `<id>` is the queue id and `E` the generated
entity name (data-model §2).

---

## Phase 1: Setup

- [X] T001 Create specs/507-work-queue/implementation-log.md with a "Baseline" section: the `main` commit (`1171f10`), the Core pin (0.13.0, `1509497…`), `behavior engine-info`, and the output of `scripts/check-models-compat.py --print` saved as the established identity baseline (requirement 7).
- [X] T002 [P] Create the empty packages crates/behavior-models/src/work_queue/ (`mod.rs` declaring input, resolve, decls, lower, ownership, identity), models/conformance/`__init__.py` and models/examples/work_queue/`__init__.py`, and register `mod work_queue` in crates/behavior-models/src/lib.rs.

---

## Phase 2: Foundational — regression nets (plan phase 0; blocks everything)

- [X] T003 Add scripts/tests/test_pinned_identity_baseline.sh and register it with the script tests in scripts/gates.sh. It does two things:
  - it runs the existing gate `scripts/check-models-compat.py`. The reference cases are Python builders in models/fixtures/reference_cases.py, so a Rust test cannot rebuild them;
  - it compares `scripts/check-models-compat.py --print` byte-for-byte with the baseline captured in T001: every receipt, ModelHash, BehaviorHash and canonical document.

  Run it: it must pass on unchanged code. Any later failure means **stop and investigate**. Never edit the fixture or the baseline to pass (research R3 requirement 7).
- [X] T004 [P] Add scripts/tests/test_conformance_506_bytes.sh. It runs `scripts/conformance.sh quick` on the current code, stores `target/conformance/506-report.json` as the baseline in `target/conformance/baseline/`, and compares later runs byte-for-byte (FR-037, SC-008). Run it: it must pass.

---

## Phase 3: Foundational — D1(a) transition state bindings (plan phase 1; research R3, R13)

- [X] T005 Write failing tests in crates/behavior-models/tests/state_machine_bindings.rs:
  - (a) a transition with `state` bindings `{"doc": "Document"}` lowers with parameters subject, then bindings sorted by name, then inputs and context. An authored effect `doc.status := …` on a non-owned field is admitted;
  - (b) the machine's semantic version is 3 and its canonical format is `behavior.model.state_machine.v3`, with `state` on every transition, recorded as (name, 505 declaration identity), sorted by name;
  - (c) the same machine authored with the bindings in a different order has an identical ModelHash;
  - (d) a machine without bindings is version 2 with a canonical document byte-identical to today's;
  - (e) an authored effect writing the owned field through a binding of the subject's type is `TRANSITION_WRITES_OWNED_FIELD`;
  - (f) an unknown binding entity is `UNKNOWN_ENTITY` and a binding name colliding with an input or context name is `DUPLICATE_PARAMETER`.
- [X] T006 Write a failing test in crates/behavior-models/tests/ownership.rs. A hand-written module where a v3 transition of machine `m` binds a second entity of `m`'s subject type and a **generated** path would write its owned field through that binding must fail the final pass. In other words, the exemption covers only the generated status effect through the subject parameter (R3 requirement 4).
- [X] T007 Implement `TransitionInput.state: Vec<(String, String)>` in crates/behavior-models/src/state_machine/input.rs; binding resolution in state_machine/resolve.rs (unknown entity, duplicate names); and lowering in state_machine/lower.rs (state parameters after the subject, sorted by name; `TRANSITION_WRITES_OWNED_FIELD` through any parameter of the subject type).
- [X] T008 Implement the content-chosen semantic version in crates/behavior-models/src/identity.rs and lib.rs:
  - `STATE_MACHINE_SEMANTIC_VERSION` becomes the pair of language levels 2 and 3;
  - version 3 is chosen iff any transition has a binding;
  - the v3 canonical document lists `state` per transition with 505 identities;
  - the v2 path stays byte-identical.

  The receipt records the chosen version. T003 must still pass.
- [X] T009 Narrow the StateMachine exemption in `final_pass` in crates/behavior-models/src/composition.rs to "the owner's generated action writing the owned field through its subject parameter". Make T006 pass. T003 must still pass.
- [X] T010 Pass transition bindings through RequestFlow: crates/behavior-models/src/request_flow/input.rs and resolve.rs, with the same version rule (flow 3 with embedded machine 3 iff any transition binds; creation bindings unaffected), and request_flow/identity.rs (v3 format). Add tests in crates/behavior-models/tests/request_flow_composition.rs. T003 must still pass.
- [X] T011 Add `state=` to `StateMachine.transition` in python/behavior/models.py and `RequestFlow.transition` in python/behavior/request_flow.py, plus the glue in crates/behavior-py/src/models.rs (tracer binds the extra parameters by position after the subject). Add tests in python/tests/test_state_machine_bindings.py: a transition updating a bound `Document` atomically with the status; aliasing two bindings to one id behaves as Core decides; another model's ownership (an Approval decision field) rejects a write through a binding.
- [X] T012 Extend the compat gate. Add reference cases `sm_bind_*` and `rf_bind_*` (one language level per case) to models/fixtures/reference_cases.py, and record the new `state_machine` 3/1 and `request_flow` 3/1 tuples with `scripts/check-models-compat.py --write` (no `--supersede`). Add a self-test in scripts/tests/test_models_compat.sh: coexisting 2 and 3 tuples pass, and a case mixing standalone 2 and 3 machines fails "no single version tuple". `git diff models/fixtures/compat.json` must show only additions.

**Checkpoint**: T003 is green with zero changes to established tuples. D1(a) is usable by any model.

---

## Phase 4: Foundational — contribution kinds (plan phase 2; research R2)

- [X] T013 Write failing tests in crates/behavior-models/tests/composition.rs for two kind-neutral contribution kinds:
  - `Contribution::Invariant {model_id, name, entity, body}` is added after declarations;
  - `Contribution::Read {model_id, name, params, projection}` is added after definitions.

  Both are visible to hand-written behavior and collide with existing names as `DUPLICATE_NAME`. The 502 neutrality test is extended: no WorkQueue vocabulary in composition.rs, identity.rs, source_map.rs or ids.rs.
- [X] T014 Implement both kinds in crates/behavior-models/src/composition.rs and kinds.rs, through the public builder (`add_invariant`, `read_projection`, `add_read`), with deterministic ordering by (model id, name).

---

## Phase 5: Foundational — shared conformance package (plan phase 10; research R9)

- [X] T015 Move the kind-neutral parts of models/examples/purchase_approval/report.py into models/conformance/report.py: the statuses and outcome exceptions, readiness entries, `record_evidence`, the canonical writer, `gate_failures` and the pytest hooks. Parameterize them by feature id, declared scenarios, readiness registry and report path. models/examples/purchase_approval/report.py and conftest.py become thin users of the shared package. T004 must pass (the 506 report is byte-identical).
- [X] T016 Add the obligations machinery to models/conformance/report.py (data-model §7, research R10):
  - `obligation(vo, dependencies=[{class, check}], assumptions=[...])`;
  - a VO's status is the weakest of its dependencies' statuses, ordered `failed < blocked < inconclusive < unverifiable < unsupported < passed`;
  - `assumptions` never contribute a status;
  - `complete` requires every check and every obligation to be passed.

  Write the self-tests first in scripts/tests/test_conformance_report.sh: one failing dependency makes the VO fail; an assumption cannot make a VO passed; a VO missing from the run is `failed` ("not run"); a readiness entry with `run: true` whose check yields exactly the documented limitation is `blocked`, and any other result keeps its own status (FR-035, as in 506).
- [X] T017 Change scripts/conformance.sh to `conformance.sh <feature> [quick|full|complete]` (features 506 and 507; the report path per feature; the BEHAVIOR_CLI_BINARY rejection kept). Change scripts/gates.sh to run `conformance 506 quick` and `conformance 507 quick`, and update python/tests/test_gate_runner.py. Extend scripts/tests/test_conformance_report.sh to the feature argument.

---

## Phase 6: Foundational — WorkQueue core (plan phases 3–5, 7; blocks all stories)

- [X] T018 Write failing tests in crates/behavior-models/tests/work_queue_resolve.rs for every resolution error (research R5):
  - `INVALID_MAX_ATTEMPTS` for M = 0 and for M = i64::MAX;
  - `TIME_NOT_ORDERED` for plain `Int` and for a nominal without `order`;
  - `MISSING_AUTHORIZATION` for each of enqueue, claim, cancel, recover and exhaust;
  - `RESERVED_PARAMETER` for each generated name `worker`, `now`, `lease_until`, `available_at`, `generation`, `expected_generation` and `id`;
  - `PAYLOAD_FIELD_RESERVED` for each of `status`, `attempts`, `owner`, `lease_until` and `available_at`;
  - `MISSING_CAPABILITY` for each of the nine;
  - `WORK_ITEM_BINDING` for a binding of the queue's own entity type.
- [X] T019 Implement crates/behavior-models/src/work_queue/input.rs (`WorkQueueInput`, `CapabilityInput`, data-model §1) and work_queue/resolve.rs, which produces `ResolvedWorkQueue` with:
  - the names: `E = EncodeEntityName(id)`, `<E>Status`, actions `<id>__<capability>`;
  - the payload sorted by name;
  - the time type checked for `order`;
  - M;
  - per capability, the authored parameters sorted by name.

  Make T018 pass.
- [X] T020 Write failing golden tests in crates/behavior-models/tests/work_queue_lower.rs for the declarations of data-model §2:
  - the enum cases in the order `READY | LEASED | SUCCEEDED | FAILED | CANCELLED`;
  - the entity fields sorted by name, with `owner: Option<Id<A>>` and `lease_until: Option<T>`;
  - the invariant body exactly `ValidWork` (spec Context), with `⇔` as Bool equality and `⇒` as `¬a ∨ b`;
  - `<id>__terminal`;
  - the reads `<id>__ready(now)` and `<id>__expired(now)` with an empty projection item list, and `is_some` before every ordering against `lease_until`.
- [X] T021 Implement crates/behavior-models/src/work_queue/decls.rs: the enum and the entity (through the 503 contribution path), and the invariant, derived value and reads (through T014). Make T020 pass.
- [X] T022 Write failing golden tests in crates/behavior-models/tests/work_queue_lower.rs for every generated action of data-model §3. For each action they check:
  - the parameter order;
  - the generated requirements, in order, ahead of the authored guards;
  - the generated effects;
  - the generated postconditions (`attempts == generation` on renew, complete, retry and fail; `attempts == expected_generation + 1` on claim);
  - the source and target status sets (data-model §4);
  - that no action has a terminal source.

  These are the IR checks of premises P1 and P3–P7 in contracts/safety-arguments.md.
- [X] T023 Implement crates/behavior-models/src/work_queue/lower.rs:
  - enqueue as a creation: status READY literal, attempts 0, owner and lease `none`, `available_at`, payload; authored state bindings, effects and commands, as in RequestFlow;
  - the eight transitions as one embedded `ResolvedStateMachine` `(E, status, {READY}, 𝒯)`, lowered by the unchanged StateMachine lowerer, with the generated fragments of data-model §3 placed before the authored ones;
  - authored effects on the subject refused with `AUTHORED_WORK_EFFECT`;
  - authored transition bindings passed through D1(a).

  Make T022 pass.
- [X] T024 Write failing tests, then implement crates/behavior-models/src/work_queue/identity.rs and the `AnyModel::WorkQueue` dispatch in kinds.rs:
  - `WORK_QUEUE_SEMANTIC_VERSION = 1` and `WORK_QUEUE_LOWERER_VERSION = 1` in lib.rs;
  - the canonical document `behavior.model.work_queue.v1` (research R6: M, payload, worker and time by 505 identity, the capabilities with their parameters and Core-hashed bodies; no display or location);
  - the receipt with the generated declarations and the embedded machine identity and version;
  - the source map from capability fragments to Core items.

  Tests in crates/behavior-models/tests/work_queue_identity.rs.
- [X] T025 Implement the Python API python/behavior/work_queue.py (`WorkQueue` and its nine capability methods, research R7; `work_entity`; `terminal`), export it in python/behavior/`__init__.py`, and add the glue in crates/behavior-py/src/models.rs (the capability tracer binds parameters by position in the data-model §3 order). Add python/tests/test_work_queue_build.py: the reference queue builds and admits, every R5 error surfaces as a typed `ModelBuildError` with its code, and `module.model_receipts` contains the `work_queue` receipt.
- [X] T026 Create the 507 suite skeleton:
  - models/examples/work_queue/workflow.py: `Document`, `Worker`, `User`, the `Time` nominal with `order`, the reference queue `document_work` (M = 3, payload `{"document": Id[Document]}`, authorizations through an `admin` context), a RequestFlow `document_request` whose `submit` transition emits `ProcessDocument`, the command `DocumentProcessed` emitted on complete, and an effect on the bound `Document` (`processed := true`);
  - harness.py: store helpers in the style of 506, `now` given explicitly per step;
  - report.py: the declared scenarios W01–W22, W23a, W23b, W24 and W25, the obligations VO-001–VO-012 with their dependencies from the spec's VO table, and an empty readiness registry;
  - conftest.py;
  - expected.json: hand-written outcomes for every reference step of the spec's Reference history and every scenario.

**Checkpoint**: the queue builds and admits. `scripts/conformance.sh 507 quick` runs and reports every scenario as `failed` ("not run").

---

## Phase 7: User Story 1 — Author a queue and run work to completion (P1) 🎯 MVP

**Goal**: build, enqueue, read, claim and complete through the public interfaces.

**Independent Test**: `pytest models/examples/work_queue/test_us1_lifecycle.py` against
expected.json.

- [X] T027 [P] [US1] Write failing scenario tests in models/examples/work_queue/test_us1_lifecycle.py for spec US1 acceptance scenarios 1–6:
  - W01 (receipt fields; enqueue → READY, attempts 0, no owner, no lease);
  - W02 (`ready(now)` before and after `available_at`; claim before it denied);
  - W03 (claim with `expected_generation = 0`, `lease_until > now` → LEASED, owner, generation 1);
  - W05 (complete with the current generation while live → SUCCEEDED, owner and lease cleared);
  - FR-024 (enqueue with a used id → `ENTITY_ID_ALREADY_USED`, one item);
  - FR-029 (two `ready(now)` reads return the same ids and leave the head unchanged; two claims evaluated from that state, only one commits).
- [X] T028 [US1] Make T027 pass, fixing only crates/behavior-models/src/work_queue/ and python/behavior/work_queue.py. Record each deviation in implementation-log.md.
- [X] T029 [P] [US1] Write and pass python/tests/test_work_queue_enqueue.py:
  - the enqueue authorization guard denies an unauthorized actor;
  - authored enqueue state bindings and effects commit atomically with the creation;
  - an authored enqueue command is emitted only on commit.

**Checkpoint**: MVP. A queue can be authored and work completed.

---

## Phase 8: User Story 2 — Fence stale and unauthorized workers (P1)

**Goal**: only the current, committed claim may finish work.

**Independent Test**: `pytest models/examples/work_queue/test_us2_fencing.py`.

- [X] T030 [P] [US2] Write failing scenario tests in models/examples/work_queue/test_us2_fencing.py:
  - W04 (two claims from one head: exactly one commits, the other `STATE_CONFLICT`, repeated 20 times with different seeds);
  - W06 (another worker completes → DENY);
  - W07 (complete with generation k after a reclaim to k + 1, including by the same worker → DENY);
  - W08 (complete with `now = lease_until` and `now > lease_until` → DENY);
  - claim with a stale `expected_generation` → DENY;
  - a claim against a moved head → `STATE_CONFLICT`;
  - lease valid at evaluation, expired at commit: complete evaluated with `now < lease_until` and committed later with an unchanged head **succeeds**. The test asserts this documented boundary (spec, Time) and that the decision records the evaluation `now`;
  - concurrent recover and late complete from one head: at most one commits; a complete with `now ≥ lease_until` is denied regardless.
- [X] T031 [US2] Make T030 pass (fixes only in work_queue/ and the Python API).
- [X] T032 [P] [US2] Add property tests in crates/behavior-models/tests/work_queue_laws.rs (proptest, ≥ 1,000 cases) and python/tests/test_work_queue_properties.py (seeded, ≥ 200 sequences). They run random capability sequences with random workers, generations and `now` through the Store and check:
  - SA-MONO: attempts' ∈ {attempts, attempts + 1}, with +1 only on a committed claim;
  - SA-FENCE: a request with a generation below the committed one is never ALLOW;
  - ValidWork holds after every commit.

**Checkpoint**: fencing holds on scenarios and on random histories.

---

## Phase 9: User Story 3 — Retry, renew, recover, exhaust, fail and cancel (P2)

**Goal**: the remaining capabilities behave as the capability table says.

**Independent Test**: `pytest models/examples/work_queue/test_us3_capabilities.py`.

- [X] T033 [P] [US3] Write failing scenario tests in models/examples/work_queue/test_us3_capabilities.py, from constructed states. They cover:
  - W09 (renew to a later deadline, generation unchanged);
  - W10 (renew of an expired lease → DENY; renew to the same deadline → DENY);
  - W11 (retry with attempts < M and `available_at ≥ now` → READY at that time, attempts unchanged; retry with `available_at < now` → DENY);
  - W12 (retry at attempts = M → DENY);
  - W13 (recover an expired lease with attempts < M → READY; without authorization → DENY);
  - W14 (exhaust at attempts = M → FAILED; exhaust with attempts < M → DENY; recover at attempts = M → DENY);
  - W15 (cancel READY and LEASED with authority → CANCELLED; without it → DENY; a later completion by the former owner → DENY);
  - fail with a current lease → FAILED;
  - every capability on SUCCEEDED, FAILED and CANCELLED → DENY (FR-013);
  - the M = 1 edge case;
  - `expired(now)` lists exactly the expired leases.
- [X] T034 [US3] Make T033 pass (fixes only in work_queue/ and the Python API).

**Checkpoint**: all nine capabilities verified against the table.

---

## Phase 10: User Story 4 — The lifecycle cannot be bypassed (P2)

**Goal**: full-module ownership with typed build errors.

**Independent Test**: `cargo test -p behavior-models work_queue_ownership` and
`pytest models/examples/work_queue/test_us4_ownership.py`.

- [X] T035 [P] [US4] Write failing tests in crates/behavior-models/tests/work_queue_ownership.rs for the R4 table:
  - a hand-written action writing each owned field (`payload` fields, `attempts`, `owner`, `lease_until`, `available_at`) → `UNAUTHORIZED_WORK_WRITE`;
  - a hand-written creation of `E` → `UNAUTHORIZED_WORK_CREATION`;
  - a removal → `UNAUTHORIZED_WORK_REMOVAL`;
  - a `status` write → the StateMachine error;
  - a write through a non-subject binding of type `E` from another model's transition (D1(a)) → `UNAUTHORIZED_WORK_WRITE`;
  - an Approval or RequestFlow action writing `E` → rejected;
  - an allowed writer writing a field outside its row (for example renew writing `available_at`) → rejected.
- [X] T036 [US4] Implement crates/behavior-models/src/work_queue/ownership.rs as in research R4: per (action, parameter of type `E`, field), every action of the final module, no reliance on any exemption, allowed writers only through their subject. Make T035 pass.
- [X] T037 [P] [US4] Write and pass scenario tests in models/examples/work_queue/test_us4_ownership.py:
  - W16 (each forbidden writer and each `WORK_ITEM_BINDING` through Python, with its typed code);
  - W22 (a second model claiming `<id>__claim` or the entity name → composition error);
  - W25 (two queues `a` and `b` sharing `Worker` and `Time`: queue `a`'s actions cannot write `b`'s items at build, both invariants hold after a mixed history, and the two ModelHashes are distinct and reported separately);
  - the RequestFlow composition (US4 AS6: an emitted `ProcessDocument` followed by a separate enqueue referencing the request id; two ordinary commits).

**Checkpoint**: every forbidden writer fails the build. Several queues are independent.

---

## Phase 11: User Story 5 — Replay, verify and compare across bindings (P3)

**Goal**: replay, identity, cross-binding equality, lost-ack, atomic effects, verification and
evidence.

**Independent Test**: `BEHAVIOR_FULL_PROPERTIES=1 scripts/conformance.sh 507 full`.

- [X] T038 [P] [US5] Write models/examples/work_queue/run.py (as in 506). It:
  - runs the reference history steps 1–10;
  - checks each step's result, conflict and position, and the full final state (statuses, attempts, owners, the bound `Document`, the committed commands), against expected.json;
  - runs data and behavior replay;
  - exits 1 on any mismatch.
- [X] T039 [P] [US5] Write scenario tests in models/examples/work_queue/test_us5_replay.py:
  - W17 (both replays of the reference history reproduce the state and every decision);
  - W18 (a recorded record with an altered `now`, an altered `lease_until` in the evidence, or an altered worker in the context → replay or integrity verification fails);
  - W21 (direct evaluation with a parent snapshot missing `Worker` → Core rejects);
  - W23a (resubmitting the original claim bundle after a dropped acknowledgement → `already: true`, the original record, no new record);
  - W23b (re-evaluating instead → DENY for claim, `ENTITY_ID_ALREADY_USED` for enqueue);
  - W24 (complete with the bound `Document` effect and `DocumentProcessed`: the candidate is exactly those effects and intents; the Store commits them atomically; nothing is in the command stream before commit).
- [X] T040 [P] [US5] Write W19 tests in crates/behavior-models/tests/work_queue_identity.rs:
  - the ModelHash changes with M, the payload schema, an authorization guard and a field added to `Worker` (505 exact dependency);
  - the BehaviorHash changes where semantics change;
  - display names, descriptions and authoring order change nothing.
- [X] T041 [US5] Write crates/behavior-models/tests/work_queue_reference.rs (W20, research R12). It authors the reference queue through `WorkQueueInput` directly, independently of the Python glue, and writes its ModelHash and BehaviorHash to `BEHAVIOR_CONFORMANCE_DIR` (default `target/conformance`). models/examples/work_queue/test_us5_agreement.py then compares them with the Python build. Extend scripts/conformance.sh 507 to run the Rust half first, and to report its failure as in 506.
- [X] T042 [US5] Write models/examples/work_queue/test_obligations.py. It fills every VO dependency of report.py from its source (research R10):
  - **SMT**, full mode only: the attestation's per-(action, kind) outcomes — `preservation` for every action (VO-001 enqueue, VO-002, VO-004, VO-006), `postcondition` on claim, renew, complete, retry and fail (VO-003, VO-005 local parts), `evaluation_error` on claim (VO-004). A non-proven result keeps its own status, and a Core limitation becomes `blocked` with its issue through the readiness registry (FR-035);
  - **structural**: SA-MONO, SA-FENCE, SA-TERM, SA-OWN and SA-EXCL pass only when every premise check (T022, T035, T036) and every negative control (T043) passes;
  - **test**: the named scenario checks;
  - **contract**: the genesis rejection (`GENESIS_INVALID` for an invalid imported item), `STATE_CONFLICT` (W04), the backend conformance run (full mode) and replay (W17);
  - **genesis provenance** (spec §Genesis and provenance): a genesis with an imported LEASED item at generation 2 and an expired deadline. Its deadline is not treated as live (`expired(now)` lists it); after recover and a new claim (generation 3), a request with generation 2 is denied; the report records "imported lease: trusted state, no recorded claim" as an assumption of VO-001.

  Every obligation that relies on time, Store atomicity or the migration contract lists its assumptions: "now is the trusted evaluation time", "ordinary transitions only" and "conforming backend".
- [X] T043 [US5] Write the negative controls in models/examples/work_queue/test_negative_controls.py (contracts/safety-arguments.md, FR-036). Each variant is a **hand-authored module** built only inside the test, never through WorkQueue. It mirrors the generated IR of data-model §2–§3 with exactly one premise removed, following the probe pattern of research R1. The test asserts that the corresponding check **fails** on the variant and passes on the generated module:
  - NC-writer: the mirror plus a hand-written action that lowers `attempts`, which the WorkQueue build rejects. On the variant, an old generation completes;
  - NC-generation: `attempts <= generation` in place of `==`; W07 then allows the old token;
  - NC-expiry: no `now < lease_until`; W08 then allows the late completion;
  - NC-bound: without the claim guard `attempts < M` **and** without the bound conjuncts of `ValidWork`, a fourth claim commits (attempts = 4 > M). With the invariant present and only the guard removed, the fourth claim is refused by the invariant instead of a DENY. This shows that the bound rests on `ValidWork` (premise P6);
  - NC-mig-reset: a Core migration resetting `attempts`; an old generation satisfies `Current` again;
  - NC-mig-reactivate: a migration READY → LEASED with the same generation and owner; an old token completes. ValidWork holds in both migration controls.

  Variants are built in the test only. The shipped lowering never has a switch.
- [X] T044 [US5] Add to models/README.md a WorkQueue section and a "Transition state bindings" subsection under StateMachine. The README must cover:
  - the meaning, the capability table, the trust boundary (Time) and the ownership table;
  - genesis and provenance;
  - the migration contract (rules 1–7: enforced by Core vs host obligations);
  - the guarantees and non-guarantees;
  - the language levels (R13);
  - the principle that a binding never grants authority.

**Checkpoint**:
- full mode: every VO has its dependencies; the SMT dependencies are passed or `blocked` with a Core issue; the negative controls fail as intended;
- `run.py` exits 0.

---

## Phase 12: Polish & cross-cutting

- [X] T045 Add `wq_*` reference cases (one with transition bindings, embedded machine 3; one without, embedded machine 2; two queues in one module) to models/fixtures/reference_cases.py. Record the `work_queue` tuples with `scripts/check-models-compat.py --write`. `git diff models/fixtures/compat.json` must show only additions, and T003 must be green.
- [X] T046 [P] Update api/public-api.json (`WorkQueue`, `state=` on transitions) and scripts/check-terms.sh if WorkQueue tests need allowed vocabulary (never the word `plugin`).
- [X] T047 [P] Measure `scripts/conformance.sh 507 quick` (reviewed budget ≤ 35 s; originally ≤ 30 s, explicitly amended in plan Performance Goals). Share the reference-history walk and immutable command export without weakening any check. Keep 8 quick histories and 200 full histories. Enforce the budget in the runner after report generation, and record measurements and the reason for the amendment in implementation-log.md.
- [X] T048 Run the quickstart.md commands 1–7 and record the results in implementation-log.md. Include the 506 byte comparison (T004) and `git diff --stat models/fixtures/compat.json` (additions only).
- [X] T049 Run `scripts/gates.sh` and `BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh`, sequentially and in the background. Both must reach `gates: OK`. Record the conformance summaries of 506 and 507 in implementation-log.md.

---

## Dependencies & Execution Order

```text
Setup (T001–T002)
  → Regression nets (T003–T004)
    → D1(a) bindings (T005–T012) ──┐
    → Contribution kinds (T013–T014) ├─→ WorkQueue core (T018–T026)
    → Shared conformance (T015–T017) ┘
         → US1 (T027–T029)  MVP
         → US2 (T030–T032)     ┐
         → US3 (T033–T034)     ├─ independent of each other once the core exists
         → US4 (T035–T037)     ┘
         → US5 (T038–T044)  needs US1–US4 (the evidence references their checks)
           → Polish (T045–T049)
```

- T003 must stay green from T005 on. A red T003 stops the work.
- T015 needs T004, and T017 needs T015.
- T023 needs T007–T011 (bindings) and T014. T026 needs T025 and T017.
- T042 needs T022, T035, T036 and T043.

## Parallel Opportunities

- Phase 3 and Phase 4 are independent: T005–T012 and T013–T014 touch different files, except
  composition.rs. Run T009 and T014 sequentially.
- Phase 5 (T015–T017) is Python and script work, parallel to the Rust phases 3–4.
- After Phase 6: US2, US3 and US4 in parallel (different test files; implementation fixes in
  different areas of work_queue/).
- In US5: T038, T039 and T040 in parallel.

## Parallel Example: User Story 2

```text
Task: "T030 [US2] fencing scenarios in models/examples/work_queue/test_us2_fencing.py"
Task: "T032 [US2] property tests in crates/behavior-models/tests/work_queue_laws.rs and python/tests/test_work_queue_properties.py"
```

## Implementation Strategy

1. **Safety nets first**: T003 and T004 are taken before any change and stay green.
2. **Shared infrastructure next**: D1(a), the contribution kinds and the shared report. Each is
   independently useful and is checked against the baseline.
3. **MVP = US1**: a queue that can be authored, claimed and completed.
4. **Then US2** (the core safety promise), **US3 and US4** in parallel, and **US5** last, since
   its evidence aggregates the earlier checks.
5. **Stop rules**:
   - an established identity changes → stop and investigate (requirement 7);
   - a Core limitation in a verifier result → `blocked` with a Core issue, and never a model
     change to make it green (FR-035);
   - a capability Core cannot express → a planning blocker (FR-039).
