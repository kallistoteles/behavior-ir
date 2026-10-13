# Implementation Log: 502 RequestFlow Model

**Date**: 2026-10-12 | **Branch**: `502-request-flow` (stacked on 501) | **Core Release**: 0.12.0
(`b525da46d4282166418a90141f0c94fd099e9b47`), unchanged.

## Result (T054, T055)

`nix develop -c scripts/gates.sh` passes every gate: core pin, public surface, fetch core, cargo
fmt, clippy, cargo test, maturin develop, pytest (425 passed, 4 skipped), mypy, models compat
(two tuples), determinism check, ownership, terms, workflows and script tests. All quickstart
scenarios pass; `scripts/check-core-pin.sh` reports v0.12.0 unchanged.

New Rust suites in `crates/behavior-models/tests`:

| Suite | Tests | Notes |
|---|---|---|
| request_flow_staging | 8 | laws D2–D6, including authored forward references (D5) |
| request_flow_resolve | 8 | every data model §3 rule; schema golden; `Id`, never `Ref` |
| request_flow_create | 8 | creation layout; SC-006 transition equivalence; whole-module equivalence with a hand-written module; `INITIALIZER_*` |
| request_flow_ownership | 7 | the full ownership matrix (R7) |
| request_flow_identity | 6 | canonical document golden; semantic changes; receipt and source map; 1,000-case permutation and metadata property (seed 502) |
| request_flow_composition | 5 | namespaces, `DUPLICATE_MODEL_ID`, `CONFLICTING_OWNERSHIP`, `GENERATED_NAME_COLLISION`; 1,000-case supply and declaration order property |
| ids | 6 | `EncodeEntityName` golden and 2,000-case injectivity and round-trip property |

New Python suites: `test_model_staging` (D2 over every pinned 501 case), `test_request_flow`
(the lifecycle through `evaluate`), `test_request_flow_ownership`, `test_request_flow_lifecycle`
(1,000 seeded traces, `trace ∈ Q_entry · R_T*`), `test_request_flow_authority` and
`test_request_flow_composition` (D5 visibility: actions, `Id`/`Ref` fields, reads, queries,
guards).

No change was made to behavior-ir-core. Its main checkout shows the maintainer's in-progress
store files (`crates/behavior-store/…`, `docs/persistence.md`), which are not from this feature.
Core source was only read from Cargo's checkout of the pinned revision.

## Pinned reference hashes (`models/fixtures/compat.json`)

The 501 `state_machine` tuple is byte-identical (the diff only adds lines). New tuple:
`request_flow`, model semantic version 1, lowerer version 1, embedded `state_machine` 1/1,
Core 0.12.0.

| Case | BehaviorHash | RequestFlowHash | Embedded StateMachine hash | Entity |
|---|---|---|---|---|
| rf_purchase_request | `sha256:6a51f91049227d70d7b61dcd53f1e4e23127c8329e6ad0cb21f6bc1d2b182062` | `sha256:daaff141…ccb352` | `sha256:f4ae4732…959188` | `PurchaseRequest` |
| rf_two_flows_local_submit | `sha256:98c6b300c763d24b699bdb30f60c24e2ac20f6aea76ea6a3c4a76a6fb49f920b` | purchase_request `sha256:e5ada30a…13468b`, leave_request `sha256:76107e6a…d45132` | `sha256:a2e560ee…4a63f7`, `sha256:4af97001…a35e` | `PurchaseRequest`, `LeaveRequest` |
| rf_with_standalone_machine | `sha256:8cabd65707a101b365b714f6db4aea285a7244aa1df55521cbb0f5b38ce6ccf2` | `sha256:e5ada30a…13468b` (budget machine `sha256:54d5ff23…b8604`) | `sha256:a2e560ee…4a63f7` | `PurchaseRequest` |

## Findings during implementation

- **Core's implicit `id` field.** Every admitted entity has `id: Id(Self)`, which is never
  declared. The identical-declaration check (D4, D6) skips exactly that field.
- **Authored forward references (D5).** P2 first admitted a snapshot of the authored
  declarations, which fails when an authored entity holds `Id[Request]`: Core checks declarations
  together and the request entity does not exist yet. Core's semantics allow the forward
  reference; only our snapshot order did not. P2 now falls back to deriving the schema from the
  flow input alone, declaring it, and checking actor, enum and collisions against the completed
  declarations. On that path a missing actor appears as `CORE_ADMISSION_FAILED` rather than
  `UNKNOWN_ENTITY`, because the declarations as a whole cannot be admitted. No Core change was
  needed.
- **Commands after P2.** A command payload may name the request entity (`Id[Request]`), and Core
  checks command fields when the command is added. Commands are therefore declared after P2
  (phase P2b; the staging contract and T011 were updated).
- **An ill-typed creation stays in the builder.** `add_action` does not type-check `Create`
  values given as wire expressions, so lowering admits after each creation to report the error
  against it. Core's `Builder` cannot take an action back, so after such a failure every later
  admission repeats it. Lowering stops at the first inadmissible creation instead of reporting
  the same error once per creation.
- **Duplicate model ids in P2.** Two supplied models with one id would otherwise surface in P2 as
  the declaration clash they cause. P2 now reports `DUPLICATE_MODEL_ID` first, like `compose`.
- **Core outcome codes.** A failing guard expression gives `ERROR` with `EVALUATION_ERROR` and a
  "division by zero" message. There is no `DIVISION_BY_ZERO` code, which T031's text assumed;
  the test pins Core's actual outcome. Evaluations under the command profile need complete
  snapshot facts (`universe`), as for any Core module with commands.
- **Kind registry.** Request vocabulary may not appear in the kind-neutral files (the neutrality
  test). Kind dispatch therefore lives in `kinds.rs` (`AnyModel`, `declare_models`,
  `compose_models`), and T008's planned `ModelInput::RequestFlow` variant became this registry
  enum, so `ModelInput` stays exactly 501's.
- **Compat gate.** Cases are grouped by tuple. A receipt with embedded models adds their
  versions to its key. A case containing a flow and a standalone machine belongs to the flow's
  tuple when the embedded versions cover the standalone machine.

## Deviations

- **Test-first, partially.** The Rust RequestFlow modules (`request_flow/*`, `kinds.rs`) were
  written before their tests (T013–T015, T025–T026); the tests followed immediately and found the
  `id`-field, poisoned-builder and forward-reference issues above. The Python layer and US2–US5
  went test-first.
- **T008** implemented as the `AnyModel` registry enum (see above), not as a `ModelInput` variant.
- **T031** asserts `EVALUATION_ERROR`, Core's real code, instead of `DIVISION_BY_ZERO`.

## Anti-pattern audit (plan.md)

| Anti-pattern | Evidence |
|---|---|
| A second status semantics in RequestFlow | transitions go through `lower_machine` → 501 `lower_transition`; `transitions_equal_a_standalone_state_machine`, `the_flow_module_equals_the_hand_written_module` |
| Entry states declared or stored twice | `RequestFlowInput` has no entry field; `sm_input` is the one derivation; `entry_states_are_derived_from_creation_targets` |
| Running lowering to obtain the entity handle | `declare_models` uses `decls`/`input_schema` only; the Python handle's name comes from `encode_entity_name` |
| Generating the status enum | `UNKNOWN_ENUM` test; P2 declares entities only |
| Hidden authority, clock or identity | `test_without_a_guard_anyone_may_approve`, `test_time_comes_from_context_only` (no clock, randomness or UUID in the wire module) |
| Implicit existence for actors | `test_requester_and_assignee_are_identities_not_references`, schema golden |
| Two receipts per flow / an id-rule exception | `one_receipt_with_an_embedded_identity_record`; `Composed.supplied`; `model_ids_are_one_namespace_across_kinds` |
| Overwriting or renaming on a name clash | `DECLARATION_COLLISION`, `GENERATED_NAME_COLLISION` tests |
| Wire JSON as an interface | `check-public-surface.sh` limits `into_wire` to `request_flow/create.rs` |
| Python deciding the schema | `RequestFlow.bind_schema` drift check against the P2 `DeclaredSchema` |
| Changing 501 outputs | `test_model_staging.py`, compat gate, empty removal diff of `compat.json` |

## Convergence (Phase 9, T057–T061)

`/speckit-converge` found five partial gaps; all are closed:

- **T057 (SC-002).** `test_a_transition_commits_iff_its_source_set_and_guards_allow` covers
  every status × transition pair of the reference flow, with an admitted and a refused actor
  for guarded transitions. The source sets are written out in the test, not read from the
  build. `test_every_creation_starts_in_exactly_its_target` covers every creation.
- **T058 (research R11).** A drifted request-entity handle (name, field type, missing or extra
  field) fails the build with the `bind_schema` error.
- **T059 (Edge Cases, "Assignee disabled").** This found a binding bug: an initializer setting
  `assignee: None` on a flow without assignee support failed as `FRAGMENT_INVALID`
  (`TYPE_MISMATCH`, an untyped `none`) before Rust's initializer check could name the field.
  The creation tracer now passes a placeholder for fields outside the schema, so Rust reports
  `INITIALIZER_UNKNOWN_FIELD { field: "assignee" }`; Rust still decides. A transition effect on
  `assignee` gives Core's `UNKNOWN_FIELD`.
- **T060 (data model §3).** Not fixable on the public API: while the declarations cannot be
  admitted, every node constructor fails, and Core names a missing type only in its error
  message. The deviation is recorded in data-model.md §3, and the test pins Core's own
  errors (`UNKNOWN_ENTITY` for the actor, `UNKNOWN_TYPE` for the enum). Classification:
  existing Core semantics without a structured public error field; no Core change is needed.
- **T061 (T001).** scripts/tests/test_check_public_surface.sh negative-tests the wire guard on
  a copy of the tree. The guard now also rejects `serde_json::to_string`/`to_vec`/`to_writer`
  in the model crate.

## Convergence (Phase 10, T062)

The hard rule "serde serialization nowhere in the model crate" (tasks.md) is precise as: no
serialization to text or bytes (`serde_json::to_string`/`to_vec`/`to_writer`, enforced by
scripts/check-public-surface.sh). `serde_json::to_value` of a `WType` in
`identity::params` (since 501) is by design: canonical documents hold parameter types as
`"type": <WType>` (data model §7). The guard's comment now says exactly this.
