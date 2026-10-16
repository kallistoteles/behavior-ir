# Contract: Declaration references in model identity (feature 505)

Applies to every model kind: StateMachine, RequestFlow and Approval.

## 1. Rule

```text
ModelIdentity = tagged_hash("behavior.model.v1", Canonical(ModelDefinition, ReferencedDeclarations))
```

A canonical model document records each type it references as the type's name together with
the declaration identity Core assigns to it in the admitted module. A model's identity depends on
exactly the declarations it references and on nothing else in the module.

## 2. What counts as a reference

| Reference | Recorded as |
|---|---|
| A subject, actor or state-binding entity | `{entity, entity_hash}` (plus `name` for a state binding) |
| An enum, nominal or entity mentioned by a parameter or payload type, including inside `Option` and in `Exact(Some(n))`, `Id`, `Ref` and `Entity` | an entry `name → hash` in that parameter's `types` |
| A state field's enum (StateMachine, RequestFlow status) | `enum_hash`, unchanged |

Built-in types (`Bool`, `Int`, `Decimal`, `String`, `Exact(None)`) are not references. A
parameter with only built-in types has `types: {}`.

## 3. Resolution

1. The `(kind, name)` pair is looked up in the admitted module's name table. The kind is `enum`
   for `Enum`, `nominal` for `Nominal` and `Exact(Some)`, and `entity` for `Id`, `Ref` and
   `Entity`.
2. Exactly one declaration resolves the pair. If the name is missing, or is declared only under
   another kind, the build fails with `MODEL_IDENTITY_FAILED`. The message names the model, the
   referring item (subject, actor, state binding, or parameter or field name) and the type.
3. The identity is Core's, rendered with `hash_display`. No model computes or copies a
   declaration's content.

## 4. Laws

| Law | Statement |
|---|---|
| I1 Exact dependency | Any change to a referenced declaration changes the model hash, including a field the model does not use. |
| I2 Locality | A change to a declaration the model does not reference never changes the model hash. |
| I3 Names count | Renaming a referenced type changes the model hash, even with an identical body. |
| I4 Order independence | Parameter order, declaration order and supply order never change the model hash (`types` keyed by name, parameter lists sorted by name). |
| I5 One implementation | All kinds use the same helpers. Approval's document is byte-identical to its feature-503 form. |
| I6 Lowering unaffected | Changing the identity rule changes no generated item and no BehaviorHash. |

## 5. Out of scope

Dependency minimization (depending only on the parts of a declaration a model uses) and
behavioral equivalence of models.
