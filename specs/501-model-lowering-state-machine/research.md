# Research: Deterministic Model Lowering and StateMachine Model

**Feature**: 501-model-lowering-state-machine | **Date**: 2026-10-12

All findings are against the pinned Core Release `v0.12.0` (commit
`b525da46d4282166418a90141f0c94fd099e9b47`, `core-release.json`). The source was read in the
cargo checkout of that commit; nothing in behavior-ir-core was changed or checked out. Paths
below are relative to `crates/behavior-core/src/` of that commit unless stated otherwise.
Everything named here is reachable through the public facade `behavior_engine` (its `lib.rs`
re-exports, listed in core `api/engine-surface.txt`).

## R1. Public facade inventory (FR-035 check 1)

Decision: everything StateMachine needs to **produce** and to **inspect** is reachable through
the public `behavior_engine` facade. No gap requires a Core change.

| Need | Public API (path in `behavior_engine`) | Notes |
|---|---|---|
| Build a module from declarations, hand-written actions and generated actions | `builder::Builder` (`new`, `with_semantic_profile`, `declare_enum`, `declare_entity`, `add_action`, `add_action_with_commands`, `finish`) | `finish` admits through `admit_wire`, the same path as JSON decoding (`builder.rs:1205`). |
| Typed expression nodes for fragments | `builder::Node`, created by `Builder::{param, field, lit, op, in_, select, lambda, …}` | Every node is type-checked by Core's `check_expr` when it is created (`builder.rs:640`). |
| Action parameter scopes (state, input, context) | `Builder::push_scope(ScopeSite::Action, Vec<wire::WParam>, Loc)` / `pop_scope`; `wire::{WParam, Role, WType}` | `Role = State \| Input \| Context` (`wire.rs:58`). |
| Generated state requirement `state ∈ F_t` | `Builder::in_(node, values, loc)` | `In` hashes its values **in the given order** (`admit/hash.rs:242`), so the lowerer must canonicalize the order (R6). |
| Generated `set(state, q_t)` | the `effects` argument of `add_action`: `(param, field, Node, Loc)` | Checked by `Builder::check_effect` (`builder.rs:905`). |
| Postconditions | the `postconditions` argument of `add_action` | |
| Command intents | `builder::CommandEmissionSpec` with `add_action_with_commands` (requires `SemanticProfile::CommandIntents`) | Every action is re-admitted when it is added (`builder.rs:224`). |
| Inspect the final module | `semantic::Module` → `actions()`, `entities()`, `enums()`, `name_table()`, `hash()`, `behavior_version()` | The admitted module is the complete set of ordinary writes (R7). |
| Inspect actions | values returned by `Module::actions()`: `params()` (name, `role()`, `ty().to_wire_type()`), `preconditions()`, `effects()` (`param()`, `field()`, `value()`, `hash()`), `creates()` (`entity()`, `fields()`), `removes()`, `command_emissions()` (`hash()`), `postconditions()`, `hash()` | These types are not named in the facade, but their public methods can be called on the values (methods, not variant matching). |
| Canonical semantic identity of fragments | `hash()` on semantic `Expr`, `Effect`, `CommandEmission`, `ActionItem`; `Hash = [u8; 32]`; `semantic::types::hash_display` | `admit/hash.rs:1-4`: "semantic content only: no source locations, no names for humans, no JSON". |
| BehaviorHash | `semantic::Module::hash()` / `behavior_version()` (`sha256:…`) | A sorted name table of `(kind, name) → item hash` (`admit/hash.rs:644`). |
| Canonical hashing for an ecosystem document | `canonical::{to_canonical_string, tagged_hash}` | `SHA-256(UTF8(tag) ‖ 0 ‖ canonical JSON)`, floats rejected (`canonical.rs:105-130`). |
| Name grammar | `Builder` rejects an invalid name with `DUPLICATE_NAME` | Grammar `[A-Za-z_][A-Za-z0-9_]*` (`wire::is_identifier`, `wire.rs:509`). |
| Throwaway probe over a finished module | `Builder::for_module(&Module)` | Used for the entry-state check (R8). |

Gap classification:

| Gap found | Class | Resolution |
|---|---|---|
| G1: no public way to *match* on the kind of a semantic `Expr` (for example "is this an enum literal?"). `ExprKind` is not re-exported. | Facade exposure (not semantic) | **Not needed.** The entry-state check uses hash equality against Core-built literals (R8). No Core change. |
| G2: `Node` does not expose a semantic hash before admission. | Facade exposure (not semantic) | **Not needed.** Fragment identities are read from the admitted module, at positions the lowerer controls (R5). |
| — | Semantic capability missing | **None.** Every StateMachine construct lowers to existing Core constructs. |

Rejected workarounds (anti-patterns): inspecting `serialize::to_wire_json` output to classify
expressions, which would make raw Wire JSON a model interface; computing literal hashes by
re-implementing Core's encoder; and Python-side classification of initial values.

## R2. Fragment transport: why fragments cannot leave their builder

Finding: a `Node` carries the identity of the `Builder` that created it, plus the scope
bindings it captured. `check_node` rejects nodes from another builder (`BUILDER_MISMATCH`) or
from another binding of the same name (`SCOPE_MISMATCH`) (`builder.rs:589-612`).

Decision: **one Core `Builder` per module, shared by the binding and the model crate.** The
model crate never receives detached fragments. It receives `&mut Builder`, opens each
transition's scope itself, and calls back into the binding to trace the author's fragments
inside that scope (data-model §5, lowering contract §4). The Python binding already uses
exactly one builder per `BehaviorModule` (`python/behavior/module.py:27`), and `PyBuilder`
wraps `behavior_engine::builder::Builder` (`crates/behavior-py/src/lib.rs:1001`).

Rationale: this uses Core's own construction checks unchanged and needs no expression
transport format. Raw Wire JSON, a model expression language and lambda capture are all
avoided.

Alternatives rejected: (a) serializing `Node::into_wire()`, which is wire syntax that includes
locations: raw Wire JSON as model interface. (b) A model expression AST translated later, which
would be a second expression language. (c) Binding-side closures stored and evaluated later,
which would make a binding object model semantics.

## R3. Binding environment of fragments

Finding: expression hashes include parameter names (`ExprKind::Field { param, field }` and
`Param(p)` are hashed by name, `admit/hash.rs:210-216`). Action hashes include
`(name, role, type)` of every parameter (`admit/hash.rs:436`).

Decision: every name a fragment can refer to is a **model-semantic, Rust-resolved name**:
- the subject parameter name: the machine's `subject_param`, defaulting to
  `snake_case(E)`, computed by Rust;
- input parameter names: declared per transition;
- context parameter names: declared per transition.

Inputs and context are mappings `ParameterId → Type`. Rust orders them by name (code point), not by declaration order, because Core invokes by name and order would only be an ABI artifact. The binding receives these names from Rust and binds the author's syntax to them by
**position** (subject first, then inputs, then context, in the order Rust returns). The
author's Python parameter names, lambda identity, object identity and source text never reach
Rust. Two lambdas `lambda r, c: c.actor == r.approver` and `lambda req, ctx: ctx.actor ==
req.approver` therefore trace to the same Core nodes and the same hashes.

Rationale: a fragment's Core identity then depends only on model semantics. Because the names
are part of the model's semantic version (not the lowerer's), ModelHash stays independent of
the lowerer version.

## R4. Action-name grammar and injective encoding (FR-013)

Finding: Core action names must match `[A-Za-z_][A-Za-z0-9_]*` (`wire.rs:509`), and duplicate
names are rejected with `DUPLICATE_NAME` (`builder.rs:937`).

Decision: semantic ids (machine ids and transition ids) use the **semantic-id grammar**
`ID = [a-z][a-z0-9]*(_[a-z0-9]+)*`: lowercase, starting with a letter, single underscores only
between alphanumeric runs, no leading, trailing or doubled underscore. The action name is
`encode(m, t) = m ‖ "__" ‖ t`.

Proof of injectivity. For `m, t ∈ ID`, neither contains `"__"`, `m` does not end with `_` and
`t` does not start with `_`. So `encode(m, t)` contains exactly one occurrence of `"__"`: the
separator. An occurrence starting inside `m` would require `"__"` in `m` or `m` ending in `_`;
one starting inside `t` would require `"__"` in `t`; an overlap `"___"` would require `m` to end
or `t` to start with `_`. Decoding by splitting at the unique `"__"` recovers `(m, t)`, so
`encode` is injective. Every result matches Core's grammar, because it starts with a lowercase
letter and contains only `[a-z0-9_]`.

No escaping is needed, because ids outside `ID` are rejected at resolution with
`INVALID_SEMANTIC_ID`. A hand-written action can still take a generated name; that is
reported as a collision, never renamed (R9).

## R5. ModelHash: canonical encoding and fragment identity (FR-008, FR-035 check 4)

Decision: ModelHash needs an ecosystem-only canonical **document schema** but no new hashing
algorithm:

```text
ModelHash(M) = tagged_hash("behavior.model.v1", CanonicalModelDocument(M))
```

This uses Core's public `canonical::tagged_hash` (canonical JSON with sorted keys, no floats).
The document (identity contract §2) contains only semantic facts. Unordered collections are
sorted arrays. Each Core fragment is represented by its Core semantic hash in `sha256:` display
form.

Fragment identity: after lowering and admission, the lowerer reads each fragment's semantic
hash from the admitted generated action, at positions fixed by the lowering contract:
- guards: `preconditions()[1..]`;
- postconditions: `postconditions()[1..]`;
- extra effects: `effects()` other than the owned-field effect;
- commands: `command_emissions()`.

Core computes these hashes from semantic content only. Two authoring syntaxes that produce the
same Core semantics therefore produce the same ModelHash (SC-003).

Consequence: ModelHash is defined for admitted models (spec FR-008: "each admitted resolved
model"). Its definition depends only on model-semantic facts and Core semantic identities, not
on the lowerer version, because R3 fixes all fragment-visible names in the model semantics.

Domain separation: the tag names the hashed object. The model kind and the model semantic
version are fields inside the document, so documents of different kinds or versions can never
collide.

## R6. Canonical ordering inside the generated action

Finding: action hashes keep the **order** of preconditions, effects and postconditions
(`admit/hash.rs:436-448`), and `In` keeps value order (`admit/hash.rs:242`). The module hash is
order-independent (sorted name table).

Decision: the lowerer emits a fixed layout (lowering contract §3). Source states inside
`In(subject.f, …)` are sorted by Unicode code point of the case name. Author fragments keep
their author-declared order **within** each list, because the conjunction order of
preconditions affects which failure Core reports (evaluation semantics). Unordered model
collections are entry states, transitions, source-state sets and the set of models. Fragment
lists are ordered: they are sequences in Core action semantics, and the spec's unordered list
(FR-027) does not include them.

## R7. Ownership can be enforced completely (FR-017, FR-035 check 2)

Finding: in a module, the only ordinary operations that can write an entity field are action
effects (`ActionItem::effects()`) and creations (`ActionItem::creates()`). Derived values,
invariants, constraints, reads and command emissions cannot write. Removal
(`ActionItem::removes()`) removes an entity and does not write a field. The admitted
`semantic::Module` lists every action by name.

Decision: the final composition pass runs over the **admitted** module (not over binding
data), so it sees every write regardless of how it was authored:
- for each action `a`, each effect `(p, f)`: resolve `p` in `a.params()` to its type; if
  `ty().to_wire_type() == WType::Entity(E)` and `(E, f)` is owned by machine `m`, then `a`
  must be one of `m`'s generated actions, otherwise `UNAUTHORIZED_OWNED_FIELD_WRITE`;
- for each creation `c` with `c.entity() == E` and `(E, f)` owned: entry-state check (R8).

Migrations (feature 009) are separate artifacts admitted by `migration::admit_migration`.
They are not part of a module and fall outside composition, in line with spec §"historical
entities".

## R8. Static entry-state check without inspecting expression kinds (FR-021)

Decision: the initial value expression `x` of the owned field in a creation is accepted iff
`x.hash() ∈ { hash(lit(T_f, q)) | q ∈ Q_entry }`. Literal hashes are obtained from Core
itself. The lowerer runs `Builder::for_module(&composed)`, opens a closed scope, builds
`lit(Enum(T_f), q)` for each entry state, and wraps them in a throwaway global invariant
`op("==", [lit, lit])`. It then finishes the probe and reads
`global_invariants()[probe].body().children()[0].hash()`. The probe module is discarded and
never emitted.

Semantics: hash equality ⇔ the same typed literal (Core hashes are injective in practice over
semantic content, with type included). Any expression-derived value, such as an input, a
derived reference or a computation, is rejected with `ENTRY_STATE_NOT_STATIC`, even when it
happens to always evaluate to an entry state. v1 does not prove ranges. This is the
conservative reading of FR-021 ("rejected unless every possible value is shown to be an entry
state").

Rationale: only public construction and inspection APIs are used, and Core decides what a
literal of the enum is.

**Probe correction (verified in implementation, `crates/behavior-models/tests/core_assumptions.rs`):** `Builder::for_module` cannot be used, because its `finish` admits only the builder's own declarations, so the enum is unknown (`UNKNOWN_TYPE`). The probe is a fresh `Builder::new()` that re-declares `T_f` with `EnumInfo.values` read from `module.enums()`. It asserts that the probe's enum hash equals the module's (Core hashes enum literals through the enum declaration hash), then reads the literal hashes. Order preservation in admitted actions (analysis U2) is verified in the same test file.

## R9. Collisions and determinism of composition (FR-014, FR-015)

Decisions:
- Models are supplied as a set and sorted by `model_id`. A duplicate `model_id` is
  `DUPLICATE_MODEL_ID`.
- Ownership claims are collected before anything is lowered. Two claims on the same `(E, f)`
  give `CONFLICTING_OWNERSHIP { field, models: sorted }`.
- Generated actions are added in canonical order `(model_id, transition_id)`. If Core rejects a
  generated name with `DUPLICATE_NAME`, the error is `GENERATED_NAME_COLLISION { name,
  model_id, transition_id }`. All errors are collected, sorted and reported together; nothing
  is renamed.
- Protocol order: the binding traces all ordinary declarations and hand-written actions
  **before** calling `compose`. Collision errors are therefore always reported by the model
  crate, independent of authoring order.
- StateMachine v1 contributes **only actions**. It never declares entities, enums or fields,
  so enum and field contribution conflicts cannot arise for this model kind. The composition
  layer still defines the rule for future kinds: identical-or-error by Core item hash.

## R10. Version contract and fixture enforcement (FR-001, FR-001a)

Decision: the version tuple is `(model_kind, model_semantic_version, lowerer_version,
core_release)`. `core_release` is `{version, commit}` from `core-release.json`, embedded in the
crate at build time. The compatibility manifest `models/fixtures/compat.json` maps each
reference case to `ModelHash → BehaviorHash` per tuple.

Gate `models compat` (added to `scripts/gates.sh`) does two things:
1. recomputes every case under the current tuple and requires equality;
2. compares the manifest with its version at the merge base (`git merge-base HEAD main`). An
   entry for an *existing* tuple whose BehaviorHash or ModelHash changed fails the gate, even
   when the manifest was edited. A changed mapping is valid only under a new tuple, created by
   a version bump in the same change.

Deleting an established tuple's entries also fails, unless the tuple's `core_release` is no
longer the pinned one (a Core Release update rotates `core_release`, creating a new tuple).

## R11. Repository integration findings

- `specs/011-core-ecosystem-split/contracts/ownership.md` rule 3 assigns `crates/**` to
  **core**. A new ecosystem row `crates/behavior-models/**` must be added before rule 3, or the
  ownership and public-surface gates fail. This is an ecosystem-repository change.
- `scripts/check-public-surface.sh` checks only `crates/behavior-py/Cargo.toml`. It must also
  check `crates/behavior-models/Cargo.toml`, whose only Core dependency must be
  `behavior-engine`.
- `api/public-api.json` lists the Python package's public names (`test_public_api.py`). New
  names (`StateMachine`, `ModelBuildError`, `LoweringReceipt`) must be added.
- `pytest` already runs `python/tests` and `models` (`pyproject.toml:28`); `models/` is
  ecosystem-owned (ownership rule 18 and rule 25).

## R12. Guard evaluation error for tests (FR-025)

Decision: tests use integer division by zero inside a guard, which yields Core's ordinary
`DIVISION_BY_ZERO` evaluation error (`eval.rs:1194`). No special construct is introduced.

## R13. Existing worked example (FR-036)

Decision: `models/examples/state_machine/lower.py` stops being executable lowering logic. It is
replaced by a reference **input** (the same `Order.status` machine authored with the new API)
plus an equivalence test. The test asserts that the Rust-lowered action admits to the same
Behavior IR, by action hash, as the hand-written three-statement action documented in
`models/README.md`, apart from the deliberate name change to `order__submit`.
`models/README.md` is rewritten to point at the Rust crate as the single authority.

## R14. Testing stack

- Rust: `cargo test` in `crates/behavior-models`, `proptest` for permutation and property laws
  (constitution III recommends it). `proptest` is a dev-dependency only, with a fixed seed
  configuration so the gate is deterministic.
- Python: pytest end-to-end through `behavior` (binding, Core consumer test), already in the
  gates.
- Determinism: model receipts for the reference cases are added to
  `scripts/determinism-check.sh` (run twice, byte-compare).
