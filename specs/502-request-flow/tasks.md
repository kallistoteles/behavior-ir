---

description: "Task list for feature 502: RequestFlow Model"
---

# Tasks: RequestFlow Model

**Input**: Design documents from `specs/502-request-flow/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/{declaration-staging, request-flow-contract}.md. Builds on feature 501 (branch
`501-model-lowering-state-machine`).

**Tests**: Required. Spec §41 lists 20 required tests, and constitution III (Test-First) is
non-negotiable. In every phase, write the tests first and see them fail.

**Organization**: Grouped by user story (spec.md), so each story can be implemented and checked
on its own.

**Hard rules for every task**:
- No change to behavior-ir-core.
- `behavior-engine` is the only Core dependency.
- `into_wire` only in `crates/behavior-models/src/request_flow/create.rs`; `to_wire_json` and
  serde serialization nowhere in the model crate.
- Python records input only; Rust decides.
- **Never** copy StateMachine logic: transitions go through the 501 functions (research R6).
- Any apparent Core need → STOP and classify it (missing public facade exposure versus
  genuinely missing semantics).

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: module skeleton, guard narrowing, versions and error codes.

- [X] T001 Narrow the wire guard in scripts/check-public-surface.sh: `to_wire_json` and `serde_json::to_string` of Core types stay forbidden everywhere in crates/behavior-models/src. `into_wire` is allowed **only** in crates/behavior-models/src/request_flow/create.rs. Negative-test both: a probe file elsewhere using `into_wire` → FAIL; a probe in create.rs → PASS (research R4).
- [X] T002 Create crates/behavior-models/src/request_flow/{mod.rs,input.rs,resolve.rs,decls.rs,create.rs,ownership.rs,identity.rs} with module docs that cite the contracts, and register `pub mod request_flow;` in crates/behavior-models/src/lib.rs. In the same task, update the 501 neutrality test `the_model_crate_is_domain_neutral` in crates/behavior-models/tests/composition.rs so its scope is exactly the shared modules: `state_machine/`, `ids.rs`, `composition.rs`, `identity.rs` and `source_map.rs`. `request_flow/` holds the kind's own terms. `lib.rs` (the kind registry) and `error.rs` (the shared catalogue of every kind's codes) are outside the scan. Without this, the test fails as soon as `request_flow/` exists.
- [X] T003 Add `REQUEST_FLOW_SEMANTIC_VERSION: u32 = 1` and `REQUEST_FLOW_LOWERER_VERSION: u32 = 1` to crates/behavior-models/src/lib.rs (data model §9).
- [X] T004 Add the error codes to crates/behavior-models/src/error.rs, each with its stage, payload, `code()`, `to_json()` and sort key. Extend tests/errors.rs to cover each.

  | Codes | Stage |
  |---|---|
  | `UNKNOWN_ENUM`, `RESERVED_FIELD`, `NO_CREATION`, `CAPABILITY_ID_COLLISION`, `INITIALIZER_MISSING_FIELD`, `INITIALIZER_UNKNOWN_FIELD`, `INITIALIZER_SETS_STATUS`, `INITIALIZER_DUPLICATE_FIELD` | resolve / lower |
  | `DECLARATION_COLLISION`, `DECLARATION_MISMATCH` | set |
  | `UNAUTHORIZED_REQUEST_CREATION`, `UNAUTHORIZED_REQUESTER_WRITE` | compose |

  There is no `MULTIPLE_REQUEST_CREATION`: creation lowering emits exactly one `Create`, and extra effects can't create entities, so it would be unreachable (SC-008). T020 asserts it with `debug_assert!`, and T014 tests it structurally (each creation action has exactly one `creates()` entry).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: entity-name encoding, declaration staging (P2) and the supplied-versus-internal
model sets. Every story depends on these.

**⚠️ CRITICAL**: no story work before this phase is complete.

### Tests (write first, see them fail)

- [X] T005 [P] Add `EncodeEntityName` tests to crates/behavior-models/tests/ids.rs:
  - golden: `purchase_request` → `PurchaseRequest`, `a_1b` → `A_1b`, `a1b` → `A1b`, `x_2_y` → `X_2Y`;
  - a proptest (≥ 2,000 cases, `RngSeed::Fixed(502)`) over `[a-z][a-z0-9]{0,4}(_[a-z0-9]{1,4}){0,4}`: injective, `decode_entity_name ∘ encode_entity_name = id`, and every result matches `[A-Za-z_][A-Za-z0-9_]*` (research R3, SC-010).
- [X] T006 [P] Write the staging-law tests in two files. The 501 reference cases are defined in Python, so their hash regression is a Python test.
  - **D2 (Rust),** crates/behavior-models/tests/request_flow_staging.rs: `declare_models` makes no builder call for StateMachine-only sets (the builder's `finish` result is identical before and after, including the wire module);
  - **D2 (Python),** python/tests/test_model_staging.py: every case of models/fixtures/compat.json's state_machine tuple, rebuilt through `BehaviorModule` with the P2 call active, has its pinned BehaviorHash and ModelHash;
  - **D3:** permuting the order of authored entity declarations gives an identical BehaviorHash;
  - **D4:** a builder whose request entity differs from `Decls(m)` → `DECLARATION_MISMATCH`;
  - **D6:** an authored entity with the generated name and a different field list → `DECLARATION_COLLISION`; an identical authored entity is accepted.

### Implementation

- [X] T007 Implement `encode_entity_name(&SemanticId) -> String` and `decode_entity_name(&str) -> Option<SemanticId>` in crates/behavior-models/src/ids.rs, exactly as in research R3: capitalize letter-led runs and prefix digit-led runs with `_`. Document the injectivity proof in rustdoc.
- [X] T008 Generalize `ModelSpec` in crates/behavior-models/src/lib.rs to a model-kind enum: `ModelInput::StateMachine(..)` (existing) and `ModelInput::RequestFlow(RequestFlowInput)`. Keep every 501 call site and test compiling unchanged by giving `ModelSpec` constructors for the existing shape.
- [X] T009 Implement the declaration merge in crates/behavior-models/src/composition.rs (`pub fn declare_models(builder, &[ModelSpec]) -> Result<Vec<DeclaredSchema>, Vec<ModelError>>`):
  - compute `Decls(m)` for each model in canonical model-id order (StateMachine → none; if the union is empty, return without touching the builder, law D2);
  - snapshot P1 via `builder.finish(None)`;
  - apply identical-or-error per research R2: identical → skip; differing entity or any other item kind → `DECLARATION_COLLISION`;
  - `declare_entity` the rest;
  - return `DeclaredSchema { model_id, entity, fields }`.
- [X] T010 In `compose` (crates/behavior-models/src/composition.rs), re-derive `Decls(m)` and require the admitted snapshot to hold an identical declaration (same entity name and Core entity hash), else `DECLARATION_MISMATCH`. Split the model set into *supplied* (receipts, `DUPLICATE_MODEL_ID`) and *internal* (supplied StateMachines ∪ embedded `SM(RF)`; ownership, lowering) (research R9).
- [X] T011 Expose `declare_models` and `encode_entity_name` through crates/behavior-py/src/models.rs. Call `declare_models` in python/behavior/module.py `BehaviorModule.__init__` after authored enums, nominals and entities, and **before** commands and before tracing derived values, invariants, constraints, actions and reads (P2, declaration-staging contract §1). Commands come after P2 because their payloads may name the generated entity. Add stubs to python/behavior/_engine.pyi.
- [X] T012 Run `cargo test -p behavior-models` and `python -m pytest -q python/tests models`, and confirm T005–T006 pass and every 501 test is unchanged.

**Checkpoint**: staging is in place, and StateMachine builds are byte-identical to 501.

---

## Phase 3: User Story 1 - Author a request flow and run its lifecycle as ordinary behavior (Priority: P1) 🎯 MVP

**Goal**: a flow lowers to a request entity, creation actions and the embedded StateMachine's
transition actions, and runs through the public engine.

**Independent Test**: spec US1: the full reference lifecycle, erasure, and equivalence with a
standalone StateMachine.

### Tests (write first, see them fail)

- [X] T013 [P] [US1] Write crates/behavior-models/tests/request_flow_resolve.rs, one test per data-model §3 rule:
  - `INVALID_SEMANTIC_ID`, `UNKNOWN_ENTITY` (actor), `UNKNOWN_ENUM`;
  - `RESERVED_FIELD` (`requester`, `status`, and `assignee` when enabled);
  - `INVALID_PARAMETER` (duplicate payload field, creation parameter named `request_id`);
  - `NO_CREATION`, `CAPABILITY_ID_COLLISION` (creation id = transition id), `UNKNOWN_STATE` (creation target);
  - schema golden: fields sorted by name, `requester: Id(A)`, `status: Enum(T_s)`, `assignee: Option(Id(A))` only when enabled, **never `Ref`** (FR-003);
  - entity name = `encode_entity_name(flow_id)`.
- [X] T014 [P] [US1] Write crates/behavior-models/tests/request_flow_create.rs:
  - golden creation layout per data model §5: params `request_id: Input Id(E)`, then inputs, state and context each sorted by name; pre `G_c`; one `creates()` entry with `status = lit(q_c)` plus initializer values; effects `E_c`; post `P_c`; commands `K_c`;
  - action name `encode(f, c)`;
  - erasure: the module holds only the authored items, `E`, and `f__*` actions;
  - structurally, every creation action has exactly one `creates()` entry, of type `E`.
- [X] T015 [P] [US1] Write the equivalence tests in crates/behavior-models/tests/request_flow_create.rs:
  - (a) SC-006: for the reference flow, build a standalone StateMachine over the same generated entity, field `status`, subject param `f`, entry states `{q_c}` and transitions, and assert every transition action's Core action hash is equal (research R6);
  - (b) the constitution's equivalence pair: build a **hand-written module**, with an authored entity of the same name and canonically sorted fields, hand-written creation actions with the same parameters, preconditions, `Create`, effects and postconditions, and the same standalone StateMachine. Assert that the whole module's BehaviorHash equals the RequestFlow module's (the 501 worked-example pattern, for flows).
- [X] T016 [P] [US1] Write python/tests/test_request_flow.py with the reference lifecycle through `behavior.evaluate` (with complete snapshot facts and an identity fact for creation):
  - `create` → Draft with requester, payload and `assignee = none`;
  - then submit → start_review (sets the assignee) → return → resubmit → start_review → approve, each committing exactly the expected `status` change;
  - `approve` from Draft gives Core's ordinary denial;
  - the wire module contains only ordinary items (SC-001).

### Implementation

- [X] T017 [US1] Implement `RequestFlowInput`, `CreationInput` and `CreationFragments` in crates/behavior-models/src/request_flow/input.rs (data model §1). Add a `FragmentSource` extension so a binding can trace creation scopes (`trace_creation`) besides transition scopes.
- [X] T018 [US1] Implement `resolve` in crates/behavior-models/src/request_flow/resolve.rs → `ResolvedRequestFlow` (data model §2–§4), with every §3 error, the canonical schema, the creation parameter order of §4, and `sm_input(RF)`. `sm_input` is the **only** place `entry_states = { q_c }` is computed, with `subject_entity = encode_entity_name(f)`, `state_field = "status"`, `subject_param = Some(f)`. The machine is then resolved with `state_machine::resolve::resolve` (no copy).
- [X] T019 [US1] Implement `Decls(RF)` in crates/behavior-models/src/request_flow/decls.rs from the resolved schema only, without lowering (law D1), and wire it into `declare_models`.
- [X] T020 [US1] Implement creation lowering in crates/behavior-models/src/request_flow/create.rs:
  - push the creation scope (§4), call `trace_creation`, and run the static initializer checks (data model §5: missing, unknown, `status`, duplicate);
  - build `lit(Enum(T_s), q_c)`;
  - convert the request id and value nodes with `into_wire()` into one `WLifecycle::Create`;
  - call `add_action` or `add_action_with_commands`, mapping `DUPLICATE_NAME` → `GENERATED_NAME_COLLISION`;
  - pop the scope, also on error.
- [X] T021 [US1] Integrate RequestFlow into `compose` in crates/behavior-models/src/composition.rs: resolve each flow, add `SM(RF)` to the internal set, lower creations, then lower the embedded machine's transitions with `state_machine::lower::lower_transition` (unchanged), in canonical `(model_id, construct id)` order.
- [X] T022 [US1] Python: implement `RequestFlow` in python/behavior/request_flow.py:
  - `RequestFlow(flow_id, *, actor, status, payload, assignee=False, display_name=None)`;
  - `.create(id, *, to, inputs=None, state=None, context=None, values, requires=(), effects=(), ensures=(), emits=())`;
  - `.transition(...)`, same as `StateMachine.transition`.
  It records raw input only. `request_entity` is a class with an `EntityDecl` whose name is `_engine.encode_entity_name(flow_id)` and whose fields are filled from P2's `DeclaredSchema`; the type view is checked against the Python-side declared types and raises on drift (research R11). Implement creation and transition tracers binding by position (data model §4). Export `RequestFlow` from python/behavior/__init__.py and add it to api/public-api.json.
- [X] T023 [US1] Extend crates/behavior-py/src/models.rs to accept RequestFlow input dicts and to call the Python creation tracer (`{initializer, guards, effects, postconditions, commands}`).
- [X] T024 [US1] Run `cargo test -p behavior-models` and `python -m pytest -q python/tests/test_request_flow.py`, and confirm T013–T016 pass.

**Checkpoint**: a RequestFlow runs end to end as ordinary Core behavior (MVP).

---

## Phase 4: User Story 2 - The lifecycle guarantee cannot be bypassed (Priority: P1)

**Goal**: the ownership matrix (research R7) and derived entry states hold under composition
with hand-written behavior.

**Independent Test**: spec US2: manual create, status write and requester write are rejected,
an assignee write is accepted, and entry states are derived.

### Tests (write first, see them fail)

- [X] T025 [P] [US2] Write crates/behavior-models/tests/request_flow_ownership.rs:
  - creations `create_draft → Draft`, `import_submitted → Submitted` → the embedded machine's `entry_states == {Draft, Submitted}`, and `RequestFlowInput` has no entry-state field (compile-level);
  - a hand-written create of `E` → `UNAUTHORIZED_REQUEST_CREATION { action }`;
  - status written by a hand-written action, by a creation's `E_c` over an `S_c` binding of type `E`, or by a transition's extra effect → `UNAUTHORIZED_OWNED_FIELD_WRITE` / `TRANSITION_WRITES_OWNED_FIELD`;
  - requester written by a hand-written action, by a transition effect, or by `E_c` over `S_c: E` → `UNAUTHORIZED_REQUESTER_WRITE`;
  - assignee written by a hand-written action and by a transition effect → accepted.
- [X] T026 [P] [US2] Add initializer tests to crates/behavior-models/tests/request_flow_create.rs: missing `requester`, missing payload field, missing `assignee` when enabled, unknown field, `status` in the initializer, a field given twice → the exact `INITIALIZER_*` error naming the creation and field.
- [X] T027 [P] [US2] Write python/tests/test_request_flow_ownership.py mirroring T025–T026 through `BehaviorModule(models=[flow], actions=[...])`, asserting `ModelBuildError.errors` codes and payloads.
- [X] T028 [P] [US2] Write the lifecycle trace property in python/tests/test_request_flow_lifecycle.py (SC-003, research R12). With a seeded `random.Random(502)`, generate ≥ 1,000 traces of random creations and transitions, including invalid ones, through the public engine. Apply committed changes to an in-test state, and assert that the first status ∈ `Q_entry` and every observed status change ∈ `R_T`.

### Implementation

- [X] T029 [US2] Implement the RequestFlow ownership rules from research R7 in crates/behavior-models/src/request_flow/ownership.rs. `final_pass` in crates/behavior-models/src/composition.rs calls them through a kind-neutral hook, without naming request concepts:
  - every `creates()` of `E` must be in `CreationActions(RF)`;
  - no effect writes `(p : E, requester)`.
  The status rules stay the existing 501 code applied to the internal machine set.
- [X] T030 [US2] Run `cargo test -p behavior-models --test request_flow_ownership` and `python -m pytest -q python/tests/test_request_flow_ownership.py python/tests/test_request_flow_lifecycle.py`, and confirm T025–T028 pass.

**Checkpoint**: `trace ∈ Q_entry · R_T*` is enforced and tested.

---

## Phase 5: User Story 3 - Explicit authority, data, time and notifications (Priority: P2)

**Goal**: no hidden semantics. Authority, time and notifications come only from explicit
fragments; `S_c` and `E_c` are atomic.

**Independent Test**: spec US3.

### Tests (write first, see them fail)

- [X] T031 [P] [US3] Write python/tests/test_request_flow_authority.py:
  - without an `approve` guard, the requester, the assignee and any other actor may all approve;
  - with `requires=[lambda r, actor: actor == r.assignee]`, only the assignee may;
  - a `cancel` guard on the requester behaves the same way;
  - creation and transition guards true / false / `DIVISION_BY_ZERO`, giving Core's ordinary outcomes and no request created on failure.
- [X] T032 [P] [US3] Add tests to python/tests/test_request_flow_authority.py:
  - commands emitted on creation and on approve are in the same commit; a denied step emits nothing;
  - a `created_at` payload field from `context.now` stores the given value, and the wire module contains no clock or identity generation.
- [X] T033 [P] [US3] Add tests to python/tests/test_request_flow_authority.py:
  - an atomic creation with `state={"budget": Budget}` and `effects=[lambda request_id, …, budget: set_(budget.reserved, …)]` commits the request and the budget change in one decision;
  - a failing postcondition rolls both back;
  - two state bindings aliasing one entity give Core's ordinary binding error (no RequestFlow rule);
  - removing an actor referenced as requester or assignee is allowed (`Id`, not `Ref`).

### Implementation

- [X] T034 [US3] Support command emissions on creations through `trace_creation` → `CreationFragments.commands` in crates/behavior-py/src/models.rs and python/behavior/request_flow.py (`emits=[... emit(...)]`), reusing the 501 `Emission` type.
- [X] T035 [US3] Run `python -m pytest -q python/tests/test_request_flow_authority.py` and confirm T031–T033 pass. If any requires behavior beyond the contract (§2), STOP and report it instead of adding semantics.

---

## Phase 6: User Story 4 - Deterministic identity, provenance and reproducibility (Priority: P2)

**Goal**: RequestFlowHash, one receipt per flow with an embedded identity record, a provenance
chain, and compatibility pinning.

**Independent Test**: spec US4.

### Tests (write first, see them fail)

- [X] T036 [P] [US4] Write crates/behavior-models/tests/request_flow_identity.rs:
  - a golden canonical document per data model §7: enum referenced by `enum_hash`, embedded `state_machine.document` + `model_hash`, creations with initializer hashes, `S_c`/`I_c`/`C_c` sorted, no entity name or schema hashed separately;
  - RequestFlowHash = `tagged_hash("behavior.model.v1", doc)`;
  - every tested semantic change changes it (payload type, creation target, initializer, guard, transition, assignee flag, `S_c`).
- [X] T037 [P] [US4] Add proptests (≥ 1,000 cases, fixed seed 502) to crates/behavior-models/tests/request_flow_identity.rs: random permutations of payload, creations, transitions, inputs, state bindings, context and source states, and random metadata → equal RequestFlowHash, module and BehaviorHash (SC-005).
- [X] T038 [P] [US4] Add receipt and source-map tests to crates/behavior-models/tests/request_flow_identity.rs:
  - one supplied flow → exactly one receipt (`model_kind: "request_flow"`) with `embedded: [{kind: "state_machine", id: f, model_semantic_version, lowerer_version, model_hash}]`;
  - source-map entries: creation → `f/creation/<c>`; transition → `f/transition/<t>` with `embedded: "state_machine:f/transition/<t>"`; entity → `f/request_entity`; fields → `f/request_entity/<field>`;
  - relative locations; metadata never changes either hash.
- [X] T039 [P] [US4] Extend scripts/tests/test_models_compat.sh:
  - a manifest with both tuples passes;
  - editing an RF case hash → FAIL;
  - an RF tuple whose embedded StateMachine lowerer version changed must be a new tuple (an old entry with a changed hash → FAIL);
  - the 501 tuple stays byte-identical.

### Implementation

- [X] T040 [US4] Implement the RequestFlow canonical document and RequestFlowHash in crates/behavior-models/src/request_flow/identity.rs, reusing the kind-neutral helpers of crates/behavior-models/src/identity.rs (fragment hash reading, `model_hash`):
  - read creation fragment hashes from the admitted creation action: preconditions, effects, postconditions, command emissions, and `creates()[0].fields()` excluding `status`;
  - embed `identity::canonical_document` and `model_hash` of `SM(RF)` (no second encoding).
- [X] T041 [US4] Implement the RequestFlow receipt, the embedded identity record and source-map entries in crates/behavior-models/src/request_flow/identity.rs. Use the kind-neutral receipt and relative-path helpers of crates/behavior-models/src/source_map.rs, and run graph diagnostics for the embedded machine through the existing `diagnostics`, under the flow's `model_ref`.
- [X] T042 [US4] Extend `LoweringReceipt` in python/behavior/models.py with an optional `embedded` field (present only for flows) and update its drift check deliberately.
- [X] T043 [US4] Extend scripts/check-models-compat.py: group cases by tuple key; a tuple key includes an `embedded` member only when receipts have one (so the 501 key is unchanged); RF case entries pin `behavior_hash`, `models`, `embedded`, `entities` (research R10).
- [X] T044 [US4] Add the reference cases `rf_purchase_request`, `rf_two_flows_local_submit` and `rf_with_standalone_machine` to models/fixtures/reference_cases.py (request-flow contract §5), and record the new tuple with `--write`. Verify the 501 tuple is unchanged with `git diff models/fixtures/compat.json`.
- [X] T045 [US4] Run the US4 tests, `scripts/tests/test_models_compat.sh` and `scripts/determinism-check.sh`, and confirm T036–T039 pass.

---

## Phase 7: User Story 5 - Flows compose with each other, with state machines and with hand-written behavior (Priority: P3)

**Goal**: order-independent composition, collisions as errors, and generated declarations
visible to hand-written behavior (law D5).

**Independent Test**: spec US5.

### Tests (write first, see them fail)

- [X] T046 [P] [US5] Write python/tests/test_request_flow_composition.py, covering visibility (D5), the integration you emphasized. Hand-written:
  - an action with `request: flow.request_entity` and an input `Id[flow.request_entity]`;
  - an authored entity with `Id[flow.request_entity]` and `Ref[flow.request_entity]` fields;
  - a read and a query (`select(flow.request_entity)`);
  - a guard reading request fields.
  All are admitted and behave as ordinary Core.
- [X] T047 [P] [US5] Add composition tests to crates/behavior-models/tests/request_flow_composition.rs:
  - two flows with `submit` → both `f__submit` actions;
  - a flow and a standalone machine with the same id, or two flows with one id → `DUPLICATE_MODEL_ID`;
  - a standalone machine claiming the flow's `(E, status)` → `CONFLICTING_OWNERSHIP`;
  - a hand-written action named `f__create` → `GENERATED_NAME_COLLISION`.
  - (Spec US5 scenario 2, a creation id equal to a transition id, is covered by T013's `CAPABILITY_ID_COLLISION` test and not repeated here.)
- [X] T048 [P] [US5] Add a proptest (≥ 1,000 cases) to crates/behavior-models/tests/request_flow_composition.rs: random supply order of flows and machines, and random authored declaration order → an identical module and BehaviorHash, or an identical sorted error list.

### Implementation

- [X] T049 [US5] Fix any composition gaps the tests reveal in crates/behavior-models/src/composition.rs. Expected: none beyond T009–T010 and T021.
- [X] T050 [US5] Run `python -m pytest -q python/tests/test_request_flow_composition.py` and `cargo test -p behavior-models --test request_flow_composition`, and confirm T046–T048 pass.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T051 [P] Add a RequestFlow section to models/README.md: the meaning, the lifecycle theorem, the ownership matrix, staging (P1–P4), the API with `request_entity`, and the Id-not-Ref rule.
- [X] T052 [P] Re-run and, if needed, tighten the domain-neutrality test from T002 in crates/behavior-models/tests/composition.rs after all stories. Confirm that `state_machine/`, `ids.rs`, `composition.rs`, `identity.rs` and `source_map.rs` contain no request terms, so a future model can reuse them.
- [X] T053 [P] Add a Behavior Models line for RequestFlow to README.md.
- [X] T054 Run every quickstart.md scenario, including the StateMachine regression and `scripts/check-core-pin.sh`.
- [X] T055 Run `scripts/gates.sh` and record the result in specs/502-request-flow/implementation-log.md, with the pinned RF hashes, any deviations, and the anti-pattern audit evidence per plan row.
- [X] T056 Run `/speckit-converge` against specs/502-request-flow/ and resolve any findings before the PR.

---

## Dependencies & Execution Order

```text
Setup → Foundational (staging, encoding) → US1 ─┬─> US2 ─┐
                                                ├─> US3 ─┼─> US5 → Polish
                                                └─> US4 ─┘
```

- US1 is the MVP. US2 needs US1's generated actions; US3 needs US1's creation tracer;
  US4 needs US1's admitted actions; US5 needs US2's ownership rules and US4's fixtures.
- Within each story: tests → input/resolve → lowering → composition → binding → verification.
  Files shared within a story (`composition.rs`, `request_flow.py`) are edited sequentially.

### Parallel opportunities

- Setup: T003 and T004.
- Foundational tests: T005 and T006.
- Each story's test tasks (marked [P]).
- After US1: US2, US3 and US4 can proceed in parallel.
- Polish: T051–T053.

## Parallel Example: User Story 1

```bash
Task: "RequestFlow resolution rules in crates/behavior-models/tests/request_flow_resolve.rs"  # T013
Task: "Creation layout golden test in crates/behavior-models/tests/request_flow_create.rs"   # T014
Task: "Reference lifecycle through the public engine in python/tests/test_request_flow.py"   # T016
```

## Implementation Strategy

1. **MVP:** Setup, then Foundational (with the 501 regression test green), then US1, then
   validate.
2. **Release with US2:** US2 is also P1, so don't release RequestFlow without the bypass
   guarantees.
3. **Then:** US3 and US4 (in parallel), then US5, then Polish, then converge.

**Stop conditions**: any Core need; any test that needs Python to decide semantics; any
temptation to copy StateMachine code. Fix the Rust side or stop and report.

---

## Phase 9: Convergence

- [X] T057 Add an exhaustive test to python/tests/test_request_flow.py: for every status × transition pair of the reference flow (models/fixtures/rf_purchase_request.py), the transition commits iff the status is in its source set and its guards hold; and for every creation, the new request has exactly its declared initial status. The lifecycle property (T028) only checks that committed changes are edges, not that every enabled transition commits per SC-002 (partial)
- [X] T058 Add a test to python/tests/test_request_flow.py that `RequestFlow.bind_schema` raises `RuntimeError` when the Python view of `request_entity` differs from the P2 `DeclaredSchema` (entity name, a field type, a missing or extra field), and that `BehaviorModule` surfaces it per plan: research R11 drift check (partial)
- [X] T059 Add tests in python/tests/test_request_flow_ownership.py for a flow with `assignee=False` whose initializer sets `assignee` (→ `INITIALIZER_UNKNOWN_FIELD`) and whose transition effect sets `assignee` (→ a blocking unknown-field error) per spec Edge Cases "Assignee disabled" (partial)
- [X] T060 In the deferred P2 path of `declare_models` in crates/behavior-models/src/kinds.rs, report `UNKNOWN_ENTITY` (actor) and `UNKNOWN_ENUM` (status) instead of `CORE_ADMISSION_FAILED` when they are the cause, without a Core change; if that is not possible on the public API, record the deviation in specs/502-request-flow/data-model.md §3 and keep the test in crates/behavior-models/tests/request_flow_staging.rs pinning it per plan: data model §3 (partial)
- [X] T061 Add scripts/tests/test_check_public_surface.sh (run by scripts/gates.sh): a probe using `into_wire` outside crates/behavior-models/src/request_flow/create.rs → FAIL, inside it → PASS; and make scripts/check-public-surface.sh also reject `serde_json::to_string`/`to_vec` in crates/behavior-models/src per T001 (partial)

---

## Phase 10: Convergence

- [X] T062 Correct the comment above the wire guard in scripts/check-public-surface.sh: the model crate does use `serde_json::to_value` for `WType` in canonical documents (crates/behavior-models/src/identity.rs `params`, specified as `"type": <WType>` in data-model §7), so say that the guard forbids serialization to text or bytes (`to_string`/`to_vec`/`to_writer`), not "anything with serde"; record the same precision for the tasks.md hard rule in specs/502-request-flow/implementation-log.md per T061 (partial)
