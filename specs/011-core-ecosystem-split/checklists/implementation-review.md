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
