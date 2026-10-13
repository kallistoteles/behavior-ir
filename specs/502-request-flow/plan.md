# Implementation Plan: RequestFlow Model

**Branch**: `502-request-flow` (stacked on `501-model-lowering-state-machine`) | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/502-request-flow/spec.md`

## Summary

RequestFlow is a new model kind in the existing `behavior-models` crate:

```text
L_RF(R) = Compose( Decls(R), Creates(R), L_SM(SM(R)) )
```

- `Decls(R) = { E }`: the generated request entity, named `EncodeEntityName(f)`, with
  canonically ordered fields.
- `Creates(R)`: one ordinary creation action per creation spec.
- `L_SM(SM(R))`: the **unchanged** 501 StateMachine resolver, lowerer, identity and ownership
  pass, applied to a derived machine whose entry states are computed only from the creation
  targets.

The architectural change is **declaration staging**. Hand-written behavior must be able to
reference the generated entity, so the build protocol gains a declaration phase (P2) between
authored declarations and hand-written tracing. P2 is a no-op for declaration-free models, so
StateMachine builds and their pinned hashes cannot change.

**Planning acceptance criterion**: the generated request entity exists early enough for
arbitrary hand-written behavior to reference it, while RequestFlow lowering remains a pure
composition of generated declarations, creation behavior and the unchanged authoritative
StateMachine lowerer. Research R1 (staging), R6 (reuse) and the declaration-staging contract
establish this.

## Technical Context

**Language/Version**: Rust stable (edition 2024, pinned toolchain); Python 3.13 binding.

**Primary Dependencies**: `behavior-engine` (Core 0.12.0, pinned), `serde_json`, `thiserror`;
dev only `proptest`; binding `pyo3` 0.26. No new dependency.

**Storage**: N/A. Receipts are values; the compatibility manifest is a committed fixture.

**Testing**: `cargo test -p behavior-models` (unit, golden and property tests); pytest
(binding, end-to-end through the public engine, the lifecycle trace property); `models compat`
gate; determinism check.

**Target Platform / Project Type**: unchanged from 501 (library plus Python extension).

**Performance Goals**: none beyond fitting the gate run. Property suites use ≥ 1,000 cases.

**Constraints**: no Core change; `behavior-engine` only; no Wire JSON as an interface
(`into_wire` allowed only in `request_flow/create.rs`, research R4); deterministic outputs;
501's pinned hashes are unchanged.

**Scale/Scope**: about 5 new Rust source files, extensions to `composition.rs`, `identity.rs`,
`source_map.rs`, `error.rs` and the Python glue, one new Python module, fixtures, and gate
script changes.

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core | No AI; lowering is deterministic. ✅ |
| II. Validated outputs | Binding input is untrusted: typed resolution and typed errors. ✅ |
| III. Test-First | Tests first per phase; property tests for the laws (`proptest`, seeded Python traces). ✅ |
| IV. Reproducibility | Canonical orders everywhere (fields, creations, parameters); identical-or-error merge; the compat gate extended to flows and to embedded machine versions. ✅ |
| V. Explicit state, auditability | The lifecycle theorem is tested; receipts carry the flow → state machine → Core provenance chain. ✅ |
| VI. Simplicity | No new crate or dependency. StateMachine is reused through existing functions, not a trait framework. The only new abstraction is `Decls(m)`, required by staging. ✅ |
| Bindings: no core semantics in a binding | Python records input and holds a typed handle, drift-checked against Rust's schema. ✅ |
| `behavior-engine` only | Unchanged. The `into_wire` exception is the public creation input type (R4) and is limited to one file by the guard. ✅ |
| Gates = `scripts/gates.sh` | Unchanged set; `models compat` extended. ✅ |

**Result: PASS** before and after design. Complexity Tracking is empty.

## Planning answers (spec FR-030)

| Question | Answer | Evidence |
|---|---|---|
| The public engine builds the entity and creation actions | `declare_entity`; `add_action(_with_commands)` with `WLifecycle::Create` | research R2, R4 |
| Unauthorized creation and requester writes are detectable | `ActionItem::creates()` and `effects()` over the admitted module | research R7 |
| StateMachine is invoked directly | `resolve`, `lower_transition`, `canonical_document`, `diagnostics`, `final_pass` | research R6 |
| Fragments stay Core fragments | creation and transition scopes opened by Rust; `into_wire` only as the creation input type | research R4 |
| Generated names are injective | `EncodeEntityName` (proof plus proptest) and 501 `encode`; creation and transition ids share one namespace | research R3; data model §3 |
| No Core change | none needed | research R13 |

## Project Structure

### Documentation (this feature)

```text
specs/502-request-flow/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/
│   ├── declaration-staging.md      # P1–P4 protocol, laws D1–D6
│   └── request-flow-contract.md    # meaning, runtime behavior, rules, API, reference flows
├── checklists/requirements.md
└── tasks.md                        # /speckit-tasks
```

### Source Code

```text
crates/behavior-models/src/
├── lib.rs                     # MODIFIED: declare_models(), ModelSpec gains the RequestFlow kind, versions
├── ids.rs                     # MODIFIED: encode_entity_name, decode_entity_name
├── composition.rs             # MODIFIED: declaration merge, supplied vs internal model set, kind-neutral check hook
├── identity.rs                # MODIFIED: kind-neutral helpers only
├── source_map.rs              # MODIFIED: kind-neutral receipt and path helpers only
├── error.rs                   # MODIFIED: new codes (data model §6, staging contract §4)
└── request_flow/              # NEW: all request-specific code lives here
    ├── mod.rs
    ├── input.rs               # RequestFlowInput, CreationInput, CreationFragments
    ├── resolve.rs             # → ResolvedRequestFlow; schema; sm_input (the only Q_entry derivation)
    ├── decls.rs               # Decls(RF): the request entity declaration (P2)
    ├── create.rs              # creation lowering (the only file allowed to call into_wire)
    ├── ownership.rs           # creation and requester rules, called by final_pass through a kind-neutral hook
    └── identity.rs            # RF canonical document, receipt, embedded record, source-map entries
crates/behavior-models/tests/  # NEW: request_flow_*.rs (resolve, decls, create, ownership, identity, laws)
crates/behavior-py/src/models.rs   # MODIFIED: declare_models, RF input, creation tracer, encode_entity_name
python/behavior/request_flow.py    # NEW: RequestFlow API, request_entity handle (drift-checked)
python/behavior/module.py          # MODIFIED: P2 call between authored declarations and tracing
python/tests/test_request_flow*.py # NEW: lifecycle, ownership, authority, composition, trace property
models/fixtures/reference_cases.py # MODIFIED: rf_* cases
models/fixtures/compat.json        # MODIFIED: new tuple for request_flow (501 tuple byte-identical)
scripts/check-models-compat.py     # MODIFIED: per-case tuples, embedded versions
scripts/check-public-surface.sh    # MODIFIED: into_wire allowed only in request_flow/create.rs
api/public-api.json                # MODIFIED: RequestFlow
models/README.md                   # MODIFIED: RequestFlow section
```

**Structure Decision**: a module inside the existing crate, per spec §42. No new crate.

## Implementation phases (tests first in each)

1. **Staging infrastructure**: `Decls`, `declare_models`, identical-or-error merge, the
   StateMachine no-op regression test against every 501 manifest case.
2. **`EncodeEntityName`**: implementation plus proof-backed property test.
3. **RequestFlow resolve**: schema, creations, `sm_input`, every resolution error.
4. **Creation lowering**: scope protocol, initializer checks, `WLifecycle::Create`.
5. **Composition**: supplied versus internal model sets, ownership matrix (creation, requester),
   embedded machine through the existing paths.
6. **Identity and provenance**: RF canonical document, embedded record, source map,
   diagnostics.
7. **Python**: `RequestFlow`, `request_entity` handle, the P2 call in `BehaviorModule`,
   receipts.
8. **Compat and determinism**: manifest tuple extension, `rf_*` cases, gate tests.
9. **Lifecycle property and docs**.

## Test plan (from laws and the spec's required tests)

| Requirement or law | Test | Kind |
|---|---|---|
| Spec test 1, SC-001 | create → submit → start_review → return → resubmit → start_review → approve through the public engine | integration (Py) |
| Spec tests 2–3, FR-012 | two creations (Draft, Submitted) → `entry_states = {Draft, Submitted}`; a flow input has no entry-state field | unit + integration |
| Spec tests 4–6, SC-004 | hand-written create of `E`, status writes (hand-written, `E_c`, transition effect), requester writes → exact errors; assignee write OK | negative (Rust + Py) |
| Initializer, FR-009 | missing, unknown, `status`, duplicate field → static errors per creation and field | negative |
| Spec tests 8–9 | creation and transition guards true / false / `DIVISION_BY_ZERO` | integration (Py) |
| Spec test 10 | commands on creation and approve; a denied step emits nothing | integration (Py) |
| Spec tests 11–12 | two flows with `submit`; creation id = transition id → `CAPABILITY_ID_COLLISION` | composition |
| Spec tests 13–14, SC-005 | permutations of payload, creations, transitions, input, state, context, source states and model supply order → equal RequestFlowHash, module, BehaviorHash (≥ 1,000) | property |
| Spec test 15, SC-006 | flow transition actions = standalone StateMachine transition actions by Core action hash | equivalence |
| Spec test 16 | source map: creation → flow creation; transition → flow transition + embedded state-machine transition; entity and fields mapped; locations relative | unit |
| Spec tests 17–18, SC-007 | the module holds only declared items, `E`, and `f__*` actions; admitted by the public engine | golden + integration |
| Spec test 19, SC-009 | compat gate: RF tuple pinned; edited hash → FAIL; SM lowerer bump forces a new RF tuple | script |
| Spec test 20, SC-003 | seeded random traces (≥ 1,000) satisfy `trace ∈ Q_entry · R_T*` | property (Py) |
| SC-010, FR-002 | `EncodeEntityName` injective, decodable, Core-valid over the grammar (≥ 2,000) | property |
| D2: SM regression | `BehaviorHash_new protocol(M) = BehaviorHash_pinned(M)` for every 501 manifest case; P2 makes no builder call for StateMachine-only sets | regression |
| D5: visibility | hand-written: an action with an `Id[request_entity]` input; an authored entity with `Id` and `Ref` fields to `E`; a read and a query over `E`; a guard reading request fields | integration (Py) |
| D3, D6 | an authored entity named `E` → `DECLARATION_COLLISION`; an identical one → accepted; authored declaration order permuted → equal hashes | negative + property |
| FR-003 | the generated schema has `Id(A)` / `Option(Id(A))`, never `Ref`; removing an actor that requests point at is allowed | golden + integration |
| One receipt, FR-025 | one supplied flow → exactly one receipt, with an `embedded` state-machine record; a standalone machine with the flow's id → `DUPLICATE_MODEL_ID` | unit |
| Domain neutrality | `state_machine/`, `ids.rs`, `composition.rs`, `identity.rs` and `source_map.rs` contain no request terms; scope updated in Setup (T002), re-checked in Polish (T052) | unit |
| Equivalence pair (constitution) | the RequestFlow module's BehaviorHash equals a hand-written module (authored entity, hand-written creations, standalone StateMachine) | equivalence |

## Anti-pattern audit

| Anti-pattern | Prevented by |
|---|---|
| A second status semantics in RequestFlow | transitions only through the 501 functions (R6); the SC-006 equivalence test |
| Entry states declared or stored twice | the input has no entry field; `sm_input` is the only derivation |
| Running lowering to obtain the entity handle | `Decls` is separate (D1); the handle name comes from `encode_entity_name` alone |
| Generating the status enum | `status_enum` must be authored (`UNKNOWN_ENUM`); never declared by P2 |
| Hidden authority, clock or identity | the runtime table has none; tests show a non-guarded `approve` is allowed for anyone |
| Implicit existence for actors | `Id` only; a removal test |
| Two receipts per flow / an id-rule exception | supplied versus internal sets (R9) |
| Overwriting or renaming on a name clash | identical-or-error merge; collision tests |
| Wire JSON as an interface | the guard limits `into_wire` to `create.rs`; `to_wire_json` and serialization are forbidden |
| Python deciding the schema | the handle is drift-checked against `DeclaredSchema` from Rust |
| Changing 501 outputs | D2 regression test, compat gate |

## Decision table

| Concern | Decision | Enforcement |
|---|---|---|
| Build order | P1 authored → P2 model declarations → P3 tracing → P4 compose | declaration-staging contract; Python `BehaviorModule` |
| Entity name | `EncodeEntityName(f)` (not PascalCase) | proof plus property test |
| Entity field order | sorted by field name | golden schema test; permutation property |
| Status enum | authored, referenced by Core hash | `UNKNOWN_ENUM`; identity document |
| Entry states | derived only in `sm_input` | no input field; unit test |
| Status writes | embedded machine only | 501 final pass |
| Requester writes | creation initializer only | `UNAUTHORIZED_REQUESTER_WRITE` |
| Request creation | flow creation actions only | `UNAUTHORIZED_REQUEST_CREATION` |
| Creation extra effects | over declared state bindings `S_c`, confirmed by the author; create plus `E_c` is atomic (R5) | normal ownership pass; `S_c` in RequestFlowHash |
| Capability names | `encode(f, id)` over creation ∪ transition ids | `CAPABILITY_ID_COLLISION` |
| Receipts | one per supplied model, with an embedded identity record | unit tests; Python receipts |
| Reproducibility | RF tuple includes the embedded machine's versions | compat gate |
| Core changes | none | stop-gate, as in 501 |

## Semantic ownership

| Fact | Owner | Representation |
|---|---|---|
| status domain `Q` | the authored enum `T_s` | `Cases(T_s)`, never copied |
| request schema | RequestFlow | `Decls(R)`, derived from `f`, `A`, `T_s`, `Π`, assignee |
| entry states | RequestFlow creations | `{q_c}`, passed once into `SM(R)` |
| transition relation `R_T` | embedded StateMachine | 501 resolved machine |
| status evolution | StateMachine | 501 lowering and ownership |
| request creation | RequestFlow | creation actions |
| requester | RequestFlow (creation only) | initializer; immutability rule |
| assignee, payload | ordinary behavior | ordinary fields |
| authority | the author | explicit guards only |
| runtime meaning | Core | `Meaning_Core(L_RF(R))` |

## Phase ownership

| Phase | Owner |
|---|---|
| Authored enum, actor and entity declarations | ordinary Behavior API (binding, P1) |
| Request entity declaration | RequestFlow (`Decls`) via common model infrastructure (P2) |
| Declaration merge and resolution | common model infrastructure (`composition::declarations`) |
| Hand-written tracing | Behavior authoring (binding, P3) |
| Creation lowering | RequestFlow (`request_flow/create.rs`) |
| Status transition lowering | StateMachine (501 `lower_transition`, unchanged) |
| Final ownership checks | common model composition (`final_pass`, extended) |
| Identity, receipts, source maps | common model infrastructure, with kind-specific documents |
| Runtime meaning | behavior-engine / Core |

## Complexity Tracking

No constitution violations.
