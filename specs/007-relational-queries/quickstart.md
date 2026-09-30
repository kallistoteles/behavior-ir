# Quickstart: Relational Queries

Validation guide for feature 007. References: [contracts/query-api.md](contracts/query-api.md),
[data-model.md](data-model.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace
pytest python/tests && mypy
scripts/determinism-check.sh                          # + query histories, order permutations
cargo test --release --workspace -- --ignored         # SC-002/003/004/007/008
```

## 2. Decide over sets (US1, US2)

`python -m examples.orders.run`:

1. `close_customer(c1)` with only closed orders: allowed; the record shows the query instance "open
   orders of c1" with no members.
2. After `place_order` for c1: `close_customer(c1)` is denied; the recorded members are exactly that
   order.
3. `sum_` of order amounts is exact; `min_`/`max_` of an empty set are absent.
4. Creating an employee with an existing personnel number is denied by the module invariant.

## 3. Resulting state (US3)

- `place_order`'s postcondition sees the created order in `sum_` and `count`.
- A test action that changes a captured value (a customer's credit limit) records a second query
  instance and derives its resulting-state result.

## 4. Replay and verification (US4)

- Records with query facts replay without a store; tampering with a recorded member or field value
  is found at its position.
- `behavior verify tests/fixtures/wire/valid/orders.json` reports the proven, counterexample and
  inconclusive cases of `tests/fixtures/verify/queries.expected.json` (SC-005).
- The conformance suite includes the new query cases for the reference and Python dict backends.
