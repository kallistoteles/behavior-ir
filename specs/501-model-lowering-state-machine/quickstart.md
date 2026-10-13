# Quickstart: validating feature 501

How to check that the feature works end to end once it is implemented. Semantics are in
[contracts/](contracts/), and the data shapes are in [data-model.md](data-model.md).

## Prerequisites

- The repository on branch `501-model-lowering-state-machine`, with the pinned Core Release
  installed: `scripts/fetch-core.sh`.
- No active local Core override (constitution: Bindings and Packaging).
- The binding built: `maturin develop`.

## 1. All gates

```sh
scripts/gates.sh
```

Expected: `gates: OK`, including the new `models compat` gate and the extended `public
surface` and `ownership` gates.

## 2. Rust laws

```sh
cargo test -p behavior-models
```

Expected: the law suites L1–L9 (lowering contract §2) pass, with ≥ 1,000 property cases per
permutation law.

## 3. Reference machine through the public engine

```sh
python -m pytest -q python/tests/test_state_machine.py
```

Expected:
- the `purchase_request` transition matrix: commit exactly when the state is in the
  transition's source set and its guards hold; otherwise Core's ordinary denial or evaluation
  error (state-machine contract §2);
- the admitted module contains only declared items plus `purchase_request__*` actions;
- each receipt has `model_hash`, both versions, `core_release` and `behavior_hash`, and its
  source map is total.

## 4. Ownership and composition negatives

Covered by the same suite. Each case in composition contract §6 yields exactly the listed
result, identically for every model supply order.

## 5. Worked example equivalence (FR-036)

```sh
python -m pytest -q models/examples/state_machine
```

Expected: the `order` reference input lowers to `order__submit`, whose Core action hash equals
the hand-written three-statement equivalent. `lower.py` no longer exists.

## 6. Version immutability (SC-009)

```sh
scripts/tests/test_models_compat.sh
```

Expected: editing an expected hash in `models/fixtures/compat.json` without bumping a version
fails the gate. The same output change together with a `LOWERER_VERSION` bump and a new tuple
passes.

## 7. Determinism

```sh
scripts/determinism-check.sh
```

Expected: the model receipts of all reference cases are byte-identical across two runs.

## 8. No Core change

```sh
git -C ../behavior-ir-core status --short   # unchanged by this feature
scripts/check-core-pin.sh
```

Expected: the pin is unchanged at Core Release 0.12.0, and there are no edits in the Core
repository.
