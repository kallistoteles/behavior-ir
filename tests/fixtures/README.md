# Shared test fixtures

These fixtures are shared by the Rust tests (`crates/*/tests`), the CLI tests, and the Python
tests (`python/tests`). All of them assert against the same bytes.

| Path | Content |
|------|---------|
| `typing_cases.json` | Typing table run against the engine (the only type checker) |
| `hash_vectors.json` | Frozen content-hash vectors; changes require a reviewed regeneration (feature 004 changed `expr_sub`, `expr_mul`, `expr_div`: decimal arithmetic is exact) |
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

## Feature 004

| Path | Content |
|------|---------|
| `frozen_versions.json` | Behavior versions (semantic hashes) of every fixture module, recorded before feature 004; modules whose meaning does not change must keep them |
| `migration_004.json` | Migration table: fixture modules whose behavior version changed in feature 004, with the reason (all others keep their frozen identity) |
| `requests/004/` | Requests and expectations for exact ratios and exact general-decimal arithmetic (003 subset format) |

## Feature 005

| Path | Content |
|------|---------|
| `wire/valid/ledger.json` | Accounts (`active`, two-decimal `balance`) with `transfer`, the no-op `touch`, and `freeze`: the persistence fixture module |
| `store/hash_vectors.json` | Frozen hashes of the persistence documents (entity content and version, state, evidence policy, genesis, commit bundle, transition record) |

## Feature 006

| Path | Content |
|------|---------|
| `wire/valid/accounts.json` | Wire IR 0.5: customers, accounts with `owner: Ref<Customer>`, audit notes with a plain `Id<Customer>`; creation, removal, `exists`, `referenced` (invalid 0.5 cases in `wire/invalid/`: `create_*`, `remove_*`, `lifecycle_in_0_4`) |
| `requests/006/` | Plain-evaluation requests with `facts` sections (existence, identities, references) and expectations in the 004 subset format |
| `frozen_versions_006.json` | Behavior versions and item hashes of every fixture module before feature 006; modules without 006 forms must keep them (SC-006) |
| `verify/lifecycle.expected.json` | Verification expectations for `wire/valid/accounts.json`: seeded defects (negative initial balance, unguarded removal of a referenced customer), proven guarded actions, and the real `switch_and_remove` counterexample |

## Feature 007

| Path | Content |
|------|---------|
| `wire/valid/orders.json` | Wire IR 0.6: customers, orders and employees queried as sets (`select`, `where`, set algebra, `count`, `any`, `all`, `sum`, `min`, `max`, `unique`), a module invariant, and the verification cases of SC-005 (invalid 0.6 cases in `wire/invalid/`: `query_*`, `nested_query`, `exists_in_filter`, `mixed_set_types`, `sum_of_string`, `min_of_unordered`, `module_invariant_reads_input`) |
| `frozen_versions_007.json` | Behavior versions and item hashes of every fixture module before feature 007, including the 006 lifecycle module (SC-006) |
| `requests/007/` | Plain-evaluation requests whose `facts` give complete `universe` sections per entity type, with expectations in the 004 subset format |
