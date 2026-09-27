# Quickstart: Fixed-Scale Decimals

Validation guide for feature 003. Contracts:
[numeric-semantics.md](contracts/numeric-semantics.md), [engine-api.md](contracts/engine-api.md).

## Prerequisites

`nix develop`, then `maturin develop`.

## 1. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test --workspace            # includes rounding property tests and 001/002 regressions
pytest python/tests
mypy
scripts/determinism-check.sh      # also covers fixed-scale fixtures
```

Expected: all 001/002 behavior versions, hash vectors, records, and attestation outcomes are
unchanged (SC-004); attestation hashes change only through the verifier version (0.3.0).

## 2. Grid and range on input (US1)

```bash
behavior eval tests/fixtures/wire/valid/fixed_scale.json tests/fixtures/requests/003/<case>.json
```

- `amount = "100.505"` → `INVALID_INPUT`, reason `OFF_GRID` naming the field and scale 2.
- `amount = "100.5"` → accepted; the record shows `"100.50"` (`record_version "0.3"`).
- Two amounts near the range limit added → `ERROR` `numeric overflow in …`, never a rounded value.

## 3. Rescale (US2)

- `rescale((amount / 3) * 2, Money, half_even)` with `amount = 20.00` → `13.33`; the trace step
  carries `{"exact": "40/3", "rounding": "half_even", "result": "13.33"}`.
- The nested variant → `13.34` with two rescale entries.
- `amount * 1.25 <= budget` evaluates without any rescale entry.
- Admission of `set_(invoice.fee, invoice.amount * Decimal("0.25"))` fails with
  `LOSSY_CONVERSION` naming `rescale`; so does `Money(decimal_expr)`.
- The six modes reproduce `tests/fixtures/numeric/rounding.json` in Rust and Python.

## 4. Verification (US3)

```bash
behavior verify tests/fixtures/verify/purchase_money2_remaining.json; echo "exit=$?"
```

Expected: exit 0; `within_budget` is `proven` for `approve` with the precondition
`amount <= remaining(project)` (inconclusive in feature 002 with general decimals, SC-001).
`purchase_money2.json` (no budget precondition): exit 1, counterexample amounts with exactly two
decimals, confirmed `DENY`. The example modules with two-decimal Money have no inconclusive
finding caused by money arithmetic (SC-002), and verify within 10% or 1 s of the general-decimal
versions (SC-005).

## 5. Compatibility

- `behavior admit` of every 001/002 wire fixture prints the same behavior version as before.
- `model.to_wire_json()` of a module without fixed-scale types still writes `ir_version "0.1"` /
  `"0.2"`.
