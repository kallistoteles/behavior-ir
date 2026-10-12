# Contract: Model Lowering

**Feature**: 501 | Applies to every model kind. StateMachine is the first instance.

## 1. The lowering function

For model kind `K`, model semantic version `s`, lowerer version `l` and Core Release `c`:

```text
L_{K,s,l,c} : (ModuleDecls, ModelSet_K) → Result<Module_c, [ModelError]>
```

- `ModuleDecls`: the ordinary declarations and hand-written actions already traced into the
  shared Core `Builder` (research R2).
- `ModelSet_K`: a *set* of resolved models (data-model §3), keyed by `model_id`.
- `Module_c`: an ordinary admitted `behavior_engine::semantic::Module` of Core Release `c`.
- Errors are a sorted, deduplicated list. There is never a partial module.

Meaning is Core's: `Meaning(M) := Meaning_Core(L(M))`. No model evaluator exists.

## 2. Laws (each one is a test obligation; see the plan's test matrix)

| # | Law | Statement |
|---|---|---|
| L1 | Determinism | `M₁ = M₂ ⇒ L(M₁) = L(M₂)`, including `BehaviorHash`. |
| L2 | Purity | `L` reads only its arguments and the compiled constants `(s, l, c)`. No clock, randomness, environment, filesystem enumeration, network, process identity or hash-map iteration. |
| L3 | Order independence | Permuting entry states, source-state sets, transitions or the model set, or interleaving the tracing order of hand-written actions, leaves ModelHash, the module and BehaviorHash unchanged. |
| L4 | Erasure | The admitted module contains only Core items. No name, item or value refers to a model; executing it needs no model object. |
| L5 | Core admissibility | `L` returns `Ok` only if Core admitted the module through `Builder::finish`. |
| L6 | Ownership preservation | In `Ok(B)`, every write to an owned `(E, f)` is in one of the owning machine's generated actions, and every creation of `E` initializes `f` to a literal entry state. |
| L7 | Reproducible identity | `ModelHash(M₁) = ModelHash(M₂) ⇒ BehaviorHash(L(M₁)) = BehaviorHash(L(M₂))` for fixed `(K, s, l, c)` and identical ordinary behavior. |
| L8 | Version immutability | For a fixed `(K, s, l, c, ModelHash)`, the BehaviorHash recorded in the compatibility manifest never changes (§7). |
| L9 | Metadata neutrality | Changing only display metadata, source locations or the source map changes neither ModelHash nor BehaviorHash. |

## 3. Generated action layout (StateMachine, lowerer version 1)

For machine `m = (E, f, Q_entry, 𝒯)` and transition `t = (F_t, q_t, G_t, E_t, P_t, K_t)` with
subject parameter `s`:

```text
action  encode(m.id, t.id)
params  (s: state E) ++ inputs(t) ++ context(t)             // data-model §4
pre     [ In(s.f, sort(F_t)) ] ++ G_t                        // generated first, then author order
effects [ (s, f, lit(T_f, q_t)) ] ++ E_t                     // exactly one write of f
post    [ s.f == lit(T_f, q_t) ] ++ P_t
commands K_t                                                 // CommandIntents profile only
```

- `sort` is Unicode code-point order of case names (research R6).
- When `|F_t| = 1` the generated precondition is still `In(s.f, [q])`, not `==`. One form,
  so there is no special case in the lowered IR.
- The module's semantic profile is the binding's choice. With `K_t ≠ ∅` it must be
  `CommandIntents`, otherwise `FRAGMENT_INVALID { core_code: UNSUPPORTED_IR_VERSION }`.

## 4. Build protocol (one Core Builder per module)

```text
1. binding: declare enums, entities, commands; trace all hand-written actions, derived values,
            invariants and reads into Builder B
2. rust:    resolve every ModelInput against B's declarations  → ModelSet or errors
3. rust:    ownership table and set-level checks (contract composition-and-ownership §2)
4. rust:    for each (model_id, transition_id) in canonical order:
              push_scope(Action, params) ; fragments = source.trace(B, scope)
              validate fragments (data-model §6) ; add_action[_with_commands] ; pop_scope
5. rust:    module = B.finish(root)                            // Core admission
6. rust:    final composition pass over `module` (ownership, entry states)
7. rust:    fragment identities → ModelHash per model; receipts; diagnostics
```

Steps 2–7 are one Rust entry point (`compose`). A binding cannot skip or reorder them.
Errors from steps 2–4 are collected over all models and transitions before returning. Step 5
runs only if steps 2–4 produced no errors.

## 5. Binding obligations and prohibitions

A binding:
- constructs `ModelInput` and implements `FragmentSource` (data-model §2, §5);
- traces fragments only inside the scope Rust opened, using the names Rust supplied, bound by
  position;
- may raise early, friendly errors, which are not authoritative.

A binding must not:
- sort, deduplicate, default, rename or validate model semantics;
- compute ModelHash, generated names or ownership;
- add generated actions or call Core admission for models itself;
- pass lambdas, binding objects, source text or Wire JSON as model semantics.

## 6. Error catalogue (typed; `code` is stable API)

| Code | Stage | Payload |
|---|---|---|
| `INVALID_SEMANTIC_ID` | resolve | id, model_ref |
| `UNKNOWN_ENTITY`, `STATE_FIELD_NOT_ENUM`, `UNKNOWN_STATE` | resolve | names |
| `EMPTY_ENTRY_STATES`, `EMPTY_SOURCE_STATES`, `DUPLICATE_TRANSITION_ID` | resolve | model_ref |
| `INVALID_PARAMETER` | resolve | name |
| `DUPLICATE_MODEL_ID` | set | model_id |
| `CONFLICTING_OWNERSHIP` | set | entity, field, sorted model_ids |
| `FRAGMENT_INVALID` | lower | model_ref, core_code, core_message |
| `TRANSITION_WRITES_OWNED_FIELD` | lower | model_ref |
| `PURE_SELF_TRANSITION` | lower | model_ref |
| `GENERATED_NAME_COLLISION` | lower | action name, model_ref |
| `CORE_ADMISSION_FAILED` | finish | Core admission errors (verbatim) |
| `UNAUTHORIZED_OWNED_FIELD_WRITE` | compose | action, entity, field, owner model_id |
| `ENTRY_STATE_NOT_STATIC` | compose | action, entity, field |
| `ENTRY_STATE_NOT_ALLOWED` | compose | action, entity, field, state |

Sort key: `(stage order, code, model_ref or action, detail)`. Equal inputs give an identical
error list.

## 7. Versioning contract and compatibility gate

- `(K, s, l, c, ModelHash) → BehaviorHash` is immutable. Version identifiers identify
  semantic functions, not source-code releases.
- Bump `s` when the meaning or admitted structure of the model language changes. Bump `l` when
  the same model contract lowers to different Core IR. `c` rotates with the Core Release pin.
- `models/fixtures/compat.json` (format `behavior.model_compat.v1`):

  ```text
  { tuples: [ { model_kind, model_semantic_version, lowerer_version, core_release,
                cases: { <case>: { model_hash, behavior_hash } } } ] }
  ```

- Gate `models compat` (in `scripts/gates.sh`):
  1. every case of the **current** tuple recomputes to the recorded hashes;
  2. against `git merge-base HEAD main`: every case of every tuple present there is
     byte-identical here, so changed or removed entries of an established tuple fail. The one
     exception is a tuple whose `core_release` is no longer pinned, which may be removed.
  3. the current tuple must contain every reference case.

  Editing an expected hash therefore never passes without a new tuple, and a new tuple needs a
  version change in code.

## 8. No-Core-change stop gate

If implementation finds a construct that cannot be lowered with the APIs in research R1,
work stops and a gap report is filed. The report states: the StateMachine requirement, the
missing operation, its class (facade exposure versus semantic capability), and why existing
Core constructs cannot express it. The maintainer then chooses a Core feature branch.
Inconvenience is not evidence of a missing primitive. At planning time, no such gap exists
(research R1).
