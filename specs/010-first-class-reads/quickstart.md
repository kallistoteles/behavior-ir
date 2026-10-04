# Quickstart: First-Class Reads

A validation guide. References: [contracts/read-api.md](contracts/read-api.md),
[contracts/read-wire.md](contracts/read-wire.md), [data-model.md](data-model.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace && pytest python/tests && mypy
scripts/determinism-check.sh          # + read fixtures and the lab_reads example, twice
scripts/release-check.sh              # + read skill examples in the clean environment
cargo test --release --workspace -- --ignored   # + the 10,000 × 5 projection (SC-004)
```

## 2. Read the current state (US1)

Run `python -m examples.lab_reads.run`. It shows:
- a count of active cultures and an open-order total read from a store, with no bound placeholder
  entity;
- the store's head, records and state identity are byte-identical before and after;
- a read with a wrong input type gives `INVALID_INPUT` listing every problem;
- a division by zero gives `EVALUATION_ERROR` with its location;
- a binding to an unknown identity is refused before evaluation.

## 3. Projections (US2)

- A query projection of `[name, stage, age]` gives the matching cultures sorted by `id`, each
  with exactly those keys. A culture without a `stage` shows `"stage": null`. The record's facts
  hold a field fact for every projected stored field of every member.
- An entity projection of one order gives one dict, and an unknown order id is refused.
- A derived item that divides by zero for one member gives an `EVALUATION_ERROR` naming `Order#…`
  and the item, with no partial list.
- Admission fixtures in `tests/fixtures/reads/invalid/` give:
  - `UNKNOWN_PROJECTION_ITEM` for `customer.name`;
  - `DUPLICATE_PROJECTION_ITEM`;
  - `READ_CALL_NOT_ALLOWED` for an action that calls a read;
  - `DUPLICATE_CAPABILITY`.

## 4. The past (US3)

- Read at position 4, commit five more transitions, then read at position 4 again: the records
  are byte-identical.
- On the `schema_evolution` store, a read at a pre-migration position with the V1 module
  succeeds. With the V2 module it gets `SCHEMA_MISMATCH`.

## 5. Evidence (US4)

```bash
behavior read tests/fixtures/reads/lab.json tests/fixtures/reads/requests/open_total.json > rec.json
behavior read-replay tests/fixtures/reads/lab.json rec.json          # exit 0
# edit rec.json: change "value" or one fact, then:
behavior read-replay tests/fixtures/reads/lab.json rec.json          # exit 1, first differing path
```

In Python, `store.replay_read(model, record)` matches. A record with a forged `data_version`
or `record_id` does not.

## 6. Capabilities (US5)

- `behavior read-intent lab.json intent.json host.json --record rec.json` prints only `result`,
  `value` and `record_id`. `rec.json` holds the facts, including the internal field a derived
  value read.
- The intent fixtures in `tests/fixtures/read_intents/` (unknown read, an action name, missing or
  extra targets and inputs, a wrong type, an inline definition) are each rejected with every
  problem listed.
- Adding a declared read to a module changes `behavior_version` but not `schema_hash`, so a store
  keeps evaluating with no migration.

## 7. Verification

`behavior verify` on the lab module reports `evaluation_error` for `read:average_ph` (a division
by the member count), with a confirmed counterexample: a state with no measurements. The fixed
read verifies. `VERIFIER_VERSION` is `0.6.0`, and the existing verify fixtures are unchanged.
