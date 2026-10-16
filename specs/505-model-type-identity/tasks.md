---

description: "Tasks for feature 505: declaration identity of named types in StateMachine and RequestFlow hashes"
---

# Tasks: Declaration identity of named types in StateMachine and RequestFlow hashes

**Input**: Design documents from `specs/505-model-type-identity/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R8), data-model.md,
contracts/declaration-references.md, contracts/compat-supersession.md, quickstart.md

**Tests**: Required. Constitution III makes tests test-first, and the spec's acceptance
scenarios and SC-001–SC-005 are test obligations. Every test task is written and seen to fail
before its implementation task.

**Organization**: by user story. US1 and US2 are both P1. US2 depends on US1 because a
RequestFlow embeds a StateMachine document. US3 (P2) depends on both, because the manifest can
only be re-pinned once both documents are final.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1, US2, US3 (spec.md)

Paths are relative to the repository root `~/repos/behaviour-ir`. Commands run inside
`nix develop`.

---

## Phase 1: Setup

**Purpose**: capture the baseline that FR-008 and SC-003 are measured against.

- [X] T001 Capture the baseline manifest with `git show main:models/fixtures/compat.json` into the session scratchpad. Record in a new specs/505-model-type-identity/implementation-log.md, section "Baseline", the main commit and, for every reference case, its BehaviorHash, generated hashes and ApprovalHashes, so quickstart 4 can compare against it.

---

## Phase 2: Foundational (shared identity rule, contract `declaration-references.md`)

**Purpose**: one kind-neutral implementation of the reference rule (R1, R2, FR-003, FR-004,
FR-010), with Approval's output byte-identical.

**⚠️ CRITICAL**: US1 and US2 build on these helpers.

- [X] T002 Write unit tests in the `#[cfg(test)]` module of crates/behavior-models/src/identity.rs for the shared helpers (data-model §1):
  - `named_types` returns the named types of `Enum`, `Nominal`, `Exact(Some(n))` (kind nominal), and `Id`, `Ref` and `Entity` (kind entity), also inside nested `Option`. It returns nothing for `Bool`, `Int`, `Decimal`, `String` and `Exact(None)`.
  - A typed parameter list is sorted by name, each `types` map is keyed by type name in code-point order, and a type mentioned twice appears once.
  - A parameter with only built-in types has `"types": {}`.
  - An entity reference is `{entity, entity_hash}`, with the hash equal to `behavior_engine::schema::declarations(module)[entity]`.
  - A name that is missing, or declared only under another kind (an enum named where an entity is expected), fails with `MODEL_IDENTITY_FAILED`, and the message names the model, the referring item and the type.

  Build the modules with `behavior_engine::builder::Builder`. Run the tests and see them fail.
- [X] T003 Move the helpers from crates/behavior-models/src/approval/identity.rs into crates/behavior-models/src/identity.rs: `named_types`; the typed-parameter function, renamed `typed_params`; and `entity_ref`. Each takes the model reference and the referring item, so that errors name both (FR-010). Resolve through the admitted module's `name_table()`, matching the kind via `Kind::as_str()` (`"enum"`, `"nominal"`, `"entity"`; R2). Delete the name-only `params()`. Make crates/behavior-models/src/approval/identity.rs call the moved helpers, without changing its JSON. T002 green.
- [X] T004 Check that Approval is byte-identical:
  - `cargo test -p behavior-models --test approval_identity` passes unchanged;
  - run `maturin develop` first, so the gate builds with the changed Rust code and not a stale `_engine`; then `python scripts/check-models-compat.py` still reports `OK` (StateMachine and RequestFlow are still v1 at this point, so every tuple, including the approval tuples, must reproduce);
  - `cargo clippy -p behavior-models --all-targets -- -D warnings` is clean.

**Checkpoint**: one implementation of the rule. Nothing pinned has changed yet.

---

## Phase 3: User Story 1 - A StateMachine's hash follows the declarations it references (Priority: P1) 🎯 MVP

**Goal**: the StateMachine document v2 (data-model §2): the subject as `{entity, entity_hash}`
and typed transition inputs and context, at semantic version 2 with format
`behavior.model.state_machine.v2` (FR-001, FR-006, FR-007, FR-011).

**Independent Test**: quickstart 1. Build one machine with one difference in a referenced
declaration, or in an unrelated one, and compare StateMachineHashes.

### Tests for User Story 1 (write first, see them fail)

- [X] T005 [US1] In crates/behavior-models/tests/identity.rs, update the golden test `canonical_document_has_exactly_the_semantic_facts` to the v2 document:
  - `"format": "behavior.model.state_machine.v2"`;
  - `"model_semantic_version": 2`;
  - `"subject": {"entity": "PurchaseRequest", "entity_hash": <schema::declarations hash>}`;
  - every input and context parameter as `{name, type, types}`, with the `approve` transition's context `actor: Id<Actor>` carrying `{"Actor": <hash>}`.

  Update `receipt_has_exactly_the_contract_fields` so it expects `model_semantic_version` 2 and `lowerer_version` 1.
- [X] T006 [US1] Add `declaration_dependencies_change_the_machine_hash` to crates/behavior-models/tests/identity.rs (acceptance 1–3, 6, 7; SC-001). Start from the reference machine (common::declarations, common::machine) and give `submit` an unused context `mode: Enum(Mode)` and an unused input `amount: Nominal(Amount)`. Each of the following changes the StateMachineHash. Do not assert anything about BehaviorHash here: a changed declaration changes the module. That lowering is unchanged for unchanged inputs is checked against the baseline in T018.
  - `Mode`'s cases change;
  - `Amount`'s operations change;
  - `Amount`'s scale changes;
  - `Amount`'s underlying type changes (`Int` to `Decimal`);
  - an unused field is added to the subject `PurchaseRequest`;
  - the input type `Id<Actor>` is used, then `Actor` gains a field;
  - `Mode` is renamed (same cases).
- [X] T007 [US1] Add `unrelated_declarations_and_order_keep_the_machine_hash` to crates/behavior-models/tests/identity.rs (acceptance 4, 5; SC-002, I2, I4). None of these changes the StateMachineHash:
  - an enum, nominal or entity is added that no parameter names;
  - an unrelated entity's fields change;
  - authored declarations are declared in reverse order;
  - input and context parameters are listed in reverse order.

### Implementation for User Story 1

- [X] T008 [US1] In crates/behavior-models/src/identity.rs, make `canonical_document` produce the v2 document of data-model §2:
  - `"format": "behavior.model.state_machine.v2"`;
  - `"subject": entity_ref(...)`;
  - `"inputs"` and `"context"` built with `typed_params(...)`;
  - the `field` with `enum_hash` unchanged.

  Set `STATE_MACHINE_SEMANTIC_VERSION = 2` in crates/behavior-models/src/lib.rs and keep `STATE_MACHINE_LOWERER_VERSION = 1`. T005–T007 green.
- [X] T009 [US1] Update python/tests/test_state_machine.py line 318 to `(r.model_semantic_version, r.lowerer_version) == (2, 1)`. Fix any other StateMachine receipt expectation that `cargo test -p behavior-models` or `pytest python/tests -k state_machine` reports, except manifest-pinned hashes, which belong to US3.

**Checkpoint**: StateMachineHash follows exactly the referenced declarations. The manifest is
stale until US3; the `models compat` gate is expected to fail here.

---

## Phase 4: User Story 2 - A RequestFlow's hash follows its actor, payload, input, context and state types (Priority: P1)

**Goal**: the RequestFlow document v2 (data-model §3), at semantic version 2 with format
`behavior.model.request_flow.v2`. Its embedded machine is v2 from US1 (FR-002, FR-011).

**Independent Test**: quickstart 2. Build the reference flow with one difference in a
referenced declaration and compare RequestFlowHashes.

### Tests for User Story 2 (write first, see them fail)

- [X] T010 [US2] In crates/behavior-models/tests/request_flow_identity.rs, update the golden test `canonical_document_references_instead_of_copying` to the v2 document:
  - `"format": "behavior.model.request_flow.v2"`;
  - `"model_semantic_version": 2`;
  - `"actor": {"entity": "User", "entity_hash": ...}`;
  - typed `payload`, `inputs` and `context`;
  - each `state` binding as `{name, entity, entity_hash}`;
  - the embedded `state_machine.document.format == "behavior.model.state_machine.v2"` (the current line 83).

  Update `one_receipt_with_an_embedded_identity_record` so it expects `model_semantic_version` 2 for both the flow and the embedded machine, with `lowerer_version` 1.
- [X] T011 [US2] Add `declaration_dependencies_change_the_flow_hash` to crates/behavior-models/tests/request_flow_identity.rs (US2 acceptance 1–3, 5, 6; SC-001). Start from `rf::flow()` with `rf::authored()`. Each of the following changes the RequestFlowHash:
  - a payload field of enum type `Priority` has its cases changed;
  - an unused creation input of nominal type has its operations changed, and separately its scale;
  - an unused creation context of enum type has its cases changed;
  - a creation state binding `budget: Budget` is added, and then `Budget` gains a field;
  - the actor `User` gains a field;
  - an unused context enum on a transition has its cases changed. This changes both the embedded machine hash and the RequestFlowHash.
- [X] T012 [US2] Add `unrelated_declarations_keep_the_flow_hash` to crates/behavior-models/tests/request_flow_identity.rs (acceptance 4; SC-002): an added, unreferenced declaration, and a change to an entity the flow does not reference, leave the RequestFlowHash equal.

### Implementation for User Story 2

- [X] T013 [US2] In crates/behavior-models/src/request_flow/identity.rs, produce the v2 document of data-model §3:
  - `"format": "behavior.model.request_flow.v2"`;
  - `"actor": entity_ref(...)`;
  - `"payload"`, and each creation's `"inputs"` and `"context"`, built with `typed_params(...)`;
  - each `"state"` binding as `{name, entity, entity_hash}` via `entity_ref`.

  Set `REQUEST_FLOW_SEMANTIC_VERSION = 2` in crates/behavior-models/src/lib.rs and keep `REQUEST_FLOW_LOWERER_VERSION = 1`. T010–T012 green.
- [X] T014 [US2] Fix any other RequestFlow receipt expectation that `cargo test -p behavior-models` or `pytest python/tests -k request_flow` reports, except manifest-pinned hashes. The existing permutation property in request_flow_identity.rs must stay green unchanged (SC-005).

**Checkpoint**: both documents are final. `cargo test -p behavior-models` is green.

---

## Phase 5: User Story 3 - The change is versioned, pinned and otherwise invisible (Priority: P2)

**Goal**: explicit supersession in the gate (contract `compat-supersession.md`), the manifest
re-pinned with one tuple per case, and evidence that lowering and Approval are unchanged
(FR-007, FR-008, FR-009; SC-003, SC-004).

**Independent Test**: quickstart 4–8.

### Tests for User Story 3 (write first, see them fail)

- [X] T015 [US3] Add the ten cases of contract compat-supersession.md §6 to scripts/tests/test_models_compat.sh, built with the file's existing `--computed` fixtures and a temporary git merge base:
  1. a valid v1→v2 supersession passes;
  2. removal without `supersedes` → `was removed`;
  3. a replacement missing a case → `invalid supersession` (lost case);
  4. another kind → `invalid supersession` (kind);
  5. a mixed vector (one component higher, one lower) → `invalid supersession` (version);
  6. an equal vector → `invalid supersession` (version);
  7. an approval tuple superseded by higher `co_supplied` versions passes;
  8. `supersedes` naming a tuple that is still present, or one never established → fails;
  9. an established tuple edited in place → `was changed`;
  10. `--write --supersede` records the v2 tuple with `supersedes`, removes v1, and the gate then passes.

  Run the tests and see cases 1, 7 and 10 fail on the current gate.

### Implementation for User Story 3

- [X] T016 [US3] Implement explicit supersession in scripts/check-models-compat.py (contract §2–§4):
  - an optional `supersedes` list of full tuple keys on a tuple;
  - rule 2 accepts an established tuple's absence only under §3(a) or §3(b), where (b) has three conditions: same kind and pinned Core, a vector ⪰ and ≠ (incomparable when the embedded or co-supplied kind sets differ), and every case name of the removed tuple present in the replacement;
  - the error messages `established tuple … was removed` and `invalid supersession of … by …: <condition>`;
  - a `supersedes` entry naming a tuple that is still present, or one not established at the merge base, fails;
  - `--write --supersede` records new tuples and moves validly superseded keys into their `supersedes`, while plain `--write` never removes.

  Update the module docstring with the three concepts: identity, supersession, history. T015 green.
- [X] T017 [US3] Run `maturin develop` and then `python scripts/check-models-compat.py --write --supersede`. This records the state_machine 2, request_flow 2 (embedded state_machine 2) and re-keyed approval tuples (`co_supplied` 2) in models/fixtures/compat.json, removes their predecessors and lists them in `supersedes`. Check that there is one tuple per case, that every replacement has the same case names as the tuple it supersedes, and that `python scripts/check-models-compat.py` reports `OK`.
- [X] T018 [US3] With the engine rebuilt by T017's `maturin develop`, compare the new manifest with the T001 baseline (quickstart 4, 5). For every case, BehaviorHash and generated hashes are equal. Every ApprovalHash is equal. Only StateMachine and RequestFlow model hashes and the embedded machine hashes differ. Record the comparison table in specs/505-model-type-identity/implementation-log.md. A difference outside those fields is a defect: stop and fix it, do not pin it.
- [X] T019 [US3] Run `pytest python/tests/test_model_staging.py` (quickstart 8). It reads its pinned cases from the manifest and must pass without edits. Edit it only if the manifest's one-tuple-per-case shape requires it.

**Checkpoint**: the gate proves that no case was lost and that nothing outside model identity
changed.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T020 [P] Update the "Model identity" section of models/README.md:
  - a model's identity depends on the exact declarations it references (subject, actor, state bindings, parameter and payload types) and on nothing else in the module;
  - an unused field in a referenced entity changes it (exact dependency, not behavioral equivalence);
  - a superseded compat tuple is replaced through `supersedes`.
- [X] T021 Complete specs/505-model-type-identity/implementation-log.md with the gates result, the quickstart table (scenarios 1–10 with results), the pinned v2 hashes, the supersession records and any deviations.
- [X] T022 Run `BEHAVIOR_FULL_PROPERTIES=1 cargo test -p behavior-models` (SC-005, quickstart 9) and `scripts/gates.sh` (quickstart 10). Both must be green: `gates: OK`, fmt, clippy `-D warnings`, models compat, determinism and the script tests.

---

## Dependencies & Execution Order

- **Setup (T001)** comes before T018.
- **Foundational (T002–T004)** blocks US1 and US2.
- **US1 (T005–T009)** comes before US2, because the RequestFlow golden embeds the v2 machine document.
- **US2 (T010–T014)** comes after US1.
- **US3:** T015–T016 depend only on Foundational and can run in parallel with US1 and US2. T017–T019 need US1 and US2 complete.
- **Polish (T020–T022)** comes after US3. T020 can run any time after US2.

## Parallel Opportunities

- T015 and T016 (gate and script tests, Python and shell) run alongside T005–T014 (Rust).
- T020 (README) runs alongside T017–T019.
- Within US1, T006 and T007 are in the same file, so they run sequentially. T005 is in the same file too.
- Within US2, T010–T012 are in the same file and run sequentially.

```text
# Example: while US1 is implemented in Rust
Task: "T015 [US3] supersession cases in scripts/tests/test_models_compat.sh"
Task: "T016 [US3] explicit supersession in scripts/check-models-compat.py"
```

## Implementation Strategy

1. **MVP (US1)**: the shared rule plus the StateMachine v2 document. StateMachineHash then
   follows its declarations, and quickstart 1 passes. The manifest is stale at this point, so it
   is not a releasable state on its own.
2. **US2**: the RequestFlow v2 document. Both P1 stories hold.
3. **US3**: supersession and re-pinning make the change releasable. The gate is green again,
   with evidence that lowering and Approval are unchanged.
4. **Polish**: README, implementation log, full-tier properties and gates.
