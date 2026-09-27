# Quickstart: Exact Arithmetic Closure

Validation guide for feature 004. Contracts: [numeric-semantics.md](contracts/numeric-semantics.md),
[engine-api.md](contracts/engine-api.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace            # includes the SC-001 proptest (≥ 10,000 cases)
pytest python/tests
mypy
scripts/determinism-check.sh
cargo test --release -p behavior-verify --test perf -- --ignored
```

## 2. Bounded exact domain (US1)

- `behavior admit tests/fixtures/wire/invalid/exact_bound_exceeded.json` → `EXACT_BOUND_EXCEEDED`
  at the long division chain, stating required and supported bits.
- Every other valid fixture admits.

## 3. Ratios (US2)

`behavior eval tests/fixtures/wire/valid/exact_closure.json tests/fixtures/requests/004/<case>.json`:

- `share` (`10.00 / 30.00`) → trace shows `1/3`.
- `scaled_share` → `rescale((amount / budget) * total, Money, half_even)` rounds once.
- `share_threshold` (`amount / budget <= 0.25`) → exact comparison, no rescale entry.
- Admission of `wire/invalid/ratio_stored.json` → `LOSSY_CONVERSION`;
  `wire/invalid/exchange_rate.json` (`SEK / JPY`) → `TYPE_MISMATCH`.

## 4. General decimals (US4)

- `behavior eval tests/fixtures/wire/valid/project_margin.json …` → `margin` is `0.67` / `1/3`,
  decisions unchanged in outcome.
- `wire/invalid/decimal_sum_stored.json` (`dec := x - y`) → `LOSSY_CONVERSION`;
  a constant store `10 / 2` admits.
- `behavior admit` of a `"0.3"` document → `UNSUPPORTED_VERSION` with the migration message.

## 5. Verification (US3, SC-007)

- `behavior verify tests/fixtures/verify/share_bound.json` → the share property is proven; the
  variant without `amount <= budget` has a confirmed counterexample.
- The feature-002 rounding fixtures are decided under exact semantics (no inconclusive check
  caused by decimal arithmetic).
