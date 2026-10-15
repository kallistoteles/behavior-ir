# Contract: Model Declaration DAG (extends the 502 declaration-staging contract)

**Feature**: 503 | Common model infrastructure. It applies to every model kind. No model kind
orders its own declarations.

## 1. Phases (502's, with P2 generalized)

| Phase | Call | Change in 503 |
|---|---|---|
| P1 | binding declares authored enums, nominals and entities | unchanged |
| P2 | `declare_models(builder, &models)` | a topological pass over `G_D`; contributions are enums or entities; then model definitions (derived values) |
| P2b | binding declares commands | unchanged |
| P3 | binding traces hand-written behavior | may reference generated declarations **and generated derived values** |
| P4 | `compose(builder, models, root)` | re-derives declarations (D4) **and definitions (D4′)** |

## 2. The graph

```text
Contrib(M)  = the declarations model M produces, each an Enum or an Entity
Needs(M)    = the type names M's contributions reference (Id, Enum, Ref targets of its fields)
G_D = (M, E)     E = { M_i → M_j | i ≠ j, Needs(M_j) ∩ names(Contrib(M_i)) ≠ ∅ }
```

- Authored declarations are not nodes: they exist before P2.
- `Needs(M)` is computed from `M`'s input alone (law D1): no lowering, no fragments.
- A name that is neither authored nor contributed is a resolution error of the model that
  needs it (`UNKNOWN_ENTITY` / `UNKNOWN_ENUM`).

**Order.** Kahn's algorithm, where the ready set is a `BTreeSet` keyed by canonical model id.
The order is unique for a given model set and independent of supply or Python insertion order.

**Cycle.** If nodes remain once no node is ready, the build fails with
`DECLARATION_CYCLE { models }`: the remaining models, sorted. Nothing is declared.

**Merge.** Each contribution, in topological order, is identical-or-error against the
declarations already present (D6). Enums are compared by cases and Core enum hash, entities as
in 502. The collision rule belongs to this merge, not to any model kind: different semantic
origins may collide, and every collision is an error, never a rename.

**Definitions: signatures first, bodies in dependency order.** A model contributes
`DefinitionSpec { name, kind, params, result, deps }`. `deps` is the set of other definitions
its body references.

1. **Signatures.** After all declarations, every signature `(name, params, result)` is fixed and
   returned to the binding with the declared schemas. Typed handles (for example
   `approval.approved(x)`) check their argument against the signature without needing a body.
   Every name is claimed; a taken name is `GENERATED_NAME_COLLISION`.
2. **Bodies.** The definition graph `G_F` has an edge `f → g` iff `g.deps ∋ f`. Bodies are built
   and added with `add_derived` in a topological order of `G_F`, so a body is typed only after
   every definition it references. Ready definitions are taken by `(model id, name)`. A cycle
   is `DEFINITION_CYCLE { names }` (sorted), and nothing is added.
3. Every definition is added before P3, so a definition exists before any use, but never before
   its dependencies.

## 3. Laws

| # | Law |
|---|---|
| D1–D6 | as in 502, unchanged |
| D7 | **Topological determinism:** the declaration and definition sequence is a function of the model *set*: any supply order gives the same builder calls. |
| D8 | **Acyclicity:** the build proceeds only if `G_D` is acyclic; a cycle gives the same `DECLARATION_CYCLE` in every supply order. |
| D9 | **Regression:** for sets without edges and without definitions (every 501 and 502 set), the builder call sequence equals 502's, so `H_501^new = H_501^pinned` and `H_502^new = H_502^pinned` for every manifest case. |
| D10 | **Definition order:** a definition body is added only after all its `deps`; the order is a function of the definition set; a cycle in `G_F` gives `DEFINITION_CYCLE`. |
| D4′ | **Definition stability:** P4 re-derives each definition and requires the builder's item to have the same Core hash, otherwise `DEFINITION_MISMATCH`. |

## 4. Forward references (502 D5, extended)

If the authored declarations alone do not admit because they name a contributed type, P2 takes
the fallback path. It:
- derives every contribution from inputs alone;
- declares them in topological order;
- admits the result;
- runs every resolution and collision check on the completed declarations;
- then adds the definitions.

Core's own error is reported unchanged when even the completed declarations do not admit (502
data model §3 note).

## 5. Errors

`DECLARATION_CYCLE { models }`, `DEFINITION_CYCLE { names }` and
`DEFINITION_MISMATCH { name, model_ref }` (all stage `set`). Plus 502's `DECLARATION_COLLISION`, `DECLARATION_MISMATCH` and `UNKNOWN_ENUM`.
