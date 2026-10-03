# Quickstart: Schema Evolution and Migration

A validation guide. References: [contracts/migration-api.md](contracts/migration-api.md),
[contracts/migration-wire.md](contracts/migration-wire.md), [data-model.md](data-model.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace && pytest python/tests && mypy
scripts/determinism-check.sh          # + the migration history fixture and the migration example
scripts/release-check.sh              # + migration skill examples in the clean environment
cargo test --release --workspace -- --ignored   # + the 100,000-entity migration (SC-005)
```

## 2. Schema binding (US5)

- `behavior schema-hash` gives equal hashes for two modules that differ only in actions, and
  different hashes once an enum gains a value.
- On a store, a behavior-only change evaluates. A module that differs in any entity type, even
  one the action does not touch, gets `SCHEMA_MISMATCH` before any decision.

## 3. Migrate in place (US1, US2)

Run `python -m examples.schema_evolution.run`. It goes through these steps:

1. Create a store under V1 (cultures with `MediumKind {MS, WPM}`, orders with an optional region),
   then commit transitions.
2. **Broaden:** apply V1→V2, which adds `B5`, renames `medium` to `medium_type`, adds an optional
   `notes` field, changes `ph` from `Int` to `Decimal(1)` and drops `legacy_code`. The summary shows the copied, transformed, new and
   dropped fields.
3. **Narrow too early:** apply V2→V3 (`region` becomes required, guarded by the requirement
   "every order has a region"). It is refused with `MIGRATION_REQUIREMENT_FAILED`, naming how
   many orders lack a region. The store is unchanged.
4. **Backfill:** run an ordinary V2 action that fills each order's region.
5. **Narrow:** apply V2→V3 again; this time it commits.
6. **Check:** V3 actions commit, and V1 and V2 actions get `SCHEMA_MISMATCH`.
   `store.schema_history()` lists V1 at 0, V2 at the first migration and V3 at the second, and
   old states load exactly as written.

## 4. Replay and verification (US3, US4)

- `replay_data` and `replay_behavior` (given the modules and migrations) report no divergence from
  genesis to head. A tampered migrated value or migration record is found at its position.
- `behavior migration verify` on the fixtures:
  - correct migrations are proven, under their named requirements;
  - a migration mapping a value onto a constraint violation gives a confirmed counterexample;
  - target module invariants over migrated types are inconclusive (precision debt), and are
    checked in full at application.
