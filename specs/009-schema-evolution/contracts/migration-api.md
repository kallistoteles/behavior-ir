# Contract: Migration API

## Python

```python
from behavior import Migration, enum_map, strict_enum_map, strict_unwrap, all_, select

m = Migration(
    source=model_v1, target=model_v2,
    requires={"every_order_has_region": lambda: all_(select(V1.Order), lambda o: o.region.is_some())},
    transforms={
        V1.Order: lambda old: {"region": strict_unwrap(old.region)},  # other fields: copied
        V1.Culture: lambda old: {
            "medium_type": enum_map(old.medium, {V1.Medium.MS: V2.Medium.MS,
                                                 V1.Medium.WPM: V2.Medium.WPM}),
            "notes": None,
        },
    },
    drops={V1.Culture: ["legacy_code"]},
    retire=[V1.AuditNote],
)
```

| Call | Result |
|---|---|
| `m.admit()` | `MigrationAdmission(ok, errors, hash, summary)`; `summary[type] = {copied, transformed, new, dropped}` |
| `behavior.verify_migration(m, profile=None)` | `Attestation` whose checks include the local ones per type and field, plus `referential_integrity` and `module_invariant` |
| `model.schema_hash` | `"sha256:…"` |
| `store.migrate(m, commit_time=…, evidence=None)` | `CommitResult`; raises `CommitRefused` with `MIGRATION_*`, `RETIRED_TYPE_NOT_EMPTY`, `SCHEMA_MISMATCH`, or `StateConflict` |
| `store.schema_at(state_ref)`, `store.schema_history()` | `SchemaRef(hash, declarations, since, migration_record)`, `[SchemaRef]` oldest first |
| `store.data_version()` | what a migration authorization binds |
| `authorize_migration(m, store, policy=…, attestation=…, now=…)` | `Authorization` bound to the migration and the store state |
| `replay_behavior(store, models, migrations=[m, …])` | `ReplayReport` across schema generations |
| `behavior.apply_migration(m, entities)` | plain mode over a supplied source universe: `{result: "MIGRATED", entities, requirements, report}` or `{result: <refusal code>, message, rule, entities, count}` |
| evaluation under a mismatched module | `CommitRefused("SCHEMA_MISMATCH")` from `store.evaluate`, before any decision |

## Rust

- `behavior_core::schema(&Module) -> StoreSchema`
- `behavior_core::admit_migration(&Module, &Module, &str) -> Result<Migration, AdmissionResult>`
- `behavior_core::apply_migration(&Migration, &Module, &Module, &dyn EvaluationFacts) -> MigrationOutcome`
- `Store::migrate(&mut self, &Migration, &Module, &Module, commit_time, evidence) -> R<Committed>`
- `Store::schema_at(&StateRef)`, `Store::schema_history()`
- `replay::replay_behavior(store, modules, migrations, from, to)`. Migrations are keyed by hash;
  the old signature remains for stores without migrations.
- `behavior_verify::verify_migration(&Migration, &Module, &Module, &Profile, cache, solver)`
- `governance::authorize_migration(...)`

## CLI

| Command | Output, exit code |
|---|---|
| `behavior schema-hash <wire>` | the SchemaHash; 0 |
| `behavior migration admit <source> <target> <migration>` | the resolved summary and hash; 0, or 2 with errors |
| `behavior migration verify <source> <target> <migration>` | the attestation; 0 if verified, 1 if not, 3 if the solver is missing |
| `behavior migration apply <source> <target> <migration> <facts>` | the migrated universe and report; 0, or 1 when refused (requirement, transform or result) |

## Evidence policy

```json
{"format": "behavior.evidence_policy.v1", "require": "none",
 "migration": {"require": "commit_authorization", "trusted_execution_policies": ["sha256:…"]}}
```

`migration` is optional. When absent, migrations need the same evidence as actions.
