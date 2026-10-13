# Quickstart: validating feature 502

How to check RequestFlow end to end once it is implemented. Semantics are in
[contracts/](contracts/), and data shapes in [data-model.md](data-model.md).

## Prerequisites

- Branch `502-request-flow` (stacked on 501), `nix develop`, `scripts/fetch-core.sh`,
  `maturin develop`.
- No local Core override.

## 1. All gates

```sh
scripts/gates.sh
```

Expected: `gates: OK`. `models compat` reports both tuples (state_machine and request_flow),
and the 501 entry is unchanged.

## 2. Rust laws and rules

```sh
cargo test -p behavior-models
```

Expected: the 501 suites pass unchanged, and the new `request_flow_*` suites pass:
`EncodeEntityName` property, staging laws D1–D6, ownership matrix, StateMachine equivalence,
permutation properties.

## 3. End-to-end through the public engine

```sh
python -m pytest -q python/tests/test_request_flow*.py
```

Expected:
- the reference lifecycle commits step by step with ordinary Core outcomes;
- bypass attempts give the exact build errors;
- guards, commands and context time behave as in contract §2;
- hand-written behavior referencing the request entity works;
- the seeded trace property holds over ≥ 1,000 traces.

## 4. StateMachine regression

```sh
python3 scripts/check-models-compat.py && cargo test -p behavior-models --test request_flow_staging
```

Expected: every 501 reference case reproduces its pinned BehaviorHash under the new protocol.

## 5. No Core change

```sh
scripts/check-core-pin.sh
```

Expected: Core 0.12.0 is unchanged, and no edits were made in the Core repository by this
feature.
