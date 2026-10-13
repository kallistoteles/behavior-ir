# Implementation Plan: Deterministic Model Lowering and StateMachine Model

**Branch**: `501-model-lowering-state-machine` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/501-model-lowering-state-machine/spec.md`

## Summary

Add one binding-neutral Rust crate, `crates/behavior-models`, that is the only authority for
Behavior Models. It owns model resolution, validation, canonical identity (ModelHash),
composition and ownership, generated names, lowering, receipts and source maps. StateMachine is
its first model kind.

Lowering writes ordinary actions into the **same Core `Builder`** the binding uses for the
module. Fragments (guards, effects, postconditions, commands) are traced by the binding
*inside scopes Rust opens*. A final pass over the admitted Core module then enforces ownership
and entry states.

Everything goes through the public `behavior-engine` 0.12.0 facade. Research found no semantic
gap and no facade gap that needs a Core change (research R1). Python gains a thin
`StateMachine` authoring API through `behavior-py`.

**Planning principle**: planning is complete when another engineer can implement the feature
without making a new semantic decision. All semantic choices are fixed in
[data-model.md](data-model.md) and [contracts/](contracts/). The only freedom left is in naming
and module layout.

## Technical Context

**Language/Version**: Rust (stable, edition 2024, pinned by `rust-toolchain.toml`); Python
3.13 for the binding layer.

**Primary Dependencies**: `behavior-engine` (Core Release 0.12.0, exact git rev in the
workspace), `serde_json`, `thiserror` (constitution: typed library errors). Dev only: `proptest`. Binding: `pyo3` 0.26 (existing).

**Storage**: N/A. Receipts are returned values; the compatibility manifest is a committed
JSON fixture.

**Testing**: `cargo test` (unit, property, golden) in `crates/behavior-models`; `pytest`
(`python/tests`, `models/`) for the binding and Core consumer tests; `scripts/gates.sh` with a
new `models compat` gate; `scripts/determinism-check.sh`.

**Target Platform**: same as the existing package (Linux x86_64, abi3 wheel).

**Project Type**: library (Rust crate plus Python binding extension).

**Performance Goals**: not a product goal. Build-time lowering of the reference machines is
negligible next to Core admission. The property suites must fit inside the existing gate run
(≥ 1,000 cases per law, SC-002).

**Constraints**: no Core change; no internal Core crate; no raw Wire JSON as model interface;
deterministic, byte-identical outputs; `unsafe` forbidden; typed errors; no `unwrap` or
`expect` outside tests.

**Scale/Scope**: one new crate (≈ 9 source files), one new Python module, fixtures, and a
handful of existing scripts and manifests touched (listed below).

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core, Probabilistic Edge | No AI in this feature. Lowering is deterministic code. ✅ |
| II. Every AI Output Is Validated | N/A (no AI). Model input from bindings is treated as untrusted: typed resolution, typed errors. ✅ |
| III. Test-First | The test plan below is derived from the laws. Tests are written first per phase (tasks will order them). Property tests use `proptest`. ✅ |
| IV. Reproducibility and Replay | Laws L1–L3 and L7–L8. `BTree*` only, sorted canonical orders, fixed-seed property runs, determinism-check extension, immutable compatibility manifest. ✅ |
| V. Explicit State and Auditability | StateMachine is the explicit state model. Typed error catalogue; receipts and source maps give traceability from model to Core. ✅ |
| VI. Simplicity | One crate, no crate split. No expression language; Core fragments are reused. No new hashing algorithm; Core's `tagged_hash` is reused. Only the `proptest` dev-dependency is new. ✅ |
| Bindings and Packaging: "A binding MUST NOT implement core semantics" / "A model lowers completely … never introduces runtime semantics" | The model crate depends only on `behavior-engine`. Python decides nothing semantic (lowering contract §5). Lowered modules are pure Core (law L4). ✅ |
| "The only core dependency is the public `behavior-engine` crate" | Yes. `check-public-surface.sh` is extended to the new crate (research R11). ✅ |
| Quality gates = `scripts/gates.sh` | New gate `models compat` added; existing gates cover the crate (`clippy --workspace`, `pytest … models`). ✅ |

**Result: PASS.** No violations, so Complexity Tracking is empty. Re-check after Phase 1:
**PASS**. The design added no dependency, crate or abstraction beyond those listed.

## Boundary decisions

### What Rust (`behavior-models`) decides, exclusively

Resolution and defaults (including `subject_param`); semantic-id validation; set semantics and
canonical order; state domain derivation; ownership table; generated names; transition scope
parameters; fragment validation (both layers); generated action layout; the Core admission
call; final composition pass; entry-literal probe; ModelHash; receipts; source maps;
diagnostics; error ordering; versions.

### What Python may do

Turn syntax into `ModelInput`; trace author fragments inside a Rust-opened scope using
Rust-given names, bound by position; attach source locations and display metadata; raise
early, non-authoritative errors; expose receipts, diagnostics and `ModelBuildError`.

### What Python must not do

Sort, deduplicate, default, rename or validate model semantics; compute names, hashes or
ownership; add generated actions; pass lambdas, objects, source text or Wire JSON across the
boundary as semantics.

## FR-035 planning answers

| # | Question | Answer | Evidence |
|---|---|---|---|
| 1 | Which public APIs build every lowered object? | `builder::Builder` (`push_scope`, `field`, `lit`, `in_`, `op`, `add_action`, `add_action_with_commands`, `finish`), `builder::{Node, CommandEmissionSpec}`, `wire::{WParam, Role, WType, Loc}` | research R1 |
| 2 | How does final composition enforce ownership and entry states? | Pass over the admitted `semantic::Module`: `actions()` → `params()`, `effects()`, `creates()`; entry literals by Core-hash equality via a `Builder::for_module` probe | research R7, R8; composition contract §4–5 |
| 3 | Which identifiers are reused for generated identities? | Core's semantic item hashes for fragments; model semantic ids for names, with `encode(m, t) = m__t`, proven injective under Core's identifier grammar | research R4, R5 |
| 4 | Does ModelHash need a new canonical encoding? | A new document *schema* (`behavior.model.state_machine.v1`) only. The algorithm is Core's public `canonical::tagged_hash` | research R5; identity contract §2 |
| 5 | Is a Core change needed? | No. Gaps G1 and G2 are facade-exposure gaps that the design does not need. No semantic gap | research R1 |

## Project Structure

### Documentation (this feature)

```text
specs/501-model-lowering-state-machine/
├── spec.md
├── plan.md                      # this file
├── research.md                  # Phase 0
├── data-model.md                # Phase 1
├── quickstart.md                # Phase 1
├── contracts/
│   ├── lowering-contract.md
│   ├── composition-and-ownership.md
│   ├── identity-and-source-map.md
│   └── state-machine-contract.md
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
crates/behavior-models/              # NEW: the single model authority
├── Cargo.toml                       # deps: behavior-engine (workspace), serde_json; dev: proptest
└── src/
    ├── lib.rs                       # compose() entry point, versions, public types
    ├── ids.rs                       # semantic-id grammar, encode(m, t)
    ├── identity.rs                  # canonical model document, ModelHash
    ├── composition.rs               # model set, ownership table, final pass, entry probe
    ├── source_map.rs                # receipts, source map, diagnostics
    ├── error.rs                     # typed error catalogue, sorting
    └── state_machine/
        ├── input.rs                 # ModelInput, FragmentSource trait
        ├── resolve.rs               # → ResolvedStateMachine
        └── lower.rs                 # action layout, fragment validation
crates/behavior-models/tests/        # law and property tests, golden cases
crates/behavior-py/                  # MODIFIED: depends on behavior-models; exposes compose
python/behavior/models.py            # NEW: StateMachine authoring API (no semantics)
python/behavior/module.py            # MODIFIED: BehaviorModule(models=[...]) → Rust compose
python/tests/test_state_machine.py   # NEW: binding and Core consumer tests
models/README.md                     # REWRITTEN: Rust crate is the authority
models/examples/state_machine/       # lower.py REMOVED; reference input + equivalence test
models/fixtures/compat.json          # NEW: compatibility manifest
scripts/check-models-compat.py       # NEW: gate (lowering contract §7)
scripts/tests/test_models_compat.sh  # NEW: gate self-test (SC-009)
scripts/gates.sh                     # MODIFIED: + "models compat"
scripts/check-public-surface.sh      # MODIFIED: also checks crates/behavior-models
scripts/determinism-check.sh         # MODIFIED: + model receipts
specs/011-core-ecosystem-split/contracts/ownership.md   # MODIFIED: + crates/behavior-models/** ecosystem (before rule 3)
api/public-api.json                  # MODIFIED: + StateMachine, ModelBuildError, LoweringReceipt
Cargo.toml                           # MODIFIED: workspace member
```

**Structure Decision**: one Rust crate in the existing workspace. The dependency chain is
`behavior-py → behavior-models → behavior-engine`, and `behavior-py` keeps its direct
`behavior-engine` dependency for ordinary Core APIs. No crate split until later models show the
need (spec Assumptions).

## Implementation phases (order matters; tests first in each)

1. **Repository plumbing**: ownership row, workspace member, public-surface check extended,
   empty crate passing the gates.
2. **Ids and encoding**: `ID` grammar and `encode`, with an injectivity property test.
3. **Composition and ownership** (before StateMachine lowering, per the planning brief): model
   set, ownership table, final pass, entry probe. Tested with Core modules built directly
   through `Builder`, using stand-in generated actions.
4. **StateMachine resolve**: `ModelInput` → `ResolvedStateMachine`, all resolution errors.
5. **StateMachine lower**: scope protocol, fragment validation, action layout, collision
   mapping.
6. **Identity**: canonical document, ModelHash, receipts, source map, diagnostics.
7. **Python binding**: `StateMachine`, `BehaviorModule(models=…)`, `ModelBuildError`,
   public-api update.
8. **Worked example migration** (FR-036) and README rewrite.
9. **Compatibility gate and determinism**: manifest, gate script and self-test, determinism-check
   extension.

## Test plan, derived from laws

| Law or requirement | Test (location) | Kind |
|---|---|---|
| L1 determinism | lower the same input twice, and in two processes (determinism-check); compare module, BehaviorHash and receipt bytes | property + gate |
| L3 order independence | proptest: random permutations of entry states, source sets, transitions, model set and hand-written trace order → equal ModelHash, module and BehaviorHash (≥ 1,000 cases) | property |
| L4 erasure | the admitted module's name table contains only the declared items plus `encode(m, t)` actions; no other item | golden |
| L5 admissibility | every reference case is admitted by `Builder::finish`, and by `behavior.admit` from Python | integration |
| L6 ownership | hand-written writer, `TRANSITION_WRITES_OWNED_FIELD`, non-entry create, dynamic create, two owners → exact errors; reads and removal accepted | negative |
| L7 identity | equal ModelHash ⇒ equal BehaviorHash across generated equivalent inputs (different syntax, different Python names) | property |
| L8 version immutability | gate self-test: edited expected hash without a version bump → FAIL; with a bump → PASS (SC-009) | script test |
| L9 metadata neutrality | mutate display names, descriptions and source locations → both hashes unchanged | property |
| Transition matrix (SC-001) | for every state × transition of `purchase_request`, invoke through the public engine: commit iff state ∈ F_t and guards hold; otherwise Core's ordinary denial | integration |
| Guards | guard true, guard false, guard `DIVISION_BY_ZERO` → Core outcomes | integration |
| Entry states | literal entry OK, literal non-entry → `ENTRY_STATE_NOT_ALLOWED`, input-derived → `ENTRY_STATE_NOT_STATIC` | negative |
| Name encoding | proptest over `ID × ID`: `encode` injective, decodable, Core-valid; invalid ids → `INVALID_SEMANTIC_ID` | property |
| Collisions | two machines with `submit` OK; same field → `CONFLICTING_OWNERSHIP`; hand-written name clash → `GENERATED_NAME_COLLISION`; each identical under all supply orders | property + negative |
| Fragment validation | non-Bool guard, ill-typed effect, effect on owned field, node from another builder or scope → typed errors | negative |
| Resolution errors | each code in lowering contract §6 has at least one test (SC-008) | negative |
| Diagnostics | the four diagnostics are emitted and the build still succeeds | unit |
| Source map | total and one-to-one; does not affect hashes | unit + property |
| Core consumer | Python builds, admits, invokes and replays a lowered module using only the `behavior` public API | integration |
| US5 RequestFlow | test-only generator → StateMachine with `return`/`resubmit` cycle plus ordinary create; runs end-to-end | integration |
| FR-036 | the `order` reference input lowers to an action whose Core action hash equals the hand-written equivalent | equivalence |
| Self-transitions | pure → `PURE_SELF_TRANSITION`; with an extra effect or command → OK; multi-source including target → OK | negative + unit |

## Anti-pattern audit

| Anti-pattern | Where it could creep in | Prevented by |
|---|---|---|
| Python performs semantic normalization before Rust | `StateMachine.__init__` sorting or deduplicating sets | lowering contract §5; ModelInput passes raw lists; Rust is tested with duplicate and unsorted input |
| Raw Wire JSON as model interface | serializing `Node::into_wire()` to send fragments | shared-builder protocol (research R2); a grep in `check-public-surface.sh` forbidding `into_wire`/`to_wire_json` in `crates/behavior-models` |
| ModelHash based on authoring syntax | hashing lambda source or Python names | fragment identity = Core semantic hash (R5); names from Rust (R3); L7 property test with different syntax |
| Duplicate expression language | a `ModelExpr` type in the crate | the data model has no expression type; fragments are `Node`s |
| Source-order-dependent generated identifiers | index-based names | `encode(m, t)` from ids only; L3 property test |
| Automatic renaming to resolve collisions | suffixing `_2` | `GENERATED_NAME_COLLISION`; the collision test asserts the error |
| Nonexistence as a pseudo-state | `open: → Draft` transitions | no creation construct in the data model; creation only via the composition check |
| Hand-written actions bypassing the owned field | effects not inspected | final pass over the admitted module (composition contract §4–5) |
| Golden fixtures silently rewritten | editing `compat.json` | merge-base comparison in the gate (lowering contract §7) |
| Model runtime logic surviving lowering | model-named items, hooks | L4 erasure test; the Core consumer test invokes the module without using any model object |

## Decision table

| Concern | Decision | Enforcement |
|---|---|---|
| Authoritative lowerer | Rust crate `behavior-models` | crate and API boundary; bindings only implement `FragmentSource`; `check-public-surface.sh` |
| Model input boundary | raw `ModelInput`, no binding semantics | lowering contract §5; tests with unsorted and duplicate input |
| State domain | `Q := Cases(T_f)`, never stored | by construction (no field in `ResolvedStateMachine`) |
| Entry states | literal cases ⊆ Q, non-empty | resolution errors; composition pass |
| Transition identity | `(machine_id, transition_id)`, both ∈ `ID` | `INVALID_SEMANTIC_ID` |
| Transition capability | `encode(m, t) = m__t`, proven injective | encoder plus property test |
| Creation | external to StateMachine | composition pass: Core-hash equality with entry literals |
| Open expressions | typed Core fragments in a Rust-opened scope | Core checks (layer 1) plus model checks (layer 2) |
| Fragment names | model-semantic parameter names, positional binding | data-model §4; L7 test with renamed lambda parameters |
| Owned field writes | owner's generated actions only | final composition pass over the admitted module |
| Collisions | explicit typed errors, never renaming | composition contract §2–3 |
| ModelHash | semantic content, Core fragment hashes | `tagged_hash("behavior.model.v1", canonical document)` |
| Metadata and source maps | provenance only | L9 test; produced after finish |
| Version reproducibility | `(K, s, l, c, ModelHash) → BehaviorHash` immutable | `models compat` gate with merge-base check |
| Diagnostics | warnings only, never verification | separate type; build success asserted |
| Worked example | not executable lowering any more | `lower.py` removed; equivalence test |
| Core changes | none needed | planning stop-gate (lowering contract §8); research R1 |

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A future Core Release changes builder signatures or hash layout | `core_release` in the version tuple rotates the manifest. Lowering is re-validated by the gate on every pin update. |
| The probe's global-invariant name could clash with a user item | the probe is a separate `Builder::for_module` instance that is never emitted, named `__probe` (valid in Core, impossible as a model id) |
| `add_action_with_commands` re-admits the module on every call (cost) | acceptable for build-time sizes; watched in the gate run |
| Python lambdas with keyword-only parameters or `*args` | the binding rejects them early; Rust is unaffected |

## Complexity Tracking

No constitution violations. Nothing to justify.
