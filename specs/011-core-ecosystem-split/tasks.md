---
description: "Task list for feature 011: Core and Ecosystem Repositories"
---

# Tasks: Core and Ecosystem Repositories

**Input**: Design documents from `/specs/011-core-ecosystem-split/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: These are required. The constitution says Principle III, test-first, is
NON-NEGOTIABLE. Every new check is written and run against the current tree, where it must fail,
before the change that makes it pass. Moved tests are not new code; their evidence is the
conformance digest and their pass counts before and after the move.

**Organization**: Tasks are grouped by user story. Paths without a prefix are in this repository
(`behavior-ir`, the ecosystem after stage 3). Paths prefixed `core:` are in `../behavior-ir-core`.

**Phase order**: Phase order follows the dependencies, not only the priorities.

- US6's core half (Phase 4) produces the first Core Release.
- US2 (Phase 5) pins that release.
- US6's ecosystem half (Phase 8) releases the result.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6 from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Starting state, tooling, and the evidence that the move changes nothing.

- [X] T001 Confirm the starting state: feature 010 (0.10.0) is committed and merged to `main`, and
  `git status` is clean. Create the branch `011-core-ecosystem-split` from `main`. Run
  `scripts/release-check.sh 0.10.0 --skip-gates` once and record the result in
  `specs/011-core-ecosystem-split/checklists/implementation-review.md`, which is created here with
  a "Baseline" section.
- [X] T002 [P] Add `cargo-zigbuild` and `gh` to the dev shell packages in `flake.nix`. Check that
  `nix develop -c cargo zigbuild --version` and `gh --version` work. `git-filter-repo` stays out
  of the flake and is used once through `nix shell nixpkgs#git-filter-repo`.
- [X] T003 [P] Write `scripts/conformance-digest.sh [--core-dir DIR]`. It prints canonical JSON
  `{path: sha256}` (sorted keys, one trailing newline) over every file under
  `<core-dir>/tests/fixtures/**` and `<core-dir>/schema/**`. It also includes the stdout of each
  CLI call that `scripts/determinism-check.sh` makes, keyed `cli:<label>`. Run it twice and check
  that the outputs are byte-identical.
- [X] T004 Record the baseline: run
  `scripts/conformance-digest.sh > specs/011-core-ecosystem-split/digest-before.json` on the T001
  commit. Commit the file. It is the reference for SC-003 and SC-009.
- [X] T004a Decide whether `behavior-ir-core` is public or private, and ask the user. If it is
  private, create a fine-grained token with read-only Contents on `behavior-ir-core` and store it
  as `CORE_READ_TOKEN` in `behavior-ir`'s Actions secrets. Record the decision, never the token,
  in `checklists/implementation-review.md` under "Baseline". This blocks T068.

---

## Phase 2: Foundational — Preflight (Blocking, FR-022)

**Purpose**: Make the boundary real inside one repository before anything moves. Each criterion
is a check that is seen failing on today's tree, then fixed in its own commit.

**⚠️ CRITICAL**: The extraction (Phase 3) must not start until `scripts/preflight.sh` passes all
five criteria.

### Checks first (each must fail on the T001 tree)

- [X] T005 [P] Write `scripts/check-boundary.sh`. It fails, naming the offender, if either of
  these holds:
  - `cargo metadata --format-version 1` shows that any of `behavior-core`, `-verify`, `-store`,
    `-cli` or `-engine` depends on `behavior-py`;
  - a file under a core rule of `contracts/ownership.md` contains any of these:
    - `behavior-py`, `behavior._engine` or `python/behavior`;
    - `skills/behavior-(authoring|application|verification)`;
    - `examples\.(invoice|project_margin|accounts|ledger|orders|lab_reads|schema_evolution|tryout)\b`;
    - a root-anchored
      `(^|[^/\w])examples/(invoice|project_margin|accounts|ledger|orders|lab_reads|schema_evolution|tryout)/`.

  Core paths that only contain `examples/` or `skills/` are not matches, for example
  `crates/*/examples/`, `cargo run --example` and `.claude/skills/behavior-engine-development/`.
  A test case in `scripts/tests/test_check_boundary.sh` asserts that
  `crates/behavior-store/examples/history.rs` and the `cargo run -q -p behavior-store --example`
  lines pass, and that a planted `python/behavior` reference fails. The allow-list is
  `specs/011-*/**` and `docs/proposals/**`. Run the script; record any references found.
- [X] T006 [P] Write `scripts/check-ownership.py`. It parses the rule table in
  `specs/011-core-ecosystem-split/contracts/ownership.md` and matches every path from
  `git ls-files --cached --others --exclude-standard`. The first matching rule wins.
  - It exits 1 and lists any path that matches no rule.
  - It warns about any path that a later rule with a different owner also matches.
  - `--list core` prints the filter-repo path list: `core` and `both` rules, as
    `glob:`-prefixed lines.

  Run it and fix any unmatched paths by adding rules, not by moving files.
- [X] T007 [P] Write `scripts/check-public-surface.sh`.
  - **Default (provider) mode:** extract every `pub use` path from
    `crates/behavior-engine/src/lib.rs` (including those inside `pub mod` blocks, written as
    `ns::Item`), sort them, and diff against `api/engine-surface.txt`. Fail on any line ending
    in `*`, and on any difference.
  - **`--consumer` mode:** fail if `crates/behavior-py/Cargo.toml` has any core dependency other
    than `behavior-engine`, or if
    `rg -n 'behavior_(core|store|verify|cli)::' crates/behavior-py python` has matches. Name each
    match.

  Run it; it fails because the facade does not exist.
- [X] T008 [P] Write `scripts/preflight.sh`. It runs five criteria in order and prints
  `preflight: <criterion> OK|FAILED`:
  1. `check-boundary.sh`;
  2. `check-public-surface.sh`;
  3. `check-ownership.py`;
  4. **core alone**: copy the files listed by `check-ownership.py --list core` into `mktemp -d`,
     then run `cargo test -q --workspace` and `scripts/determinism-check.sh --core` there;
  5. `check-public-surface.sh --consumer`.

  It exits 1 if any criterion fails. Run it and record the failures in the review checklist.
- [X] T009 [P] Add `crates/behavior-cli/tests/cli_engine_info.rs::engine_info_comes_from_the_engine`.
  It asserts that `behavior engine-info` stdout equals `behavior_engine::engine_info()` serialized
  canonically. It fails to compile because `behavior-engine` does not exist.

### Fixes (each in its own commit)

- [X] T010 Inventory the surface: list every `behavior_core::…`, `behavior_store::…`,
  `behavior_verify::…` and `behavior_cli::…` path used in `crates/behavior-py/src/lib.rs` and
  `crates/behavior-cli/src/*.rs`, including `use` groups. Expand group imports to single items.
  Write the union, sorted and with the facade's namespaces, to `api/engine-surface.txt`. The
  namespaces are as in `contracts/behavior-engine.md`:
  - root, `builder`, `wire`, `semantic`, `schema`, `serialize`, `read`, `migration`, `intent`;
  - `store` (from `behavior_store`) and `verify` (from `behavior_verify`).

  No `*` lines: each item of `documents` and `governance` is listed by name.
- [X] T011 Create `crates/behavior-engine/` with:
  - `Cargo.toml`: workspace version, edition and license; `[lints] workspace = true`;
    dependencies `behavior-core`, `behavior-verify`, `behavior-store` and `serde_json`;
  - `src/lib.rs`: `#![forbid(unsafe_code)]`, a crate doc stating "the only supported
    programmatic API of Behavior Core", and explicit `pub use` and `pub mod` re-exports exactly
    matching `api/engine-surface.txt`.

  Add the crate to `[workspace] members` and `[workspace.dependencies]` in `Cargo.toml`.
- [X] T012 Move `engine_info()` from `crates/behavior-cli/src/lib.rs` to
  `crates/behavior-engine/src/lib.rs`, unchanged (same keys, same values). `behavior-cli` calls
  `behavior_engine::engine_info()`. Make T009 pass. `cli_engine_info.rs` and
  `python/tests/test_versions.py` must stay green.
- [X] T013 Switch `crates/behavior-cli` to depend on `behavior-engine` only. In
  `crates/behavior-cli/Cargo.toml`, replace the `behavior-core`, `behavior-verify` and
  `behavior-store` dependencies with `behavior-engine.workspace = true`. Rewrite the imports in
  `crates/behavior-cli/src/*.rs` to `behavior_engine::`. CLI tests may keep internal crates as
  dev-dependencies only. All `crates/behavior-cli/tests/*` pass.
- [X] T014 Switch `crates/behavior-py` to depend on `behavior-engine` only. In
  `crates/behavior-py/Cargo.toml`, remove `behavior-core`, `behavior-verify`, `behavior-store`
  and `behavior-cli`, and add `behavior-engine.workspace = true`. Rewrite the imports in
  `crates/behavior-py/src/lib.rs` to `behavior_engine::`. `engine_info` calls
  `behavior_engine::engine_info()`.
  - Remove the `cli` pyfunction and its registration (lines ~2150–2175).
  - Remove `cli` from `python/behavior/_engine.pyi`.
- [X] T015 Write `python/tests/test_cli_entry.py::test_the_console_script_runs_the_bundled_binary`.
  It asserts that `behavior._cli` resolves `behavior/_bin/behavior` and that the command's
  `engine-info` equals `behavior.versions()` engine fields. It also asserts that a missing
  binary exits 2 with `behavior: the bundled core CLI is missing (…/_bin/behavior)`. Run it and
  see it fail.
- [X] T016 Rewrite `python/behavior/_cli.py`:
  - `main()` locates `importlib.resources.files("behavior") / "_bin" / "behavior"`, flushes
    stdout, and calls `os.execv(path, [path, *sys.argv[1:]])`.
  - If the binary is missing it prints the T015 message to stderr and exits 2.

  Add `python/behavior/_bin/` to `.gitignore`. Add
  `include = [{ path = "python/behavior/_bin/behavior", format = "wheel" }]` under
  `[tool.maturin]` in `pyproject.toml`.
- [X] T017 Write `scripts/stage-cli.sh`. Before the split it runs
  `cargo build --release -p behavior-cli` and copies `target/release/behavior` to
  `python/behavior/_bin/behavior` with mode 0755. Call it from `scripts/release-build.sh` before
  `maturin build`. For the wheel, build with
  `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28 -p behavior-cli`. Document
  `scripts/stage-cli.sh` as a step before `maturin develop` in `README.md`. Make T015 and every
  `python/tests/test_cli_entry.py` test pass.
- [X] T018 Split `scripts/determinism-check.sh` by ownership (research R10) into `--core`
  (default when no Python engine is importable) and `--ecosystem` sections:
  - `--core`: CLI admit, eval and replay, reads, migrations, store examples.
  - `--ecosystem`: `release/smoke.py`, the `examples.*.run` modules, and the bundled CLI over
    `$BEHAVIOR_CORE_DIR/tests/fixtures/wire/valid` twice.

  With no flag, both sections run, as today.
- [X] T019 Write `scripts/gates.sh`, the single gate list for Spec Kit's implement phase and CI
  (FR-034). It runs, in order:
  1. `cargo fmt --all --check`;
  2. `cargo clippy --workspace --all-targets -- -D warnings`;
  3. `cargo test -q --workspace`;
  4. `cargo build -q --workspace`;
  5. `scripts/stage-cli.sh`;
  6. `maturin develop -q`;
  7. `python -m pytest -q python/tests`;
  8. `mypy`;
  9. `scripts/determinism-check.sh`;
  10. `scripts/check-boundary.sh`;
  11. `scripts/check-public-surface.sh`;
  12. `scripts/check-public-surface.sh --consumer`.

  It stops at the first failure and names the step. Change `scripts/release-check.sh` step 1 to
  call it.
- [X] T020 Remove the stray references T005 found from core files, or move them into the
  allow-list with a reason. Fix every path T006 left unmatched. Run `scripts/preflight.sh` and
  check that all five criteria print OK.
- [X] T021 Re-run `scripts/conformance-digest.sh` and diff it against
  `specs/011-core-ecosystem-split/digest-before.json`. It must be identical. Run
  `scripts/gates.sh`. Record both results and the preflight output in
  `checklists/implementation-review.md`, under "Preflight".

**Checkpoint**: The boundary is enforced in one repository. The binding and the CLI see only
`behavior-engine`, and the digest is unchanged.

---

## Phase 3: User Story 1 — The core stands alone (Priority: P1) 🎯 MVP

**Goal**: `behavior-ir-core` holds the full semantics with its history. It builds, tests and runs
its conformance suite with no reference to the ecosystem.

**Independent Test**: quickstart §2. Clone the core alone and run `scripts/gates.sh`,
`scripts/check-consumer.sh` and the digest diff. History is present for moved files.

### Tests for User Story 1

- [X] T022 [P] [US1] Create `core:consumer/`, a crate outside the core workspace:
  - `Cargo.toml` with `[workspace]` (standalone) and only
    `behavior-engine = { path = "../crates/behavior-engine" }`. The path is replaced by a git rev
    in the release check.
  - `tests/capabilities.rs` with one test per FR-006b capability: build and admit a module, run
    an action, run a declared read, open an `InMemoryBackend` store and commit and replay, run a
    migration, verify with the solver absent (inconclusive is acceptable), run an intent through
    the authorization path, and check that `engine_info()` reports `engine == CARGO_PKG_VERSION`
    of the engine.

  First, write it in this repository as `consumer/`, so it is part of the extracted paths. Add
  rule `consumer/**` → core, which already exists in ownership.md. Check that it fails to
  compile when one re-export is removed from the facade.
- [X] T023 [P] [US1] Write `scripts/check-consumer.sh`. It runs `cargo test --manifest-path
  consumer/Cargo.toml`. With `--rev <sha>` it builds in a temp copy whose dependency is
  `behavior-engine = { git = "https://github.com/kallistoteles/behavior-ir-core", rev = "<sha>" }`.
  Add `exclude = ["consumer"]` to `[workspace]` in `Cargo.toml`.

### Implementation for User Story 1

- [ ] T024 [US1] Generate the extraction list:
  `scripts/check-ownership.py --list core > /tmp/core-paths.txt`. Then:
  1. Take a fresh `git clone --no-local` of this repository at the head after T023 (the preflight
     plus `consumer/` and `check-consumer.sh`) into a temp directory.
  2. Run `nix shell nixpkgs#git-filter-repo -c git filter-repo --paths-from-file
     /tmp/core-paths.txt` there.
  3. Check that `git log --oneline -- crates/behavior-core/src/read.rs` shows more than one
     commit.
- [ ] T025 [US1] In `../behavior-ir-core`:
  1. `git fetch <temp-clone> HEAD`.
  2. `git merge --allow-unrelated-histories FETCH_HEAD -m "Extract Behavior Core from behavior-ir
     <ecosystem sha> (feature 011)"`.
  3. Resolve the `README.md` and `LICENSE` conflicts: keep MIT, and keep the core README's
     tagline on top.
  4. Push to a branch `011-core-extraction`, not `main`, until Phase 4 is green.
- [ ] T026 [US1] Trim the core-side copies of the "both" files:
  - `core:Cargo.toml`: members are `behavior-core`, `-verify`, `-store`, `-cli` and `-engine`;
    remove `behavior-py`; keep `exclude = ["consumer"]`.
  - `core:Cargo.lock`: regenerate with `cargo metadata` offline; no version changes.
  - `core:.gitignore`: drop the Python-only entries except `.venv/`.
  - `core:flake.nix`: drop maturin and auditwheel; keep rust, z3, zig, cargo-zigbuild, python3
    (the scripts use it) and gh.
- [ ] T027 [US1] In `core:scripts/determinism-check.sh`, drop the `--ecosystem` section
  (T018). The default runs the core section. In `core:scripts/gates.sh`, drop steps 5–8
  (stage-cli, maturin, pytest, mypy) and step 12, and add `scripts/check-consumer.sh`.
- [ ] T028 [P] [US1] Rewrite `core:README.md`:
  - what the core is, with the tagline;
  - how to build and test (`nix develop -c scripts/gates.sh`);
  - the public contract: `behavior-engine`, the CLI, `schema/`, conformance fixtures;
  - "Specifications: core features are numbered 012 onward; 001–007, 009, 010 live here;
    ecosystem features (500+) live in behavior-ir";
  - a link to the ecosystem for the Python package.
- [ ] T029 [P] [US1] Trim `core:docs/versioning.md`. Keep the version table, the release version
  policy, the compatibility promise and the releases section for the engine. Add
  `behavior-engine` and `api/engine-surface.txt` to the "removing or changing a public API
  element" rule. Remove `api/public-api.json` and the Python binding row; replace them with
  "Bindings: versioned by the ecosystem (behavior-ir)".
- [ ] T030 [P] [US1] In `core:.claude/skills/behavior-engine-development/SKILL.md`, replace paths
  and commands that assume the Python package (maturin, pytest) with the core gates. Add a
  section "The public door": every item a consumer needs goes through `behavior-engine` and
  `api/engine-surface.txt`.
- [ ] T031 [US1] Add Spec Kit to the core:
  - Copy `.specify/` (minus `feature.json`) and `.claude/skills/speckit-*` from this repository,
    so the Spec Kit version is identical.
  - Keep `core:.specify/memory/constitution.md` at 1.0.0 unchanged, apart from adding
    `scripts/gates.sh` to "Development Workflow and Quality Gates" as a PATCH amendment to 1.0.1
    via `/speckit-constitution` in the core.
  - Copy `specs/011-core-ecosystem-split/` to `core:specs/011-core-ecosystem-split/`, with a
    first line "Reference copy; owned by behavior-ir".
- [ ] T032 [US1] Run in a fresh clone of the `011-core-extraction` branch (quickstart §2):
  - `scripts/gates.sh`;
  - `scripts/check-consumer.sh`;
  - `scripts/conformance-digest.sh | diff - specs/011-core-ecosystem-split/digest-before.json`,
    which must print nothing (SC-009);
  - `scripts/check-boundary.sh`;
  - `rg -l 'behavior-py|behavior\._engine|python/behavior'` outside the allow-list, which must
    find nothing (SC-001).

  Record the test counts against the T021 counts in `checklists/implementation-review.md`, under
  "Core alone".

**Checkpoint**: The core stands alone on its extraction branch (US1 acceptance 1–3).

---

## Phase 4: User Story 6 (core half) — Core CI and the first Core Release (Priority: P2)

**Goal**: Core validation and releases run from scripts, orchestrated by `core-ci` and
`core-release`. The tag `v0.10.1` produces the assets the ecosystem pins.

**Independent Test**: A pull request to the core runs `core-ci`. A wrong-version tag is refused
before building. A local `release-verify` matches the published checksums.

### Tests for User Story 6 (core)

- [ ] T033 [P] [US6] Write `core:scripts/check-tag.sh <ref>`. It refuses (exit 1) and names the
  values in each of these cases:
  - the tag is not annotated (`git cat-file -t` ≠ `tag`);
  - `${ref#v}` ≠ `[workspace.package] version`, naming both versions;
  - the tag's commit is not an ancestor of `origin/main`;
  - with `GITHUB_TOKEN` or `GH_TOKEN` set: any required check of the commit is not `success`,
    queried with `gh api repos/{repo}/commits/<sha>/check-runs`. The refusal names each failing or
    missing check. Without a token (a local run) the script prints
    `check-tag: CI status not verified (no token)` and continues.

  Before writing the script, write `core:scripts/tests/check-tag.bats`-style shell tests in
  `core:scripts/tests/test_check_tag.sh`. They build a temp repository with each case,
  including a stubbed `gh` that reports a green and a red check run, and must fail before the
  script exists.
- [ ] T034 [P] [US6] Write `core:scripts/tests/test_release_build.sh`. It runs
  `scripts/release-build.sh 0.10.1 <tmp>` twice and asserts:
  - identical `SHA256SUMS`;
  - the presence of `release-manifest.json`, `SHA256SUMS`, `behavior-conformance-0.10.1.tar.gz`
    and `behavior-0.10.1-x86_64-linux-manylinux_2_28`;
  - that the manifest has
    `format == "behavior.core_release_manifest.v1"`, `release`, `tag`, `commit`, `versions`
    equal to `behavior engine-info`, `public_surface.crate == "behavior-engine"`,
    `public_surface.items_sha256 == sha256(api/engine-surface.txt)`, `artifacts` sorted by
    `file` and excluding the manifest and `SHA256SUMS`, and
    `solver == {"name": "z3", "version": "4.16.0"}`;
  - that the manifest is canonical JSON with a trailing newline.

  It fails before T035.

### Implementation for User Story 6 (core)

- [ ] T035 [US6] Rewrite `core:scripts/release-build.sh <version> <out>`. It runs under
  `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)`,
  `RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo"`
  and `CARGO_INCREMENTAL=0`. It builds:
  1. the CLI with `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28 -p
     behavior-cli`, copied to `<out>/behavior-<v>-x86_64-linux-manylinux_2_28`;
  2. `behavior-conformance-<v>.tar.gz` from `schema/`, `tests/fixtures/` and `README.md` with
     `tar --sort=name --mtime=@$SOURCE_DATE_EPOCH --owner=0 --group=0 --numeric-owner | gzip -n`;
  3. `SHA256SUMS`;
  4. `release-manifest.json` per `data-model.md`.

  Make T034 pass.
- [ ] T036 [US6] Rewrite `core:scripts/release-check.sh <version> [--skip-gates]`:
  - step 1: `scripts/gates.sh` and `scripts/check-consumer.sh`;
  - step 2: build twice into temp directories and compare `SHA256SUMS` (reproducibility);
  - step 3: the built CLI's `engine-info` reports `engine == <version>`;
  - step 4: in a clean `env -i PATH=/usr/bin:/bin` shell, the CLI admits and evaluates
    `tests/fixtures/wire/valid/invoice.json` with its request;
  - step 5: the conformance tarball unpacks to files whose digest equals
    `scripts/conformance-digest.sh` over the tree.

  Drop every Python step from today's script.
- [ ] T037 [US6] Rewrite `core:scripts/release.sh <version>`:
  `check-tag.sh v<version>` → `release-check.sh` → `release-build.sh <v> dist/v<v>`, and write
  `dist/v<v>/NOTES.md`, which lists the versions from the manifest. It neither tags nor
  publishes.
- [ ] T038 [P] [US6] Write `core:scripts/release-verify.sh v<version>`. It downloads the release
  assets with `gh release download`, rebuilds from the tag in a `git worktree`, and diffs the
  `SHA256SUMS`. It prints `release-verify: identical` or the differing files.
- [ ] T039 [P] [US6] Write `core:scripts/check-workflows.sh`. It fails if any
  `.github/workflows/*.yml`, or a script it calls, references `.specify`, `.claude`, `specs/` or
  `feature.json` (FR-035). It also fails if any `uses:` is not pinned by a 40-hex SHA. Add it to
  `core:scripts/gates.sh`.
- [ ] T040 [US6] Write `core:.github/workflows/core-ci.yml` per `contracts/workflows.md`:
  - triggers: `pull_request` and `push` to `main` and `dev`;
  - `permissions: contents: read`;
  - concurrency cancels superseded runs;
  - job `gates` runs `nix develop -c scripts/gates.sh`;
  - job `consumer` runs `nix develop -c scripts/check-consumer.sh`;
  - nix through `DeterminateSystems/nix-installer-action@<sha>` and
    `actions/checkout@<sha>`.

  No other logic.
- [ ] T041 [US6] Write `core:.github/workflows/core-release.yml`:
  - trigger: `push: tags: ['v*']` only;
  - `permissions: contents: write`;
  - no concurrency cancel;
  - steps: checkout with `fetch-depth: 0`, `nix develop -c scripts/check-tag.sh
    "$GITHUB_REF_NAME"`, `nix develop -c scripts/release.sh "${GITHUB_REF_NAME#v}"`, then
    `gh release create "$GITHUB_REF_NAME" dist/$GITHUB_REF_NAME/* --verify-tag --notes-file
    dist/$GITHUB_REF_NAME/NOTES.md`;
  - then `nix develop -c scripts/check-consumer.sh --rev "$(git rev-parse
    "$GITHUB_REF_NAME^{commit}")"` (FR-006d). It needs the pushed tag, so it runs after
    publishing. If it fails, the workflow fails, and the fix is a new patch release; the tag is
    never moved.
- [ ] T042 [US6] Bump `core:Cargo.toml` `[workspace.package] version` to `0.10.1`. Add a
  "Release 0.10.1 is a patch release" paragraph to `core:docs/versioning.md`: additive public
  Rust surface; no identity, format or verifier change. Update the version pins in the core tests
  that assert the release version (`core:crates/behavior-core/tests/versions.rs`,
  `core:crates/behavior-cli/tests/cli_engine_info.rs`).
- [ ] T043 [US6] Configure branch protection on `core:main` requiring `gates` and `consumer`.
  This is a precondition of T044. Then merge `011-core-extraction` into `core:main` through a
  pull request and confirm that `core-ci` is green. Push a test tag `v0.10.2` and confirm that `core-release` refuses before building,
  naming 0.10.2 and 0.10.1. Delete only that unreleased test tag.
- [ ] T044 [US6] Tag `git tag -a v0.10.1 -m "Behavior Core 0.10.1"` on the green `main` commit
  and push it. Confirm that the GitHub Release has the four assets. Run
  `scripts/check-consumer.sh --rev <v0.10.1 sha>` locally; it must pass. Run
  `scripts/release-verify.sh v0.10.1` locally; it must print identical (SC-011, core). Record the
  release URL, the commit SHA and the asset checksums in `checklists/implementation-review.md`,
  under "Core Release".

**Checkpoint**: Core v0.10.1 exists as an immutable tag with verified assets.

---

## Phase 5: User Story 2 — The ecosystem uses only the public core contract (Priority: P1)

**Goal**: This repository becomes the ecosystem. It consumes the core only as the pinned v0.10.1
release, through `behavior-engine`, with no core checkout.

**Independent Test**: quickstart §4. Run the pin check, the surface check, fetch-core and the
gates. Each of the three negative checks fails and names its offender.

### Tests for User Story 2

- [ ] T045 [P] [US2] Write `scripts/tests/test_check_core_pin.sh`. It covers four cases:
  - a matching pin passes;
  - a `Cargo.toml` rev ≠ `core-release.json` commit fails, naming both;
  - a `Cargo.lock` source rev ≠ commit fails;
  - an active `[patch."https://github.com/kallistoteles/behavior-ir-core"]` in
    `.cargo/config.toml` fails, naming the path. The exception is
    `BEHAVIOR_DEV_CORE_PATH` set while `CI` is unset.

  It fails before T049.
- [ ] T046 [P] [US2] Write `scripts/tests/test_fetch_core.sh`. With a local fake release
  directory (`BEHAVIOR_CORE_RELEASE_DIR`) it checks:
  - matching checksums unpack into `.core/0.10.1/` and install the CLI at
    `python/behavior/_bin/behavior` with mode 0755;
  - a corrupted tarball fails, naming the file, the expected and the actual checksum;
  - a CLI whose `engine-info` engine ≠ the pinned version fails, naming both.

  It fails before T050.
- [ ] T047 [P] [US2] Write `python/tests/test_versions.py::test_a_skewed_core_is_refused`. It
  monkeypatches the compiled engine's version and the declared core version so they differ, then
  imports through `behavior._versions.check_core()`. It asserts an `ImportError` whose message
  names both versions. It also writes
  `test_versions_report_the_core`: `behavior.versions()["core"] == {"version": …, "commit": …}`
  from `core-release.json`. It also writes `test_binding_and_core_versions_may_differ`: with
  ecosystem version 0.11.0 and declared core 0.10.1 (engine 0.10.1), the import succeeds, and
  `versions()["binding"]` is the ecosystem version. All three fail before T052.

### Implementation for User Story 2

- [ ] T048 [US2] Remove the core paths in one commit, "Move Behavior Core to behavior-ir-core
  (feature 011)". Delete every path whose owner is `core` in `contracts/ownership.md`, using
  `scripts/check-ownership.py --list core-only`. Add `--list core-only` to the script: it
  excludes `both` rules. Keep `specs/011-*`. Workspace changes:
  - `Cargo.toml`: `members = ["crates/behavior-py"]`, with `[workspace.dependencies]`
    `behavior-engine = { git = "https://github.com/kallistoteles/behavior-ir-core", rev =
    "<v0.10.1 sha>" }`;
  - `Cargo.lock`: regenerate with `cargo update -p behavior-engine` only.
- [ ] T049 [US2] Write `core-release.json` at the repository root, exactly per `data-model.md`:
  `format` is `behavior.core_pin.v1`, plus `version`, `tag`, `commit`, `repository`, and
  `assets.conformance` and `assets.cli`, each with `file` and `sha256` from T044. It is
  canonical JSON. Write `scripts/check-core-pin.sh`, which checks invariants 1–4 of
  `data-model.md`, using `cargo metadata` for the resolved source. Make T045 pass.
- [ ] T050 [US2] Write `scripts/fetch-core.sh`. It reads `core-release.json`, downloads both
  assets with `gh release download <tag> -R kallistoteles/behavior-ir-core`, or copies them from
  `BEHAVIOR_CORE_RELEASE_DIR`. It then:
  1. verifies the checksums;
  2. unpacks the tarball to `.core/<version>/`;
  3. installs the CLI to `python/behavior/_bin/behavior`;
  4. checks the CLI's `engine-info`;
  5. prints `export BEHAVIOR_CORE_DIR=…`.

  Add `.core/` to `.gitignore`. Make T046 pass. Replace `scripts/stage-cli.sh` with a call to
  `fetch-core.sh`, and delete `stage-cli.sh`.
- [ ] T051 [US2] Point the Python tests at the fetched core. In `python/tests/conftest.py`, set
  `CORE_DIR = Path(os.environ.get("BEHAVIOR_CORE_DIR", REPO_ROOT / ".core" / <pinned version>))`
  and `FIXTURES = CORE_DIR / "tests" / "fixtures"`. Fail with an instruction to run
  `scripts/fetch-core.sh` when the directory is missing. Update the hard-coded `ROOT / "tests" /
  "fixtures"` and `ROOT / "schema"` uses in `python/tests/test_binding_equivalence.py` and
  `python/tests/test_schema.py`, and any other `rg 'tests/fixtures|"schema"' python/tests` hits,
  to use `CORE_DIR`.
- [ ] T052 [US2] Make the core version visible and refuse skew:
  - `scripts/release-build.sh` and `maturin develop` (via a `build.rs` in
    `crates/behavior-py` reading `../../core-release.json`) embed the declared core version and
    commit as `env!("BEHAVIOR_CORE_VERSION")` and `env!("BEHAVIOR_CORE_COMMIT")`.
  - `_engine.engine_info()` adds `"core": {"version", "commit"}`.
  - `python/behavior/_versions.py` gains `check_core()`. It compares `engine_info()["engine"]`
    with the declared core version and raises `ImportError("behavior: built against core <a>
    but declares core <b>")`. `python/behavior/__init__.py` calls it at import.
  - Replace `_check_versions(__version__, _engine.ENGINE_VERSION)` in
    `python/behavior/__init__.py`: the binding version no longer has to equal the engine
    version. The binding's own version is the ecosystem release version and is reported as
    `versions()["binding"]`.
  - `behavior.versions()` returns `core`.

  Update `python/behavior/_engine.pyi` and the `versions` entry in `api/public-api.json`. Make
  T047 pass.
- [ ] T053 [US2] Extend `scripts/check-public-surface.sh --consumer` (now the ecosystem's only
  mode). It also fails if any tracked ecosystem file matches ownership rules 2–8, 10, 12, 14, 15
  or 19, which would be a copied core file. Remove the provider mode from the ecosystem copy.
- [ ] T054 [US2] Trim the ecosystem's copies of the "both" files:
  - `scripts/determinism-check.sh`: keep only the `--ecosystem` section, using
    `$BEHAVIOR_CORE_DIR` and the bundled CLI.
  - `scripts/gates.sh`: `check-core-pin.sh`, `check-public-surface.sh --consumer`,
    `fetch-core.sh`, `cargo fmt --check`, clippy for `behavior-py`, `maturin develop`, pytest,
    mypy, then `determinism-check.sh`.
  - `docs/versioning.md`: the package policy; link to the core's format table. The Binding row
    becomes "the ecosystem release version; requires exactly the core release in
    `core-release.json` (checked at import)". New row "Core: the exact core release a package
    bundles (`core-release.json`)".
  - `README.md`: the ecosystem's purpose, install, the dev loop (`fetch-core.sh`, then
    `maturin develop`), the local core override (`BEHAVIOR_DEV_CORE_PATH` plus an untracked
    `.cargo/config.toml` patch, never in CI), and "Specifications: ecosystem features are
    numbered 500 onward; 008 and 011 live here; core features live in behavior-ir-core".
- [ ] T055 [US2] Run quickstart §4 in a fresh clone of this repository, with no
  `../behavior-ir-core` visible (`mv` it away temporarily):
  - `scripts/gates.sh` passes;
  - `scripts/conformance-digest.sh --core-dir .core/0.10.1` matches `digest-before.json` except
    the `cli:` keys, which are compared through the bundled CLI and must match too (SC-003,
    SC-008);
  - the three negative checks fail, each naming its offender.

  Record the results in `checklists/implementation-review.md`, under "Ecosystem".

**Checkpoint**: The ecosystem builds and tests against the released core alone (US2 acceptance
1–3).

---

## Phase 6: User Story 3 — One semantic truth across authoring paths (Priority: P2)

**Goal**: There is at least one Python-versus-wire equivalence pair per wire IR form, and a
drift names the fixture and the item.

**Independent Test**: quickstart §5. `test_binding_equivalence.py` passes, and an injected DSL
change fails with the fixture and item hash named.

- [ ] T056 [P] [US3] Write
  `python/tests/test_binding_equivalence.py::test_drift_names_the_fixture_and_item`. It
  monkeypatches one DSL emission, for example reversing an entity's field order, then runs the
  comparison helper. It asserts that the failure message contains the fixture name and the
  differing item name and hash. It fails if the current helper reports only a boolean or a whole
  document diff.
- [ ] T057 [P] [US3] Write `python/tests/test_binding_equivalence.py::test_every_wire_form_has_a_pair`.
  It asserts that `DOMAINS` covers, by inspecting each pair's wire document, at least one module
  with each of these:
  - entities and types;
  - lifecycle actions (`create`/`remove`);
  - queries and module invariants;
  - fixed-scale or exact arithmetic;
  - a migration document;
  - a declared read.

  It fails on migrations and reads.
- [ ] T058 [US3] Make the comparison helper in `python/tests/test_binding_equivalence.py` report
  per item: compute both admitted modules' item hash maps and report the first differing
  `(kind, name, wire hash, python hash)` with the fixture name. Make T056 pass.
- [ ] T059 [US3] Add the reads pair: `examples/lab_reads/model.py` against
  `$BEHAVIOR_CORE_DIR/tests/fixtures/reads/modules/lab.json`. Compare behavior versions and every
  item hash, including `Kind::Read` items. If the DSL output differs, the wire fixture is the
  authority. Fix the DSL emission only if it is a binding bug. Otherwise record the difference as
  a finding and do not change core fixtures (FR-021).
- [ ] T060 [US3] Add the migrations pair: `examples/schema_evolution/` (the Python migration
  definition) against the matching `$BEHAVIOR_CORE_DIR/tests/fixtures/migration/` document.
  Compare the migration's canonical identity and the source and target behavior versions. Make
  T057 pass.
- [ ] T061 [US3] Write `python/tests/test_binding_equivalence.py::test_dsl_reproduces_frozen_python_fixtures`.
  For each `$BEHAVIOR_CORE_DIR/tests/fixtures/wire/python/*.json`, build the same module from
  `python/tests/fixtures/*_model.py` and assert byte equality of the canonical wire (ownership
  rule 6).

**Checkpoint**: SC-004 holds in ecosystem CI.

---

## Phase 7: User Story 4 — Users install one thing (Priority: P2)

**Goal**: The released wheel bundles the binding, the core engine and the core's `behavior` CLI,
and reports both releases.

**Independent Test**: quickstart §6. A clean-venv install, then the smoke scenario, the skill
examples, `versions()["core"]` and `behavior engine-info`, which agree.

- [ ] T062 [P] [US4] Extend `release/smoke.py` to assert that `behavior.versions()["core"]`
  matches `core-release.json`, and that `subprocess.run(["behavior", "engine-info"])` reports
  `engine == versions()["core"]["version"]`. Run it against the current dev install; it fails
  until T063 is done.
- [ ] T063 [US4] Update `scripts/release-build.sh` (ecosystem):
  - run under the reproducibility environment of T035 (`SOURCE_DATE_EPOCH`, `RUSTFLAGS` remaps);
  - call `scripts/fetch-core.sh` before `maturin build`, so the verified core CLI is in the
    wheel;
  - add the `core` section to `release-manifest.json` (`version`, `tag`, `commit`, `repository`
    from `core-release.json`; data-model.md);
  - take `versions` from the bundled CLI's `engine-info`, not from `cargo run -p behavior-cli`;
  - replace the assertion `versions["engine"] == <release version>` with
    `versions["engine"] == core-release.json version`. The release version (the ecosystem's)
    is checked against `Cargo.toml` `[workspace.package] version`.
- [ ] T064 [US4] Update `scripts/release-check.sh` (ecosystem):
  - step 1: `scripts/gates.sh`;
  - step 2: also run `auditwheel show` on the bundled CLI inside the unpacked wheel;
  - new step: the wheel's `behavior/_bin/behavior` is byte-identical to the pinned CLI asset;
  - new step: build twice and compare `SHA256SUMS`;
  - step 7 (versions): compare against the manifest, including `core`;
  - steps 3–9 otherwise stay as they are.

  Make `python/tests/test_release_scripts.py` match the new steps: update the expected step
  names first, and see the test fail.
- [ ] T065 [US4] Run `scripts/release-check.sh 0.10.1`. It must print `release-check: OK`. Then
  run quickstart §6 in a clean venv and record the `versions()["core"]` and `engine-info` output
  in `checklists/implementation-review.md` (SC-005).

**Checkpoint**: One install gives the user everything, and the package names its core.

---

## Phase 8: User Story 6 (ecosystem half) — Ecosystem CI and release (Priority: P2)

**Goal**: `ecosystem-ci` and `ecosystem-release` orchestrate the ecosystem scripts. The first
ecosystem release, `v0.10.1`, states the core it bundles.

**Independent Test**: An ecosystem pull request runs the four jobs with no core checkout. A
wrong tag is refused before building. Local and published checksums match.

- [ ] T066 [P] [US6] Copy `core:scripts/check-tag.sh` with its tests to `scripts/check-tag.sh`
  and `scripts/tests/test_check_tag.sh`, adapted to read the version from `Cargo.toml`
  `[workspace.package]`. Run the tests; the copy carries its own tests and must pass.
- [ ] T067 [P] [US6] Copy `core:scripts/check-workflows.sh` and `core:scripts/release-verify.sh`
  to `scripts/`, adapted to the ecosystem's assets. Add `check-workflows.sh` to
  `scripts/gates.sh`.
- [ ] T068 [US6] Write `.github/workflows/ecosystem-ci.yml` per `contracts/workflows.md`:
  - triggers: `pull_request` and `push` to `main` and `dev`;
  - `permissions: contents: read`;
  - jobs `pin` (`check-core-pin.sh`), `surface` (`check-public-surface.sh --consumer`), `gates`
    (`fetch-core.sh` then `gates.sh`) and `package` (`release-check.sh --skip-gates <version>`);
  - every step is `nix develop -c …`;
  - `GH_TOKEN: ${{ secrets.CORE_READ_TOKEN || github.token }}` for `fetch-core.sh` and for
    cargo's git fetch, with `CARGO_NET_GIT_FETCH_WITH_CLI: true`;
  - actions pinned by SHA.
- [ ] T069 [US6] Write `.github/workflows/ecosystem-release.yml`:
  - trigger: tags `v*` only;
  - `permissions: contents: write`;
  - steps: `check-tag.sh`, `fetch-core.sh`, `release.sh <v>`, then `gh release create` with
    notes that state the bundled core version, tag and commit (FR-029).
- [ ] T070 [US6] Amend the ecosystem constitution through `/speckit-constitution` to 1.1.0 (MINOR).
  Add the section "Bindings and Packaging":
  - bindings never implement core semantics;
  - the only core dependency is `behavior-engine`;
  - the core is pinned exactly in `core-release.json`;
  - the equivalence fixtures are required;
  - the package bundles the core.

  Name `scripts/gates.sh` in "Development Workflow and Quality Gates", and state the 500+
  feature range. File: `.specify/memory/constitution.md`.
- [ ] T071 [US6] Merge the ecosystem branch to `main` through a pull request and confirm that
  `ecosystem-ci` is green with no core checkout. Configure branch protection requiring `pin`,
  `surface`, `gates` and `package`. Confirm that the merge created no release (US6
  acceptance 3).
- [ ] T072 [US6] Bump `Cargo.toml` `[workspace.package] version` to `0.10.1`. Add a
  "Release 0.10.1" paragraph to `docs/versioning.md`: split into core and ecosystem; bundles core
  v0.10.1; no identity or format change. Tag
  `git tag -a v0.10.1 -m "Behavior 0.10.1 (core v0.10.1)"` and push. Confirm the GitHub Release
  and its notes. Run `scripts/release-verify.sh v0.10.1`; it must print identical (SC-011,
  ecosystem). Record the results in `checklists/implementation-review.md`, under "Ecosystem
  Release".

**Checkpoint**: Both repositories validate and release the same way locally and in CI.

---

## Phase 9: User Story 5 — Deciding where a new concept belongs (Priority: P3)

**Goal**: The admission criterion, the classification table and the model rules are written
down, and a tested state-machine lowering shows a model compiling to plain IR.

**Independent Test**: quickstart §7. The lowering test passes. The document classifies the 12
concepts.

- [ ] T073 [P] [US5] Write `models/examples/state_machine/test_state_machine.py`. Given a state
  machine `{states: [DRAFT, SUBMITTED], transitions: [{name: submit, from: DRAFT, to:
  SUBMITTED}]}` over an `Order.status` enum, it asserts:
  - `lower(sm)` returns a module whose `submit` action has exactly one precondition
    (`status == DRAFT`), one effect (`set status = SUBMITTED`) and one postcondition
    (`status == SUBMITTED`);
  - the module admits through `behavior.admit`;
  - the wire document contains no key or string naming "state machine";
  - lowering twice gives byte-identical wire.

  It fails because `lower` does not exist.
- [ ] T074 [US5] Write `models/examples/state_machine/lower.py`. It is a pure function from the
  state machine dict to a DSL module using only public `behavior` API (`entity`, `action`,
  `requires`, `set_`, `ensures`). Make T073 pass. Add `models` to `testpaths` in
  `pyproject.toml`.
- [ ] T075 [P] [US5] Write `core:ARCHITECTURE.md`:
  - the layer diagram;
  - the terminology (Core, Binding, Model, Adapter);
  - the dependency direction with the forbidden edges;
  - the core admission criterion ("Must the evaluator understand this construct for its
    semantics to be correct?");
  - the 12-row classification table exactly as in the spec: State machine, Workflow, Approval
    flow and CRUD convenience API are Model/library; Entity identity, Option semantics, Exact
    arithmetic, Queries, Schema migrations, Persistence/replay and Command intents are Core;
    State-machine visualization is Tooling;
  - the invariants.

  It never uses "plugin" or "extension" for models (FR-013). Add a reference from
  `core:PRINCIPLES.md` (new closing section "Where a concept belongs").
- [ ] T076 [P] [US5] Write `models/README.md`:
  - models lower deterministically and completely (FR-009);
  - one authoritative lowering per model (FR-010);
  - the procedure for a non-lowerable construct: redesign it, or propose a core primitive in a
    core feature (FR-011);
  - optional model identity, with the module hash authoritative (FR-012);
  - a link to the core's `ARCHITECTURE.md`;
  - a pointer to `examples/state_machine/`.
- [ ] T077 [P] [US5] Add a short "Where does it belong?" section to
  `skills/behavior-authoring/SKILL.md` and `skills/behavior-application/SKILL.md`. It states the
  criterion and links `models/README.md`. Run the skill checks (`python/tests/test_skills.py`).
- [ ] T078 [US5] Add a check to `scripts/check-workflows.sh`'s sibling,
  `scripts/check-terms.sh`, called from `gates.sh` in both repositories: `rg -i
  '\b(plugin|extension)s?\b'` over `models/` (ecosystem) and `ARCHITECTURE.md` (core) must find
  nothing. Write the check, see it fail on a planted word, then remove the word.

**Checkpoint**: The rule exists before the first model.

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T079 Prove every required gate (SC-010). In each repository, open one throwaway pull
  request per gate listed in `contracts/workflows.md` §Proving each gate. Confirm each one turns
  its job red, then close it unmerged. Record the run URLs in `checklists/implementation-review.md`.
- [ ] T080 [P] Verify Spec Kit numbering (SC-012) without keeping the features:
  - In a scratch clone of each repository, run `.specify/scripts/bash/create-new-feature.sh
    --json --number 500 "probe"` (ecosystem) and the same with no number (core). They must
    create `specs/500-probe` and `specs/012-probe`.
  - A second ecosystem call with no number must create `501`.

  Record the outputs, then delete the scratch clones.
- [ ] T081 [P] Classify-by-document check (SC-007): give the 12 concepts to a reviewer, or use a
  scripted lookup over the `ARCHITECTURE.md` table that maps each concept to exactly one layer.
  Record the result in `checklists/implementation-review.md`.
- [ ] T082 [P] Update `PRINCIPLES.md` references and every in-repo link that pointed at moved
  files:
  - `rg -n 'docs/(persistence|verification)\.md|PRINCIPLES\.md|tests/fixtures|schema/'` over
    `README.md`, `skills/` and `docs/` in the ecosystem;
  - rewrite each hit to a URL into `behavior-ir-core` at tag `v0.10.1`, or to
    `$BEHAVIOR_CORE_DIR` in scripts.
- [ ] T083 [P] Update the auto-memory roadmap
  `~/.claude/projects/-home-kalle-repos-deterministic-ai-system/memory/skills-library-next.md`:
  011 is done; general invocation is next as **core 012**, in `behavior-ir-core`; ecosystem
  features are numbered 500+.
- [ ] T084 Complete `checklists/implementation-review.md` with these sections:
  - Principles;
  - Constitution, with a test-first evidence note for each check task (T005–T009, T015,
    T022–T023, T033–T034, T045–T047, T056–T057, T062, T064, T073, T078);
  - Requirements, a FR-001 to FR-035 table with evidence;
  - Success criteria SC-001 to SC-012;
  - Deviations;
  - Follow-ups, including that `engine-info` does not report the migration IR version (adding it
    is a contract change for a later core feature).

---

## Dependencies & Execution Order

### Phase dependencies

```text
Setup (1) → Preflight (2) → US1 core alone (3) → US6-core release (4)
                                                       │ v0.10.1 tag, assets
                                                       ▼
                                     US2 ecosystem (5) → US3 (6) → US4 (7) → US6-ecosystem (8)
US5 (9): T073–T074, T076–T077 need only Phase 5; T075 needs Phase 3 (core repo exists).
Polish (10): after all.
```

- The **preflight (Phase 2)** blocks everything. The extraction must start from a tree where the
  boundary is already checked.
- **US2 depends on the first Core Release (T044).** Its pin needs a tag, a commit and asset
  checksums.
- **US3 and US4** need US2's fetched core (`$BEHAVIOR_CORE_DIR`, the bundled CLI).
- **The ecosystem release (T072)** needs US3 and US4 green, because `release-check` runs their
  tests.

### Within phases

- The check and test tasks come first, and each must be seen failing: T005–T009 before
  T010–T021; T022–T023 before T024 (T024 extracts the tree that includes them); T033–T034 before T035; T045–T047 before T048–T052;
  T056–T057 before T058–T061; T062 before T063; T073 before T074.
- T010 → T011 → T012 → T013 → T014 run in sequence: same crates, and the inventory feeds the
  facade.
- T024 → T025 → T026 → T027 → T031 → T032 run in sequence: extraction steps.

## Parallel Opportunities

- **Phase 1**: T002 and T003.
- **Phase 2 checks**: T005, T006, T007, T008 and T009 are separate scripts and tests.
- **Phase 3**: T022 and T023 together; then T028, T029 and T030, which are separate core docs.
- **Phase 4**: T033 and T034; then T038 and T039.
- **Phase 5 tests**: T045, T046 and T047.
- **Phase 6**: T056 and T057.
- **Phase 9**: T073, T075, T076 and T077 (T075 is in the core repository).
- **Phase 10**: T080, T081, T082 and T083.

### Example: Phase 2 checks

```text
Task: "T005 scripts/check-boundary.sh"
Task: "T006 scripts/check-ownership.py"
Task: "T007 scripts/check-public-surface.sh"
Task: "T009 cli_engine_info.rs::engine_info_comes_from_the_engine"
```

## Implementation Strategy

### MVP (US1)

Phases 1–3: the preflight passes and the core stands alone on its extraction branch, with
history and an unchanged digest. This is the irreversible part, and it is fully checkable before
any tag or deletion in the ecosystem. **Stop and review** before Phase 4 tags anything. Tags are
never moved.

### Incremental delivery

1. Phases 1–3, MVP: review `core:011-core-extraction`.
2. Phase 4: core CI and the core release `v0.10.1`. This is the first irreversible public step,
   so confirm with the user before pushing the tag.
3. Phase 5: the ecosystem conversion. The core paths are removed here; confirm before T048.
4. Phases 6–7: equivalence and the one-install package.
5. Phase 8: ecosystem CI and the release `v0.10.1`. Confirm before pushing the tag.
6. Phase 9: the architecture documents and the lowering example.
7. Phase 10: gate proofs, numbering probe, review.

### Outward-facing steps that need explicit approval

Each of these publishes, or cannot be undone. Per the project rules, they run only when the user
asks:

- pushing to `behavior-ir-core` (T025, T043);
- branch protection (T043, T071);
- the release tags (T044, T072);
- deleting the core paths here (T048);
- opening throwaway pull requests (T079).
