# Contract: `behavior-engine`, the public Rust surface

`behavior-engine` is the only supported programmatic Rust API of Behavior Core. Bindings, the
CLI and the external consumer depend on it alone.

## Rules

1. Every exported item is listed explicitly. `pub use x::*` is forbidden, and
   `check-public-surface.sh` fails on it and names the line.
2. The facade has no logic, with one exception: `engine_info()`, which moves here from
   `behavior-cli` unchanged (same JSON, same keys).
3. The namespaces mirror today's module paths. The binding migrates by replacing the crate name
   only.
4. `api/engine-surface.txt` equals the exported set. A difference fails core CI.
5. Internal crates (`behavior-core`, `behavior-store`, `behavior-verify`, `behavior-cli`) are not
   part of the contract. The ecosystem must not name them in a manifest or in source
   (FR-007, checked by `check-public-surface.sh --consumer`).

## Initial export set

Derived from what `crates/behavior-py/src/lib.rs` and `crates/behavior-cli/src/lib.rs` use at
0.10.0. The exact list is produced in the preflight (T: "identify the surface") and frozen into
`api/engine-surface.txt`. The table below is the expected shape.

| Namespace | Items (from) | Capability (FR-006b) |
|---|---|---|
| root | `admit`, `admission_result`, `evaluate`, `evaluate_intent`, `replay`, `canonical`, `engine_info` (core, cli) | admit, evaluate, versions |
| `builder` | the module builder types and functions (core::builder) | build modules |
| `wire` | wire document types (core::wire) | build modules |
| `semantic` | `module::Module` and the semantic types the binding inspects (core::semantic) | admit |
| `schema` | schema types and functions (core::schema) | migrate, stores |
| `serialize` | `to_wire_json`, `read_document` (core::serialize) | build modules, read |
| `read` | `admit_read`, `ReadSource`, `evaluate_read`, `evaluate_read_request`, `replay_read`, `ReadIntent`, `evaluate_read_intent`, `ReadRecord`, `ReadResponse`, `ReadExecution` (core::read) | read |
| `migration` | the migration items the binding uses (core::migration) | migrate |
| `intent` | intent types used by the binding (core::intent) | authorize |
| `store` | `Store`, `Backend`, `InMemoryBackend`, `documents::*` items by name, `replay::{replay_data, replay_behavior_with}`, `conformance::run`, `store::genesis_for` (store) | stores |
| `verify` | `verify`, `verify_migration`, `Profile`, `CheckKind`, `solver::Z3Process`, `governance::*` items by name (verify) | verify, authorize |

`documents::*` and `governance::*` above stand for the explicit item lists the preflight writes
out. They are not wildcard re-exports.

## External consumer (FR-006d)

`consumer/` in the core is a crate **outside** the workspace (`[workspace]` with
`exclude = ["consumer"]`). Its only dependency is
`behavior-engine = { git = "<core repo>", rev = "<commit under test>" }`. In CI the dependency is
patched to the checked-out commit's path, because the commit is not yet pushed for a pull
request; the release workflow uses the real git rev of the tag.

Its test exercises each FR-006b capability once:

- build and admit a module;
- evaluate an action;
- run a declared read;
- open an in-memory store, commit and replay;
- migrate;
- verify with the solver absent (inconclusive is fine);
- authorize an intent;
- call `engine_info`.

It fails to compile if any needed item is not exported.

## Versioning

The facade is versioned with the core (`[workspace.package] version`):

- Adding an item is a patch bump.
- Removing or changing an item is a minor bump (0.x), per `docs/versioning.md` in the core.
