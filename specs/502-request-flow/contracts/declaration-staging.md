# Contract: Declaration Staging (extends the 501 build protocol)

**Feature**: 502 | Common model infrastructure. It applies to every model kind.

## 1. Phases

| Phase | Call | Owner | Reads | Writes |
|---|---|---|---|---|
| P1 | binding declares enums, nominals and entities | ordinary Behavior API | — | authored declarations |
| P2 | `declare_models(builder, &models) -> Result<Vec<DeclaredSchema>, Vec<ModelError>>` | common model infrastructure | P1 snapshot | model declarations (identical-or-error) |
| P2b | binding declares commands | ordinary Behavior API | P1 + P2 | commands; after P2, because a command's payload may name a generated entity (`Id<Request>`) and Core checks command field types when the command is added |
| P3 | binding traces derived, invariants, constraints, actions, reads | Behavior authoring | P1 + P2 declarations | hand-written behavior |
| P4 | `compose(builder, models, root)` | common model infrastructure, delegating to each kind | everything | generated actions; final checks; receipts |

A binding MUST call P2 before P3 whenever any supplied model contributes declarations. It MAY
call P2 for every model set: P2 is a no-op for declaration-free sets.

## 2. Laws

| # | Law |
|---|---|
| D1 | **Separation:** `Decls(m)` depends only on `m`'s input and P1 declarations, never on lowering, fragments or P3. |
| D2 | **No-op for declaration-free models:** if `⋃ Decls(m) = ∅`, P2 performs no builder call, so builds of 501 models are unchanged byte for byte (StateMachine `Decls = ∅`). |
| D3 | **Canonical merge:** contributions are applied in model-id order; each is identical-or-error against existing items. Neither authored nor contributed declaration order affects any hash. |
| D4 | **Stability:** P4 re-derives `Decls(m)` and requires the builder's declaration to equal it (same name and entity hash). Otherwise `DECLARATION_MISMATCH`. |
| D5 | **Visibility:** every P3 construct can reference model declarations: parameter types, `Id[...]` and `Ref[...]` fields of authored entities, queries, reads and guards. |
| D6 | **No overwrite:** a name clash is an error (`DECLARATION_COLLISION`), never a rename or replacement. |

## 3. `DeclaredSchema` (returned by P2)

`{ model_id, entity: name, fields: [(field, WType)] }`, in the canonical field order. Bindings
use it only to fill typed handles and to check them for drift. It carries no authority beyond
the Rust derivation.

## 4. Errors

`DECLARATION_COLLISION { name, model_ref, existing_kind }` (stage `set`),
`DECLARATION_MISMATCH { name, model_ref }` (stage `set`), `UNKNOWN_ENUM { name, model_ref }`
(stage `resolve`).
