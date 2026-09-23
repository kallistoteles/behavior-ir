# Shared test fixtures

These fixtures are shared by the Rust tests (`crates/*/tests`), the CLI tests, and the Python
tests (`python/tests`). All of them assert against the same bytes.

| Path | Content |
|------|---------|
| `typing_cases.json` | Typing table run against both the engine and the Python DSL checker |
| `hash_vectors.json` | Frozen content-hash vectors; changing one requires new hash tags |
| `versions.json` | Behavior versions and item hashes of the Python example models and variants |
| `wire/valid/` | Hand-written wire IR that must be admitted |
| `wire/invalid/` | Hand-written wire IR with `<case>.expected.json` listing the expected errors |
| `wire/python/` | Golden wire IR emitted by the Python DSL fixtures |
| `requests/` | Evaluation requests against `wire/valid/` |
| `records/` | Golden decision records, one per request |
| `intents/` | Recorded structured intents, with `<name>.expected.json` for rejections |
| `host/` | Host contexts (state, context, data version) supplied by the trusted caller |

Golden files are generated from the implementation, reviewed by a human, and then frozen.
