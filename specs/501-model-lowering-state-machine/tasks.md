---

description: "Task list for feature 501: Deterministic Model Lowering and StateMachine Model"
---

# Tasks: Deterministic Model Lowering and StateMachine Model

**Input**: Design documents from `specs/501-model-lowering-state-machine/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/{lowering-contract, composition-and-ownership, identity-and-source-map, state-machine-contract}.md

**Tests**: Required. Spec §38 lists mandatory test categories, and constitution III
(Test-First) is non-negotiable. In every phase, write the tests first and see them fail.

**Organization**: Tasks are grouped by user story (spec.md), so each story can be implemented
and checked on its own.

**Hard rules for every task** (lowering contract §5, §8):
- Never edit `behavior-ir-core`, and never add an internal Core crate dependency.
- Never use `Node::into_wire` or `serialize::to_wire_json` in `crates/behavior-models`.
- Python never sorts, deduplicates, defaults, renames or validates model semantics.
- If a construct cannot be lowered with the APIs in research R1, STOP and write a gap report
  (lowering contract §8) instead of working around it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: a new, empty crate that passes every existing gate.

- [X] T001 Add the row `| crates/behavior-models/** | ecosystem | the model authority (feature 501) |` **before** rule 3 (`crates/**` → core) in specs/011-core-ecosystem-split/contracts/ownership.md, and renumber the following rows (research R11)
- [X] T002 Create crates/behavior-models/Cargo.toml (`[lib]`, `[lints] workspace = true`, dependencies `behavior-engine.workspace = true`, `serde_json = "1"` and `thiserror` (constitution: typed errors via `thiserror` in libraries), dev-dependency `proptest`), and add `"crates/behavior-models"` to `members` in Cargo.toml
- [X] T003 Create crates/behavior-models/src/lib.rs with `#![forbid(unsafe_code)]` and a module doc stating the crate is the single authority for Behavior Models (spec FR-007)
- [X] T004 [P] Extend scripts/check-public-surface.sh to check crates/behavior-models/Cargo.toml for internal Core crates exactly as it does for behavior-py, and to fail on any `into_wire` or `to_wire_json` occurrence under crates/behavior-models/src (plan anti-pattern audit)
- [X] T005 [P] Create crates/behavior-models/build.rs that reads `../../core-release.json` and exports `CORE_RELEASE_VERSION` and `CORE_RELEASE_COMMIT` as compile-time env values, with `cargo:rerun-if-changed`
- [X] T006 Run `scripts/gates.sh` and confirm it passes with the empty crate. Commit the setup.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the error catalogue, semantic ids, versions, input types and the `compose` skeleton
that every story builds on.

**⚠️ CRITICAL**: no user story work can begin until this phase is complete.

### Tests (write first, see them fail)

- [X] T007 [P] Write crates/behavior-models/tests/ids.rs. Cover:
  - accept and reject cases for the grammar quoted verbatim from data-model §3 / research R4: `ID = [a-z][a-z0-9]*(_[a-z0-9]+)*` ("lowercase, starting with a letter, single underscores only between alphanumeric runs, no leading, trailing or doubled underscore");
  - a proptest (≥ 1,000 cases, fixed seed) over `ID × ID` asserting that `encode(m, t) = m ‖ "__" ‖ t` is injective, that splitting at the unique `"__"` recovers `(m, t)`, and that every result matches Core's grammar `[A-Za-z_][A-Za-z0-9_]*`.
- [X] T008 [P] Write crates/behavior-models/tests/errors.rs. Assert that a shuffled list of every error code in lowering contract §6 sorts deterministically by `(stage order, code, model_ref or action, detail)`, and that duplicates are removed.

### Implementation

- [X] T009 [P] Implement crates/behavior-models/src/ids.rs: a `SemanticId` newtype validated against `ID` (error `INVALID_SEMANTIC_ID`), `encode(&SemanticId, &SemanticId) -> String` and `decode(&str)`.
- [X] T010 [P] Implement crates/behavior-models/src/error.rs: a `ModelError` enum with exactly the codes of lowering contract §6 and their payloads, stable `code()` strings, `Ord` implementing the sort key from T008, and a `sorted_unique(Vec<ModelError>)` helper (typed errors via `thiserror` style, no `unwrap`).
- [X] T011 Add version constants to crates/behavior-models/src/lib.rs: `STATE_MACHINE_SEMANTIC_VERSION: u32 = 1`, `STATE_MACHINE_LOWERER_VERSION: u32 = 1`, and `CORE_RELEASE: CoreRelease { version, commit }` from build.rs (data-model §10).
- [X] T012 Implement crates/behavior-models/src/state_machine/input.rs: `ModelInput` and `TransitionInput` exactly as data-model §2 (raw lists, no defaults, `subject_param: Option<String>`, `inputs` and `context` as ordered `(name, WType)` lists, `DisplayMeta`, `SourceLoc`), the `FragmentSource` trait, `TransitionScope { params: Vec<WParam>, subject_param, owned_field }` and `TransitionFragments` (data-model §5).
- [X] T013 Implement the `compose(builder: &mut Builder, models: Vec<ModelInput>, sources: …, root: Option<&str>) -> Result<BuildOutput, Vec<ModelError>>` skeleton in crates/behavior-models/src/lib.rs. Fix the step order of lowering contract §4 (resolve → set checks → lower in canonical order → finish → composition pass → identity) with stubs, and collect errors across steps 2–4 before returning.
- [X] T014 Create the test fixture helper crates/behavior-models/tests/common/mod.rs. It builds the `purchase_request` declarations (enum `RequestStatus {Draft, Submitted, InReview, Approved, Rejected}`, entity `PurchaseRequest { status: RequestStatus, approver: Id<Actor>, approved_by: Option<Id<Actor>> }`, entity `Actor`, command profile) into a `Builder`, and provides a Rust `FragmentSource` implementation that builds fragments from closures over `(&mut Builder, &TransitionScope)` using only the names in the scope. **Spike, before any US work** (analysis U1, U2): (a) check that the entry-literal probe of research R8 works (`Builder::for_module` plus a closed-scope global invariant over `lit(Enum(T_f), q)` admits, and `children()[0].hash()` equals the hash of the same literal used in a real create). If it does not, try fallback 1: a fresh `Builder` re-declaring `T_f` with the same cases. If both fail, STOP with a gap report (lowering contract §8). (b) Check that an admitted action keeps the given order of preconditions, effects, postconditions and command emissions. If it does not, switch fragment identity to hash membership and update identity contract §2.

**Checkpoint**: `cargo test -p behavior-models` runs, and T007 and T008 pass.

---

## Phase 3: User Story 1 - Author a state machine that runs as ordinary behavior (Priority: P1) 🎯 MVP

**Goal**: a StateMachine authored in Python lowers, in Rust, to ordinary actions that Core
admits and runs. Transitions are enforced only by ordinary Core outcomes.

**Independent Test**: author `purchase_request`, build it, admit it through the public engine,
invoke every transition from every state, and inspect the module for model-specific items
(spec US1).

### Tests for User Story 1 (write first, see them fail)

- [X] T015 [P] [US1] Write crates/behavior-models/tests/resolve.rs, with one test per resolution rule of data-model §3:
  - `INVALID_SEMANTIC_ID`, `UNKNOWN_ENTITY`;
  - `STATE_FIELD_NOT_ENUM` ("typed `Enum(T)` (not `Option`, not nominal)");
  - `UNKNOWN_STATE` for entry, source and target cases;
  - `EMPTY_ENTRY_STATES`, `EMPTY_SOURCE_STATES`, `DUPLICATE_TRANSITION_ID`;
  - `INVALID_PARAMETER` (names in Core's identifier grammar and pairwise distinct);
  - duplicate cases in input sets are silently deduplicated;
  - `subject_param` defaults to `snake_case(E)` (`PurchaseRequest` → `purchase_request`) when absent.
- [X] T016 [P] [US1] Write crates/behavior-models/tests/lower.rs:
  - a golden layout test for `approve` matching lowering contract §3 exactly: params `(purchase_request: state PurchaseRequest) ++ context`, pre `[In(s.status, ["InReview"])] ++ G_t`, effects `[(s, status, lit(Approved))] ++ E_t`, post `[s.status == lit(Approved)] ++ P_t`, commands `K_t`;
  - a multi-source transition whose `In` values are sorted by code point;
  - `|F_t| = 1` still produces `In`, not `==`;
  - action name `purchase_request__approve`.
- [X] T017 [P] [US1] Add fragment-validation tests to crates/behavior-models/tests/lower.rs (data-model §6):
  - a non-Bool guard → `FRAGMENT_INVALID{NOT_BOOLEAN}`;
  - an ill-typed effect → `FRAGMENT_INVALID`;
  - a node from another builder → `FRAGMENT_INVALID{BUILDER_MISMATCH}`;
  - commands without the command profile → `FRAGMENT_INVALID{UNSUPPORTED_IR_VERSION}`;
  - an extra effect on `status` → `TRANSITION_WRITES_OWNED_FIELD`;
  - valid fragments but a module Core cannot admit → `CORE_ADMISSION_FAILED` with Core's errors verbatim.
- [X] T018 [P] [US1] Add self-transition tests to crates/behavior-models/tests/lower.rs:
  - `F_t = {q} = {q_t}` with no extra effect and no command → `PURE_SELF_TRANSITION`;
  - the same with an extra effect, or with a command → OK;
  - `F_t = {A, B}`, `q_t = B` → OK.
- [X] T019 [P] [US1] Add an erasure and admissibility test to crates/behavior-models/tests/lower.rs: the admitted module's `name_table()` holds exactly the declared items plus `encode(m, t)` actions, and nothing names a state machine (laws L4, L5).
- [X] T020 [P] [US1] Write python/tests/test_state_machine.py:
  - the transition matrix for every state × transition of `purchase_request` through `behavior.invoke`: commit iff state ∈ F_t and the guards hold, otherwise Core's ordinary precondition denial (SC-001);
  - guard true, guard false, and a guard dividing by zero (Core `DIVISION_BY_ZERO`) (research R12);
  - an extra effect `approved_by = context.actor` and a command intent are part of the same commit;
  - the module round-trips through `behavior.admit`, invoke and replay using only the public `behavior` API (Core consumer test).
- [X] T021 [P] [US1] Add a binding-boundary test to python/tests/test_state_machine.py. Lambdas with different Python parameter names (`lambda r, c:` vs `lambda req, ctx:`) and unsorted or duplicate Python sets produce an identical admitted module, because Python passes raw input and binds by position.

### Implementation for User Story 1

- [X] T022 [US1] Implement crates/behavior-models/src/state_machine/resolve.rs: `resolve(&ModelInput, decls) -> Result<ResolvedStateMachine, Vec<ModelError>>`.
  - Use the data-model §3 fields: no stored `Q`; `entry_states` and `from_states` as `BTreeSet`; `transitions` as `BTreeMap<SemanticId, _>`; `action_name = encode(model_id, transition_id)`.
  - Read the enum's cases and the entity's field types from the shared Builder's declarations through public APIs only.
  - Apply every rule in the data-model §3 table.
- [X] T023 [US1] Implement transition scope construction in crates/behavior-models/src/state_machine/lower.rs: params in data-model §4 order `(subject_param, Role::State, Entity(E))`, then inputs `(name, Role::Input, type)` **sorted by name**, then context `(name, Role::Context, type)` **sorted by name**. Inputs and context are mappings `ParameterId → Type`; declaration order is never used.
- [X] T024 [US1] Implement the lowering loop in crates/behavior-models/src/state_machine/lower.rs. For each transition in `transition_id` order:
  - `push_scope(ScopeSite::Action, params)`, then `source.trace(builder, &scope)`;
  - layer-1 checks via `check_condition` and `check_effect`;
  - the layer-2 check `TRANSITION_WRITES_OWNED_FIELD`;
  - the pure self-transition check;
  - build the generated nodes `in_(field(s, f), sorted F_t)`, `lit(Enum(T_f), q_t)` and `op("==", …)`;
  - call `add_action`, or `add_action_with_commands` when `K_t ≠ ∅`, with the exact lowering contract §3 layout;
  - `pop_scope`, also on error.
- [X] T025 [US1] Map Core `BuildError`s from fragment construction and `add_action` to `FRAGMENT_INVALID{core_code, core_message}` in crates/behavior-models/src/state_machine/lower.rs (a `DUPLICATE_NAME` on the action name is handled in US4, T061).
- [X] T026 [US1] Wire resolve, lower and `builder.finish(root)` into `compose` in crates/behavior-models/src/lib.rs. Map an admission failure to `CORE_ADMISSION_FAILED` with Core's errors verbatim, and return `BuildOutput { module, receipts: [], diagnostics: [] }` (receipts are filled in US3).
- [X] T027 [US1] Add PyO3 glue in crates/behavior-py/src/models.rs, registered in crates/behavior-py/src/lib.rs, and add `behavior-models` to crates/behavior-py/Cargo.toml:
  - a `compose(builder, model_inputs, fragment_tracers, root)` function taking the existing `PyBuilder`'s inner `Builder`;
  - a `FragmentSource` implementation that calls a Python callable with the Rust-provided `TransitionScope` (param names and roles, in order) and converts the returned `PyNode`s;
  - Rust `ModelError`s converted to a Python list of dicts `{code, …payload}`, unchanged.
- [X] T028 [US1] Implement python/behavior/models.py:
  - `StateMachine(model_id, *, subject, state_field, entry_states, subject_param=None, display_name=None, description=None)`;
  - `.transition(transition_id, *, from_, to, inputs=None, context=None, requires=(), effects=(), ensures=(), emits=(), display_name=None, description=None)`.
  Both only record raw `ModelInput` (lists as given, no sorting, no dedup, no defaults) plus source locations. The fragment tracer creates `EntityVar` and param proxies from the **Rust-given names** and calls the author's callables positionally (lowering contract §5).
- [X] T029 [US1] Add `ModelBuildError(BehaviorError)` with `.errors` (the Rust list) to python/behavior/errors.py.
- [X] T030 [US1] Extend `BehaviorModule` in python/behavior/module.py with `models=[...]`. After every ordinary declaration and hand-written action has been traced into the session's builder, call the Rust `compose` (protocol step 1 before steps 2–7), raise `ModelBuildError` on errors, and keep the admitted module.
- [X] T031 [US1] Export `StateMachine` and `ModelBuildError` from python/behavior/__init__.py and add both to `python.names` in api/public-api.json.
- [X] T032 [US1] Migrate the worked example in models/examples/state_machine/ (FR-036, research R13):
  - delete models/examples/state_machine/lower.py;
  - add models/examples/state_machine/machine.py with the `order` machine (`submit: DRAFT → SUBMITTED` over `Order.status`) authored with `StateMachine`;
  - rewrite models/examples/state_machine/test_state_machine.py to assert that the action `order__submit` has the same Core action hash as the hand-written three-statement action `requires(order.status in [DRAFT]); set_(order.status, SUBMITTED); ensures(order.status == SUBMITTED)`.
- [X] T033 [US1] Run `cargo test -p behavior-models` and `python -m pytest -q python/tests/test_state_machine.py models`, and confirm T015–T021 pass.

**Checkpoint**: US1 works on its own. It is the MVP: a StateMachine runs as ordinary Core behavior.

---

## Phase 4: User Story 2 - A state machine cannot be silently bypassed (Priority: P1)

**Goal**: the final composition pass rejects every unauthorized write to an owned field and
every creation that does not start in a literal entry state.

**Independent Test**: purchase_request plus a hand-written `force_approve`, plus creations to
Draft, Approved and a dynamic value (spec US2).

### Tests for User Story 2 (write first, see them fail)

- [X] T034 [P] [US2] Write crates/behavior-models/tests/ownership.rs with the composition contract §6 rows on writes:
  - a hand-written `force_approve` setting `status` → `UNAUTHORIZED_OWNED_FIELD_WRITE{action, entity, field, owner}`;
  - a hand-written action that only reads `status` → OK;
  - a hand-written action removing a `PurchaseRequest` → OK;
  - a generated action of machine A writing a field owned by machine B (on the same entity) → `UNAUTHORIZED_OWNED_FIELD_WRITE`.
- [X] T035 [P] [US2] Add creation tests to crates/behavior-models/tests/ownership.rs:
  - a create with `status = lit(Draft)` → OK;
  - `status = lit(Approved)` → `ENTRY_STATE_NOT_ALLOWED{state: Approved}`;
  - `status = input.initial` → `ENTRY_STATE_NOT_STATIC`;
  - `status` from a derived value that always yields Draft → `ENTRY_STATE_NOT_STATIC` (conservative v1, research R8);
  - no creation of `PurchaseRequest` anywhere → OK.
- [X] T036 [P] [US2] Add a no-escape-hatch test to crates/behavior-models/tests/ownership.rs: call `compose` with every public construction option the crate offers and assert that a hand-written owned-field writer is always rejected. The review item for this goes into T074 (FR-018).
- [X] T037 [P] [US2] Write python/tests/test_state_machine_ownership.py mirroring T034–T035 through `BehaviorModule(models=[…])`. Assert `ModelBuildError.errors` codes and payloads, and that no module is produced.

### Implementation for User Story 2

- [X] T038 [US2] Implement `OwnershipTable` (`BTreeMap<(entity, field), model_id>`) and `GeneratedAction` bookkeeping in crates/behavior-models/src/composition.rs (data-model §8).
- [X] T039 [US2] Implement the write check of the final pass in crates/behavior-models/src/composition.rs (composition contract §4 "Writes"). Iterate `module.actions()` by name. For each `effects()` entry, resolve `param()` in `params()`, read `ty().to_wire_type()`, and on `WType::Entity(X)` with `(X, field)` owned by `m`, require `action ∈ Generated(m)`.
- [X] T040 [US2] Implement the entry-literal probe in crates/behavior-models/src/composition.rs (research R8):
  - a fresh `Builder::new()` that `declare_enum`s `T_f` with `module.enums()[T_f].values`, then `push_scope(ScopeSite::Closed, [])` (`Builder::for_module` does not work; see the spike in tests/core_assumptions.rs);
  - for each case `q` of `T_f`, `lit(WType::Enum(T_f), "q")` and `add_global_invariant("__probe_<i>", op("==", [lit, lit]))`;
  - `finish(None)`, then read `global_invariants()[name].body().children()[0].hash()`;
  - assert that the probe's enum hash equals the module's, otherwise `CORE_ADMISSION_FAILED`;
  - return `{case → Hash}`. Discard the probe module and never emit it.
- [X] T041 [US2] Implement the creation check of the final pass in crates/behavior-models/src/composition.rs (composition contract §4 "Creations"). For `creates()` with `entity() = X` and `(X, f)` owned, take the expression of `f` in `fields()` and compare its `hash()` against the probe: an entry case → OK; another case → `ENTRY_STATE_NOT_ALLOWED`; otherwise → `ENTRY_STATE_NOT_STATIC`.
- [X] T042 [US2] Run the composition pass after `finish` inside `compose` in crates/behavior-models/src/lib.rs. On any error return no module (law L6).
- [X] T043 [US2] Run `cargo test -p behavior-models --test ownership` and `python -m pytest -q python/tests/test_state_machine_ownership.py`, and confirm T034–T037 pass.

**Checkpoint**: US1 and US2 together give a StateMachine that cannot be bypassed.

---

## Phase 5: User Story 3 - Deterministic identity, provenance and explanation (Priority: P2)

**Goal**: ModelHash, lowering receipts, source maps and diagnostics, together with the
determinism laws and the version compatibility gate.

**Independent Test**: build the same machine under permutations and metadata changes; compare
ModelHash and BehaviorHash; check the source map; check that the gate rejects unversioned
changes (spec US3, SC-002, SC-003, SC-005, SC-009).

### Tests for User Story 3 (write first, see them fail)

- [X] T044 [P] [US3] Write crates/behavior-models/tests/identity.rs:
  - a golden canonical document for `purchase_request`, exactly the identity contract §2 shape;
  - transitions sorted by `id`; `entry_states` and `from` sorted by code point;
  - inputs and context in declared order; fragment lists in author order as Core `sha256:` hashes;
  - no display or source data;
  - the subject as entity name only, and the field with enum name plus enum item hash;
  - a ModelHash of the form `sha256:<64 hex>`.
- [X] T045 [P] [US3] Write crates/behavior-models/tests/laws.rs with proptests, ≥ 1,000 cases each and a fixed seed:
  - L3: random permutations of entry states, source sets and transitions → equal ModelHash, module and BehaviorHash;
  - L9: random display names, descriptions and source locations → equal ModelHash and BehaviorHash;
  - L7: equal ModelHash ⇒ equal BehaviorHash over generated equivalent inputs;
  - every tested semantic change (entry state, transition, guard, effect, id, enum) → different ModelHash (SC-003);
  - random permutations of each transition's input and context declarations → equal ModelHash, equal generated parameter identities and order, equal BehaviorHash (spec clarification on parameter order, FR-006).
- [X] T046 [P] [US3] Add a source-map test to crates/behavior-models/tests/identity.rs:
  - every generated action has exactly one entry `{core: {kind: "action", name, item_hash}, model: "<model_id>/transition/<id>"}`;
  - every transition maps to exactly one action;
  - one `{kind: "field"}` entry exists for the owned field;
  - entries are sorted;
  - changing only source-map data leaves both hashes unchanged (SC-005, law L9).
- [X] T047 [P] [US3] Add a receipt test to crates/behavior-models/tests/identity.rs: the receipt is canonical JSON `behavior.model_receipt.v1` with exactly the fields `model_hash, model_kind, model_semantic_version, lowerer_version, core_release{version, commit}, behavior_hash, source_map`; `behavior_hash == module.behavior_version()`; two builds give byte-identical receipts.
- [X] T048 [P] [US3] Add diagnostics tests to crates/behavior-models/tests/identity.rs: `UNREACHABLE_STATE`, `NO_OUTGOING_TRANSITIONS`, `NO_INCOMING_TRANSITIONS` and `UNUSED_STATE` are emitted as warnings for a crafted machine, and the build still succeeds (FR-031, SC-008).
- [X] T049 [P] [US3] Write scripts/tests/test_models_compat.sh. In a temporary git repository it checks:
  - an unchanged manifest → PASS;
  - an edited expected `behavior_hash` in an established tuple → FAIL;
  - a removed case of an established tuple → FAIL;
  - a new tuple after a `lowerer_version` bump with new hashes → PASS;
  - removing a tuple whose `core_release` is no longer pinned → PASS (SC-009, lowering contract §7).
- [X] T050 [P] [US3] Add an explanation test to python/tests/test_state_machine.py. A Core denial of `purchase_request__approve` from Draft is explained through the receipt's source map as transition `approve` of `purchase_request`, while the Core record names only the Core action.

### Implementation for User Story 3

- [X] T051 [US3] Implement crates/behavior-models/src/identity.rs:
  - build `CanonicalModelDocument` (identity contract §2) from the `ResolvedStateMachine` and the admitted module;
  - read fragment hashes at the lowering contract §3 positions (`preconditions()[1..]`, `postconditions()[1..]`, `effects()[1..]`, `command_emissions()`) via `hash_display`;
  - read the enum item hash from `name_table()`;
  - compute `ModelHash = tagged_hash("behavior.model.v1", doc)` via `behavior_engine::canonical`.
- [X] T052 [US3] Implement the receipts and source map in crates/behavior-models/src/source_map.rs (data-model §9, identity contract §3–4). They are produced only after the composition pass and are emitted as canonical JSON via `canonical::to_canonical_string`.
- [X] T053 [US3] Implement the diagnostics in crates/behavior-models/src/source_map.rs (state-machine contract §3). Use graph reachability from `Q_entry` over `(F_t → q_t)` edges, ignoring guards. Diagnostics carry `severity: "warning"` and no verification wording.
- [X] T054 [US3] Fill `BuildOutput.receipts` and `BuildOutput.diagnostics` in `compose` in crates/behavior-models/src/lib.rs, and expose them as `BehaviorModule.model_receipts` and `.model_diagnostics` through crates/behavior-py/src/models.rs and python/behavior/module.py. Add `LoweringReceipt` to python/behavior/__init__.py and api/public-api.json.
- [X] T055 [US3] Create models/fixtures/compat.json (`behavior.model_compat.v1`) with the current tuple `(state_machine, 1, 1, core 0.12.0/b525da4…)` and the cases `purchase_request`, `order`, `request_flow_reference` and `two_machines_same_local_ids`, generated by a small `--write` mode that refuses to overwrite established tuples.
- [X] T056 [US3] Implement scripts/check-models-compat.py (lowering contract §7):
  - recompute every case of the current tuple and require equality;
  - against `git merge-base HEAD main`, require every established tuple's cases to be byte-identical, except removal of tuples whose `core_release` is no longer pinned;
  - require the current tuple to cover all reference cases.
- [X] T057 [US3] Add `gate "models compat" python3 scripts/check-models-compat.py` to scripts/gates.sh (after pytest), and add the model receipts of the reference cases to the `ecosystem` section of scripts/determinism-check.sh (run twice, byte-compare). Set `fetch-depth: 0` on the checkout step in .github/workflows/ecosystem-ci.yml, and make the gate FAIL with a clear message (never pass silently) when `git merge-base HEAD main` is unavailable (analysis O1).
- [X] T058 [US3] Run `cargo test -p behavior-models --test identity --test laws`, `scripts/tests/test_models_compat.sh` and `scripts/determinism-check.sh`, and confirm T044–T050 pass.

**Checkpoint**: every build is reproducible, explainable and version-guarded.

---

## Phase 6: User Story 4 - Several models and hand-written behavior compose deterministically (Priority: P2)

**Goal**: several machines plus hand-written behavior in one module. Results do not depend on
supply or trace order, and every collision is an explicit error.

**Independent Test**: compose two machines and hand-written actions in every order; check the
same-field and name-collision errors (spec US4).

### Tests for User Story 4 (write first, see them fail)

- [X] T059 [P] [US4] Write crates/behavior-models/tests/composition.rs:
  - two machines on different entities, both with transition `submit` → OK, with actions `a__submit` and `b__submit` (spec US4 scenario 5);
  - two machines on different fields of one entity → OK;
  - two machines on the same `(E, f)` → `CONFLICTING_OWNERSHIP{entity, field, models: sorted}`;
  - a duplicate `model_id` → `DUPLICATE_MODEL_ID`;
  - a hand-written action named `purchase_request__approve` → `GENERATED_NAME_COLLISION{name, model_ref}` (never renamed).
- [X] T060 [P] [US4] Add proptests to crates/behavior-models/tests/laws.rs (≥ 1,000 cases each): random supply order of the model set and random trace order of the hand-written actions → identical module and BehaviorHash, or an identical sorted error list for the failing cases from T059 (law L3).

### Implementation for User Story 4

- [X] T061 [US4] Implement the set-level checks in crates/behavior-models/src/composition.rs (composition contract §2):
  - sort models by `model_id`; `DUPLICATE_MODEL_ID`;
  - build the ownership table before lowering; `CONFLICTING_OWNERSHIP`;
  - map a Core `DUPLICATE_NAME` on a generated action to `GENERATED_NAME_COLLISION`;
  - collect all errors and sort them with `sorted_unique`.
- [X] T062 [US4] Make `compose` lower all models in canonical `(model_id, transition_id)` order in crates/behavior-models/src/lib.rs, and document in its rustdoc that the binding must trace all ordinary behavior first (protocol step 1).
- [X] T063 [US4] Implement the contribution rule placeholder in crates/behavior-models/src/composition.rs: the StateMachine kind declares "actions only". Add an `INCOMPATIBLE_CONTRIBUTION` error code reserved for future kinds, with a unit test showing that StateMachine never contributes declarations.
- [X] T064 [P] [US4] Write python/tests/test_state_machine_composition.py mirroring T059 through `BehaviorModule(models=[…])`, including passing `models` in reversed order.
- [X] T065 [US4] Run `cargo test -p behavior-models --test composition --test laws` and `python -m pytest -q python/tests/test_state_machine_composition.py`, and confirm T059, T060 and T064 pass.

**Checkpoint**: independent machines compose without coordinating local names.

---

## Phase 7: User Story 5 - The foundation carries a future RequestFlow without Core changes (Priority: P3)

**Goal**: prove that the infrastructure is general enough for RequestFlow, using a test-only
generator.

**Independent Test**: generate the RequestFlow lifecycle as a StateMachine plus an ordinary
create action, run the return/resubmit cycle, and check that StateMachine contains no request
concepts (spec US5).

- [X] T066 [P] [US5] Write python/tests/test_request_flow_reference.py:
  - a test-only generator `request_flow(entity, field)` emits a `StateMachine` (Draft → Submitted → InReview → {Approved, Rejected, NeedsChanges}, NeedsChanges → Submitted via `resubmit`, entry state Draft) and an ordinary hand-written `create_request` action with `status = Draft`;
  - build it, admit it, and invoke `submit → start_review → return → resubmit → start_review → approve` through the public engine;
  - all transitions commit in that order (SC-007).
- [X] T067 [P] [US5] Add a domain-neutrality test in crates/behavior-models/tests/composition.rs that scans the crate's src/ for `request`, `approv` and `assignee` in code and string literals, ignoring comments and rustdoc, and asserts there are no matches (FR-032).
- [X] T068 [US5] Run `python -m pytest -q python/tests/test_request_flow_reference.py` and `cargo test -p behavior-models`, and confirm T066 and T067 pass with no change to crates/behavior-models/src.

**Checkpoint**: the RequestFlow path is shown to work without Core or StateMachine changes.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T069 [P] Rewrite models/README.md:
  - the Rust crate `crates/behavior-models` is the single authority;
  - the rules of `contracts/lowering-contract.md` (laws L1–L9, the no-Core-change stop gate);
  - the StateMachine meaning `Q := Cases(T_f)`, `t = (F_t, q_t, G_t, E_t, P_t, K_t)`;
  - the new worked example;
  - no mention of the deleted `lower.py`.
- [X] T070 [P] Add rustdoc to every public item of crates/behavior-models/src/lib.rs, citing the contract section it implements, so the doc does not drift from the contracts.
- [X] T071 [P] Add a short "Behavior Models" section to README.md that points to models/README.md and the StateMachine API (no semantics restated).
- [X] T072 Run every scenario in specs/501-model-lowering-state-machine/quickstart.md, including §8: `git -C ../behavior-ir-core status` shows no change made by this feature, and `scripts/check-core-pin.sh` passes.
- [X] T073 Run `scripts/gates.sh` end to end and record the result in specs/501-model-lowering-state-machine/implementation-log.md, with the final ModelHash and BehaviorHash of each reference case.
- [X] T074 Re-check the plan's anti-pattern audit against the implementation, row by row, and record the evidence (file and line, or test name) for each row in specs/501-model-lowering-state-machine/implementation-log.md.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** → **Foundational (Phase 2)** → user stories → **Polish (Phase 8)**.
- **US1 (Phase 3)** depends only on Phase 2. It is the MVP.
- **US2 (Phase 4)** depends on US1 (it needs generated actions to authorize, T024–T026).
- **US3 (Phase 5)** depends on US1. Its ModelHash needs admitted generated actions. It can run
  in parallel with US2.
- **US4 (Phase 6)** depends on US2 (the ownership table, T038) and on US1.
- **US5 (Phase 7)** depends on US1 and US2 (a create action checked against entry states). Its
  compat case (T055) is added in US3.
- **Polish** depends on all stories.

```text
Setup → Foundational → US1 ─┬─> US2 ─┬─> US4 ─┐
                            │        └─> US5 ─┤
                            └─> US3 ──────────┴─> Polish
```

### Within each story

Tests → types and resolution → lowering and composition → binding → verification run. Files
shared within a story (`lower.rs`, `composition.rs`, `lib.rs`) are edited sequentially.

### Parallel opportunities

- Phase 1: T004 and T005.
- Phase 2: tests T007 and T008, then T009 and T010.
- US1 tests: T015–T021 (separate files, or separate test functions written before any
  implementation).
- US2 tests: T034–T037. US3 tests: T044–T050. US4: T059, T060 and T064. US5: T066 and T067.
- After US1: US2 and US3 can proceed in parallel. After US2: US4 and US5 can proceed in
  parallel.
- Polish: T069–T071.

---

## Parallel Example: User Story 1

```bash
# Write all US1 tests first (they must fail):
Task: "Resolution rule tests in crates/behavior-models/tests/resolve.rs"            # T015
Task: "Golden layout tests in crates/behavior-models/tests/lower.rs"                # T016
Task: "Transition matrix + Core consumer tests in python/tests/test_state_machine.py" # T020
```

## Parallel Example: User Story 3

```bash
Task: "Canonical document golden test in crates/behavior-models/tests/identity.rs"  # T044
Task: "Permutation and metadata law proptests in crates/behavior-models/tests/laws.rs" # T045
Task: "Compat gate self-test in scripts/tests/test_models_compat.sh"                # T049
```

---

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1 Setup, then Phase 2 Foundational.
2. Phase 3 (US1): a StateMachine runs as ordinary Core behavior, and the old Python lowerer is
   gone.
3. **STOP and validate**: T033, plus the transition matrix (SC-001).

Note that the MVP alone does not yet keep ownership (US2). Do not release a StateMachine API
before US2 is done, because spec US2 is also P1.

### Incremental delivery

1. US1 → US2: the first releasable increment (both P1).
2. US3: identity, receipts and the version gate. Required before any lowering output is relied
   on externally.
3. US4: several machines per module.
4. US5: the RequestFlow readiness proof.
5. Polish, then a full `scripts/gates.sh` run.

### Stop conditions

- Any need to edit behavior-ir-core, or an API missing from research R1 → stop and file a gap
  report (lowering contract §8). Do not work around it.
- Any test that requires Python to sort, normalize or decide model semantics → the design is
  being violated. Fix the Rust side instead.

---

## Notes

- [P] tasks touch different files and have no dependency on an incomplete task.
- Commit after each task or logical group. Tests must be seen failing before their
  implementation.
- The Core pin stays at 0.12.0 throughout. Do not run gates with a local Core override.

## Phase 9: Convergence

- [X] T075 Expose command emissions on transitions in the Python binding: give `StateMachine.transition` an `emits=[...]` parameter (callables returning a command name and payload, bound by position like the other fragments) in python/behavior/models.py; let the Python session builder use the command profile when a module declares commands (python/behavior/module.py, crates/behavior-py/src/lib.rs); pass the emissions through `TransitionFragments.commands` in crates/behavior-py/src/models.rs; add a Python test where `approve` emits a command in the same commit per FR-022, US1/AC1 (partial)
- [X] T076 Make lowering receipts reproducible across checkouts: relativize every source-map location (machine and transition `source`) with the same root rule Core's `Builder::finish` applies to module locations (the `root` argument, else the common directory of all files), in crates/behavior-models/src/source_map.rs; add a test that two builds from different absolute paths give byte-identical receipts per Constitution IV (partial)
- [X] T077 Report transitions unreachable from entry states: add an `UNREACHABLE_TRANSITION` warning diagnostic for each transition none of whose source states is reachable from `Q_entry` (guards ignored) in crates/behavior-models/src/source_map.rs, with a test in crates/behavior-models/tests/identity.rs, per FR-031 (partial)
- [X] T078 Resolve the receipt API shape per T054: either add a typed `LoweringReceipt` (read-only view over the receipt dict) to python/behavior/__init__.py and api/public-api.json, or record in contracts/identity-and-source-map.md that Python receipts are canonical dicts and update T054's note in implementation-log.md (partial)
