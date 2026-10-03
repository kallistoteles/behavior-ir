# Research: Schema Evolution and Migration

Phase 0 of [plan.md](plan.md). The spec's clarifications (session 2026-10-02, Q1–Q5) are taken as
given; this file decides how to realize them.

## Current state

The facts the decisions build on:

- **Entity declaration hashes already cover their types.**
  - `admit::hash::entity` hashes the type of each field.
  - Enums and nominals are hashed by their **content** hash (`href(&e.hash)`), so adding an enum
    value changes the declaration hash of every entity using it.
  - `Id<T>` and `Ref<T>` are hashed by the target's **name**.
- **The schema is fixed at genesis.**
  - `Genesis.entity_declarations` maps each entity name to its declaration hash.
  - It is read wherever the store needs a declaration: entity content hashing
    (`EntityVersion::content(declaration)`), evaluation and commit checks (the *touched-types*
    comparison in `store.rs`), data and behavior replay, and genesis seeding.
- **Head:** `Head { state_ref, acc_num, acc_den, last_record }`. It is a mutable pointer that each
  commit rewrites; it is not a historical document.
- **Records:** `TransitionRecord { …, bundle: CommitBundle, new_versions, created, removed,
  ref_changes, result_state, … }`. `CommitBundle` is specific to actions: action record, read set,
  write set, facts.
- **Evidence policy:** `EvidencePolicy { format, require: none|commit_authorization,
  trusted_execution_policies? }`. It is fixed in the genesis and content-hashed.
- **Commits:** the backend `commit(expected, versions, removals, ref_changes, record, head)` is
  generic. It does not care what kind of transition wrote the versions.

## R1. SchemaHash

- **Decision:**
  - `SchemaHash = H(behavior.store_schema.v1, sorted [(entity name, entity declaration hash)])`.
  - A module's store schema is its entity declarations. Enums and nominals not reachable from a
    persisted field don't count, by construction: they enter only through the declaration hashes.
  - **Exposure:**
    - `behavior_core` gains `schema(module) -> StoreSchema { hash, declarations }`;
    - Python gets `model.schema_hash`;
    - the CLI gets `behavior schema-hash <wire>`.
  - **Existing stores:** a store's genesis schema is `SchemaHash(genesis.entity_declarations)`,
    computed and not stored. Every existing store and genesis therefore keeps its bytes.
- **Rationale:** FR-001, Q1. The declaration hashes already capture "the exact persisted meaning".
  The schema hash is just their set.
- **Alternatives considered:**
  - **Hashing the whole module:** this would require a migration for behavior-only changes,
    which violates FR-004.
  - **Adding enum hashes separately:** redundant, since they are already inside the declaration
    hashes.

## R2. Exact store-schema binding

- **Decision:**
  - **The new check.** Evaluation and commit replace the touched-types comparison with
    `schema(module).hash == store.schema_at(position).hash`. A mismatch is checked first and
    refused with `StoreError::SchemaMismatch { store, module, differing: [entity names] }`, code
    `SCHEMA_MISMATCH`. The differing names come from comparing the two declaration maps.
  - **Bundles.** A bundle still lists the touched declarations (unchanged format), now checked
    against the schema at the evaluated position.
  - **Plain evaluation (no store)** is unaffected: there is no store schema to bind to.
- **Rationale:** FR-005, Q1. It is a minor version bump (0.9.0): a module that differs only in
  untouched types used to commit and now does not.
- **Alternatives considered:** keeping touched types with an added global check — two rules for one
  concept; rejected.

## R3. Schema history in the store (no byte change until a migration)

- **Decision:**
  - **Head.** `Head` gains `schema: Option<SchemaRef>`, omitted when `None`. `None` means the
    genesis schema.
    `SchemaRef { hash, declarations, since: position, migration_record: hash }`.
  - **Migration records** carry the previous schema reference. So the schema at any position is
    found by walking back from the head through migration records only: O(number of migrations),
    and never by scanning ordinary records.
  - **Store API:**
    - `store.schema_at(position)`;
    - `store.schema_history() -> [(since_position, SchemaRef)]`, with the genesis schema at
      position 0.
  - **Content hashing** of entity versions uses the declaration at the version's
    `created_at` position.
- **Rationale:** FR-001, FR-002, FR-018, FR-025.
  - Stores that never migrate keep identical head, genesis and record bytes.
  - Lookups stay cheap.
  - Historical values are always hashed and interpreted under their own schema.
- **Alternatives considered:**
  - **A schema field on every record:** this changes every new record's bytes even without
    migrations.
  - **Scanning all records:** O(history) on every open.

## R4. The migration document (`behavior.migration`, migration IR 0.1)

- **Decision:** A migration is a **separate wire document**. It is not a module item, because a
  module describes one schema, and a migration relates two. Shape:

  ```json
  {"migration_ir": "0.1", "name": "region_required",
   "source": "<SchemaHash A>", "target": "<SchemaHash B>",
   "constants": [{"name": "default_note", "type": {...}, "value": ...}],
   "requirements": [{"name": "every_order_has_region", "body": <closed expr over the source>}],
   "transforms": [{"entity": "Order",
                   "fields": {"region": <expr>, "status": {"op": "enum_map", ...}},
                   "drops": ["legacy_code"]}],
   "retire": ["AuditNote"]}
  ```

  - **Admission is relative to two modules:** `admit_migration(source: &Module, target:
    &Module, wire)`. Their schema hashes must equal `source` and `target`, otherwise
    `MIGRATION_SCHEMA_MISMATCH`.
  - **Resolution** (FR-007a, FR-007b):
    - every unmentioned target field whose source field has the same name and type hash
      becomes `{"copy": "<field>"}`;
    - a missing field is `MISSING_MIGRATION_FIELD`;
    - an unread, removed source field without a drop is `UNACKNOWLEDGED_FIELD_DROP`.
  - **Identity.** The **resolved** document is serialized canonically and hashed with a new tag
    (`behavior.migration.v1`). Names and source locations are metadata, as elsewhere.
- **Rationale:** FR-006–FR-010. Two independent schemas cannot share one module's name table; a
  separate document keeps both modules unchanged.
- **Alternatives considered:**
  - **A `migrations` section inside the target module:** the target module would change identity
    whenever a migration is added.
  - **Pointing at behavior versions instead of schema hashes:** behavior-only edits would then
    invalidate migrations.

## R5. Typing transforms across two schemas

- **Decision:** Migration expressions are type-checked against a **two-sided declaration table**.
  - **Sides.** Every named type (enum, nominal, entity, `Id`) is qualified by side, source or
    target. In the wire format, a named type in a migration may carry `"side": "target"`; the
    default is source.
  - **Scopes.**
    - A transform's scope is `old` (the source entity) plus the constants.
    - Requirements are closed expressions over the source module: queries, derived values and
      literals, with no parameters. They are checked like module invariants.
  - **Assignment.** A transform expression of source type `S` may be assigned to a target field of
    type `T` only if:
    - `S` and `T` have the same type hash (identical meaning), which also covers primitives; or
    - an explicit conversion produces `T`.
  - **Conversion forms:**
    - `enum_map(x, [[src_value, tgt_value]…])` must cover every source value
      (`UNMAPPED_ENUM_VALUE` otherwise);
    - `strict_enum_map(x, …)` may omit values, which makes a narrowing site;
    - `strict_unwrap(x)` turns `Option<T>` into `T`, and is a narrowing site;
    - `wrap(target nominal, x)` and `rescale(x, target nominal, rounding)` work as in behavior,
      with target-side nominals;
    - target-typed literals.
  - **Ids.** An `Id`/`Ref` field may be copied when the target entity name exists in the target
    schema; identities are preserved (FR-008).
- **Rationale:** FR-007, FR-007e, Q2, Q4. A changed enum is a different type even under the same
  name, so types must be qualified by side.
- **Alternatives considered:** implicit enum mapping by value name — rejected by Q2 (no guessed
  meaning).

## R6. Applying a migration (core plus store)

- **Decision:**
  - **Core: `apply_migration(migration, source, target, source_facts) -> MigrationOutcome`.**
    It is pure and works on supplied facts, as plain evaluation does. It proceeds in order:
    0. validate the source universe against the source module (the same validation as step 4, on
       the source side); a failure is `MIGRATION_SOURCE_INVALID`. Constraints and invariants are
       behavior, not schema, so data written under earlier behavior may break them, and the
       verifier's `Valid_A` assumption must be established before the result relies on it;
    1. evaluate every requirement on the source state; a failure is
       `MIGRATION_REQUIREMENT_FAILED`, with the requirement and the count and first ids of the
       violating entities when the requirement is `all_`/`not any_`-shaped (otherwise only the
       name);
    2. transform each entity of each changed type, independently and in canonical id order; a
       failed narrowing is `MIGRATION_TRANSFORM_ERROR`;
    3. build the target universe: transformed entities, plus unchanged types copied as they are;
    4. validate every target constraint and entity invariant per entity, then target referential
       integrity, then every target module invariant (a full evaluation over the universe);
    5. return the new values, the reference changes and a report.
  - **Store: `store.migrate(migration, source_module, target_module, commit_time, evidence)`:**
    1. check that the store's current schema equals the migration's source;
    2. read every entity at the head through `keys_at` and `version_at`;
    3. call `apply_migration`;
    4. write new versions (`revision + 1`, `created_at = position`, content hashed under the target
       declarations), the reference-index changes, the migration record and the head with the new
       `SchemaRef`;
    5. commit with **one compare-and-set** through the unchanged `Backend::commit`.

    Retired types must have no entities (`RETIRED_TYPE_NOT_EMPTY`).
- **Rationale:** FR-011–FR-014, FR-019, Q3 (entity-local, so the order is irrelevant), Q5 (runtime
  validity is unconditional). Plain-mode `apply_migration` also confirms verifier counterexamples
  and backs the CLI.
- **Alternatives considered:** streaming migration across several commits — rejected, because the
  migration must be atomic (FR-011).

## R7. The migration transition record

- **Decision:**
  - **A second record kind.** `TransitionRecord` gains `kind: Option<"migration">`, omitted for
    actions, so action records keep their bytes. For a migration, `bundle` is replaced by
    `migration: MigrationBundle`:
    - `migration_hash`;
    - source and target `SchemaRef`;
    - `requirements` (name and outcome);
    - `report`, the reviewable summary (FR-007c);
    - `verification`, an optional cited attestation hash and outcome;
    - `previous_schema`.

    `new_versions` lists every migrated version, `ref_changes` the index changes. The evidence
    fields are the same as for actions.
  - **Format version.** The record keeps tag `behavior.transition_record.v1`, and the `kind`
    field is the discriminator. Readers that predate this feature never see migration records on
    stores they wrote.
  - **Size.** A record is proportional to the migrated entities: about 200 bytes each, so about
    20 MB for 100,000. This is accepted, as for queries in 007.
- **Rationale:** FR-015, FR-019c. The record is self-describing for audit, and data replay can
  recompute state identities from it.
- **Alternatives considered:**
  - **A new record tag:** this splits every reader into two code paths for little gain.
  - **Hashes only, without values:** data replay could not recompute entity content without the
    store.

## R8. Evidence per transition kind

- **Decision:**
  - **Policy.** `EvidencePolicy` gains an optional `migration: { require,
    trusted_execution_policies? }`, omitted when absent. The default (absent) means "as for
    actions", so existing policies keep their bytes and hashes.
  - **Commit authorization for a migration.** `governance::authorize_migration(policy,
    migration, source, target, migration record, attestation?, waivers, signatures, now)` binds
    an authorization to the migration transition, the same way `authorize` binds one to an action
    transition. An execution policy can therefore require a verified migration attestation, and
    the store's evidence policy requires that authorization for migrations.
- **Rationale:** FR-019a–c, Q5. One mechanism, which can differ per kind.
- **Alternatives considered:** a hard engine rule "migrations need proofs" — rejected by Q5.

## R9. Verifying migrations

- **Decision:** `behavior_verify::verify_migration(migration, source, target, profile) ->
  Attestation`. The attestation's subject is the migration hash instead of a behavior version,
  and `VERIFIER_VERSION` becomes 0.5.0.
  - **Local check, per changed type T.** A symbolic source entity `e` is assumed to satisfy:
    - the source constraints and invariants of T;
    - membership of `e` in `select(T)` on a symbolic source state S;
    - every requirement on S, using 007's relational summaries, with `e` as a known candidate.
      So `all_(select(T), p)` yields `p(e)` exactly, as in 007.

    It then proves:
    - (a) every narrowing site succeeds;
    - (b) every target constraint and entity invariant of T holds for `transform(e)`;
    - (c) no evaluation error occurs.

    Each is a check with the usual outcomes. Counterexamples are confirmed by running
    `apply_migration` on a one-entity source universe, with witness slots as in 007.
  - **Whole-state check.**
    - **Referential integrity** is **proven** if every reference field of a migrated type is
      copied or mapped to the same target entity name and no type is retired. Otherwise it is
      **inconclusive**.
    - **Target module invariants** mentioning migrated types are **inconclusive** unless the
      invariant and every field it reads are identical copies (proven). This is precision debt;
      application checks them in full (FR-021).
  - **Requirements are named.** Results that rely on requirements carry
    `"under": [requirement names]`.
  - **Source rules are assumed, then established.** The local check assumes the source module's
    constraints and invariants. The store does not guarantee them for data written under earlier
    behavior of the same schema, so `apply_migration` validates the source state first (R6
    step 0), and a proof is never relied on for data it does not cover.
- **Rationale:** FR-020, FR-021, Q4, Q5. It reuses the 007 machinery for requirements. The
  local/whole split is the spec's two-part contract.
- **Alternatives considered:** symbolic whole-state migration — this is set-to-set reasoning about
  transformed predicates, beyond the current model, so inconclusive is honest.

## R10. Replay across generations

- **Decision:**
  - **Data replay**, at each migration record:
    - recompute every migrated version's content hash under the target declarations and the
      state identity;
    - check that the record's `previous_schema` matches the walked schema;
    - continue with the target schema.
  - **Behavior replay** takes `modules` (behavior versions mapped to modules, as today) **and**
    `migrations` (migration hashes mapped to `(migration, source, target)`). At a migration record
    it:
    1. reconstructs the source universe at the parent (as `rederive` does);
    2. re-runs `apply_migration`;
    3. compares the requirements' outcomes, the new versions and the reference changes.

    A divergence is reported at that position as `decision`.
- **Rationale:** FR-016, FR-017.
- **Alternatives considered:** none simpler; trusting recorded versions would break the replay
  guarantee.

## R11. Python, CLI and skills

- **Decision:**
  - **Python:**
    - `Migration(source=model_v1, target=model_v2, requires={name: lambda: ...},
      transforms={OrderV1: lambda old: {...}}, drops={...}, retire=[...], constants={...})`;
    - helpers `enum_map`, `strict_enum_map`, `strict_unwrap` (plus the existing `rescale` and
      nominal constructors, resolved to the target side when a target class is used);
    - `m.admit()`, `verify_migration(m)`, `m.summary()` (FR-007c);
    - `store.migrate(m, commit_time=…, evidence=…)`, `store.schema_history()`;
    - `model.schema_hash`.
  - **CLI.** The CLI has no persistent store, so "apply" is plain-mode, over a supplied source
    universe:
    - `behavior schema-hash <wire>`;
    - `behavior migration admit <source> <target> <migration>` (prints the resolved summary);
    - `behavior migration verify …`;
    - `behavior migration apply … <facts>`.

    Schema history is a store API, not a CLI command (FR-023).
  - **Skills (FR-024):**
    - authoring gets "Changing a schema", covering what needs a migration, writing one, the
      staged path and requirements;
    - verification gets migration outcomes;
    - application gets applying migrations, schema mismatch handling and backfills.

    All with runnable examples under the existing skill checks.
- **Rationale:** FR-022–FR-024.

## R12. Versions and compatibility

- **Decision:**
  - **Release.** 0.9.0, a minor bump: the binding rule changed (R2) and there is a new
    document kind.
  - **Version numbers.** Wire IR stays 0.6, since modules are unchanged. Migration IR starts at
    0.1. Records keep tag `.v1`, with optional `kind`. `VERIFIER_VERSION` becomes 0.5.0.
  - **Byte stability.** Every existing module, record, genesis, head (of a non-migrated store),
    evidence policy and golden keeps its bytes. The frozen identity tests run unchanged.
  - **New fixtures:**
    - `tests/fixtures/migration/` (valid and invalid migration documents with expected errors);
    - `tests/fixtures/verify/migration_*.expected.json`;
    - a fixture history crossing two migrations.
- **Rationale:** FR-025, SC-007, `docs/versioning.md`.
