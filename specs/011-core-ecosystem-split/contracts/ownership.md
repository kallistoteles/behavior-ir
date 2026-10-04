# Contract: placement by semantic ownership

**Rule (FR-023):** *if changing a document could change what a Behavior program means, it belongs
to the core.* Everything else belongs to the ecosystem.

The table is the authoritative manifest that `scripts/preflight.sh` checks. Rules are globs over
the repository's files (tracked, plus untracked files that are not ignored). The first matching
rule wins, and every file must match a rule. "Both" means a file of that name exists in each
repository after the split, each owned there (separate copies that may diverge). The core
filter-repo path list is every `core` and `both` rule.

| # | Glob | Owner | Note |
|---|---|---|---|
| 1 | `crates/behavior-py/**` | ecosystem | the native extension of the Python binding |
| 2 | `crates/behavior-engine/**` | core | new facade (created in the preflight) |
| 3 | `crates/**` | core | behavior-core, -verify, -store, -cli |
| 4 | `consumer/**` | core | external consumer (FR-006d) |
| 5 | `schema/**` | core | wire, migration and read schemas |
| 6 | `tests/fixtures/wire/python/**` | core | frozen DSL output that core tests read (`constraints.rs`, `pretty_roundtrip.rs`); the ecosystem's equivalence test asserts the DSL still reproduces it |
| 7 | `tests/fixtures/**` | core | conformance fixtures, including `bindings/` (wire side of equivalence) |
| 8 | `api/engine-surface.txt` | core | public Rust surface |
| 9 | `api/public-api.json` | ecosystem | the Python package's public API |
| 10 | `docs/persistence.md`, `docs/verification.md`, `docs/proposals/**` | core | semantic documents |
| 11 | `docs/versioning.md` | both | the core gets the format table and policy for the engine (`docs/versioning.md`); the ecosystem keeps the package release policy and links the core's table |
| 12 | `PRINCIPLES.md`, `ARCHITECTURE.md` | core | semantics and the admission criterion; the ecosystem links them |
| 13 | `specs/008-*/**`, `specs/011-*/**` | ecosystem | packaging and the split itself (the core also keeps a reference copy of 011, rule 14) |
| 14 | `specs/0[0-1][0-9]-*/**` | core | 001–007, 009, 010, plus the 011 reference copy |
| 15 | `.claude/skills/behavior-engine-development/**` | core | engine development guidance |
| 16 | `.claude/skills/speckit-*/**`, `.specify/**` | both | separate Spec Kit installations (FR-032) |
| 17 | `skills/**` | ecosystem | consumer skills and evals |
| 18 | `python/**`, `pyproject.toml`, `release/**`, `examples/**`, `models/**` | ecosystem | binding, DSL, packaging, examples, models |
| 19 | `scripts/record-diff.py` | core | record tooling over core formats |
| 20 | `scripts/determinism-check.sh`, `scripts/release*.sh`, `scripts/gates.sh` | both | split by R10 and adapted per repository |
| 21 | `scripts/**` | both | every other script goes to both repositories; each trims what it does not use (core: T026, T027; ecosystem: T054) |
| 22 | `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `flake.nix`, `flake.lock`, `.gitignore`, `LICENSE`, `README.md` | both | each repository has its own; the ecosystem's workspace holds only `behavior-py` |
| 23 | `.github/workflows/core-*.yml` | core | |
| 24 | `.github/workflows/ecosystem-*.yml` | ecosystem | |

## Checks

- **Exactly one owner:** a path matching no rule fails the preflight and is named. Rules are
  ordered, so the first match decides. A path that a later rule with a different owner also
  matches is reported as a warning, which forces the order to stay intentional.
- **No core reference to the ecosystem** (`check-boundary.sh`, core): no file under a `core`
  rule names `behavior-py`, `behavior._engine`, `python/behavior`, a consumer skill
  (`skills/behavior-(authoring|application|verification)`), or an ecosystem example
  (`examples.<name>` or a root-anchored `examples/<name>/`). Core paths such as
  `crates/*/examples/` and `.claude/skills/behavior-engine-development/` are not matches.
  Allow-list: `specs/**` (historical records of the single repository; rewriting them would
  falsify history) and `docs/proposals/**`.
- **No ecosystem copy of core files** (`check-public-surface.sh --consumer`): after stage 3, no
  ecosystem file matches rules 2–8, 10, 12, 14, 15 or 19.
