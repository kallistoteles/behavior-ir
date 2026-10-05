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

## Ecosystem

The move commit is `Move Behavior Core to behavior-ir-core`: 773 files, of which 736 are
core-owned deletions; the history stays.

- **Pin:** core **v0.10.2** (commit `aaded16…`, the first published Core Release), not v0.10.1.
  The ecosystem release is 0.10.2, in step.
- **Fresh clone, isolated:** no `../behavior-ir-core` exists beside it.
  - `scripts/gates.sh` passes: the pin and surface checks, fetching the core from the GitHub
    Release, fmt, clippy, `maturin develop`, 232 Python tests, mypy, the determinism check,
    ownership and the script tests.
  - `behavior-engine` resolves from
    `git+https://github.com/kallistoteles/behavior-ir-core?rev=aaded16…`.
- **Conformance digest (SC-003, SC-008):** 475 keys, all equal to `digest-before.json`:
  - all 459 fixture and schema files, taken from the released archive;
  - the bundled CLI's 10 admissions, under the core's labels;
  - the 6 outputs of the smoke scenario and examples.
- **Negative checks (US2), each failing and naming its offender:**
  - `use behavior_core::admit;` gives `internal core crate used: crates/behavior-py/src/lib.rs:1`;
    a copied `docs/persistence.md` gives `core-owned file in the ecosystem`;
  - `rev` = `1baea68…` gives `Cargo.toml pins behavior-engine at 1baea68…, core-release.json
    declares aaded16…`;
  - a declared core of 0.10.9 gives `ImportError: behavior: built against core 0.10.2 but
    declares core 0.10.9`.
- **Tests seen failing first:**
  - `test_check_core_pin.sh`: 6 of 6 cases before the script existed;
  - `test_fetch_core.sh`: 5 of 5;
  - the three core-version tests in `test_versions.py`: an `AttributeError` and a missing pin
    file.
- **Adapted tests:**
  - The engine-skill test asserts that the skill now lives with the core.
  - The backend-trait test reads the pinned core's `behavior-store` source through
    `cargo metadata`.
  - Fixture and schema paths read `CORE_DIR` (`$BEHAVIOR_CORE_DIR`, default `.core/<version>`).
- **Removed here:** the core-only tools `check-boundary`, `check-consumer` and `preflight`
  (its job is done), and `stage-cli`, which `fetch-core` replaces.
- **The ecosystem's determinism check** runs the bundled core CLI over the released wire
  fixtures twice, with the core's digest labels, plus the smoke scenario and examples.

## Equivalence, package, models (local)

- **Equivalence (US3, SC-004):** `test_binding_equivalence.py` has 21 tests.
  - It covers the five domains plus three new pairs, each written in the DSL to reproduce a core
    fixture (`python/tests/fixtures/`):
    - `lab_model.py` against `reads/modules/lab.json`;
    - `cultures_v1.py` and `cultures_v2.py` against `migration/modules/`;
    - `cultures_migration.py` against `migration/valid/cultures_v1_to_v2.json`.
  - Every behavior version and item hash is identical, and so is the migration admission
    (excluding source locations).
  - The existing examples (`lab_reads`, `schema_evolution`) were not the same modules as the
    core's fixtures, which were written independently. The core fixtures stayed authoritative and
    unchanged (FR-021); the DSL counterparts were added.
  - A drift fails naming the fixture, the item and both hashes
    (`test_drift_names_the_fixture_and_item`, seen failing first).
  - `test_every_wire_form_has_a_pair` covers: entities and types, lifecycle, queries, module
    invariants, exact arithmetic, reads and migrations.
  - T061 was already covered by `test_wire.py`, which compares the DSL output byte for byte with
    the core's frozen `wire/python` fixtures, now read from the pinned release.
- **One install (US4, SC-005):** `scripts/release-check.sh` (full, with gates) printed
  `release-check: OK (behavior-0.10.2-cp313-abi3-manylinux_2_28_x86_64.whl)`.
  - Gates: 247 tests, including the slow release-script tests.
  - The wheel's `behavior/_bin/behavior` is byte-identical to the core's CLI asset and
    executable.
  - In a clean environment, `behavior.versions()` equals the manifest, including `core`.
  - The smoke scenario's new step 6 checks `versions()["core"]` and `behavior engine-info`. It
    only checks and emits nothing, so its output stays comparable.
  - Step 11: two rebuilds give identical `SHA256SUMS`.
- **Models (US5):**
  - `models/README.md` holds the rules (FR-009–FR-012) and the placement question.
  - `models/examples/state_machine/` contains the lowering and its 3 tests, seen failing first.
    One test expectation was corrected to the wire's `{"expr": …}` condition shape.
  - `models` is a pytest path.
  - `check-terms.sh` with its test is in the gates.
  - "Where does it belong?" sections were added to the authoring and application skills.
- **Workflows:** `ecosystem-ci` (jobs `pin`, `surface`, `gates`, `package`) and
  `ecosystem-release` (fetches the annotated tag, `check-tag` requires the four checks green,
  `release.sh`, publishes the wheel, `SHA256SUMS` and the manifest, with notes naming the bundled
  core). `check-workflows.sh` and its test are in the gates. `release.sh` no longer tags; it
  checks and builds an existing tag, as in the core.
- **Constitution:** 1.1.0 (MINOR), with a new section "Bindings and Packaging", `scripts/gates.sh`
  and the 500+ range.

## Polish (local)

- **SC-012:** in scratch clones, `create-new-feature.sh --number 500` gave `500-numbering-probe`
  in the ecosystem, and the next call without a number gave `501-second-probe`. In the core, a
  call without a number gave `012-core-probe`. The clones were deleted.
- **SC-007:** a scripted lookup over the core's `ARCHITECTURE.md` maps all 12 concepts to
  exactly the documented layer.
- **Links:** nothing in `skills/`, `docs/`, `examples/`, `release/` or `models/` points at a moved
  file. The README names the core's documents and links them at `v0.10.2`.
- **Memory:** the roadmap now names general invocation as core 012.

## Ecosystem Release

- **PR #1** (`011-core-ecosystem-split`): `ecosystem-ci` passed all four jobs (`pin` 53s,
  `surface` 44s, `gates` 2m5s with 243 tests, `package` 2m35s with release-check steps 2–11).
  Branch protection on `main` requires `pin`, `surface`, `gates` and `package`. `main` (from
  `9a1dc76`, bringing 009, 010 and 011) and `dev` were fast-forwarded to `33152a7`.
- **`v0.10.2` (ecosystem) failed safely.** `release.sh` reran the gates in the release workflow,
  and `test_check_tag.sh` inherited the job's `CHECK_TAG_REQUIRED`, so its `green_ci` case asked
  for checks the stub does not report. Nothing was built or published.
  - The fix: the test clears the variables it sets. This was reproduced locally first, and the
    same fix went to the core in core PR #3 (test-only, no release).
  - The fix shipped as **0.10.3** (PR #2).
- **`v0.10.3` was published, but `release-verify` reported DIFFERENT.**
  - Member by member, only maturin's CycloneDX SBOM (and `RECORD`) differed: it records the
    absolute build directory (`/home/runner/work/…` against a local path). The extension module
    and the bundled CLI were identical.
  - Fix (PR #3, **0.10.4**): `[tool.maturin.sbom] rust = false`. Release-check step 2 now refuses
    any wheel member that embeds a build directory (seen failing on the v0.10.3 wheel), and the
    check prints `SHA256SUMS`.
  - The `main` CI build of the exact commit and a local NixOS build gave the same wheel checksum
    (`394ef532…`) before tagging.
- **`v0.10.4` is the first reproducible ecosystem release:**
  <https://github.com/kallistoteles/behavior-ir/releases/tag/v0.10.4>.
  - `release-verify` printed `identical (v0.10.4)`.
  - The notes name Core 0.10.2 (`aaded16…`).
  - Installed from the published wheel with `--require-hashes`: `versions()` gives engine 0.10.2,
    core 0.10.2 (`aaded16…`) and binding 0.10.4, and `behavior engine-info` reports engine
    0.10.2.
- **The versions now differ:** ecosystem 0.10.4 bundles core 0.10.2 (FR-017).

## Requirements

| Requirement | Status | Evidence |
|---|---|---|
| FR-001 two repositories | ✓ | behavior-ir-core (Core Release v0.10.2), behavior-ir (v0.10.4) |
| FR-002, FR-003 core owns semantics, stands alone | ✓ | fresh core clone: gates, 407 tests, digest equal (Core alone) |
| FR-004 ecosystem contents | ✓ | binding, DSL, examples, skills, `models/`, package |
| FR-005 one-way dependency, checked | ✓ | `check-boundary.sh` in core gates |
| FR-006, FR-006b facade, explicit | ✓ | `behavior-engine`, 79 items, `api/engine-surface.txt`, `check-public-surface.sh` |
| FR-006c CLI not in the facade | ✓ | CLI and binding both consume the facade; the binding no longer links the CLI; the CLI is a bundled binary |
| FR-006d external consumer | ✓ | `consumer/` in core-ci; against the published revision in core-release |
| FR-006a Core Release | ✓ | annotated tag, manifest (versions, public surface), assets |
| FR-007 public contract only | ✓ | `check-public-surface.sh` (ecosystem): only `behavior-engine`, no internal crate, no copied core file |
| FR-008 no reimplemented semantics | ✓ | the binding calls the engine; constitution 1.1.0 |
| FR-009 – FR-013 models | ✓ | `models/README.md`, the state-machine lowering with tests, `ARCHITECTURE.md`, `check-terms` |
| FR-014 conformance fixtures | ✓ | the core's `tests/fixtures`, published as an archive |
| FR-015, FR-016 equivalence in CI | ✓ | `test_binding_equivalence.py` (8 pairs plus a migration pair, every wire form) in `ecosystem-ci` |
| FR-017 exact pin, independent versions | ✓ | `core-release.json`, git `rev`, `check-core-pin.sh`; ecosystem 0.10.4 on core 0.10.2 |
| FR-018 one package bundles the core | ✓ | the wheel bundles the CLI; `versions()["core"]`; clean install |
| FR-019, FR-021 no byte or semantic change | ✓ | conformance digest equal before and after (core: 595 keys; ecosystem: 475 keys) |
| FR-020 history preserved | ✓ | `filter-repo` extraction, 26 commits |
| FR-022 preflight | ✓ | five criteria, each seen failing first (Preflight) |
| FR-023 placement by ownership | ✓ | `contracts/ownership.md`, `check-ownership.py` |
| FR-024 no core checkout in ecosystem CI | ✓ | git dependency on the commit plus released assets; isolated clone with no `../behavior-ir-core` |
| FR-025, FR-026, FR-027, FR-031 CI | ✓ | `core-ci`, `ecosystem-ci` on pull requests and pushes to `main`/`dev`; separate release workflows |
| FR-028 explicit tags, never moved | ✓ | v0.10.1 (core) and v0.10.2 (ecosystem) stay unreleased; fixes shipped as new patch versions |
| FR-029 release workflow | ✓ | tag checked before building (annotation, version, on `main`, required checks green); gates rerun; notes name the core |
| FR-030 scripts authoritative | ✓ | the workflows only run scripts; `check-workflows.sh` |
| FR-032 – FR-035 Spec Kit | ✓ | two installations, constitutions 1.0.1 and 1.1.0, ranges 012+/500+, `gates.sh` used by both, no Spec Kit state in CI |

## Success criteria

| SC | Status | Evidence |
|---|---|---|
| SC-001 | ✓ | fresh core clone, gates OK, no ecosystem reference |
| SC-002 | ✓ | ecosystem gates in an isolated clone; surface check |
| SC-003 | ✓ | ecosystem digest: 475 keys equal to the baseline |
| SC-004 | ✓ | every wire IR form has a pair; identical versions and item hashes |
| SC-005 | ✓ | one `pip install --require-hashes`, smoke and skill examples (release-check) |
| SC-006 | ✓ | the determinism check in both gate lists |
| SC-007 | ✓ (scripted) | the 12 concepts map to their layers in `ARCHITECTURE.md` |
| SC-008 | ✓ | `ecosystem-ci` green with no core checkout |
| SC-009 | ✓ | core digest equal |
| SC-010 | deferred | the throwaway pull requests per gate (T079) were deferred by the user. Each check has local tests seen failing, and real CI failures were observed (core-release refusing a lightweight tag; ecosystem-release failing its gates) |
| SC-011 | ✓ | `release-verify identical` for core v0.10.2 and ecosystem v0.10.4 |
| SC-012 | ✓ | numbering probe: 500, then 501, and 012 |

## Follow-ups

- **T079 / SC-010:** one deliberately broken pull request per required gate, in both
  repositories.
- **A path-independent SBOM for the wheel**, so maturin's Rust SBOM can be turned back on.
- **The core release uploads `NOTES.md` as an asset**, because it uploads `dist/*`. The
  ecosystem workflow lists its assets explicitly; the core's should too.
- **`engine-info` does not report the migration IR version.** Adding it changes a contract, so it
  belongs to a core feature.
- **The SC-004 headroom of reads** (from 010), and the evidence-inspection and pagination
  follow-ups of 010, stay open in the core.


## Approved analysis remediation (2026-10-04)

This section records work performed after the original split. Earlier sections retain the
original release and execution evidence; this remediation does not claim to have preceded it.

- **C1 — one supported graph:** constitution 2.0.0 replaces the unconditional local path ban
  with the user-approved distinction between canonical builds and private, explicit untracked
  experiments. Official pin checks have no development or metadata bypass, reject manifest and
  active local overrides, and resolve metadata with `--locked`. Release-build, release-check
  (including `--skip-gates`) and release-verify check the pin before artifact work. The README,
  FR-024, plan, tasks and contracts use the same rule. The constitution bump is MAJOR because it
  changes the scope of a previous prohibition; no runtime behavior or package version changed.
- **C2 — honest test-first evidence:** the original T003/T017/T019 evidence remains incomplete.
  Two current digest tests cover canonical sorting, changed input and propagated CLI failure;
  these are coverage of existing code and cannot establish historical test-first compliance.
  The removed staging script was not reintroduced. Any new staging implementation must first
  have reviewed failing tests for permissions, build failure and a missing binary. The plan's
  blanket Constitution Check PASS was replaced with an explicit historical deviation.
- **C3 — models in the shared gate:** the actual command now includes `python/tests models`.
  The runner tests collect all three actual state-machine tests and deliberately break lowering
  in a disposable copy. Before changing the gate, three tests failed: expected execution order,
  model collection and the broken-lowering refusal. Afterwards all four runner tests passed.
- **C1 red/green evidence:** six new pin assertions failed before implementation (locked
  resolution, the two legacy bypasses, local override acceptance, tracked patch and path).
  Afterwards the pin suite passed. All three release-entrypoint guard tests also failed before
  the guards were added and passed afterwards; they prove no artifact operation precedes the
  dependency check.
- **Transitive pin regression:** two additional assertions first demonstrated that a correctly
  pinned engine could coexist with a path-backed or differently pinned internal core crate in
  resolved metadata. The pin check now requires the known internal core crates to resolve from
  the same exact Git source as the engine, catching overrides inherited outside the checkout.
  Both assertions passed after the change.
- **I1/I2 — core release ordering:** `../behavior-ir-core/scripts/release.sh` validates the tag,
  runs full release checks into temporary validation output, tests the consumer against that
  exact pushed revision, and then builds final artifacts. Its workflow publishes last. The new
  shell regression test first failed against the original script (trace contained only `check`)
  and then passed with order `check`, `consumer --rev <tag commit>`, `build`. It also proves that
  consumer failure stops before the final build, and a wrong tag stops before any artifact
  construction while naming both versions. The test tags exist only in disposable repositories
  and are not deleted individually. Core tag and workflow checks passed.
- **Gate setup:** the first ecosystem gate attempt stopped because the pre-existing ten
  `.agents/skills/speckit-*` files had no ownership rule. Rule 16 now classifies the Codex
  integration alongside the existing Spec Kit integrations. Their contents and the pre-existing
  integration configuration edits were not changed by this remediation.
- **Initial focused validation:** 13 Python tests passed (runner, release guards, digest,
  disposable wrong-tag rejection and the three model tests); the pin shell tests passed.
- **Final ecosystem verification:** `nix develop -c scripts/gates.sh` passed after the final
  transitive-pin correction: 256 Python tests passed, 4 existing slow release tests skipped;
  fmt, clippy, mypy (18 source files), determinism, ownership, terms, workflows and all script
  suites passed. Log: `/tmp/ecosystem-remediation-gates-complete.log`.
- **Final core verification:** the full `scripts/gates.sh` passed with 415 Cargo tests including
  the 8 external-consumer tests, 20 existing ignored stress cases, fmt, clippy, determinism,
  boundary, public surface, terms, workflows and all script suites (including release-order).
  Log: `/tmp/core-remediation-gates-compact.log`.
- **Resource recovery:** initial full-gate attempts filled the disk with Rust debug binaries,
  causing compiler failures and Python temporary-directory failures. With approval, only
  generated build caches were cleaned. The successful core run used `CARGO_BUILD_JOBS=2`,
  `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` to reduce
  build space; no source, dependency pin or test-case configuration was changed for that run.
  The successful ecosystem run used its ordinary gate command.
- **Final diff review:** both repositories passed `git diff --check`; package manifests,
  lockfiles and `core-release.json` are unchanged. Pre-existing integration edits and the
  untracked core 012 feature were preserved. No release or CI-status claim is inferred from
  the stubbed regression tests. T079 remains deferred by the user.
