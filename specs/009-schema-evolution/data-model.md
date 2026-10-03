# Data Model: Schema Evolution and Migration

## Store schema

| Field | Description | Rule |
|---|---|---|
| `hash` | `SchemaHash`, tag `behavior.store_schema.v1` | `H(sorted [(entity name, entity declaration hash)])` |
| `declarations` | entity name → declaration hash | exactly the persisted entity types; enums and nominals enter through the declaration hashes |

**Derivation:** a module's schema comes from its entity declarations. A store's genesis schema
comes from `genesis.entity_declarations`.

## SchemaRef (in Head and in migration records)

| Field | Rule |
|---|---|
| `hash`, `declarations` | the store schema in force |
| `since` | the position from which it applies (the migration's position) |
| `migration_record` | the hash of the record that introduced it |

`Head.schema` is absent while the store is under its genesis schema.

## Migration (document `behavior.migration`, IR 0.1)

| Field | Rule |
|---|---|
| `source`, `target` | SchemaHashes; admission requires the given modules to match them |
| `constants` | named, typed literals; usable in transforms |
| `requirements` | named, closed Bool expressions over the **source** state (queries, derived values, literals); no inputs, context, time or target values |
| `transforms[entity]` | for every entity type whose declaration differs and exists in both schemas: `fields` (target field → expression, or `copy`), and `drops` (removed source fields not read by any assignment). A changed type the author does not mention gets a transform built entirely by resolution |
| `retire` | source entity types absent from the target; they must be empty when applied |
| identity | the canonical hash of the **resolved** document (tag `behavior.migration.v1`) |

**Resolution rules (FR-007a, FR-007b), for each target field:**
1. an explicit expression, if given;
2. otherwise, if the source has the same field name with an equal type hash, a `Copy`;
3. otherwise, `MISSING_MIGRATION_FIELD`.

For each removed source field: if no assignment reads it, an explicit drop is required, otherwise
`UNACKNOWLEDGED_FIELD_DROP`.

**Narrowing sites:**
- `strict_unwrap`;
- `strict_enum_map`;
- a conversion into a more restricted target type.

Each is a verification obligation and a runtime check.

## Migration transition (record kind `migration`)

| Field | Content |
|---|---|
| `kind` | `"migration"` (absent means an action record) |
| `migration` | `MigrationBundle {migration_hash, source (SchemaHash), target (SchemaHash), target_declarations, previous_schema? (absent: the genesis schema), requirements: [{name, held}], report, source_validated, target_validated, verification?: {attestation_hash, result}, commit_time}`; the record's `bundle_hash` is its hash (`behavior.migration_bundle.v1`) |
| `new_versions` | every migrated entity at `revision + 1`, `created_at = position`, content hashed under the target declarations |
| `ref_changes` | incoming-reference index changes, e.g. when a reference field is renamed |
| `evidence_policy`, `authorization` | as for action records |
| `evaluated_against`, `committed_on`, `result_state`, `previous_record` | as for action records |

**Report (FR-007c):** for each migrated type, its fields classified as `copied`, `transformed`,
`new` or `dropped`, plus the number of entities migrated.

## Evidence policy (extension)

| Field | Rule |
|---|---|
| `migration` (optional) | `{require, trusted_execution_policies?}`. Absent means the same as actions. Existing policies keep their bytes and hashes |

## State transitions of a store

```text
schema A ──(actions under A)──▶ … ──(migration A→B, one position)──▶ schema B ──(actions under B)──▶ …
```

- **Invariant:** at every position exactly one store schema is in force. Evaluation and commit
  require `module.schema_hash == schema_at(position).hash` (`SCHEMA_MISMATCH` otherwise).
- **Migration preconditions**, checked in this order:
  1. the current schema equals `source`;
  2. the source state is valid under the source module's rules;
  3. the requirements hold;
  4. the transforms succeed;
  5. the target state is valid.

  Only then is the migration committed, as one compare-and-set.

## Errors

| Code | When |
|---|---|
| `SCHEMA_MISMATCH` | a module's schema differs from the store's at the evaluated position |
| `MIGRATION_SCHEMA_MISMATCH` | a migration is admitted or applied with modules or a store whose schema is not its source or target, or its source and target are equal |
| `MISSING_MIGRATION_FIELD`, `UNACKNOWLEDGED_FIELD_DROP`, `UNMAPPED_ENUM_VALUE` | admission |
| `MISSING_RETIREMENT`, `INVALID_RETIREMENT` | admission: a type the target no longer declares is not retired; a retirement of a type that is not removed |
| `INVALID_TRANSFORM` | admission: a transform of an unknown or unchanged type, an unknown target field, a drop of a field the target still declares |
| `NON_LOCAL_TRANSFORM` | admission: a transform reads other entities (a query, `exists`, `referenced`) |
| `MIGRATION_TYPE_MISMATCH` | admission: an assignment between different type hashes without an explicit conversion, including a lossy conversion without explicit rounding |
| `MIGRATION_SOURCE_INVALID` | application: the source state breaks a source-module rule (constraint, entity invariant, module invariant) that verification assumes |
| `MIGRATION_REQUIREMENT_FAILED` | application: a requirement does not hold for the source state |
| `MIGRATION_TRANSFORM_ERROR` | application: a narrowing fails, or an evaluation error occurs in a transform |
| `MIGRATION_INVALID_RESULT` | application: the target state breaks a constraint, an invariant, a module invariant or referential integrity (with the rule and the entities) |
| `RETIRED_TYPE_NOT_EMPTY` | application: a retired type still has entities |
