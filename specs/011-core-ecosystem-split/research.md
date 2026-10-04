# Research: Core and Ecosystem Repositories

All Technical Context items are resolved. The decisions are below, each with its reason and the
alternatives considered.

## R1. Shape of the public Rust surface

- **Decision**: a new crate `crates/behavior-engine` with `#![forbid(unsafe_code)]`. It consists
  only of explicit `pub use` items and `pub mod` namespaces that mirror today's paths (for
  example, `behavior_engine::read::{ReadSource, …}`). `engine_info()` moves into it from
  `behavior-cli`. The exported list lives in `api/engine-surface.txt`.
  `scripts/check-public-surface.sh` parses the facade's `pub use` lines and fails on:
  - a `*`;
  - an item missing from the list;
  - a list entry missing from the source.
- **Rationale**: FR-006 and FR-006b. Namespaced re-exports keep the binding's imports
  mechanical (`behavior_core::read::X` → `behavior_engine::read::X`), so the binding needs no
  redesign. The surface list makes every change to the surface visible in review.
- **Alternatives considered**:
  - `cargo public-api`: it needs nightly rustdoc JSON, and the toolchain is pinned stable.
  - Re-exporting whole crates (`pub use behavior_core;`): it would export every internal item.
  - A wire-only, process-based binding: rejected in the spec's assumptions.

## R2. The `behavior` command in the user package

- **Decision**: the binding no longer links `behavior-cli`. The core release publishes a
  manylinux_2_28 x86_64 `behavior` binary, built with `cargo zigbuild` from the release commit.
  The ecosystem places the verified binary at `python/behavior/_bin/behavior` and includes it in
  the wheel through maturin's `include`. `behavior._cli:main` resolves it with
  `importlib.resources` and calls `os.execv`. `_engine.cli` is removed. `_engine.engine_info`
  calls `behavior_engine::engine_info()`.
- **Rationale**:
  - FR-006c: the CLI is a sibling consumer, not part of the binding.
  - FR-018: users keep one install and the same `behavior` command.
  - The command reports the core's own version (US4, scenario 2).
  - `python -m behavior._cli` keeps working for the existing tests.
- **Alternatives considered**:
  - Keep linking `behavior_cli::run`: this violates FR-006c.
  - Build the CLI in the ecosystem with `cargo install --git --rev`: it builds core code with the
    ecosystem's toolchain settings, and the bundled tool would not be the released core artifact.
  - Drop the command from the package: this violates FR-018.
- **Risks and checks**:
  - auditwheel must accept the extra ELF file. Release-check step 2 already runs
    `auditwheel show`.
  - The executable bit must survive in the wheel. The smoke test runs `behavior engine-info`
    from the installed package.

## R3. How the ecosystem gets core fixtures and schemas without a checkout

- **Decision**: each Core Release publishes `behavior-conformance-<v>.tar.gz`, holding
  `schema/` and `tests/fixtures/` exactly as at the tag, with a deterministic tar (sorted, mtime
  = commit time, uid/gid 0). `core-release.json` in the ecosystem records its SHA-256.
  `scripts/fetch-core.sh` downloads it from the GitHub Release, verifies it and unpacks it into
  `.core/<version>/` (gitignored). `BEHAVIOR_CORE_DIR` points there, and
  `python/tests/conftest.py` takes `FIXTURES` from it. A local override
  `BEHAVIOR_CORE_DIR=../behavior-ir-core` is allowed for development only.
- **Rationale**:
  - FR-024: the fixtures come from the released artifact.
  - The fixtures are never copied (edge case "Shared fixtures").
  - The checksum binds the fixtures to the pinned release.
- **Alternatives considered**:
  - Reading the cargo git checkout under `$CARGO_HOME/git/checkouts`: this depends on cargo
    internals.
  - A git submodule: this is a core checkout.
  - Vendoring a copy: this gives a diverging copy.
- **Access**: if `behavior-ir-core` is private, ecosystem CI needs a read-only token secret
  (`CORE_READ_TOKEN`). It is used for both the cargo git fetch
  (`CARGO_NET_GIT_FETCH_WITH_CLI=true` with a credential helper) and `gh release download`. If
  the repository is public, no secret is needed. This is the only setup step outside the
  repositories.

## R4. Which equivalence fixtures belong where

- **Decision**: the wire forms (`tests/fixtures/bindings/*.json`, `tests/fixtures/wire/**`) are
  core conformance fixtures and come from the bundle. The Python forms (`python/tests/fixtures/`,
  `examples/*/model.py`) and `test_binding_equivalence.py` stay in the ecosystem. SC-004 needs at
  least one pair per wire IR form. Today's pairs cover entities, actions, lifecycle, queries,
  arithmetic and constraints. Two pairs are added:
  - migrations: the `schema_evolution` example against `tests/fixtures/migration`;
  - reads: `lab_reads` against `tests/fixtures/reads/modules/lab.json`.

  `tests/fixtures/wire/python/` was once written by the DSL, but core tests read it
  (`constraints.rs`, `pretty_roundtrip.rs`), so it stays in the core as frozen fixtures. The
  ecosystem's equivalence test asserts that the DSL still reproduces these files byte for byte.
- **Rationale**: FR-014, FR-016 and SC-004. Each pair has one side in each repository: the core
  owns the meaning, and the ecosystem owns the syntax.
- **Alternatives considered**: keeping both forms in the ecosystem. Then the wire side would no
  longer be a core conformance fixture.

## R5. History-preserving extraction

- **Decision**:
  1. Take a fresh mirror clone of `behavior-ir` at the stage-1 commit.
  2. Run `git filter-repo --paths-from-file core-paths.txt`. The file is generated from
     [contracts/ownership.md](contracts/ownership.md). No path is renamed: `docs/versioning.md`
     goes to both repositories and each copy is then trimmed to what it owns.
  3. In `behavior-ir-core`, fetch the result and run
     `git merge --allow-unrelated-histories`.
  4. Keep the core's README (extended) and LICENSE. Check that both licenses say MIT.

  The tool comes from nix (`nix shell nixpkgs#git-filter-repo`). Tags are not carried over: old
  `v*` tags stay in the ecosystem's history only, and the core's first tag is `v0.10.1`.
- **Rationale**: FR-020. Commits that touched core paths keep their messages, authors and dates.
- **Alternatives considered**:
  - `git subtree split`: it handles one directory only, and the core spans many top-level paths.
  - A plain copy with a "moved from" commit: it loses history.
- **Note**: `filter-repo` rewrites commit IDs. The commit messages remain the link between the
  repositories, and the core's first commit after the merge names the ecosystem commit it was
  extracted from.

## R6. Version line and pinning

- **Decision**:
  - Core: `v0.10.1`. Adding `behavior-engine` is additive public API, so it is a patch bump under
    `docs/versioning.md`. No format, identity or verifier version changes.
  - Ecosystem: `v0.10.1`, in step, pinning core `v0.10.1` by commit.
  - `Cargo.toml` has `behavior-engine = { git = "…/behavior-ir-core", rev = "<sha>" }`, and
    `Cargo.lock` records the same SHA.
  - `core-release.json` holds `{version, tag, commit, assets}`.
  - `scripts/check-core-pin.sh` fails unless all three agree, and fails if a `[patch]` override
    is active (`cargo metadata` source must be `git+…?rev=<sha>`).
  - The binding's Python version check compares `_engine.engine_info()["engine"]` with
    `core-release.json`'s version, which `behavior/_versions.py` embeds at build time. On
    mismatch it raises `ImportError` naming both (US2, scenario 3).
- **Rationale**: FR-006a and FR-017. Exact pinning; ranges come later.
- **Alternatives considered**:
  - A tag-based git dependency (`tag = "v0.10.1"`): a tag is a name; the commit is the identity.
  - crates.io: out of scope.

## R7. Reproducible release artifacts (SC-011)

- **Decision**: release scripts run inside `nix develop` (the flake pins the toolchain, maturin,
  zig and z3) and export:
  - `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)`;
  - `RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=$CARGO_HOME=/cargo"`;
  - `CARGO_INCREMENTAL=0`.

  Tarballs are built with `tar --sort=name --mtime=@$SOURCE_DATE_EPOCH --owner=0 --group=0
  --numeric-owner` and `gzip -n`. `release-check` gains a step `reproducible`, which builds twice
  and compares checksums. `scripts/release-verify.sh <tag>` downloads a published release and
  compares it with a local build.
- **Rationale**: FR-030 and SC-011. CI adds orchestration, not authority.
- **Alternatives considered**: comparing only the manifest's versions. That does not satisfy
  SC-011.
- **Risk**: if cross-machine bit identity of the wheel fails for a reason outside our control
  (for example, a maturin metadata timestamp), the gap is recorded and fixed before the release,
  not waived.

## R8. GitHub Actions design

- **Decision**: four workflows. Each one installs nix (`DeterminateSystems/nix-installer-action`,
  pinned by commit SHA) and runs scripts through `nix develop -c`. The workflows hold no logic
  beyond:
  - triggers;
  - the matrix-free job sequence;
  - artifact upload;
  - `gh release create`.

  Triggers:
  - CI: `pull_request` and `push` to `main` and `dev`.
  - Release: `push: tags: ['v*']` only.

  The release job:
  1. checks that the tag is annotated and that the tag version equals the declared version
     (`scripts/check-tag.sh`), before building anything;
  2. checks that the tagged commit is reachable from `main`;
  3. reruns `release-check`;
  4. publishes the GitHub Release with `dist/v<v>/*`.

  Permissions are least privilege: `contents: read` for CI and `contents: write` for release.
  Third-party actions are pinned by SHA. Details are in
  [contracts/workflows.md](contracts/workflows.md).
- **Rationale**: FR-025 to FR-031. The scripts are the authority, so a local run equals a CI run.
- **Alternatives considered**:
  - One combined workflow: forbidden by FR-031.
  - Actions marketplace steps for Rust and Python setup: these would duplicate the flake and
    could drift from local runs.

## R9. Spec Kit in two repositories

- **Decision**:
  - Core: run `specify init --here --ai claude --script sh` with the same Spec Kit version
    (1.0.11.dev0). The installation is copied from this repository's tracked `.specify/` and
    `.claude/skills/speckit-*`, so the version is identical. The constitution is copied
    unchanged at 1.0.0.
  - Ecosystem: keeps its installation. The constitution is amended to 1.1.0 (MINOR: a new
    section on binding and packaging rules) through `/speckit-constitution`.
  - Numbering: Spec Kit's sequential scheme takes the highest `specs/NNN` plus one. The first new
    feature uses `--number 012` in the core and `--number 500` in the ecosystem. After that,
    numbering is automatic.
  - The core's `specs/` holds 001–007, 009, 010 and a copy of 011, so its next automatic number
    is 012 even without the flag. The ecosystem holds 008 and 011, so its first feature needs
    `--number 500`.
  - Each repository's README and constitution state its range.
- **Rationale**: FR-032 to FR-035 and SC-012. Spec Kit itself is not changed.
- **Alternatives considered**: a shared Spec Kit repository or submodule. This adds coupling,
  and Spec Kit's scripts assume a repository-local `specs/`.
- **Gate parity (FR-034)**: the constitution's "Development Workflow and Quality Gates" section
  names `scripts/gates.sh` in each repository. This is a new script that runs exactly the CI
  gate list. Spec Kit's implement phase and `*-ci.yml` both call it.

## R10. Determinism check split

- **Decision**: `scripts/determinism-check.sh` is split by ownership:
  - Core: CLI admission, evaluation, replay, reads, migrations and the store examples.
  - Ecosystem: the smoke scenario and the Python examples, run twice. It also runs the bundled
    CLI over the fetched conformance fixtures twice, so the packaged tool is covered too.
- **Rationale**: SC-006 in both repositories, with no duplicated core checks driving the
  ecosystem's verdict.
