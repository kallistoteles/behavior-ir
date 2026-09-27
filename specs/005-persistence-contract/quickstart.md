# Quickstart: Persistence Contract

Validation guide for feature 005. References: [contracts/store-api.md](contracts/store-api.md),
[data-model.md](data-model.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace            # includes the conformance suite and the SC-001/SC-002 properties
pytest python/tests && mypy
scripts/determinism-check.sh      # extended with store histories (SC-005)
cargo test --release -p behavior-store --test perf -- --ignored     # SC-004
cargo test --release -p behavior-store --test replay -- --ignored   # SC-002 at 10,000 transitions
```

## 2. Guarded commit (US1)

Using `examples/ledger/` (accounts, `transfer`; Money scale 2):

- `python -m examples.ledger.run` shows the following:
  - The genesis is S0 (position 0).
  - One transfer is committed, giving S1 with incremented revisions for both accounts.
  - Two transfers are evaluated against S1, and the second commit raises `StateConflict` naming
    the current state and changed accounts.
  - The retried transfer, re-evaluated on S2, commits.
- A denied transfer (insufficient balance) has `bundle is None`.

## 3. Replay (US2)

- `replay_data(store)` and `replay_behavior(store, [model])` over the example history report
  `ok` with the checked count.
- The property tests in `crates/behavior-store/tests/replay.rs` tamper with one field per case,
  and each tamper is reported at the first affected position with its kind.

## 4. Conformance (US3)

- `cargo test -p behavior-store --test conformance`: the reference backend passes every case, and
  each broken backend fails its designated case.
- `pytest python/tests/test_store_conformance.py`: the same cases run against a Python dict backend.
- **The review's correctness cases** are named cases in the suite:
  - `snapshot_consistency`: commits interleaved between the reads of one evaluation never mix
    states;
  - `crash_retry`: a crash before or after the durable write shows all or nothing, and a retry
    with a new commit time is `already`;
  - `store_binding`: evidence from an identical-content store is refused;
  - `derived_dependencies`: altered read or write sets are refused.

## 5. Evidence (US4)

- The example store with `require: "commit_authorization"`:
  - a commit without an authorization raises `CommitRefused(EVIDENCE_REQUIRED)`;
  - with an authorization from `authorize(...)` for the record, the commit succeeds, and the
    record cites the evidence-policy and authorization hashes;
  - an authorization for another record raises `CommitRefused(EVIDENCE_MISMATCH)`.
