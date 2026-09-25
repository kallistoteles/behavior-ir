# Quickstart: Verifiable Behavior IR Core (v0.1)

Validation guide proving the feature works end to end. API details:
[contracts/python-api.md](contracts/python-api.md), [contracts/engine-api.md](contracts/engine-api.md),
[contracts/hashing.md](contracts/hashing.md).

## Prerequisites

Nix with flakes (the dev shell provides the pinned Rust toolchain, Python 3.13, maturin,
pytest, mypy).

```bash
nix develop
maturin develop          # builds behavior._engine into the dev shell's venv
```

## 1. Automated gates (all must pass)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace            # includes hash vectors, hash property tests, typing cases
pytest python/tests               # DSL errors at the author's line come from the engine builder
scripts/determinism-check.sh      # admission, golden and replay cases run twice, byte comparison
cargo test --release -p behavior-core -- --ignored perf   # SC-005 performance check
```

## 2. Invoice approval (US1 + US2)

`python -m examples.invoice.run` (model from the python-api contract).

| Case | Setup | Result | Record shows |
|------|-------|--------|--------------|
| allowed | amount 43200, actor manager, limit 50000 | `ALLOW` | ΔS: status → approved, approved_by → anna |
| above limit | amount 60000 | `DENY` | third precondition false with both values, no changes |
| wrong role | role `"clerk"` | `DENY` | second precondition false, third `skipped` |
| exact boundary | amount `Decimal("50000.00")`, limit 50000 | `ALLOW` | exact comparison |
| invalid start | amount `-1` | `INVALID_STATE` | `invariant_pre` false, no precondition evaluated |
| float input | amount `43200.0` | `TypeError` before the engine | – |

Running twice prints byte-identical records and the same `behavior_version`.

## 3. Construction-time errors (US1)

Each raises at the author's line when defined or compiled:

| Code in a body | Error |
|----------------|-------|
| `invoice.amount + invoice.status` | `BehaviorTypeError` |
| `invoice.amount + Decimal("10")` (Money + Decimal) | `BehaviorTypeError` |
| `invoice.approved_by == project.id` (Id[User] vs Id[Project]) | `BehaviorTypeError` |
| `invoice.approved_by <= actor.id` (Option used as value) | `BehaviorTypeError` |
| `if invoice.amount > Money(Decimal("100")):` | `BehaviorDefinitionError` |
| `set_(actor.role, "x")` (context is read-only) | `BehaviorDefinitionError` |
| `field(float)` | `BehaviorDefinitionError` |

## 4. Engine admission of untrusted wire IR (US1)

`tests/fixtures/wire/invalid/*.json` contain hand-written wire IR with type errors, unknown
names, effects on context, duplicate assignments, and a cycle. For each:
`behavior admit <file>` exits 2 with the expected error code and location, and prints no
`behavior_version`.

## 5. Identity (FR-006a–c)

```bash
python -m examples.invoice.dump > /tmp/a.json && behavior version /tmp/a.json
```

Then check, each against the golden version in `tests/fixtures/versions.json`:

- edit a comment or move a declaration to another line → version unchanged;
- reorder the entity declarations → version unchanged;
- rename `margin` → item hash of `margin` unchanged, module version changed;
- swap two preconditions → action hash and version changed;
- `behavior hashes` shows identical hashes for structurally identical expressions in
  different rules.

## 6. Derived values and cycles (US1)

`python -m examples.project_margin.run`: `evaluation_order == ["margin", "high_risk"]`; with the
guard `(p.revenue != Money(Decimal("0"))) & high_risk(p)`, `revenue = 0` does not error.
`python/tests/fixtures/cycles.py` (`a → b → c → a`): admission fails with one `CYCLE` error
naming the cycle and all three locations; no behavior version.

## 7. Capability boundary (US3)

```bash
for f in tests/fixtures/intents/*.json; do
  behavior intent /tmp/a.json "$f" tests/fixtures/host/invoice_1042.json; echo " exit=$?"
done
```

`valid_allowed` → 0; `valid_denied` → 1; `unknown_capability`, `missing_argument`,
`wrong_type`, `extra_field`, `target_mismatch` (targets invoice 9999 while the host supplies
1042) → 2, every problem listed, nothing evaluated. The host file, not the intent, supplies the
acting user.

## 8. Replay (US2)

```bash
behavior eval   /tmp/a.json tests/fixtures/requests/invoice_allowed.json > /tmp/rec.json
behavior replay /tmp/a.json /tmp/rec.json      # {"matches":true}, exit 0
```

After changing a precondition and regenerating the wire IR, replaying the old record returns
`matches: false` (behavior version differs), exit 2.
