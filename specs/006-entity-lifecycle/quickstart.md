# Quickstart: Entity Lifecycle

Validation guide for feature 006. References: [contracts/lifecycle-api.md](contracts/lifecycle-api.md),
[data-model.md](data-model.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace
pytest python/tests && mypy
scripts/determinism-check.sh                                          # + lifecycle histories
cargo test --release -p behavior-store -- --ignored                    # SC-002/003/004/007
cargo test --release -p behavior-verify --test perf -- --ignored
```

## 2. Create and remove (US1, US2)

`python -m examples.accounts.run`:

1. Open an account for customer `c1` with host-supplied identity `a42`. It is committed at revision
   1.
2. Opening `a42` again gives `ENTITY_ID_ALREADY_USED`.
3. `remove_customer(c1)` while `a42` references `c1`: denied by its guard `not referenced(c1)`.
   (The unguarded form is refused with `DANGLING_REFERENCE`, see §3 and
   `tests/fixtures/requests/006/remove_customer_unchecked_dangling.json`.)
4. Close `a42` at balance 0. It is absent from the current state, and loading it at the previous
   state returns its last version.
5. Opening `a42` again after the removal still gives `ENTITY_ID_ALREADY_USED`.
6. `remove_customer(c1)` now succeeds; binding `c1` afterwards is `ENTITY_NOT_FOUND`.

## 3. Verification (US3)

`behavior verify tests/fixtures/wire/valid/accounts.json` (expectations:
`tests/fixtures/verify/lifecycle.expected.json`):

- `open_account_unchecked` (no `initial >= 0`): a constraint counterexample, reproduced by the
  runtime with a `facts` section.
- `remove_customer_unchecked` (no `not referenced`): a `referential_integrity` counterexample.
- The guarded versions are proven.
- `switch_and_remove` has a real `referential_integrity` counterexample: an account not bound by
  the action may still reference the removed customer.

## 4. History (US4)

- `replay_data` and `replay_behavior` over a mixed history report `ok`.
- The tamper tests in `crates/behavior-store/tests/lifecycle_replay.rs` find altered creations,
  removals and reference changes at their positions.
- The conformance suite includes the 7 new lifecycle cases; the reference backend and the Python
  dict backend pass.
