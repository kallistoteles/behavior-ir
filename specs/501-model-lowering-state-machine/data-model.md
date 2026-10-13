# Data Model: Deterministic Model Lowering and StateMachine Model

**Feature**: 501-model-lowering-state-machine | **Date**: 2026-10-12

This document fixes every semantic decision about the model's data. Implementers choose
types, module layout and function names, but no semantics. All types live in the Rust crate
`crates/behavior-models`. Binding types are listed only to state what they may *not* decide.

Notation: `E` is the subject entity type, `f` the owned field, `T_f` its enum type (`WType::Enum`),
`Q := Cases(T_f)`, `ID` the semantic-id grammar of research R4.

## 1. Layers

```text
Python syntax ──► ModelInput (binding-built, no authority)
                       │  resolve (Rust)
                       ▼
                 ResolvedStateMachine (canonical, validated)
                       │  lower (Rust, into the shared Core Builder)
                       ▼
                 admitted semantic::Module ──► composition pass (Rust)
                       │
                       ▼
                 BuildOutput { module, receipts, diagnostics }
```

## 2. ModelInput: what a binding may construct

A plain-data description with no defaults applied, no sorting and no validation authority.

| Field | Type | Binding may | Binding may NOT |
|---|---|---|---|
| `model_id` | string | pass through the author's string | normalize case, invent, default |
| `subject_entity` | string (entity name) | pass the declared entity's Core name | resolve aliases |
| `state_field` | string (field name) | pass through | |
| `subject_param` | optional string | pass through if the author gave one | compute the default |
| `entry_states` | list of strings (case names) | pass in any order, duplicates included | sort, deduplicate |
| `transitions` | list of `TransitionInput` | pass in any order | sort, deduplicate, rename |
| `display` | optional metadata (§7) | pass through | |
| `source` | `SourceLoc` per construct | provide file and line | |

`TransitionInput`:

| Field | Type | Notes |
|---|---|---|
| `transition_id` | string | Validated against `ID` by Rust. |
| `from_states` | list of strings | Any order, duplicates allowed in input. |
| `to_state` | string | |
| `inputs` | list of `(name, type)` in any order | A finite mapping `ParameterId → Type`. Order is never semantic; Rust canonicalizes it (§4). |
| `context` | list of `(name, type)` in any order | Same as `inputs`. |
| `fragments` | an opaque handle the binding implements (`FragmentSource`, §5) | Rust calls it; Rust never reads binding objects. |
| `display`, `source` | metadata | |

Binding-side checks (for example "states are enum members") are allowed only as early, friendly
errors. The model crate re-checks everything and is the only authority.

## 3. ResolvedStateMachine: the authoritative representation

Produced by `resolve(input, decls)`. The `decls` come from the shared Builder's declared
entities and enums, which the binding has completed before resolution.

```text
ResolvedStateMachine {
  model_kind:        "state_machine"               // constant
  semantic_version:  1                             // MODEL_SEMANTIC_VERSION
  model_id:          Id                            // ∈ ID
  subject:           EntityName                    // E
  field:             FieldName                     // f, of type Enum(T_f)
  enum_type:         EnumName                      // T_f (Q is NOT stored)
  subject_param:     ParamName                     // input value or snake_case(E)
  entry_states:      SortedSet<Case>               // ⊆ Q, non-empty
  transitions:       SortedMap<Id, ResolvedTransition>
}

ResolvedTransition {
  transition_id:  Id                               // local to the machine
  from_states:    SortedSet<Case>                  // ⊆ Q, non-empty
  to_state:       Case                             // ∈ Q
  params:         [Param]                          // see §4
  action_name:    String                           // encode(model_id, transition_id)
}
```

- **State domain**: `Q` is never a field. Every membership check reads the enum's declared
  cases from the module's declarations. There is no state list to keep in step with the enum.
- **Sorting**: `SortedSet` and `SortedMap` use Unicode code-point order of the string. This is
  the only canonical order the crate uses.
- **Creation**: there is no creation construct, no nonexistence state, and no removal
  transition. Entry states are a composition rule (contract `composition-and-ownership.md`).

### Resolution rules (all blocking; error codes in the lowering contract §6)

| Rule | Error |
|---|---|
| `model_id`, every `transition_id` ∈ `ID` | `INVALID_SEMANTIC_ID` |
| `subject_entity` is a declared entity | `UNKNOWN_ENTITY` |
| `state_field` is a field of it, typed `Enum(T)` (not `Option`, not nominal) | `STATE_FIELD_NOT_ENUM` |
| every entry, source and target case ∈ `Cases(T_f)` | `UNKNOWN_STATE` |
| `entry_states` non-empty | `EMPTY_ENTRY_STATES` |
| `from_states` non-empty for each transition | `EMPTY_SOURCE_STATES` |
| `transition_id` unique within the machine | `DUPLICATE_TRANSITION_ID` |
| `subject_param`, input and context names ∈ Core identifier grammar and pairwise distinct | `INVALID_PARAMETER` |
| duplicate cases in input sets | deduplicated silently (set semantics, not an error) |

Pure self-transition (`from_states = {q} = {to_state}`) is checked **after** fragments are
traced. It is `PURE_SELF_TRANSITION` iff `E_t` and `K_t` are both empty (guards and
postconditions alone do not make a self-transition meaningful). A self-transition is
`q ∈ F_t ∧ q_t = q` only when `F_t = {q}`. A multi-source transition that includes its target
is legal: for those sources it is a self-loop, for the others a real transition.

## 4. Parameters and the binding environment

For transition `t`, the generated action's parameter list, in this order:

1. `(subject_param, Role::State, Entity(E))`
2. each declared input `(name, Role::Input, type)`, sorted by name (code point)
3. each declared context `(name, Role::Context, type)`, sorted by name (code point)

Parameters are canonicalized by semantic parameter identity (the name in v1). Declaration order never matters. Core invokes by name (`RequestedInvocation` bindings, input and context objects), so this order is only a derived ABI for Core's action hash. These names are **model-semantic** and enter ModelHash (contract `identity-and-source-map.md`).
Fragments are traced with these names, bound by position. Binding-side names (for example a
Python lambda's `request`, `ctx`) never reach Rust.

## 5. Fragment protocol (FragmentSource)

The Rust crate defines a callback interface that a binding implements:

```text
trait FragmentSource {
    // Called by Rust with the shared Builder whose action scope for `t` Rust has
    // already pushed, and the parameter names in §4 order. Returns Core nodes built
    // in that scope.
    fn trace(&mut self, builder: &mut Builder, scope: &TransitionScope)
        -> Result<TransitionFragments, BindingError>;
}

TransitionFragments {
    guards:         [(Node, Loc)]            // G_t, author order
    effects:        [(param, field, Node, Loc)]  // E_t, author order
    postconditions: [(Node, Loc)]            // P_t, author order
    commands:       [CommandEmissionSpec]    // K_t, author order
}
```

Rust pushes and pops the scope, calls `trace` once per transition in canonical
`transition_id` order, validates the result (§6) and adds the action. A binding cannot add the
generated action itself.

Fragment lists are **sequences**: their order is the author's, is kept, and enters ModelHash.
Reordering guards is a semantic change in Core, because Core reports the first failing
precondition.

## 6. Two-layer fragment validation

| Layer | Check | Error |
|---|---|---|
| Core | node created in this builder and scope (`BUILDER_MISMATCH`, `SCOPE_MISMATCH` from Core) | `FRAGMENT_INVALID { core_code }` |
| Core | guards and postconditions are `Bool` (`check_condition`) | `FRAGMENT_INVALID { core_code: NOT_BOOLEAN }` |
| Core | each effect is well-typed (`check_effect`) | `FRAGMENT_INVALID { core_code }` |
| Core | commands admissible (`add_action_with_commands` re-admission) | `FRAGMENT_INVALID { core_code }` |
| Model | no effect targets `(subject_param, f)`, nor any parameter of type `Entity(E)` with field `f` | `TRANSITION_WRITES_OWNED_FIELD` |
| Model | effects target only parameters of this action (Core already enforces this) | via Core |

## 7. Metadata (never semantic)

```text
DisplayMeta { display_name: Option<String>, description: Option<String> }
SourceLoc   { file: String, line: u64 }
```

These exist on the machine and on each transition. They feed the source map and diagnostics
only. They are excluded from ModelHash, and the lowerer passes `Loc` to Core only where Core
already excludes it from hashes.

## 8. Composition entities

```text
ModelSet        = SortedMap<ModelId, ResolvedModel>      // duplicate id → error
OwnershipClaim  = { entity: E, field: f, model_id }
OwnershipTable  = SortedMap<(E, f), model_id>            // conflict → error
GeneratedAction = { action_name, model_id, transition_id }
```

## 9. Build outputs

```text
LoweringReceipt {
  format:                  "behavior.model_receipt.v1"
  model_hash:              "sha256:…"
  model_kind:              "state_machine"
  model_semantic_version:  1
  lowerer_version:         1
  core_release:            { version: "0.12.0", commit: "b525da4…" }
  behavior_hash:           "sha256:…"      // of the whole composed module
  model:                   CanonicalModelDocument   // the hashed document: ModelHash is re-derivable
  source_map:              SourceMap
}

SourceMap {
  entries: Sorted [ { core: CoreRef, model: ModelRef, display?: DisplayMeta, source?: SourceLoc } ]
}
CoreRef  = { kind: "action", name, item_hash } | { kind: "field", entity, field }
ModelRef = "<model_id>/transition/<transition_id>" | "<model_id>/state_field"
```

`Diagnostic { code, severity: "warning", model_ref, message }` covers:
`UNREACHABLE_STATE`, `NO_OUTGOING_TRANSITIONS`, `NO_INCOMING_TRANSITIONS`, `UNUSED_STATE`,
`UNREACHABLE_TRANSITION`.
These never block a build and never claim verification.

## 10. Versions

| Constant | Initial | Changes when |
|---|---|---|
| `MODEL_SEMANTIC_VERSION` (StateMachine) | 1 | The meaning or admitted structure of the model language changes (for example a new transition rule, or a change to what may be declared). |
| `LOWERER_VERSION` (StateMachine) | 1 | The same model contract intentionally lowers to different Core IR (layout, names, generated postconditions). |
| `CORE_RELEASE` | from `core-release.json` | Rotated by a Core Release update. |
