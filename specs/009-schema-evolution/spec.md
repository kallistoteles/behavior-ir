# Feature Specification: Schema Evolution and Migration

**Feature Branch**: `009-schema-evolution`

**Created**: 2026-10-02

**Status**: Draft

**Input**: User description: the first feature driven by the external application experiment (the
"TCUP" application built on release 0.8.0). Schema changes must never reinterpret historical
state. A migration explicitly transforms one valid state into another valid state. It is typed,
content-addressed, deterministic, verified, recorded in history and replayable, and it is a
separate kind of transition, not an ordinary action. (Full description in the conversation
of 2026-10-02.)

## Context

The application built on 0.8.0 showed that the core ideas hold under real use:
- verification found real model errors;
- replay and traceability came without extra code;
- refusals were understandable;
- rules could be tightened without moving data.

Its most fundamental gap is **changing the schema of a store that already has history**.

Today a store's schema is fixed at creation: the genesis records each entity type's declaration.
Behavior may change freely as long as the declarations stay identical. But any change to an entity
type, such as adding an enum value, adding or renaming a field or changing a field's type, makes
every evaluation and commit on that type refuse with a declaration mismatch. The application
worked around this with its own "storage generations": a new store and a hand-written copy. That
is exactly the architecture-around-the-engine this project exists to remove. It loses the
continuity of history, and nothing proves the copy correct.

Two principles govern this feature:

> **State is meaningful only under an exact schema. Schema changes never reinterpret history.**

> **Behavior changes state; migrations change the schema under which state is valid.**

Migrations change representation; Behavior changes information. A migration never encodes a
domain rule about how entities relate. Filling data from relationships is ordinary behavior.

Schemas evolve by **broadening freely and narrowing only after proving the data already fits**.
The staged path is first-class:
1. broaden the schema by migration;
2. backfill or clean up with ordinary Behavior;
3. a migration whose source requirements prove the data is ready narrows the schema.

One more principle protects replay:

> **At every history position, exactly one persisted-state schema defines the meaning of the
> state.**

So the feature has two layers:
1. **Schema identity.** The engine always knows, at every position in a store's history, the exact
   schema under which the state is valid, and it never reads a value under a schema it was not
   written in.
2. **Migration.** An explicit, verifiable transformation from a valid state under schema A to a
   valid state under schema B, committed as its own kind of transition.

## Clarifications

### Session 2026-10-02

- Q: When a module's declarations differ from the store's schema only for entity types an action
  does not touch, should the action still be refused? → A: Yes. The whole store schema must match
  exactly (option A).
  - The **store schema** is the persisted-state schema, not the whole module:
    - entity declarations;
    - persisted field types;
    - the enum and nominal types reachable from persisted fields;
    - reference types.
  - It has its own **SchemaHash**, separate from the behavior hash. Behavior-only definitions
    (actions, rules, queries, derived values, verifier metadata) may change without migration.
  - "Touched types only" was rejected: with relational queries and module invariants, "touched" is
    no longer local, and two modules could otherwise drive one store with conflicting views of
    its schema.
  - Partial or evaluation-only compatibility was rejected for now. It may come later only as an
    explicit, provable compatibility relation.
  - This replaces 0.8's touched-types comparison, so it is a minor version bump.
- Q: In a migration transform, should fields that are unchanged be copied automatically, or must
  the author write every target field? → A: Copied automatically when name and exact type match
  (option B), but only in authoring.
  - **Implicit in authoring, explicit in resolved Migration IR.** Admission resolves every
    automatic copy into an explicit `Copy(source_field)`. The resolved migration is what is
    hashed, verified, traced and reviewed.
  - Auto-copy requires the same field name **and** the exact semantic type identity (equal type
    hash). A changed declaration, including an enum with the same name but different values,
    requires an explicit transform.
  - An explicit target assignment always overrides auto-copy.
  - Every source field that the target schema no longer has requires an explicit `Drop`, because
    removal loses information.
  - Copying by name with an assumed lossless conversion (option C) was rejected: conversions such
    as `Int → Decimal`, `Money2 → Money4` or a widened enum must be stated.
  - Principle: **preserve identical meaning by default; require intent wherever meaning or
    information may change.**
- Q: May a migration transform read other entities, or only the entity being migrated? → A:
  Entity-local only (option A).
  - A transform reads the source entity's own fields, literals and migration-declared constants.
    It never dereferences references, queries, reads `exists`, the live store, input or context.
    **Migration order is not semantic:** entities can be transformed in any order or in
    parallel with identical results.
  - Validity of the result stays **global**. After the complete target state is built, every
    target entity constraint, referential integrity and every module invariant must hold.
    Verification therefore has two parts:
    - a local proof, `Valid_A(e) → EntityValid_B(transform(e))`;
    - a whole-state proof or check, `Valid_A(S) → Valid_B(S')`.
  - Cross-entity population (for example `Order.region` from its customer) is a **backfill**, not
    a migration. Migrate to an intermediate schema with an optional field, fill it with an ordinary
    Behavior action, then migrate onward. A future relational-migration capability, reading only
    the immutable source state, can be added if real applications need it. Query-based transforms
    (option C) are deferred.
  - Principle: **migrations change representation; Behavior changes information.**
- Q: When a migration makes an optional field required, how may the transform turn `Option` into
  a plain value? → A: Through migration **source requirements** plus proven strict narrowing
  (option B).
  - **Source requirements.** These are closed predicates over the immutable source state, written
    in the same relational expression language. They are **applicability conditions**, not module
    invariants: "this migration may only be applied to source states where this holds".
    - They are part of the migration's hash.
    - They are checked against the complete source state before anything is transformed.
    - Failure is `MIGRATION_REQUIREMENT_FAILED`, and the store is unchanged.
  - **Strict narrowing** operations (Option → T, a smaller enum, a refined type) are allowed only
    where the verifier proves, at each narrowing site, that the source guarantees plus the source
    requirements imply the narrowing is safe. A requirement never authorizes a narrowing in
    general.
  - **Verification** proves `Valid_A(S) ∧ Requirements_M(S) ⇒ Valid_B(M(S))` and reports it as
    proven *under* the named requirements. Applying the migration then establishes that the
    requirements hold for the concrete source state.
  - Two refusals are distinguished:
    - `MIGRATION_REQUIREMENT_FAILED`: the migration is fine, but the data is not ready yet;
    - `MIGRATION_TRANSFORM_ERROR`: the migration's definition is not safe.
  - Principle: **broaden freely; narrow only after proving the data already fits.** The staged
    path (broaden, backfill with Behavior, prove readiness, narrow) is a first-class model, not a
    workaround.
- Q: Must a migration have a verification result with no blocking findings before a store accepts
  it, or does the store's evidence policy decide? → A: The evidence policy decides (option A).
  - **Runtime checks are mandatory and cannot be waived by any policy:**
    - the exact source schema;
    - the source requirements;
    - the deterministic transform;
    - complete validation of the target state.
  - **Static verification is evidence:** proven, counterexample or inconclusive. The evidence
    policy decides which verification state, attestations and authorizations are required to
    commit, through the same mechanism as for actions.
  - **The policy can tell the two apart.** It must be able to require different evidence by
    transition kind (action or migration), so a deployment can demand stronger governance for
    migrations. That choice stays out of Behavior semantics.
  - **A migration valid only for the concrete data is legitimate.** Its record states exactly
    what was established.
  - Principle: **runtime establishes that this transition is valid; verification establishes what
    can be proven about the transition class; governance decides which evidence is sufficient to
    trust the commit.**

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Evolve a store's schema with an explicit migration (Priority: P1)

An application has a store with history under schema V1 (for example, an enum `MediumKind` with
`MS` and `WPM`). The domain now needs schema V2, with a new value `B5`, a renamed field and a new
optional field. The author writes the V2 module and a migration from V1 to V2 that says how each
affected entity's V1 value becomes its V2 value. The migration is applied to the store as one
transition. From then on, V2 behavior evaluates and commits against the store, and V1 behavior is
refused for new transitions.

**Why this priority**: Without it, any schema change to a living store forces the application to
build its own copy-and-switch machinery outside Behavior, losing history continuity and proof.
This is the gap that blocked the external application.

**Independent Test**:
1. Create a store under V1 and commit transitions.
2. Apply a migration to V2. Commit V2 transitions.
3. Check that every V1-era transition still replays under V1, that the migration replays, and
   that the V2-era transitions replay under V2.

**Acceptance Scenarios**:

1. **Given** a store under V1 with history, **When** the V1 to V2 migration is applied, **Then**
   it commits as one atomic transition at the next history position. Every entity of a changed
   type is transformed into a valid V2 entity, and the store's current schema is V2.
2. **Given** the migrated store, **When** a V2 action is evaluated and committed, **Then** it
   succeeds exactly as on a store created under V2 with the same state.
3. **Given** the migrated store, **When** a V1 action is evaluated against the current state,
   **Then** it is refused with `SCHEMA_MISMATCH` before the action runs. Nothing is guessed or
   converted on the fly.
4. **Given** the migrated store, **When** a historical state from before the migration is read,
   **Then** it is returned exactly as written, interpreted under V1.

---

### User Story 2 - A migration can never produce an invalid state (Priority: P1)

The author writes a migration that would break a V2 rule, for example a value that maps to
nothing, a field that violates a V2 constraint, or a duplicate that breaks a V2 module invariant.
Applying it is refused as a whole, with reasons naming the rule and the entities. The store is
unchanged.

**Why this priority**: A migration rewrites every entity of a type at once. A silently invalid
result would corrupt the whole store, which is far worse than one bad transition.

**Independent Test**: Apply migrations seeded with each kind of violation to a store. Check that
each is refused with its reason, that the store's position, state identity and schema are
unchanged, and that a corrected migration then succeeds.

**Acceptance Scenarios**:

1. **Given** a migration whose transform leaves a V1 enum value without a V2 counterpart,
   **When** it is admitted, **Then** it is refused before it ever touches a store. The refusal
   names the uncovered value.
2. **Given** a migration whose output breaks a V2 entity constraint for some entity, **When** it
   is applied, **Then** the whole migration is refused and names the constraint and the entity.
3. **Given** a migration whose output breaks a V2 module invariant or leaves a dangling
   reference, **When** it is applied, **Then** it is refused, and the store is unchanged.
4. **Given** a migration that fails part-way (for example, a crash), **When** the store is
   reopened, **Then** it is either fully migrated or not migrated at all, never partially.
5. **Given** a narrowing migration whose source requirement does not hold yet (some entities still
   have no value), **When** it is applied, **Then** it is refused with
   `MIGRATION_REQUIREMENT_FAILED`, naming the requirement and the number of violating entities.
   Once a backfill action has fixed the data, the same migration applies.

---

### User Story 3 - Verify a migration before applying it (Priority: P2)

Before running a migration against a production store, the author verifies it: for every valid V1
state, does the migration produce a valid V2 state? The verifier answers as it does for actions.
- **proven:** the migration preserves validity.
- **counterexample:** a concrete V1 entity whose migrated value breaks a V2 rule, confirmed by
  actually running the migration on it.
- **inconclusive:** not decided, and blocking.

**Why this priority**: Verification is what makes Behavior worth using. A migration is the riskiest
kind of change, so it deserves the same proof obligation as actions: `Valid_A(S) → Valid_B(S')`.

**Independent Test**: Verify a set of migration fixtures with known outcomes (correct ones proven,
seeded defects found and confirmed), and check that nothing outside the verifier's model is
reported as proven.

**Acceptance Scenarios**:

1. **Given** a migration that maps every V1 value to a V2 value satisfying every V2 entity
   constraint, **When** it is verified, **Then** the preservation of each V2 constraint is proven.
2. **Given** a migration that can produce a value violating a V2 constraint, **When** it is
   verified, **Then** the result is a counterexample: a V1 entity value that the migration,
   actually run, turns into a violating V2 value.
3. **Given** a V2 property the verifier cannot decide (for example, one depending on unknown
   members of a set), **When** it is verified, **Then** it is inconclusive and blocking. It is
   never assumed.

---

### User Story 4 - History and replay span schema generations (Priority: P2)

An auditor replays a store whose history crosses one or more migrations. Every transition replays
under the schema and behavior it was made with:
- the V1 actions under V1;
- the migration as a migration from V1 to V2;
- the V2 actions under V2.

Every state identity is recomputed and matches. Tampering with a migration record or with a
migrated value is detected at its position.

**Why this priority**: Behavior's audit guarantee is that history replays without trusting stored
results. A schema change must not become the point where that guarantee stops.

**Independent Test**: Build a history V1 to V2 to V3 with transitions in each generation. Run
data replay and behavior replay from genesis to head. Tamper with a migrated entity value and
with a migration record, and check that each is detected at its position.

**Acceptance Scenarios**:

1. **Given** a history spanning migrations, **When** data replay runs, **Then** it re-applies each
   recorded migration, recomputes every state identity, and finds no divergence.
2. **Given** the same history, **When** behavior replay runs with the modules of every generation,
   **Then** every action replays under its own schema and every migration replays as recorded.
3. **Given** a tampered migrated value, or a migration record whose transform does not produce
   the recorded values, **When** replay runs, **Then** the divergence is reported at the
   migration's position.

---

### User Story 5 - Only real schema changes need migrations (Priority: P3)

The author changes only behavior (actions, rules, invariants, queries) and leaves every entity
declaration identical. This needs no migration: the new behavior evaluates against the existing
store directly, as today. Any change to an entity type's declaration needs an explicit migration.
The engine never decides by itself that two different declarations are compatible.

**Why this priority**: It keeps the common case (behavior evolution) free, and makes the
conservative rule explicit and testable before any provable-compatibility rule is introduced
later.

**Independent Test**: Apply a behavior-only change to a store and check that it evaluates without
migration. Apply a declaration change (even one as small as appending an optional field) and
check that evaluation is refused until a migration is applied.

**Acceptance Scenarios**:

1. **Given** a new module whose entity declarations are identical to the store's current schema,
   **When** its actions are evaluated, **Then** they run without any migration.
2. **Given** a module where only an entity type the action does not touch differs from the store,
   **When** the action is evaluated, **Then** it is refused anyway with `SCHEMA_MISMATCH`, before
   the action runs, because the store schema is one unit.
3. **Given** any declaration difference, however small, **When** no migration has been applied,
   **Then** evaluation and commit are refused, and the reason names the differing types.

### Edge Cases

- **Wrong source schema:** a migration whose source schema is not the store's current schema is
  refused. A migration whose source and target schemas are equal is refused at admission
  (`MIGRATION_SCHEMA_MISMATCH`).
- **Chains:** two migrations applied in sequence (V1 to V2, then V2 to V3) each commit and replay
  separately. There is no implicit composition.
- **Concurrency:** an action evaluated under V1 but committed after the migration landed is
  refused as a state conflict. The current state has moved, and so has its schema.
- **New entity types:** entity types that are new in V2 start empty after the migration.
- **Retired entity types:** a type that exists in V1 but not in V2 can be retired only if the
  migration states it explicitly. The store must hold no entities of that type at the migration's
  position; otherwise the migration is refused, naming the remaining entities.
- **Identity:** a migration preserves every entity's identity. The identity registry (never
  reusing an identity) continues across the migration.
- **References:** references survive the migration only if their target type and identity still
  exist under V2. Otherwise the migration is refused as a dangling reference.
- **Old records:** they always keep their original schema. No record, entity version or state
  identity written before the migration changes.
- **Large stores:** a migration of a store with many entities of a changed type completes as one
  transition within the performance target, or fails cleanly with nothing applied.
- **Data from related entities:** a new field whose value comes from a related entity cannot be
  filled by a migration. The path is staged:
  1. migrate to an intermediate schema where the field is optional;
  2. fill it with an ordinary, verified and replayable Behavior action;
  3. migrate onward if the field should become required. That migration declares the source
     requirement "every order has a region", which proves its strict unwrap safe. Applied too early,
     it is refused with `MIGRATION_REQUIREMENT_FAILED`, naming how many orders lack a region.

## Requirements *(mandatory)*

### Functional Requirements

**Schema identity**

- **FR-001**: Every store MUST know its current **store schema**: the persisted-state schema
  under which its current state is valid. It consists of:
  - the entity declarations;
  - the persisted field types;
  - the enum and nominal types reachable from persisted fields;
  - the reference types.

  It MUST be identified by its own content hash, the **SchemaHash**, separate from the behavior
  version. Every behavior module MUST report the SchemaHash of the store schema it declares.
- **FR-002**: For every position in a store's history, the schema under which that position's
  state is valid MUST be recoverable from the history alone.
- **FR-003**: A stored entity value MUST only ever be read, evaluated, verified or replayed under
  the schema it was written in. No operation may reinterpret an existing value under a different
  schema.
- **FR-004**: Changes to behavior-only definitions (actions, rules, queries, derived values,
  verifier metadata) that leave the store schema unchanged MUST NOT require a migration.
- **FR-005 — Exact store-schema binding**: Every evaluation against a store MUST require the
  module's SchemaHash to equal the SchemaHash recorded at the evaluated store position.
  - The comparison covers the complete persisted-state schema, not only the entity types the
    invoked action reads or writes.
  - A mismatch MUST refuse the evaluation before the action runs, with the error
    `SCHEMA_MISMATCH`, which names the store's and the module's SchemaHash and the entity types
    that differ.
  - A commit under a different schema is possible only after an explicit migration has advanced
    the store to that schema.
  - The engine never infers compatibility between different schemas.

**Migrations**

- **FR-006**: A migration MUST be a distinct kind of item, not an action. It names a source schema
  and a target schema, and contains a deterministic transform for every entity type whose
  declaration differs between them. A changed type that the author does not mention gets a
  transform built entirely by resolution (FR-007a), for example when only the field order
  changed.
- **FR-007**: A migration transform MUST be written in the same typed, exact expression language as
  behavior. It reads the old entity's own fields, literals and the migration's declared constants.
  It does not read other entities, queries, inputs, context or time. The transform produces the new
  entity's complete value under the target schema. Transforms are independent of each other, so
  the result MUST NOT depend on the order in which entities are transformed.
- **FR-007d — Source requirements**: A migration MAY declare source requirements.
  - **What they are.** Closed predicates over the source state, using source entities, queries,
    literals and pure derived values over the source state. They never use input, context, time,
    external state, target-state values or host callbacks.
  - **What they mean.** They are applicability conditions of that migration, distinct from module
    invariants.
  - **Identity.** They are part of the migration's identity.
- **FR-007e — Proven narrowing**: A transform MAY use strict narrowing operations:
  - `Option<T>` to `T`;
  - an enum to a target enum without some source values;
  - a value to a more restricted type.

  Each narrowing site is a verification obligation. The source schema's guarantees plus the
  migration's source requirements must imply that the narrowing succeeds for every source entity.
  A requirement that does not imply a site's safety does not authorize it.
- **FR-007a — Resolution**: Admission MUST resolve every transform into a complete, explicit form,
  and the resolved form is what is hashed, verified, traced, shown in diffs and recorded. For each
  target field:
  1. an explicit assignment is used if there is one;
  2. otherwise, a source field with the same name and the exact same semantic type identity
     (equal type hash) becomes an explicit `Copy(source_field)`;
  3. otherwise, admission refuses with `MISSING_MIGRATION_FIELD`, naming the field.

  A changed type declaration (for example, an enum with added or removed values) is never "the
  same type", even under the same name. It requires an explicit conversion.
- **FR-007b — Explicit drop**: A source field that is absent from the target schema and that no
  explicit assignment reads (as in a rename) loses information. It MUST be acknowledged with an
  explicit `Drop(source_field)`; otherwise admission refuses with `UNACKNOWLEDGED_FIELD_DROP`.
- **FR-007c — Reviewable summary**: For every migrated entity type, a migration MUST report its
  resolved transform, classifying each field as:
  - automatic copy;
  - explicit transform;
  - new value;
  - dropped.
- **FR-008**: A migration MUST preserve entity identity: one old entity becomes one new entity with
  the same identity and type name, or the type is explicitly retired (see FR-012). Splitting,
  merging, creating and deleting entities are out of scope.
- **FR-009**: A migration MUST be admitted (type-checked) before it can be applied. Admission MUST
  refuse:
  - a target field that is neither assigned nor auto-copyable (FR-007a), including in a changed
    type the author did not mention;
  - a removed source field without an explicit drop (FR-007b);
  - a transform that is ill-typed or lossy without an explicit rounding;
  - a mapping of a source enum value to nothing.
- **FR-010**: A migration MUST be content-addressed. Its identity covers the source and target
  schema identities, every source requirement, and every **resolved** transform, including
  automatic copies and drops. It
  never covers names or source locations beyond what the rest of Behavior hashes.
- **FR-011**: Applying a migration MUST be one atomic transition at the next history position.
  It proceeds in this order:
  1. check that the source state satisfies the **source module's** entity constraints, entity
     invariants and module invariants;
  2. check every source requirement against the complete, immutable source state;
  3. transform every entity of every changed type;
  4. validate the target state (FR-013);
  5. commit.

  A source state that breaks a source rule refuses with `MIGRATION_SOURCE_INVALID`, naming the
  rule and the entities. (Constraints and invariants are behavior, not schema, so data written
  under earlier behavior of the same schema may not satisfy them.) A failed source requirement
  refuses with `MIGRATION_REQUIREMENT_FAILED`. The refusal names the requirement and, when the
  requirement quantifies over an entity type (`all_` or `not any_` over `select(T)`), how many
  entities violate it, with examples, e.g. "17 Order entities still have region = None". A
  narrowing that fails at runtime refuses with `MIGRATION_TRANSFORM_ERROR`. In every case the
  store is unchanged.
  - It transforms every entity of every changed type that exists at that position, and changes
    the store's current schema to the target.
  - Either all of this is committed or nothing is.
  - It MUST be refused if the store's current schema is not the migration's source schema.
- **FR-012**: Entity types that are new in the target schema MUST start empty. An entity type that
  is absent from the target schema MUST be retired explicitly by the migration, and only when no
  entity of that type exists at the migration's position.
- **FR-013**: After the transform, the resulting state MUST satisfy every target-schema rule before
  the migration commits:
  - every entity constraint;
  - every entity invariant;
  - every module invariant;
  - referential integrity.

  Any violation refuses the whole migration, with reasons naming the rule and the entities.
- **FR-014**: The identity registry MUST continue across a migration. An identity used before it
  stays used after it.

**History and replay**

- **FR-015**: A migration transition MUST be recorded in the store's history. The record names the
  migration's identity, the source and target schema identities, and the old and new entity
  versions it wrote. Nothing written before it changes.
- **FR-016**: Data replay MUST re-apply each recorded migration from the previous state, recompute
  the new entity versions and the state identity, and report any divergence at the migration's
  position.
- **FR-017**: Behavior replay MUST replay each action under the schema and behavior it was recorded
  with, and each migration as that migration, across any number of schema generations.
- **FR-018**: Reading any past state MUST return values exactly as written, interpreted under that
  position's schema.

**Evidence and governance**

- **FR-019a — Mandatory runtime validity**: The checks of FR-011 and FR-013 MUST always run when a
  migration is applied: the exact source schema, source-state validation, the source
  requirements, the deterministic transform, and complete target-state validation. No evidence policy, waiver or authorization can
  skip them.
- **FR-019b — Policy-governed evidence**: Whether a migration needs a verification attestation
  (and with which outcome), a commit authorization or other evidence MUST be decided by the
  store's evidence policy, through the same mechanism as for actions. The policy MUST be able to
  set different requirements per transition kind (action or migration). The default policy treats
  both kinds alike.
- **FR-019c — What a migration record states**: A migration record MUST state:
  - the migration identity;
  - the source and target state and schema identities;
  - the evidence policy that applied;
  - any cited attestation with its verification outcome;
  - any authorization;
  - that source-state validation and target-state validation passed.

  A migration whose verification was inconclusive but whose concrete application was valid is
  recorded as exactly that.

**Concurrency**

- **FR-019**: A migration commits only on the exact state it was prepared against, like any other
  transition. An action evaluated against a state from before the migration MUST be refused at
  commit as a state conflict.

**Verification**

- **FR-020**: The verifier MUST check a migration as
  `Valid_A(S) ∧ Requirements_M(S) ⇒ Valid_B(M(S))`, and every narrowing site as
  `Valid_A(e) ∧ Requirements_M(S) ⇒ the narrowing succeeds`. A result that relies on source
  requirements MUST be reported as proven under those named requirements. `Valid_A` means the
  source module's rules. The store does not guarantee them for data written under earlier
  behavior, so application establishes them (FR-011 step 1) before relying on the proof. The
  check has two parts:
  - **Local:** every valid source-schema entity is transformed into a value satisfying every
    target-schema entity constraint and invariant.
  - **Whole-state:** referential integrity and every target module invariant hold on the resulting
    state, given a valid source state. The
  outcomes are:
  - **proven**: holds for every valid source entity;
  - **counterexample**: a source entity whose actual migrated value violates a rule, confirmed by
    running the migration;
  - **inconclusive**: not decided, and blocking.
- **FR-021**: Properties of the migrated state as a whole (module invariants, referential
  integrity) that the verifier cannot decide MUST be reported as inconclusive, never assumed. They
  are always checked at application time (FR-013).

**Authoring and tooling**

- **FR-022**: The Python binding MUST let authors declare a migration between two modules and
  write its transforms with the same expression language, including mapping enum values and
  renaming, adding and removing fields.
- **FR-023**: The command-line tool MUST:
  - report a module's SchemaHash;
  - admit and verify migrations;
  - apply a migration in plain mode to a supplied source universe.

  Schema history is a store API (Python and Rust); the command-line tool has no persistent store.
- **FR-024**: The consumer skills MUST describe migrations: when a change needs one, how to write
  and verify it, and how to apply it to a store. The skills' existing checks (runnable examples,
  verbatim excerpts, public-API references) apply.

**Compatibility**

- **FR-025**: Every existing store, record, golden file and module keeps its bytes and identity. A
  store created before this feature is a store whose whole history is under its genesis schema.
  Any new document form introduced by this feature MUST be versioned so that existing documents
  are unchanged.

### Key Entities

- **Store schema**: the persisted-state schema under which a state is valid. It consists of the
  entity declarations, the persisted field types, the enum and nominal types reachable from
  persisted fields, and the reference types. Its identity is the **SchemaHash**, separate from the
  behavior version. A store has exactly one store schema at every history position.
- **Migration**: a content-addressed item. It has:
  - a source schema and a target schema;
  - its source requirements;
  - one resolved transform per changed entity type, where every target field is an explicit
    assignment or a `Copy`, and every removed source field is a `Drop`;
  - the retired entity types;
  - declared constants.
- **Migration transition**: the committed application of a migration to a store at one position,
  a second transition kind next to actions. It records:
  - the migration;
  - both schemas;
  - the entity versions it wrote;
  - the evidence that authorized it (FR-019c).
- **Schema history**: the sequence of schemas a store has had, each with the position from which
  it applies.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The schema changes the external application needed can be done in place on a living
  store, keeping its history and identities:
  - adding an enum value;
  - renaming a field;
  - adding an optional field;
  - changing a field's type with an explicit conversion.

  No copy into a new store is required.
- **SC-002**: In generated histories that cross up to three migrations with at least 1,000
  transitions in total, data and behavior replay from genesis report no divergence. Every
  tampering with a migration record or a migrated value is detected at its position (100%).
- **SC-003**: Every seeded invalid migration in the test set is refused, either at admission or at
  application, with a reason naming the rule and the entity. The store is left unchanged in 100%
  of cases.
- **SC-004**: Migration verification fixtures produce all three outcomes as expected. Correct
  migrations are proven, seeded defects give runtime-confirmed counterexamples, and nothing
  outside the model is reported as proven.
- **SC-005**: Migrating a store with 100,000 entities of a changed type completes as one transition
  in under 60 seconds on the reference machine. A failure at any point leaves the store exactly as
  before.
- **SC-006a**: The staged path completes in place on a living store:
  1. broaden (an optional field);
  2. backfill with an ordinary action;
  3. narrow (required, with a source requirement).

  Applied before the backfill, the narrowing step is refused in 100% of cases and names the
  violating entity count.
- **SC-006**: A behavior-only change (identical declarations) needs no migration. Any declaration
  change without a migration is refused in 100% of evaluation and commit attempts.
- **SC-007**: Every existing store, record, module and golden file keeps its bytes and identity.

## Assumptions

- **Numbering.** This is feature 009. The description called it "008", but 008 is the
  agent-ready package. Read/query interfaces and action binding (proposed as the next two) are
  separate, later features.
- **No inferred compatibility.** The conservative rule applies: an identical declaration needs
  nothing, and any difference needs an explicit migration. Provably lossless evolution without
  rewriting data (for example, appending an optional field read as absent) is a future feature.
- **Transforms are entity-local** (clarified). Cross-entity population is done by an ordinary
  backfill action between migrations. A relational migration that reads only the immutable source
  state is a possible future capability.
- **Identity-preserving only.** One entity in, one entity out. Splits, merges, creations and
  deletions during a migration are out of scope; entities to delete are removed by ordinary actions
  before migrating. Renaming an entity type is a retirement (of an empty type) plus a new type.
- **Data is migrated eagerly** at the migration's position. Lazy, on-read migration is out of
  scope, since it would make a value's schema depend on when it is read.
- **One schema per store.** A store has exactly one current schema. Mixed-schema stores (some
  entity types migrated, others not) are not supported; the schema changes as one unit.
- **Version bump.** Exact store-schema binding replaces 0.8's touched-types comparison. This is a
  behavior change for existing stores and modules, and so a minor release under
  `docs/versioning.md`.
- **Evidence for migrations** (clarified). The store's evidence policy decides, per transition
  kind. Runtime validity is never optional. Extending the evidence-policy format to distinguish
  transition kinds is part of this feature; existing policies keep their meaning and bytes.
- **External acceptance.** Running the external application as a permanent acceptance suite for
  each release is valuable, but it is a separate effort outside this repository.
