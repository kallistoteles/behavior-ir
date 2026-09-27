# Shared test fixtures

These fixtures are shared by the Rust tests (`crates/*/tests`), the CLI tests, and the Python
tests (`python/tests`). All of them assert against the same bytes.

| Path | Content |
|------|---------|
| `typing_cases.json` | Typing table run against the engine (the only type checker) |
| `hash_vectors.json` | Frozen content-hash vectors; changing one requires new hash tags |
| `versions.json` | Behavior versions and item hashes of the Python example models and variants |
| `wire/valid/` | Hand-written wire IR that must be admitted |
| `wire/invalid/` | Hand-written wire IR with `<case>.expected.json` listing the expected errors |
| `wire/python/` | Golden canonical wire IR of the Python DSL fixtures, serialized by the engine |
| `requests/` | Evaluation requests against `wire/valid/` |
| `records/` | Golden decision records, one per request |
| `intents/` | Recorded structured intents, with `<name>.expected.json` for rejections |
| `host/` | Host contexts (state, context, data version) supplied by the trusted caller |

Golden files are generated from the implementation, reviewed by a human, and then frozen.

## Feature 002

| Path | Content |
|------|---------|
| `verify/` | Behavior modules with seeded defects and `<name>.expected.json` listing every expected check outcome |
| `governance/` | Execution policies, fixed test keys (test-only, never trusted by a real policy), and notes on how waivers and signatures are generated in tests |
| `requests/002/` | Requests and expectations for the runtime changes (entity constraints, binding check, state-cell change sets) |

## Feature 003

| Path | Content |
|------|---------|
| `numeric/rounding.json` | The six rounding modes as data (values, scales, expected results); computed with exact fractions and shared by Rust and Python tests |
| `wire/valid/fixed_scale.json` | Wire IR 0.3 module with a two-decimal `Money`, exact quantities, and rescales (invalid 0.3 cases in `wire/invalid/`) |
| `requests/003/` | Requests and expectations for grid/range input checks, exact arithmetic, and rescale trace entries |
| `verify/*money2*`, `verify/discount_rescale*`, `verify/rescale_modes*` | Verification fixtures with fixed-scale Money |
