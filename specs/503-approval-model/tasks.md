---

description: "Task list for feature 503: Approval Model"
---

# Tasks: Approval Model

**Input**: Design documents from `specs/503-approval-model/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/{approval-contract, declaration-dag}.md, quickstart.md. Builds on feature 502 (branch
`502-request-flow`, PR #17).

**Tests**: Required. The spec lists 30 required regression cases (spec §User Stories, plan test
table), and constitution III (Test-First) is non-negotiable. In every phase, write the tests
first and see them fail.

**Organization**: grouped by user story, so each story can be implemented and checked on its own.

**Hard rules for every task**:
- No change to behavior-ir-core. `behavior-engine` is the only Core dependency.
- `into_wire` is allowed only in `crates/behavior-models/src/request_flow/create.rs` and
  `crates/behavior-models/src/approval/lower.rs`. `to_wire_json` and serialization to text or
  bytes are forbidden everywhere in the model crate.
- Python records input only; Rust decides thresholds, names, counts, outcome and ownership.
- The decision action delta is exactly `CreateEntity(X)` plus command intents. Never add
  effects, status writes or subject mutation.
- Terminology: **DENIED** = the attempt was not permitted; **REJECT** = the recorded choice;
  **REJECTED** = the derived outcome. Test names, assertions and diagnostics use exactly these.
- Any apparent Core need → STOP and classify it ("existing Core semantics but missing public
  API" versus "genuinely unrepresentable semantic requirement").

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: module skeleton, guard extension, versions and error codes.

- [X] T001 Extend the wire guard in scripts/check-public-surface.sh so `into_wire` is allowed in exactly two files: crates/behavior-models/src/request_flow/create.rs and crates/behavior-models/src/approval/lower.rs (research R8). Add probes to scripts/tests/test_check_public_surface.sh:
  - `into_wire` appended to approval/lower.rs → PASS;
  - `into_wire` in approval/resolve.rs → FAIL;
  - all existing probes unchanged.
- [X] T002 Create crates/behavior-models/src/approval/{mod.rs,input.rs,resolve.rs,decls.rs,lower.rs,ownership.rs,identity.rs}, with module docs citing data-model.md and contracts/approval-contract.md, and register `pub mod approval;` in crates/behavior-models/src/lib.rs. Extend the neutrality test in crates/behavior-models/tests/composition.rs so that approval vocabulary is also forbidden in the shared modules: `decision`, `approve`, `reject`, `threshold`, `approval`. The scanned files are `state_machine/`, `ids.rs`, `composition.rs`, `identity.rs` and `source_map.rs`.
- [X] T003 [P] Add `APPROVAL_SEMANTIC_VERSION: u32 = 1` and `APPROVAL_LOWERER_VERSION: u32 = 1` to crates/behavior-models/src/lib.rs (data model §10).
- [X] T004 [P] Add these error codes to crates/behavior-models/src/error.rs, each with its stage, payload, `code()`, `to_json()` and sort key, and extend crates/behavior-models/tests/errors.rs to produce each (data model §11):

  | Code | Stage | Payload |
  |---|---|---|
  | `INVALID_THRESHOLD` | resolve | `model_ref`, `which` (`approve` / `reject`), `value` |
  | `DECLARATION_CYCLE` | set | `models` (sorted) |
  | `DEFINITION_CYCLE` | set | `names` (sorted) |
  | `DEFINITION_MISMATCH` | set | `name`, `model_ref` |
  | `UNAUTHORIZED_DECISION_CREATION` | compose | `action`, `entity`, `owner` |
  | `UNAUTHORIZED_DECISION_WRITE` | compose | `action`, `entity`, `field`, `owner` |
  | `UNAUTHORIZED_DECISION_REMOVAL` | compose | `action`, `entity`, `owner` |

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**:
- the model-declaration DAG (common infrastructure);
- definitions with signatures first and bodies in dependency order;
- generated-name injectivity;
- the 501 and 502 regression law.

Every story depends on these.

**⚠️ CRITICAL**: no story work before this phase is complete. The regression test T005 is
written and passing against the *current* code first; it must stay green through every later
task.

### Tests (write first, see them fail where new)

- [X] T005 Write the regression law D9 in python/tests/test_model_staging.py. The reference-case builders are Python-only (models/fixtures/reference_cases.py `CASES`, the same ones scripts/check-models-compat.py drives), so the regression lives there. Generalize the existing `test_state_machine_cases_keep_their_pinned_hashes` into `test_pinned_cases_keep_their_hashes`, parametrized over every case of every tuple in models/fixtures/compat.json (the 501 `state_machine` tuple and the 502 `request_flow` tuple). Build each case through the binding and assert that BehaviorHash, every ModelHash, every embedded hash and every generated entity name are byte-identical to the pinned values. Also add a Rust unit test in crates/behavior-models/src/composition.rs (`#[cfg(test)]`): for a StateMachine-only set, P2 makes no builder call. Both must pass before T009 and after every later task.
- [X] T006 [P] Unit tests for the topological order in crates/behavior-models/src/composition.rs (`#[cfg(test)]`), against a pure function `topo_order(nodes: BTreeSet<String>, edges: BTreeSet<(String, String)>) -> Result<Vec<String>, Vec<String>>` (contracts/declaration-dag.md §2):
  - with no edges, the order is model-id order;
  - a chain `b → a` gives `[b, a]`;
  - in a diamond, ready ties are taken in id order;
  - a cycle `a ⇄ b`, plus an independent `c`, returns `Err(["a", "b"])`;
  - a proptest (≥ 1,000 cases) over random DAGs and random insertion orders gives identical output (law D7), and every edge is respected.
- [X] T007 [P] Unit tests for definition ordering in crates/behavior-models/src/composition.rs (`#[cfg(test)]`), against `DefinitionSpec { model_id, name, kind, params, result, deps }` (contracts/declaration-dag.md §2, law D10):
  - signatures are available before any body is built;
  - bodies are added only after all their `deps`;
  - ready ties go by `(model id, name)`;
  - a cycle `f → g → f` gives `DEFINITION_CYCLE { names: [f, g] }` and adds nothing;
  - a definition name already claimed gives `GENERATED_NAME_COLLISION`;
  - **D4′ mismatch**: P2 adds a definition, then a different derived item with the same name is put in the builder before P4 (for example the body of `p__approved` with `k_A` changed). `compose_models` gives `DEFINITION_MISMATCH { name: "p__approved", model_ref }` and produces no module. The identical re-derived item is accepted;
  - a proptest (≥ 1,000 cases) shows the order is a function of the definition *set*.
- [X] T008 [P] Naming property test in crates/behavior-models/tests/ids.rs, using the existing semantic-id strategy (≥ 2,000 cases, fixed seed). With `C(p) = EncodeEntityName(p) ++ "DecisionChoice"` and `X(p) = EncodeEntityName(p) ++ "Decision"`, for all generated `p`, `q`:
  - `p ≠ q ⇒ C(p) ≠ C(q) ∧ X(p) ≠ X(q)`;
  - `C(p) ≠ X(q)`;
  - both are valid Core identifiers.

  Also add a golden test: `purchase_request_approval` → `PurchaseRequestApprovalDecisionChoice`, `PurchaseRequestApprovalDecision`; `a_1b` → `A_1bDecision`, `a1b` → `A1bDecision`. Plus a cross-kind example: RequestFlow `x_decision` and approval `x` both produce `XDecision` (asserted as equal names here, and rejected as a collision in T040).

### Implementation

- [X] T009 Generalize `DeclaredSchema` in crates/behavior-models/src/composition.rs into `Contribution = Enum { model_id, name, cases, loc } | Entity { model_id, entity, fields, loc }`, keeping the entity variant's behavior byte-identical. Enum merge is identical-or-error: same cases in the same order (compare the Core enum hash after declaring) or `DECLARATION_COLLISION`. `declare_enum` re-declaration of an identical enum is a no-op in Core (research R6). T005 stays green.
- [X] T010 Implement `topo_order` (T006) and use it in `declare_models` in crates/behavior-models/src/kinds.rs:
  - each kind provides `contrib(input) -> Vec<Contribution>` and `needs(input) -> BTreeSet<type name>`, computed from the input alone (law D1);
  - an edge `M_i → M_j` exists iff `needs(M_j)` meets the names of `contrib(M_i)`;
  - contributions are merged in topological order;
  - a cycle gives `DECLARATION_CYCLE` before anything is declared;
  - the 502 forward-reference fallback (`declare_then_admit`) declares in topological order too (contracts/declaration-dag.md §4).

  StateMachine has `contrib = ∅`. RequestFlow keeps its current contribution and `needs = ∅` among model types (its actor and enum are authored). T005, T006 and the 502 staging tests stay green.
- [X] T011 Implement definitions in crates/behavior-models/src/composition.rs and kinds.rs (T007):
  - after all contributions, collect every kind's `DefinitionSpec`s;
  - claim every name;
  - return the signatures alongside the declared schemas from `declare_models`;
  - then build and `add_derived` the bodies in `G_F` order.

  In `compose_models`, re-derive each definition and require the builder's derived item to have the same Core hash (law D4′), otherwise `DEFINITION_MISMATCH`. T007's mismatch case turns green here. Sets without definitions make no extra builder call (D9).
- [X] T012 [P] Add `pub fn decision_names(id: &SemanticId) -> (String, String)` (choice enum, decision entity) to crates/behavior-models/src/ids.rs, defined exactly as in data model §3 on top of `encode_entity_name` (T008 green).
- [X] T013 Extend `declare_models` in crates/behavior-py/src/models.rs and python/behavior/module.py so P2 also returns the definition signatures to the binding. The P1 → P2 → P2b → P3 → P4 order is unchanged. python/tests/test_model_staging.py stays green.

**Checkpoint**: the DAG, definitions and naming are in place; 501 and 502 builds are byte-identical.

---

## Phase 3: User Story 1 - Record decisions and derive a threshold outcome (Priority: P1) 🎯 MVP

**Goal**:
- `approve` and `reject` record decisions;
- the five derived values give counts and the outcome;
- one decision per actor;
- terminal outcomes are stable;
- the three-layer terminality holds, including on the child state.

**Independent Test**: `doc_approval` over an authored `Document` with actor `User`, under
thresholds (1,⊥), (2,⊥), (2,1), (3,2) and (1,1), driven through the public engine and Store
(quickstart scenarios 1–6, 13).

### Tests for User Story 1 (write first, see them fail)

- [X] T014 [P] [US1] Resolution tests in crates/behavior-models/tests/approval_resolve.rs, one per data model §3 rule:
  - `INVALID_SEMANTIC_ID`;
  - `UNKNOWN_ENTITY` for the subject and for the actor;
  - `INVALID_THRESHOLD` for `k_A = 0` and for `k_R = 0`;
  - `RESERVED_FIELD` for payload `subject`, `actor`, `choice` and `id`;
  - `INVALID_PARAMETER` for a duplicate payload field, for a parameter named `subject` or `decision_id`, and for a missing or extra capability.

  Also a golden test of the schema: fields sorted by name, `actor: Id(A)`, `choice: Enum(C)`, `subject: Id(S)`, never `Ref`. And a golden test of the parameter layout (data model §4).
- [X] T015 [P] [US1] Golden test of the derived bodies in crates/behavior-models/tests/approval_lower.rs. For `k_R = ⊥` and for `k_R = 1`, the five derived items of the admitted module have exactly the data model §5 structure:
  - `approved` excludes `rejected` explicitly;
  - `rejected = lit(false)` when `k_R = ⊥`;
  - their parameter is `s: Entity(S)`, role Read.

  Assert their mutual exclusivity on states that violate P as well, by evaluating them over seeded snapshots with arbitrary counts.
- [X] T016 [P] [US1] Golden test of the action layout in crates/behavior-models/tests/approval_lower.rs (data model §6):
  - preconditions at fixed positions `[0] Open`, `[1] p__pending(subject)`, `[2] Eligible`, `[3] ¬any(… subject … actor)`, `[4..] G_c`;
  - exactly one `creates()` of `X`, with `choice = lit(C, APPROVE | REJECT)` fixed by the capability and `subject = subject.id`;
  - `effects()` and `removes()` are empty;
  - postconditions `[0]` uniqueness and `[1]` threshold consistency, the latter only if `k_R ≠ ⊥`.
- [X] T017 [US1] **Child-state backstop test** in crates/behavior-models/src/approval/lower.rs (`#[cfg(test)] mod tests`, so it can call the crate-internal lowering function; there is no production switch). This is quickstart scenario 6. Build the decision action with the real lowering function, omitting only the preconditions. Run it through the public `behavior_engine::store::Store` with `InMemoryBackend`, from parents chosen so that each check passes on `S` and fails only on `S′ = Apply(S, CreateEntity(d))`:
  - (a) one existing decision `(s, a)`; approve by `a` again: `Count_S = 1 ≤ 1` but `Count_S′ = 2` → DENIED (postcondition), no commit, head unchanged;
  - (b) `k_A = 2, k_R = 2` with `A = 2, R = 1`; reject: false on `S′` only → DENIED, no commit;
  - (c) control: the same action with the postconditions also omitted commits the invalid state in (a) and in (b). This proves the postconditions are evaluated on the child state and are what stops it.

  Seed the parents through `Store::create` genesis (the base-state rule: such seeds are test fixtures, not valid Approval states).
- [X] T018 [P] [US1] End-to-end tests in python/tests/test_approval.py (quickstart 1–4), each named for its outcome:
  - `test_thresholds_*` for (1,⊥), (2,⊥), (2,1), (3,2) and (1,1): the outcome after each step equals the Outcome table, read through `approval.approved/rejected/pending` and the counts in a hand-written read;
  - `test_one_decision_per_actor_second_approve_is_denied`;
  - `test_approve_then_reject_is_denied`;
  - `test_veto_first_reject_is_rejected`;
  - `test_no_reject_threshold_rejections_count_but_stay_pending`;
  - `test_decision_after_approved_is_denied`;
  - `test_decision_after_rejected_is_denied`;
  - `test_lowered_module_has_no_approval_constructs`: every item of the lowered module is an ordinary Core enum, entity, derived value or action (with its pre- and postconditions), and no item kind, field or runtime construct goes beyond those (spec US1 scenario 10). Generated names that contain the approval id are allowed. The test also checks admission by the public engine.
- [X] T019 [P] [US1] Concurrency test in python/tests/test_approval_concurrency.py (quickstart 5), through the public Store binding (python/behavior/store.py). From a parent with `A = k_A − 1, R = k_R − 1`:
  - evaluate an approve by `a₁` and a reject by `a₂` against the same parent;
  - commit the approve → success;
  - commit the reject against the old parent → `STATE_CONFLICT`, nothing applied;
  - re-evaluate the reject on the new head → DENIED by the terminal guard.

  Also the parallel same-actor variant: two approvals by `a₁` with different `decision_id`s from one parent → the second gets `STATE_CONFLICT`, and its retry is DENIED by `¬HasDecision`.
- [X] T020 [P] [US1] Property suite in python/tests/test_approval_properties.py (SC-003, quickstart 13), seeded, ≥ 1,000 random sequences over 1–4 actors, random choices, random thresholds in {(1,⊥),(2,⊥),(2,1),(3,2),(1,1)}, and random Open and Eligible results. After every step:
  - no `(s, a)` pair has two decisions;
  - never `A ≥ k_A ∧ R ≥ k_R`;
  - once terminal, the outcome never changes;
  - `approved ⇒ A ≥ k_A` and at least `k_A` distinct actors approved;
  - `rejected ⇒ k_R ≠ ⊥ ∧ R ≥ k_R`.

  Include concurrent pairs on one parent (as in T019).
- [X] T021 [P] [US1] Inductive-step property in crates/behavior-models/tests/approval_laws.rs (proptest, ≥ 1,000 cases, fixed seed):
  - for a random valid parent `S` (`P(S)` holds) and a random decision action invocation, `ALLOW ⇒ P(S′)`;
  - for a random hand-written action that passes final composition (T032), `D_S′ = D_S`.

  This is the executable form of FR-008b.

### Implementation for User Story 1

- [X] T022 [P] [US1] Implement `ApprovalInput`, `CapabilityInput` and `DecisionFragments` in crates/behavior-models/src/approval/input.rs (data model §1), plus the `FragmentSource` scope call for a capability.
- [X] T023 [US1] Implement `resolve` in crates/behavior-models/src/approval/resolve.rs → `ResolvedApproval` (data model §2–§4):
  - names via `decision_names`;
  - schema sorted by field name;
  - parameters in the §4 order;
  - every §3 error, with `k_A ≥ 1` and `k_R ≥ 1` checked exactly.

  T014 green.
- [X] T024 [US1] Implement `contrib`, `needs` and the five `DefinitionSpec`s in crates/behavior-models/src/approval/decls.rs (data model §3 and §5, with `deps` as tabulated):
  - `contrib = [Enum C {APPROVE, REJECT}, Entity X]`;
  - `needs = {S, A}`.

  Register the kind as `AnyModel::Approval` in crates/behavior-models/src/kinds.rs with its `model_id`, `contrib`, `needs` and definitions. T015 green.
- [X] T025 [US1] Implement decision lowering in crates/behavior-models/src/approval/lower.rs (data model §6): open the capability scope; trace the fragments; build:
  - the preconditions in fixed order;
  - one `WLifecycle::Create`, using `into_wire` here only;
  - the postconditions (uniqueness always, threshold consistency iff `k_R ≠ ⊥`);
  - commands via `add_action_with_commands` under the command profile.

  Any Core rejection → `FRAGMENT_INVALID` with the capability's location. Lower capabilities in order approve, reject. T016 and T017 green.
- [X] T026 [US1] Wire Approval into `compose_models` in crates/behavior-models/src/kinds.rs:
  - resolution;
  - D4 and D4′ checks;
  - lowering in canonical model-id order;
  - ownership (T032) and identity (T047) hooks are added by their stories.
- [X] T027 [US1] Add the approval input, the decision-scope tracer and the definition handles to crates/behavior-py/src/models.rs, following the RequestFlow pattern (`any_model`, `fragments`, `trace_creation`).
- [X] T028 [US1] Implement python/behavior/approval.py (contracts/approval-contract.md §4):
  - `Approval(id, subject=, actor=, approve_threshold=, reject_threshold=, payload=, open_when=, eligible=)`;
  - `.approve(...)` and `.reject(...)` with `actor=`, `values=`, `inputs=`, `context=`, `requires=` and `emits=`; `actor` is an explicit callable;
  - `.decision_entity`, a typed handle drift-checked against the P2 schema like `RequestFlow.bind_schema`;
  - `.approve_count/.reject_count/.approved/.rejected/.pending(x)` → `derived_ref` to the signature from P2, which accepts only a bound entity of the subject type.

  Callables bind by position (data model §4). Export it from python/behavior/__init__.py. T018–T020 green.

**Checkpoint**: US1 runs end-to-end; all three terminality layers are tested, including the
child-state discrimination test (T017).

---

## Phase 4: User Story 2 - Decisions are immutable audit facts owned by the approval (Priority: P1)

**Goal**: complete structural ownership over every action of the final module (the premise of
the induction), no subject mutation, point-in-time eligibility.

**Independent Test**: build the approval with each bypass and check the exact errors; check
that subject state is unchanged (quickstart 7–8).

### Tests for User Story 2 (write first, see them fail)

- [X] T029 [P] [US2] Ownership tests in crates/behavior-models/tests/approval_ownership.rs, one per semantic path to `X` (data model §7 completeness), each asserting the exact error, action and field:
  - a hand-written action creating `X` → `UNAUTHORIZED_DECISION_CREATION`;
  - a hand-written action with a State parameter of type `X` writing `subject`, `actor`, `choice` and a payload field (4 cases) → `UNAUTHORIZED_DECISION_WRITE`;
  - a hand-written removal of `X` → `UNAUTHORIZED_DECISION_REMOVAL`;
  - a **StateMachine whose subject is `X`** with state field `choice` → `UNAUTHORIZED_DECISION_WRITE` on its transitions;
  - a **RequestFlow creation with a state binding of type `X`** whose effects write an `X` field → `UNAUTHORIZED_DECISION_WRITE`;
  - **another approval's action**: approval `q` cannot create `X_p`. Assert by construction (its create targets `X_q`) and that the check does not exempt it;
  - a hand-written read, query, derived value and guard over `X` → accepted.
- [X] T030 [P] [US2] Completeness test in crates/behavior-models/tests/approval_ownership.rs: for every action of every reference module (`ap_*`, T048), the union of `creates()`, `effects()` and `removes()` touching `X` lies within `p`'s two actions and consists only of their single `creates()`. This is the exact premise used by T021.
- [X] T031 [P] [US2] End-to-end tests in python/tests/test_approval_ownership.py:
  - `test_subject_untouched_by_every_decision`: subject fields are equal before and after every successful decision (SC-005);
  - `test_eligibility_change_after_decision_still_counts`: the requester becomes the deciding actor's identity afterwards;
  - `test_reused_decision_id_is_denied_by_core`;
  - `test_actor_without_entity_is_recorded`;
  - `test_decision_survives_subject_and_actor_removal`.

### Implementation for User Story 2

- [X] T032 [US2] Implement the ownership check in crates/behavior-models/src/approval/ownership.rs (data model §7) over **every** action of the admitted module, with no exemption by origin. It resolves effect and removal parameters to `WType::Entity(X)` through `params()`, as 502's requester rule does. Only `encode(p, approve)` and `encode(p, reject)` may create `X_p`; no action may write or remove `X_p`. Call it from the final pass in crates/behavior-models/src/kinds.rs, after the StateMachine and RequestFlow passes. T029 and T030 green.
- [X] T033 [US2] Document the completeness argument as the module doc of crates/behavior-models/src/approval/ownership.rs, citing `ActionItem` (`effects`, `creates`, `removes`), no cascade, and `Id` rather than `Ref` (research R5).

**Checkpoint**: P1 complete (MVP). The induction's premises (base state, local preservation,
complete effect ownership) are all tested.

---

## Phase 5: User Story 3 - Explicit window, eligibility, payload and commands (Priority: P2)

**Goal**:
- Open, Eligible and the per-choice guards are explicit Core predicates;
- the typed payload;
- commands;
- no implicit authorization.

**Independent Test**: quickstart scenarios for the open window, eligibility, payload and
commands through the public engine.

### Tests for User Story 3 (write first, see them fail)

- [X] T034 [P] [US3] Initializer tests in crates/behavior-models/tests/approval_lower.rs:
  - a missing payload field → `INITIALIZER_MISSING_FIELD`;
  - an unknown field, and each of `subject`, `actor`, `choice`, `id` → `INITIALIZER_UNKNOWN_FIELD`;
  - a duplicate field → `INITIALIZER_DUPLICATE_FIELD`;
  - an actor expression not of type `Id(A)` → `FRAGMENT_INVALID`;
  - a payload value of the wrong type → `FRAGMENT_INVALID`.

  Each names the capability and the field.
- [X] T035 [P] [US3] End-to-end tests in python/tests/test_approval_policy.py:
  - `test_open_false_is_denied`;
  - `test_eligible_false_is_denied`;
  - `test_eligible_evaluation_error_is_core_evaluation_error` (division by zero in Eligible → Core `EVALUATION_ERROR`, nothing recorded);
  - `test_reject_guard_only_applies_to_reject` (`G_R` false, `G_A` true);
  - `test_unrestricted_approval_accepts_any_actor` (no implicit `actor != requester` or `actor == assignee`; a context parameter named `actor` has no special meaning);
  - `test_payload_does_not_affect_counts`;
  - `test_decided_at_from_context` (two decisions with different context times; no clock in the module);
  - `test_command_emitted_only_on_commit`;
  - `test_denied_decision_emits_no_command`.

### Implementation for User Story 3

- [X] T036 [US3] Implement the initializer checks in crates/behavior-models/src/approval/lower.rs, before any builder call (data model §6). They reuse the 502 `INITIALIZER_*` error variants, with `model_ref` pointing at the approval capability. T034 green.
- [X] T037 [US3] Complete `open_when`, `eligible` (receiving the traced actor expression as `actor`), `requires` and `emits` in python/behavior/approval.py and the tracer in crates/behavior-py/src/models.rs. Python adds nothing semantic. T035 green.

**Checkpoint**: US3 is independently testable on `doc_approval`.

---

## Phase 6: User Story 4 - Composition with RequestFlow, StateMachine and other approvals (Priority: P2)

**Goal**:
- approval predicates guard RequestFlow transitions;
- the declaration DAG orders the flow before the approval;
- approvals compose with each other;
- a non-request subject works;
- cycles and collisions are errors.

**Independent Test**: `ap_purchase_request`, `ap_two_approvals` and `ap_document_review` in
every supply order, plus the cycle and collision cases (quickstart 9–11).

### Tests for User Story 4 (write first, see them fail)

- [X] T038 [P] [US4] DAG tests in crates/behavior-models/tests/declaration_dag.rs:
  - an approval over a RequestFlow request entity, supplied in both orders → identical module, and the flow entity is declared before `X`;
  - two approvals whose subjects are each other's decision entities → `DECLARATION_CYCLE { models: [a, b] }` in every supply order, with nothing declared;
  - **acyclic review of a decision**: approval `a` over an authored `Document`, and approval `b` whose subject is `X_a` (spec Edge Cases) → builds in both supply orders, with an identical module; `C_a` and `X_a` are declared before `C_b` and `X_b`, and a `b__approve` commits on an existing `X_a` decision;
  - an approval over an authored entity → no edges, model-id order.
- [X] T039 [P] [US4] End-to-end tests in python/tests/test_approval_composition.py:
  - `test_finalize_approved_denied_until_approved_then_commits`;
  - `test_finalize_rejected_after_veto`;
  - `test_status_stays_in_review_while_outcome_approved` (spec §30);
  - `test_two_approvals_on_one_subject_are_independent` (four capabilities: `finance_approval__approve`, `finance_approval__reject`, `legal_review__approve`, `legal_review__reject`);
  - `test_approval_on_document_without_request_flow`;
  - `test_handwritten_behavior_uses_decision_entity_and_predicates` (law D5: an action with `Id[approval.decision_entity]` input, a read over `X`, and a guard `approval.approved(doc)`);
  - `test_predicate_handle_rejects_non_subject_argument`.
- [X] T040 [P] [US4] Collision tests in crates/behavior-models/tests/declaration_dag.rs:
  - RequestFlow `x_decision` together with approval `x` → `DECLARATION_COLLISION` on `XDecision`, from the common merge, nothing renamed;
  - an authored entity named `PurchaseRequestApprovalDecision` → `DECLARATION_COLLISION`;
  - a hand-written derived value named `p__approved` → `GENERATED_NAME_COLLISION`;
  - an approval and a flow with the same id → `DUPLICATE_MODEL_ID`.

### Implementation for User Story 4

- [X] T041 [US4] Make RequestFlow's `needs` and `contrib` exact in crates/behavior-models/src/request_flow/decls.rs, so an approval over a flow's request entity gets its edge: `contrib = [Entity E]`, `needs` = the entity and enum types its fields reference that are model-produced. Keep T005 green.
- [X] T042 [US4] Ensure `declare_then_admit` (crates/behavior-models/src/composition.rs) handles authored forward references to `X` and `C` in topological order, then adds the definitions (contracts/declaration-dag.md §4). Add the D5 Python case to python/tests/test_model_staging.py.

**Checkpoint**: all composition directions work; the DAG and collisions are deterministic.

---

## Phase 7: User Story 5 - Deterministic identity, provenance and reproducibility (Priority: P2)

**Goal**: ApprovalHash, one receipt per approval, a source map including the
predicate/guard/check provenance, and compat pinning.

**Independent Test**: permutations, metadata changes and semantic changes; receipt and source
map inspection; the compat gate (quickstart 12, 14).

### Tests for User Story 5 (write first, see them fail)

- [X] T043 [P] [US5] Golden test of the canonical document in crates/behavior-models/tests/approval_identity.rs (data model §8):
  - format `behavior.model.approval.v1`;
  - subject and actor carried as `{entity, entity_hash}`;
  - thresholds, with `null` for `⊥`;
  - payload sorted;
  - capabilities sorted, each with actor, open, eligible, guards, payload and commands hashes.
- [X] T044 [P] [US5] Semantic-change tests in crates/behavior-models/tests/approval_identity.rs. ApprovalHash changes for each of:
  - `k_A`, `k_R`, `⊥ ↔ 1`;
  - payload type;
  - subject entity (including a changed RequestFlow payload of the subject);
  - actor entity;
  - Open, Eligible, a `G_A`, the actor expression, a payload initializer, a command.

  It does **not** change for display names, descriptions or source locations.
- [X] T045 [P] [US5] Permutation property in crates/behavior-models/tests/approval_identity.rs (≥ 1,000 cases, fixed seed): permute payload order, input and context order, and the supply order of approvals, flows, state machines and hand-written items. ApprovalHash, the declarations of `C` and `X`, the module and BehaviorHash are identical (SC-006).
- [X] T046 [P] [US5] Receipt and source-map tests in crates/behavior-models/tests/approval_identity.rs:
  - exactly one receipt per supplied approval, with `generated.{enum, entity, derived}` (data model §9);
  - source-map entries for `approve` and `reject` → the actions;
  - each predicate → its derived item;
  - `uniqueness` / `threshold_consistency` → postconditions `[0]` / `[1]`;
  - `terminal_guard` → precondition `[1]`;
  - `decision` / `choice` → `X` / `C`;
  - every path relative.

### Implementation for User Story 5

- [X] T047 [US5] Implement the canonical document, ApprovalHash, receipt and source map in crates/behavior-models/src/approval/identity.rs, using only the kind-neutral helpers of crates/behavior-models/src/identity.rs and source_map.rs. Fragment hashes are read from the admitted actions at the fixed positions of data model §6. Hook it into `compose_models` receipts in kinds.rs. Keep `receipt_id` working for `approval_id`. T043–T046 green.
- [X] T048 [US5] Add the reference cases `ap_document_review`, `ap_purchase_request` and `ap_two_approvals` (contracts/approval-contract.md §5) to models/fixtures/reference_cases.py, with their builders in a new models/fixtures/approval.py.
- [X] T049 [US5] Extend scripts/check-models-compat.py with the `approval` tuple key `(approval, 1, 1, Core 0.13.0)`, plus embedded and co-supplied versions per the 502 grouping. Pin the three cases in models/fixtures/compat.json: BehaviorHash, ApprovalHash, the `C` and `X` names and hashes, and the derived hashes. The existing tuples stay byte-identical (diff only adds lines).
- [X] T050 [US5] Gate tests in scripts/tests: an edited approval hash → FAIL; a changed lowerer output without a version bump → FAIL; an approval case requires its tuple key (SC-010).

**Checkpoint**: all stories complete.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T051 [P] Terms check: extend scripts/check-terms.sh so approval test files and diagnostics use DENIED for refused attempts. Fail on `rejected` used for a refused attempt in python/tests/test_approval*.py and crates/behavior-models/tests/approval_*.rs (for example the patterns `is_rejected` or `-> rejected` next to a denial assertion), allowing REJECT/REJECTED only in choice and outcome contexts. Add a negative probe in scripts/tests.
- [X] T052 [P] Document Approval in models/README.md:
  - meaning;
  - the three-layer terminality;
  - the inductive guarantee with its base-state assumption;
  - the boxed principle: global safety = valid base state + local preservation + complete effect ownership;
  - the non-goals.
- [X] T053 [P] Update api/public-api.json and python/tests/test_public_api.py for `Approval` and its handles.
- [X] T054 Re-run the neutrality test (T002) and the D9 regression (T005); confirm both are green.
- [X] T055 Run `nix develop -c scripts/gates.sh`: every gate passes, including `models compat` (the 501/502 tuples unchanged, three approval tuples by co-supplied set) and the determinism check. Confirm `scripts/check-core-pin.sh` reports Core 0.13.0 (`1509497`, pinned by feature 504) and that behavior-ir-core is untouched.
- [X] T056 Run every quickstart.md scenario (1–14) and record the results.
- [X] T057 Write specs/503-approval-model/implementation-log.md:
  - gate results and the pinned reference hashes;
  - findings during implementation;
  - deviations, with reasons;
  - an anti-pattern audit with test evidence for each plan.md row;
  - a statement that no Core change was made.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (1)**: none.
- **Foundational (2)**: after Setup. T005 first (against the current code); T006–T008 before
  T009–T013. **Blocks every story.**
- **US1 (3)**: after Foundational. The MVP.
- **US2 (4)**: after US1's lowering (T025). T030 uses the reference modules from T048, so run it
  after T048 or against `doc_approval` until then.
- **US3 (5)**: after US1 (it extends lowering and the binding).
- **US4 (6)**: after Foundational and US1. Independent of US2 and US3.
- **US5 (7)**: after US1. T048–T050 after US4 (the reference cases use RequestFlow composition).
- **Polish (8)**: after all stories.

### Within each story

Tests first, seen failing → input/resolve → decls → lower → kinds wiring → binding → Python API.

### Parallel opportunities

- Setup: T003 and T004.
- Foundational tests: T006, T007 and T008 (different files or test modules).
- US1 tests: T014, T015, T016, T018, T019, T020 and T021. T017 lives in lower.rs, so it comes
  after T025's skeleton.
- US2 tests: T029, T030 and T031. US3 tests: T034 and T035. US4 tests: T038, T039 and T040. US5
  tests: T043–T046.
- Once US1 is done, US3, US4 and US5 (up to T047) can proceed in parallel.

## Parallel Example: User Story 1

```text
T014 approval_resolve.rs   T015 approval_lower.rs (derived)   T018 test_approval.py
T019 test_approval_concurrency.py   T020 test_approval_properties.py   T021 approval_laws.rs
```

## Implementation Strategy

### MVP first (US1 + US2)

1. Setup + Foundational. Verify D9 (501 and 502 hashes unchanged) before anything else.
2. US1: decisions, outcome, the three layers (T017 is the decisive child-state test).
3. US2: complete ownership, which closes the induction.
4. **Stop and validate**: SC-002, SC-003, SC-004 and SC-005 hold.

### Incremental delivery

5. US3: policy, payload and commands.
6. US4: RequestFlow composition, DAG, collisions.
7. US5: identity and compat pinning.
8. Polish: terms, docs, gates.

## Notes

- `[P]` marks tasks on different files with no dependency on incomplete tasks.
- Commit after each task or logical group, only when the user asks.
- Any apparent Core need: stop and classify it; no Python or Wire JSON workaround.

## Phase 9: Convergence

- [X] T058 Extend the permutation property `supply_and_mapping_order_never_change_identity` in crates/behavior-models/tests/approval_identity.rs so the supply order also permutes a RequestFlow, a StateMachine and hand-written items (an authored action and read that use the decision entity or a predicate) alongside the approvals, and assert identical ApprovalHash, `C`/`X` declarations, module and BehaviorHash; keep the `property_cases()` tiers per SC-006 / T045 (partial)
- [X] T059 Add a test where an approval predicate guards a StateMachine transition (and one where it guards a RequestFlow or StateMachine creation), DENY before the threshold and ALLOW after, in python/tests/test_approval_composition.py per FR-010 / US4 (partial)
- [X] T060 Add an exhaustive outcome test for every threshold configuration in {(1,⊥),(2,⊥),(2,1),(3,2),(1,1)} and every decision sequence of up to 6 attempts by up to 4 actors, checking the observed outcome against the Outcome definition after every step; enumerate the reachable decision sets (at most 3^4 per configuration) and every attempt from each, through the public engine, in python/tests/test_approval.py or crates/behavior-models/tests/approval_laws.rs per SC-002 (partial)
- [X] T061 Add generated-capability collision tests for approvals (a hand-written action named `doc_approval__approve`, and a model-generated action with the same name) that fail with the common collision error and rename nothing, in crates/behavior-models/tests/declaration_dag.rs per US4/AC7, FR-025 and SC-009 (partial)
