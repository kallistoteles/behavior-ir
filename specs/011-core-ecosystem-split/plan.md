# Implementation Plan: Core and Ecosystem Repositories

**Branch**: `011-core-ecosystem-split` | **Date**: 2026-10-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/011-core-ecosystem-split/spec.md`

## Summary

Split Behavior into two repositories with a one-way dependency:

- `behavior-ir-core` (cloned at `../behavior-ir-core`) is the semantic authority.
- `behavior-ir` (this repository) is the ecosystem.

The boundary is a **public core contract**, not the repository line. It consists of the document
formats and schemas, the command-line tool and one public Rust crate, `behavior-engine`, which
re-exports the existing items explicitly.

The work runs in four stages. Each stage ends in a checked state.

1. **Preflight, in this repository** (FR-022). The boundary is made real before anything moves:
   - Add the `behavior-engine` facade. It owns `engine_info`.
   - The binding and the CLI depend on `behavior-engine` alone. The binding stops linking
     `behavior-cli`. The wheel bundles the `behavior` binary, and the Python launcher execs it.
   - Write a fixture and document ownership manifest.
   - Add `scripts/preflight.sh`, which proves the five criteria in a copy that holds only the
     core paths.
2. **Extraction.** A history-preserving `git filter-repo` of the core paths goes into
   `behavior-ir-core` (FR-020). The core then gets:
   - its own release scripts;
   - the external consumer build (FR-006d);
   - `core-ci` and `core-release`;
   - Spec Kit with the range 012+.

   The first **Core Release** is the tag `v0.10.1` on its exact commit.
3. **Ecosystem conversion, in this repository.** The core paths are removed in one commit.
   - `behavior-engine` is pinned by git `rev`. `core-release.json` records the core version,
     commit and asset checksums.
   - `scripts/fetch-core.sh` fetches the released conformance bundle and CLI binary, verified by
     checksum. No core checkout is needed (FR-024).
   - Add `ecosystem-ci` and `ecosystem-release`, the version-skew refusal, the public-surface
     check (FR-007), and Spec Kit with the range 500+.
4. **Architecture (US5).** Write the core admission criterion and the classification table. Add
   the models area and its rules, plus a worked state-machine lowering example checked by a
   test.

No semantics, identity or format changes (FR-019, FR-021). The before/after conformance
comparison (SC-003, SC-009) is a gate of stages 2 and 3.

**Prerequisite:** feature 010 (0.10.0) is committed and merged first. It is still uncommitted
on `010-first-class-reads`.

## Technical Context

**Language/Version**:
- Rust 1.98.1, edition 2024, pinned by `rust-toolchain.toml` in both repositories.
- Python ≥ 3.13 (binding, DSL, tests).
- Bash for the scripts.

**Primary Dependencies**:
- Existing: pyo3 0.26 (abi3-py313), maturin ≥ 1.7 with `--zig`, clap 4, serde_json, z3 4.16.0
  (external process).
- New tools, not new code dependencies:
  - `git-filter-repo`, one-time and maintainer-only;
  - GitHub Actions;
  - `cargo-zigbuild`, for a manylinux_2_28 CLI binary.

  `cargo-zigbuild` and `gh` are added to the nix flake in both repositories; `git-filter-repo` is
  run once through `nix shell`.

**Storage**: N/A. Files and Git. Stores are untouched.

**Testing**:
- cargo test, proptest, pytest, mypy `--strict`, `scripts/determinism-check.sh`.
- New:
  - `scripts/preflight.sh`;
  - the external consumer crate (`consumer/`, outside the core workspace);
  - `scripts/check-public-surface.sh` in both repositories;
  - a before/after conformance digest (`scripts/conformance-digest.sh`).

**Target Platform**:
- Linux x86_64 (manylinux_2_28), as today.
- GitHub-hosted `ubuntu-24.04` runners with nix.

**Project Type**:
- Core: Rust library workspace plus a CLI.
- Ecosystem: a Python package with a native extension, plus docs, skills and examples.

**Performance Goals**:
- No runtime change.
- CI budget: core CI ≤ 30 min, ecosystem CI ≤ 30 min. The release-check clean-environment
  steps keep their ≤ 600 s limit (SC-001 of 008).

**Constraints**:
- Extraction only (FR-021).
- Zero byte or identity change (SC-003).
- Ecosystem CI and release have no core checkout or path dependency (FR-024).
- Actions only orchestrate (FR-030).
- A local build and a published release have identical checksums (SC-011).
- No CI step reads Spec Kit's local state (FR-035).

**Scale/Scope**:
- Moves to the core: 4 Rust crates, about 400 tests, about 600 fixture files, 9 specs, 3
  semantic docs, the constitution, `PRINCIPLES.md` and one engine skill.
- Stays in the ecosystem: one native crate, the Python package (about 230 tests), 8 example
  directories and 3 consumer skills.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan satisfies it |
|---|---|---|
| I. Deterministic core, probabilistic edge | ✓ | No AI path is added or changed. The core keeps every decision; bindings and models only author (FR-008, FR-009). |
| II. AI output validated | ✓ | Unchanged. Intent validation stays in the core and is reached through `behavior-engine`. |
| III. Test-first | ✓ | Every new check is written and seen failing first: the preflight criteria, the public-surface checks, version-skew refusal, the external consumer, tag/version validation and the lowering example. Each preflight criterion starts as a failing check against today's tree (for example, `behavior-py` depends on `behavior-cli`). Moved tests are not new code; their pass counts before and after the move are the evidence (SC-009). |
| IV. Reproducibility and replay | ✓ | The conformance digest (hash vectors, goldens, records, store documents) must be identical before and after the move. Release artifacts are built reproducibly (`SOURCE_DATE_EPOCH`, `--remap-path-prefix`, pinned toolchain through nix) so local and CI checksums match (SC-011). |
| V. Explicit state and auditability | ✓ | A core release states its version, commit, format, verifier and engine versions and its public surface. An ecosystem release states the exact core version and commit it bundles. `behavior.versions()` reports both. |
| VI. Simplicity | ✓ with justification | One new crate (`behavior-engine`) and one out-of-workspace consumer crate; see Complexity Tracking. No new runtime dependency. |
| Tech constraints | ✓ | `Cargo.lock` is committed in both repositories. fmt and clippy `-D warnings` pass in both. `#![forbid(unsafe_code)]` holds in the facade. No unwrap/expect is added. |
| Quality gates | ✓ | The same scripts gate Spec Kit's implement phase and CI (FR-034). The determinism check runs in both repositories (SC-006). |

**Gate result: PASS.** No unjustified violation.

## Project Structure

### Documentation (this feature)

```text
specs/011-core-ecosystem-split/
├── plan.md              # This file
├── research.md          # Phase 0: decisions (facade, CLI bundling, fixtures, history, CI, Spec Kit)
├── data-model.md        # Phase 1: Core Release, core-release.json, ownership manifest, manifests
├── quickstart.md        # Phase 1: end-to-end validation of both repositories
├── contracts/
│   ├── behavior-engine.md        # the public Rust surface (explicit re-exports)
│   ├── core-release.md           # core tag, assets, manifest; ecosystem pin file
│   ├── ownership.md              # path-by-path placement by semantic ownership
│   ├── workflows.md              # core-ci, core-release, ecosystem-ci, ecosystem-release
│   └── spec-kit.md               # two installations, number ranges, gates
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

After the split, `specs/011-core-ecosystem-split/` stays in the ecosystem (FR-023: it governs
the repositories, not what a program means). The core receives a copy of this spec under the
same number, so its history explains its own origin. The copy is marked as owned by the
ecosystem.

### Source Code

**Core: `../behavior-ir-core`** (after extraction):

```text
behavior-ir-core/
├── Cargo.toml, Cargo.lock, rust-toolchain.toml, rustfmt.toml, flake.nix, flake.lock
├── crates/
│   ├── behavior-engine/        # NEW: the only public programmatic API (explicit re-exports)
│   ├── behavior-core/          # internal
│   ├── behavior-verify/        # internal
│   ├── behavior-store/         # internal (examples: history, lifecycle_history, …)
│   └── behavior-cli/           # sibling consumer of behavior-engine; the `behavior` binary
├── consumer/                   # NEW: external consumer crate, not a workspace member (FR-006d)
├── schema/                     # wire, migration and read schemas
├── tests/fixtures/             # conformance fixtures (all of today's tests/fixtures)
├── api/engine-surface.txt      # NEW: the facade's exported items, one per line
├── docs/                       # persistence.md, verification.md, versioning.md (formats + engine policy), proposals/
├── specs/                      # 001–007, 009, 010 (+ 011 as a reference copy)
├── PRINCIPLES.md, ARCHITECTURE.md (NEW, US5), README.md, LICENSE
├── .claude/skills/behavior-engine-development/, speckit-*
├── .specify/                   # Spec Kit; constitution 1.0.0 (unchanged); numbering note: 012+
├── scripts/                    # determinism-check, conformance-digest, check-boundary,
│                               # check-public-surface, release-check, release-build, release
└── .github/workflows/core-ci.yml, core-release.yml
```

**Ecosystem: `behavior-ir`** (this repository, after conversion):

```text
behavior-ir/
├── Cargo.toml                  # workspace = [crates/behavior-py]; behavior-engine = { git, rev }
├── Cargo.lock                  # records the exact core commit
├── core-release.json           # NEW: pinned core version, commit, asset checksums
├── crates/behavior-py/         # native extension; depends on behavior-engine only
├── python/behavior/            # binding + DSL; _cli.py execs the bundled core `behavior` binary
├── python/tests/               # reads core fixtures from $BEHAVIOR_CORE_DIR (fetched bundle)
├── models/README.md            # NEW: model rules (FR-009–FR-013); no models yet
├── models/examples/state_machine/   # NEW: worked lowering example + test (US5)
├── examples/, skills/, release/smoke.py, api/public-api.json
├── docs/versioning.md          # release policy for the package; formats doc moves to core
├── specs/                      # 008, 011; new features from 500
├── .specify/                   # Spec Kit; constitution 1.1.0 (binding + packaging rules)
├── scripts/                    # fetch-core, check-core-pin, check-public-surface,
│                               # determinism-check (ecosystem part), release-check, -build, release
└── .github/workflows/ecosystem-ci.yml, ecosystem-release.yml
```

**Structure Decision**: two repositories, as above. The core keeps today's workspace layout,
plus the facade and the consumer, so the extraction moves paths without restructuring them. The
ecosystem keeps today's Python layout. `bindings/python/` from the architecture text is **not**
introduced now: renaming paths is not extraction (FR-021), and one binding does not need it. A
later ecosystem feature may do it. `models/` is created now because US5 needs a home for the
rules.

## Phases

### Stage 1: Preflight (this repository, before the move)

Each criterion of FR-022 becomes a check in `scripts/preflight.sh`, seen failing first, then
fixed in its own commit:

| Criterion | Check | Fix |
|---|---|---|
| Core has no binding/model dependency | `scripts/check-boundary.sh`: `cargo metadata` shows that no core crate depends on `behavior-py`; no core source or manifest names `python/`, `behavior-py`, `examples/`, `skills/` or `behavior-ir` (allow-list for `specs/011` and docs) | Already true for dependencies. Remove any stray references found. |
| Public contract identifiable | `crates/behavior-engine` exists; `api/engine-surface.txt` lists every `pub use` and matches the source; no `*` re-export; the binding compiles against `behavior_engine::` only | Create the facade; switch `behavior-py` and `behavior-cli`; move `engine_info` into the facade |
| Every spec and fixture has an owner | `contracts/ownership.md` is the authority; a script checks that every tracked path matches exactly one rule | Write the manifest; resolve paths that match no rule or two rules |
| Core builds and tests alone | Copy only the core-owned paths into a temp dir; run `cargo test --workspace` and `determinism-check.sh` (core part) there | Make the determinism script's Python and examples part separable (`--core`/`--ecosystem`) |
| Ecosystem consumes the public surface | The binding's `Cargo.toml` has one core dependency, `behavior-engine`; `rg 'behavior_(core\|store\|verify\|cli)::' crates/behavior-py python` is empty; the wheel's `behavior` command is the bundled binary | Bundle the CLI binary; `_cli.py` execs it; remove `_engine.cli` |

The preflight also records the **conformance digest** before the move: SHA-256 over every
golden, hash vector, record, store document and the CLI outputs of `determinism-check.sh`.
Stage 2 and stage 3 must reproduce it.

### Stage 2: Extraction into `behavior-ir-core`

1. Run `git filter-repo --paths-from-file core-paths.txt` on a fresh clone. The paths come from
   [contracts/ownership.md](contracts/ownership.md).
2. Merge the result into `behavior-ir-core` with `--allow-unrelated-histories`, keeping its
   LICENSE and README.
3. Add `consumer/`, `scripts/check-public-surface.sh`, the core release scripts (adapted from
   today's), `.github/workflows/core-{ci,release}.yml`, and Spec Kit (`specify init --here`,
   same version and constitution).
4. Gates in the core alone: SC-001, the conformance digest equals the preflight digest (SC-009),
   and SC-006.
5. Tag `v0.10.1` (annotated) through `core-release`. Its assets:
   - `release-manifest.json`, `SHA256SUMS`;
   - `behavior-conformance-0.10.1.tar.gz` (schemas and fixtures);
   - `behavior-0.10.1-x86_64-linux-manylinux_2_28` (the CLI).

### Stage 3: Ecosystem conversion (this repository)

1. Remove the core paths in one commit; history stays.
2. Pin `behavior-engine = { git = "https://github.com/kallistoteles/behavior-ir-core", rev =
   "<v0.10.1 commit>" }` and write `core-release.json`.
3. Add `scripts/fetch-core.sh`. It downloads the pinned bundle and CLI, verifies the checksums
   from `core-release.json`, and unpacks into `.core/<version>/` (gitignored). The Python tests
   and the determinism check read `BEHAVIOR_CORE_DIR`.
4. Add the version-skew refusal (import and build) and `scripts/check-core-pin.sh`
   (`Cargo.lock` rev equals `core-release.json` commit).
5. Adapt `release-build`, `release-check` and `release`: the wheel bundles the verified CLI, and
   the manifest gains a `core` section.
6. Add `.github/workflows/ecosystem-{ci,release}.yml`. Amend the constitution to 1.1.0 through
   `/speckit-constitution`. Document the 500+ range.
7. Gates: SC-002, SC-004, SC-005, SC-008, and the conformance digest (SC-003). Then tag the
   ecosystem `v0.10.1`.

### Stage 4: Architecture (US5)

- Core: `ARCHITECTURE.md` holds the dependency direction, the terminology, the core admission
  criterion and the 12-row classification table (FR-013).
- Ecosystem:
  - `models/README.md` holds the model rules: lowering, one authoritative lowering per model,
    the non-lowerable procedure, model identity, and no "plugin" or "extension".
  - `models/examples/state_machine/` contains a tested lowering of `DRAFT --submit--> SUBMITTED`
    to `requires`/`set_`/`ensures`, admitted by the core.
  - `PRINCIPLES.md` (core) and the consumer skills refer to it.

### Verification (SC-010, SC-011)

- One deliberately broken pull request per required gate in each repository. CI must fail on
  each one. The run links are recorded in `checklists/implementation-review.md`.
- One local `release-check` per repository on the release commit. Its checksums are compared
  with the published `SHA256SUMS`.

## Complexity Tracking

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| `behavior-engine` facade crate | FR-006: one public programmatic door, so internal crates can be reorganized | Depending on the existing crates makes every internal crate public; the boundary could not be checked |
| `consumer/` crate outside the workspace | FR-006d: proves the facade alone suffices, as an external user would see it | A workspace member resolves internal crates by path and hides missing re-exports |
| CLI shipped as a bundled binary, not linked into the extension | FR-006c: the binding must not link the CLI; the CLI is a sibling consumer | Keeping `behavior_cli::run` in the extension puts the CLI inside the binding's contract |
| Conformance bundle as a release asset | FR-024: the ecosystem needs the core's fixtures without a core checkout and never copies them | Reading the cargo git checkout depends on cargo internals; vendoring the fixtures creates a diverging copy |
