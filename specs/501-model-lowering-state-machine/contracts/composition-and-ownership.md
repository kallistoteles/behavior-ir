# Contract: Composition and Ownership

**Feature**: 501 | Designed before StateMachine lowering. Applies to all model kinds.

## 1. Inputs

A composition is `(B, ModelSet)`:
- `B`: the shared Core `Builder` with all ordinary behavior traced (lowering contract §4,
  step 1);
- `ModelSet`: models keyed by `model_id`. Supplying a list is accepted, but it is converted to a
  set, and a duplicate id is `DUPLICATE_MODEL_ID`.

The result is independent of the order in which models are supplied and of the order in which
the binding traced hand-written behavior (law L3).

## 2. Set-level checks (before any lowering)

1. `model_id`s unique → else `DUPLICATE_MODEL_ID`.
2. Build `OwnershipTable: (entity, field) → model_id` from every model's claims (StateMachine
   claims exactly `(E, f)`). Two claims on one key → `CONFLICTING_OWNERSHIP { entity, field,
   models: sorted }`.
3. Contribution kinds: StateMachine v1 contributes actions only. Future kinds that contribute
   declarations follow "identical-or-error": the same name must have an identical Core item
   hash, otherwise `INCOMPATIBLE_CONTRIBUTION`. Nothing is overwritten.

## 3. Generated names

- Names are `encode(model_id, transition_id)` (research R4, injective). Distinct machines
  therefore never collide with each other, and independently authored machines may reuse local
  transition ids (`submit`, `approve`) without coordinating.
- A generated name already taken by a hand-written item → `GENERATED_NAME_COLLISION`. There is
  no renaming, suffixing or last-write-wins.

## 4. Final composition pass (over the admitted module)

Runs on `module = B.finish(..)`, the complete set of actions. For every action `a` in
`module.actions()` (sorted by name):

**Writes.** For each `e ∈ a.effects()`, let `p = e.param()` and resolve `p` in `a.params()`. If
`p.ty().to_wire_type() = Entity(X)` and `(X, e.field()) ∈ OwnershipTable` with owner `m`:
- if `a.name() ∈ Generated(m)`, the write is legal (and by construction it is the single
  generated target-state write, or the build already failed with
  `TRANSITION_WRITES_OWNED_FIELD`);
- otherwise → `UNAUTHORIZED_OWNED_FIELD_WRITE { action, entity, field, owner }`.

**Creations.** For each `c ∈ a.creates()` with `c.entity() = X` and `(X, f) ∈ OwnershipTable`:
- let `x` be the expression of field `f` in `c.fields()`;
- `x.hash() ∈ EntryLiteralHashes(m)` → legal;
- `x.hash() ∈ LiteralHashes(Q \ Q_entry)` → `ENTRY_STATE_NOT_ALLOWED { state }`;
- otherwise (input, derived, computed) → `ENTRY_STATE_NOT_STATIC`.

`LiteralHashes` is computed by the probe in research R8: Core builds each literal in a
throwaway module (a fresh `Builder` re-declaring `T_f` with the module's cases; equal enum hash
asserted), and its semantic hash is read back. The probe is never emitted.

**Not writes.** Reads, derived values, invariants, constraints, command emissions and
`removes()` are not writes and are never ownership violations. Removal is Core lifecycle
semantics (spec FR-029). Migrations are not part of a module (research R7).

## 5. Completeness argument

In an admitted Core module, field values change only through action effects and creations.
Both are enumerated in full by `ActionItem::effects()` and `ActionItem::creates()`, and the
admitted `Module` contains every action. Hence, if the pass accepts, every ordinary write to an
owned field lies inside the owner's generated actions, whose effects are fixed by the lowering
layout. And every new entity of `E` starts in `Q_entry`. That is FR-017 and FR-021, and law
L6.

No escape hatch exists: there is no option, attribute or binding flag to skip the pass.

## 6. Composition examples (become tests)

| Case | Expected |
|---|---|
| Two machines on different entities, both with transition `submit` | OK: two actions, `a__submit` and `b__submit` |
| Two machines on different fields of one entity | OK |
| Two machines on the same `(E, f)` | `CONFLICTING_OWNERSHIP`, identical for every supply order |
| Hand-written `force_approve` sets `request.status` | `UNAUTHORIZED_OWNED_FIELD_WRITE` |
| Hand-written action named `purchase_request__approve` | `GENERATED_NAME_COLLISION` |
| Hand-written create with `status = Draft` (entry) | OK |
| Hand-written create with `status = Approved` | `ENTRY_STATE_NOT_ALLOWED` |
| Hand-written create with `status = input.initial` | `ENTRY_STATE_NOT_STATIC` |
| Hand-written action reads `status`, or removes the entity | OK |
| No creation of `E` anywhere in the module | OK (spec edge case) |
