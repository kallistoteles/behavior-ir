# Implementation Plan: Declaration identity of named types in StateMachine and RequestFlow hashes

**Branch**: `505-model-type-identity` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/505-model-type-identity/spec.md`
(kallistoteles/behavior-ir#20)

## Summary

The StateMachine and RequestFlow canonical documents name their types but do not identify them.
This feature makes every type reference in them carry the Core declaration identity of what it
refers to, as Approval already does: the StateMachine's subject, the RequestFlow's actor and
state bindings, and every enum, nominal and entity mentioned by a parameter or payload type. One
kind-neutral implementation of that rule, taken from Approval, replaces the name-only helper in
`crates/behavior-models/src/identity.rs`. Approval's output stays byte-identical.

Both kinds' model semantic versions go to 2 and their document formats to `.v2`; their lowerers
are untouched, so every BehaviorHash stays the same. The compatibility gate gains explicit
supersession. A new tuple names the tuples it replaces (`supersedes`). The gate accepts the
removal only if the replacement has the same kind and Core Release, a strictly higher version
vector, and every logical reference case of the old tuple. The v1 tuples are replaced this way,
and the approval tuples are re-keyed because their co-supplied versions change.

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain), Python 3.13 (binding and gates).

**Primary Dependencies**: `behavior-engine` from the pinned Core Release 0.13.0 (admitted
module, `name_table()`, `canonical::tagged_hash`), `serde_json`, `proptest`. No new
dependency.

**Storage**: N/A. Model hashes appear only in lowering receipts and in
`models/fixtures/compat.json`.

**Testing**: `cargo test -p behavior-models`, pytest (`python/tests`, `models`),
`scripts/tests/test_models_compat.sh`, the `models compat` and determinism gates through
`scripts/gates.sh`.

**Target Platform**: Linux (Nix dev shell, CI).

**Project Type**: Library (Rust model authority with a Python binding).

**Performance Goals**: No measurable change. One name-table lookup per referenced type per
build.

**Constraints**: No Core change, no raw Wire JSON as an interface (FR-012). Lowering output is
byte-identical (FR-008). ApprovalHash is byte-identical (FR-003).

**Scale/Scope**: Two canonical documents (StateMachine, RequestFlow), one shared helper, two
version constants, one gate rule, ten reference cases re-pinned. The cases are 4
StateMachine, 3 RequestFlow, and 3 Approval (re-keyed only).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core | Pass. Identity is a pure function of the admitted module and the resolved model. No AI is involved. |
| II. Validated output | N/A. No AI output. |
| III. Test-first | Pass. Each changed document gets failing tests first: a declaration change, an unchanged declaration, a renamed type, dependency order, and an unresolved reference. The gate rule gets script tests before the change. |
| IV. Reproducibility | Pass, and this is the point of the feature. The `types` maps are `BTreeMap`-ordered, and the hash depends only on the referenced declarations (FR-005, FR-006). The existing permutation properties stay green (SC-005). |
| V. Explicit state and auditability | Pass. Receipts carry the exact declaration dependencies of each model. |
| VI. Simplicity | Pass. One shared helper replaces a name-only one, with no parallel implementation. |
| Technology constraints | Pass. `cargo fmt` and clippy `-D warnings`, typed `ModelError`, no `unwrap` outside tests, `Cargo.lock` unchanged. |
| Bindings and packaging | Pass. Hashing stays in Rust, and Python only reads receipts. Core Release 0.13.0 stays pinned. |

No violations, so the Complexity Tracking section is empty.

**Post-design re-check**: Pass. The design adds no crate, no binding API and no Core
dependency beyond the already public `name_table()` and `schema::declarations`.

## Project Structure

### Documentation (this feature)

```text
specs/505-model-type-identity/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R8
├── data-model.md        # Phase 1: the v2 documents and the shared reference shapes
├── quickstart.md        # Phase 1: validation scenarios
├── contracts/
│   ├── declaration-references.md   # the identity rule shared by every model kind
│   └── compat-supersession.md      # the gate rule for superseded tuples
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/behavior-models/src/
├── identity.rs                 # shared: named_types, typed parameters, entity references;
│                               #   StateMachine document v2 (subject, inputs, context)
├── request_flow/identity.rs    # RequestFlow document v2 (actor, payload, state, inputs, context)
├── approval/identity.rs        # uses the shared helpers; output byte-identical
└── lib.rs                      # STATE_MACHINE_SEMANTIC_VERSION = 2, REQUEST_FLOW_SEMANTIC_VERSION = 2

crates/behavior-models/tests/
├── identity.rs                 # StateMachine: declaration dependency tests, golden v2 document
├── request_flow_identity.rs    # RequestFlow: the same, plus the embedded machine
└── approval_identity.rs        # unchanged expectations (byte-identical ApprovalHash)

scripts/
├── check-models-compat.py      # explicit supersession (`supersedes`, --write --supersede)
└── tests/test_models_compat.sh # supersession tests

models/fixtures/compat.json     # v1 tuples removed, v2 tuples recorded, approval tuples re-keyed
models/README.md                # Model identity: exact declaration dependencies
```

**Structure Decision**: This is the existing workspace layout. The change is confined to
`behavior-models`' identity layer, the compatibility gate and its manifest. No Python binding
code changes. `python/tests/test_model_staging.py` reads its pinned cases from the manifest, so
it follows the new tuples without edits.

## Complexity Tracking

No constitution violations.
