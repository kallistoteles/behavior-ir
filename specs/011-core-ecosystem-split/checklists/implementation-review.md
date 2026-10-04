# Implementation Review: Core and Ecosystem Repositories

**Feature**: 011 · **Started**: 2026-10-04

## Baseline

- **Starting state (T001):**
  - Feature 010 is committed as `e88f4c3` on `010-first-class-reads`. `dev` was fast-forwarded
    to it and both branches were pushed.
  - The 011 branch was created from `dev`, not `main`. `dev` is the integration branch; `main`
    still lags at `9a1dc76`, behind features 009 and 010.
- **Release check:** `scripts/release-check.sh --skip-gates` on 0.10.0 printed
  `release-check: OK (behavior-0.10.0-cp313-abi3-manylinux_2_28_x86_64.whl)`. The script takes
  no version argument; the tasks' `release-check.sh 0.10.0` was a slip.
- **Conformance digest (T003, T004):** `digest-before.json` holds 601 keys:
  - 459 fixture and schema files;
  - 136 core outputs and 6 ecosystem outputs of the determinism check.

  Two runs were byte-identical. The determinism check gained a digest hook: with
  `BEHAVIOR_DIGEST_DIR` set, `run_twice` and the store examples record each output's SHA-256
  under its section. The hook changes no output.
- **Core repository access (T004a):** `behavior-ir-core` is **public** (user's answer,
  2026-10-04). No `CORE_READ_TOKEN` is needed. CI fetches the core and its releases
  anonymously.
- **Tooling (T002):** the dev shell has `cargo-zigbuild` and `gh`.

## Preflight

`scripts/preflight.sh` prints `boundary OK`, `surface OK`, `ownership OK`, `alone OK`,
`consumer OK`, then `preflight: OK`. `scripts/gates.sh` passes. The conformance digest is
byte-identical to `digest-before.json` (601 keys).

**Each criterion, seen failing first:**

| Criterion | Failing evidence on the T001 tree | Fix |
|---|---|---|
| boundary | `.claude/skills/behavior-engine-development/SKILL.md` named `python/behavior/` twice | The skill names the ecosystem repository instead (T020) |
| surface | `crates/behavior-engine/src/lib.rs does not exist` | Facade with 79 explicit re-exports and `engine_info` (T010–T012) |
| ownership | Passed on first run: every one of the 926 files matched a rule. The check itself was seen failing on a planted unmatched file | none needed |
| alone | `determinism-check.sh --core` did not exist | Determinism check split into sections (T018) |
| consumer | 64 lines: `behavior-py` depended on `behavior-core`, `-verify`, `-store` and `-cli`, plus inline uses | The binding depends on `behavior-engine` only (T014) |

**Other checks seen failing first:**
- `test_check_boundary.sh` failed 10 of 10 cases before `check-boundary.sh` existed.
- `engine_info_comes_from_the_engine` did not compile before the facade existed.
- `test_the_console_script_runs_the_bundled_binary` failed with `AttributeError` (`_cli.binary`).
- The external consumer (`consumer/`) passes 8 of 8 capability tests. Removing one re-export
  (`read::ReadSource`) stops it compiling and fails `check-public-surface.sh`, both naming the
  item.
- `test_check_tag.sh` failed 7 of 7 cases before `check-tag.sh` existed. The script was written
  early so the extraction carries it into the core.

**Placement:** core 729 files, ecosystem 151, both 46. `--list core` holds 775 files.

**Found during the preflight:**
- **A pre-existing flaky test.**
  - `behavior-verify` `another_solver_version_is_reported` failed about 1 run in 5 with
    `Spawn(…/z3, "Text file busy (os error 26)")`.
  - Cause: another test thread forked while the fake solver script was being written, so the
    child held the write handle (ETXTBSY).
  - Fix: the test retries the start, bounded to 100 × 10 ms, on that one error. 40 consecutive
    runs passed.
- **Historical specifications mention Python paths.** 001–010 describe the single repository.
  Rewriting them would falsify history, so `check-boundary.sh` allow-lists `specs/`, not only
  `specs/011-*` (contract amended).
- **Binary files matched.** The staged CLI binary matched `behavior_core` in the consumer check,
  which now skips binary files (`grep -I`).
- **Stale fixture paths.** A build directory shared between core copies at different temporary
  paths made test binaries look for fixtures under a deleted path. The preflight now uses one
  stable work directory (`PREFLIGHT_WORK_DIR`) for both the copy and its build.
- **Ownership rule 21.** It said "per script". It is now `both`: each repository trims its own
  scripts (T026, T027, T054).

## Core alone

The extraction is in `../behavior-ir-core`, branch `011-core-extraction`, **local only, not
pushed**.

- **History (FR-020):** `git filter-repo` kept the core path prefixes and dropped
  `crates/behavior-py/`. The result was merged with `--allow-unrelated-histories` (merge
  `ea071bd`) and has 26 commits. Its file set equals `check-ownership.py --list core` exactly
  (775 files). `crates/behavior-core/src/lib.rs` keeps all 10 of its commits; `read.rs` was
  created in 010, so it has one. The core's LICENSE is identical to this repository's.
- **Fresh clone of `011-core-extraction`:**
  - `scripts/gates.sh` passes;
  - 407 Rust tests pass and 0 fail: the 406 of 010 plus `engine_info_comes_from_the_engine`;
  - the conformance digest equals `digest-before.json` on every core key (595 = 459 files + 136
    core outputs) (SC-001, SC-009);
  - outside the allow-list, nothing names `behavior-py`, `behavior._engine` or
    `python/behavior`.
- **Core release check:** `scripts/release-check.sh --skip-gates` passes:
  - two builds give identical `SHA256SUMS`;
  - the CLI reports 0.10.1;
  - the CLI admits, evaluates and replays in an empty environment;
  - the conformance archive holds exactly the tracked schemas and fixtures.
- **Core commits (local):**
  - `chore: make the extracted core stand alone`;
  - `feat: Core Release pipeline`;
  - `docs: architecture of core and ecosystem`.
  The intermediate commits name scripts that the next commit adds, so only the last commit
  passes the gates.

## Deviations so far

- **Base branch:** branched from `dev` (the integration branch), not `main` (T001).
- **The CLI asset is a static musl binary** (`behavior-<v>-x86_64-linux-musl`), not
  manylinux_2_28. A glibc manylinux binary does not start on NixOS ("cannot run dynamically
  linked executables"), so it would have broken the wheel's `behavior` command there. The static
  binary runs on any x86_64 Linux and has no shared libraries for auditwheel to check. The core's
  `rust-toolchain.toml` gains the musl target. The contracts, data model, plan and research were
  updated.
- **Phase 4 scripts were pulled forward** into Phase 3: the release scripts, check-tag,
  check-workflows, the workflows and the 0.10.1 bump. The extracted release scripts still named
  Python paths, so the core's boundary check could not pass until they were rewritten.
  ARCHITECTURE.md (T075) and the core's terms check (T078, core part) were written at the same
  time, because the README links ARCHITECTURE.md.
- **The core's `gates.sh`** also runs `check-workflows.sh`, `check-terms.sh` and the script tests
  (`scripts/tests/test_*.sh`). The slow double release build (`slow_release_build.sh`) runs from
  `release-check.sh` instead.
- **`check-workflows.sh`** scans scripts named in a workflow only for `.specify`, `.claude/` and
  `feature.json`, not `specs/`. `check-boundary.sh` names `specs/` as its allow-list without
  reading it.

## Core Release

- **PR #1** (`011-core-extraction`): `core-ci` passed (`gates` 6m52s, `consumer` 1m4s). Core
  `main` was fast-forwarded to the checked commit `1baea68`, keeping the extracted history with
  no merge commit. Branch protection on core `main` requires `gates` and `consumer`, has no
  review requirement, and forbids force pushes and deletion.
- **`v0.10.1` failed safely.**
  - The annotated tag on `1baea68` passed `check-tag` locally. In `core-release`, `check-tag`
    refused it as "not annotated": `actions/checkout` fetches a pushed tag as a lightweight ref.
  - Nothing was built or published.
  - Under FR-028 the tag stays and is never released. The fix (fetch the tag object before
    checking it) and the bump to 0.10.2 went through PR #2, with green `core-ci`, and `main` was
    fast-forwarded to `aaded16`.
- **`v0.10.2` is the first Core Release**:
  <https://github.com/kallistoteles/behavior-ir-core/releases/tag/v0.10.2>, commit
  `aaded167d8a937e2357a0b01bf38f439c199a5e9`.
  - Every workflow step succeeded, including the external consumer built against the published
    revision (FR-006d).
  - SHA256SUMS:
    - `97a2958db8e1fa0ef78b0f4a66737806c9387e95a65723c27096cca120c0f8b1`
      `behavior-0.10.2-x86_64-linux-musl`
    - `2498ce228df242011adedaa5907627b287415e9a5266bb23206235611c264feb`
      `behavior-conformance-0.10.2.tar.gz`
    - `d8bffc0ae6dc2726beb94e8f72ffb05f99175580be1097ee0c7147e44cfba934`
      `release-manifest.json`
  - **SC-011:** `scripts/release-verify.sh v0.10.2` on this NixOS machine gives
    `release-verify: identical`. The CI build (Ubuntu 24.04) and the local build have the same
    checksums.
- **Wrong-version tag:** the planned test tag `v0.10.2`-as-wrong-version was not pushed. Its
  refusal is covered by `test_check_tag.sh` (`wrong_version`), and the real failure above
  exercised the refuse-before-build path in CI.
- **Follow-up:** the published release carries `NOTES.md` as a fifth asset, because the workflow
  uploads `dist/v<v>/*`. Write the notes outside the asset directory in a later patch.
