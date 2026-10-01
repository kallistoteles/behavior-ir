# Implementation Review: Agent-Ready Package and Skills Library

**Feature**: 008 · **Reviewed**: 2026-09-30 · **Tasks**: T001–T050

## Principles

- [x] **Bindings carry no semantics.**
  - The Python layer builds nodes and calls the engine. The console script `behavior` runs the
    engine's own CLI code in-process (`behavior_cli::run`).
  - `test_binding_equivalence.py` builds every example domain through the binding and compares
    it with a canonical wire module written directly, never through the DSL, by
    `build_fixtures.py`. Both reach the same behavior version, the same item hashes and the same
    decision records (locations excepted: they are source metadata).
- [x] **Consumers use only the released binding.**
  - The release check installs the wheel into a fresh venv outside the repository, with `cargo`
    and `rustc` asserted absent from `PATH`, using `pip --no-index --require-hashes`.
  - There it runs the smoke scenario and every skill example.
  - Skill examples and `release/smoke.py` import only public names, and a test enforces it.
- [x] **Skills describe exactly the release.** Four checks keep them honest:
  1. every example runs against the installed package and asserts its outcome;
  2. every excerpt in a `SKILL.md` is verbatim from an example;
  3. every API name or command mentioned is in `api/public-api.json`;
  4. every skill carries `release: 0.8.0` and the verbatim gap rule.
- [x] **Record the gap, never work around it.**
  - The rule appears in all three consumer skills and in `skills/README.md`, together with the
    `SEMANTIC_GAPS.md` format.
  - All three evaluation agents recorded gaps instead of approximating them
    (`evals/results.md`).

## Constitution

| Principle | Status | Evidence |
|---|---|---|
| I. Deterministic core | ✓ | No semantic change. The smoke scenario runs twice in the determinism check and byte-identically inside and outside the repository |
| II. AI output validated | ✓ | Agent-authored models pass admission; gap entries have fixed fields checked by `check_output.py` |
| III. Test-first | ✓ | Each test task preceded its implementation and was seen failing: T004–T007, T013–T016, T023, T038, T043. No automated test calls a model; the agent evaluations are manual acceptance |
| IV. Reproducibility | ✓ | Byte identity between package and repository (release-check step 8); artifacts pinned by SHA-256; the attestation records the solver version |
| V. Explicit state and auditability | ✓ | `release-manifest.json`, `api/public-api.json` and the gap log are explicit, versioned documents |
| VI. Simplicity | ✓ | No new crate. Additions justified in plan.md: zig, auditwheel, the CLI as lib + bin, the manifest |
| Tech constraints | ✓ | fmt, clippy `-D warnings`, no `unsafe`, no unwrap/expect outside tests |

## Requirements

| Requirement | Status | Evidence |
|---|---|---|
| FR-001 versioned distribution | ✓ | `scripts/release.sh` builds, checks, then tags `v<version>` locally; installation by file and hash |
| FR-002 package contents | ✓ | One wheel: DSL, engine, verifier driver, reference store, `run_conformance`, and the console script `behavior` |
| FR-003 no engine toolchain | ✓ | manylinux_2_28 abi3 wheel built with zig and checked with auditwheel; installed with no `cargo`/`rustc` on `PATH` |
| FR-003a bindings carry no semantics | ✓ | `test_binding_equivalence.py` (identities and decisions); `_cli.py` is one call into the engine |
| FR-003b cross-binding equivalence | ✓ | `tests/fixtures/bindings/` for invoice, project_margin, accounts, ledger, orders |
| FR-004 three versions | ✓ | `format_versions()` (core), `engine_info()` (CLI library), `behavior.versions()`, `behavior engine-info`, the import-time exact-match check |
| FR-005 solver prerequisite | ✓ | "verification needs the Z3 SMT solver (supported: 4.16.0); install z3 on PATH or set BEHAVIOR_Z3"; a mismatch notice for other versions; `--expect-missing-solver` in the release check |
| FR-006 public API | ✓ | `api/public-api.json`; `test_public_api.py` (Python names, CLI commands and flags, backend methods) |
| FR-007 byte identity | ✓ | release-check step 8 |
| FR-008 versioning policy | ✓ | `docs/versioning.md` |
| FR-009 old documents readable | ✓ (policy) | The version gates already refuse unknown versions. No earlier release exists to replay yet (follow-up) |
| FR-009a consumer boundary | ✓ | release-check steps 3–5 and 9; `test_examples_use_only_the_public_api`; the application skill |
| FR-010–FR-014 consumer skills | ✓ | `skills/behavior-{authoring,verification,application}/` with 13 runnable examples |
| FR-015 gap rule | ✓ | `test_the_gap_rule_is_present` |
| FR-016 gap log | ✓ | `skills/README.md` (format, example, rules) |
| FR-017 examples from known domains | ✓ | Examples adapted from the invoice, accounts, ledger, orders and project-margin domains |
| FR-018 release named | ✓ | frontmatter `release`, checked against the workspace version |
| FR-019 engine skill | ✓ | `.claude/skills/behavior-engine-development/SKILL.md`; the layout and path checks |
| FR-020 examples run in release check | ✓ | release-check step 9 |
| FR-021 references checked | ✓ | `reference_problems()` and its self-test |
| FR-022 clean-install smoke | ✓ | release-check steps 3–8 |
| FR-023 equivalence in release check | ✓ | release-check step 10 |
| SC-001 install and smoke under 10 minutes | ✓ | Steps 3–8 take about 1 minute (timer at 600 s) |
| SC-002 broken example named | ✓ | `test_a_broken_example_is_reported_by_name`; `test_release_scripts.py` (the release check names the example file) |
| SC-003 byte identity | ✓ | release-check step 8 |
| SC-004 agent authoring | ✓ | `evals/results.md`: 10 of 10 expressible requirements admitted and checked |
| SC-005 gaps recorded | ✓ | 4 of 4 inexpressible requirements recorded with every field; no workarounds |
| SC-006 verification responses | ✓ | 3 of 3 situations as the rubric expects; no rule weakened |
| SC-007 skills layout | ✓ | `test_skills_contains_exactly_the_consumer_skills`; `test_the_engine_skill_is_separate_and_its_paths_exist` |
| SC-008 no identity change | ✓ | Version bump to 0.8.0 without any golden change; regenerated fixtures unchanged; the full suite passes |
| SC-009 binding hashes equal | ✓ | `test_the_binding_builds_the_canonical_module` (5 domains) |
| SC-010 no Rust toolchain | ✓ | release-check step 3 asserts it |

## Deviations from the plan

1. **A third release script.** `scripts/release-build.sh` builds the artifacts. Both
   `release.sh` (a clean tree, then tag) and `release-check.sh` (which may build a throwaway dist
   during development) call it. `release.sh --build-only` and `release-check.sh --skip-gates`
   exist so the scripts can test themselves without recursion.
2. **Step 8 uses its own repository venv.** The in-repo comparison builds into a temporary venv,
   not the developer's. The first version ran `maturin develop` with the developer's
   `VIRTUAL_ENV` inherited, and it repointed that venv at a throwaway test repository. That was
   found in testing and fixed; the venv was restored.
3. **`engine_info()` lives in the CLI library.** `behavior-core` cannot see the store's document
   tags or `VERIFIER_VERSION`. The core provides `format_versions()`, and the store gained
   `DOCUMENT_TAGS`.
4. **Two versions subcommands.** `behavior engine-info` is new, and `behavior version <wire>`
   (a module's behavior version) is unchanged.
5. **Stale extension.** A stale non-abi3 `_engine.cpython-313-…so` in the source tree shadowed
   the abi3 build. It was deleted (a git-ignored build artifact).
6. **The "missing guarantee" case.** In 0.8.0 an unstated expectation shows up as a confirmed
   counterexample, not as an inconclusive; a stated guarantee on unknown set members gives the
   inconclusive (precision debt). The verification skill, its example (`stated_vs_unstated.py`)
   and the evaluation situation 3 teach it that way, which differs from spec US3 scenario 3.
7. **No example alignment needed (T039).** The canonical orders module is derived in the
   generator from the fixture: the example adds an `amount >= 0` guard to `place_order` and
   states four of `check_orders`' six conditions. Every other example matched its fixture item
   for item.
8. **The manual acceptance runs were done by subagents** confined to scratch repositories, and
   scored by hand against the rubrics (`evals/results.md`). Their reports led to the skill fixes
   listed there.
9. **Build warning.** zig prints a harmless linker warning ("ignoring deprecated linker
   optimization setting '1'") during the wheel build.
10. **Disk space.** `target/` had grown to 25 GB and filled the disk mid-feature.
    `target/debug/incremental` was cleared; the engine skill now mentions this.
11. **Standalone CLI binary deferred** (research R5): the console script covers the only binding.

## Follow-ups

- **A second binding and more platforms.** For example JS/TS, Java, .NET or Go, and aarch64 or
  macOS wheels. Each must pass `test_binding_equivalence.py` against `tests/fixtures/bindings/`.
- **Standalone CLI artifact**, once a non-Python binding exists.
- **Publishing to package indexes** once the experiment phase ends, and uploading `dist/v0.8.0/`
  to the git host (manual today).
- **FR-009 in practice.** The next release's check should replay the `dist/v0.8.0` smoke records
  and stores.
- **Engine observations from the agent evaluations** (semantics are frozen in 008, so none was
  changed):
  - `commit_time` refuses RFC 3339 forms with fractional seconds or offsets;
  - `set_` on a query raises a plain `AttributeError`;
  - some attestation `loc.file` values are absolute and others relative;
  - the verifier does not normalize `count(where(key == v)) == 0` onto the equality summary that
    `any_` and `unique` share.
- **Semantic gaps the agents found**, as input for the next language feature:
  - grouped or per-key invariants ("per customer"), which all three agents hit;
  - invariants over two related entities;
  - cross-entity filters;
  - bulk effects;
  - ordering.

## Code review fixes (after v0.8.0)

- **Pre-release versions failed the binding/engine check.** The wheel's version is the Cargo
  version normalized by maturin to Python's form (PEP 440: `0.9.0-rc.1` becomes `0.9.0rc1`).
  The engine reports the Cargo form, so `import behavior` would have raised `ImportError` for
  any pre-release, and release-check step 7 would have failed.
  - `_check_versions` now compares in Python's form (`_python_version`).
  - `release-build.sh` writes the binding version in that form into `release-manifest.json`.
  - Test: `test_versions.py::test_engine_versions_compare_in_python_form`, covering rc, alpha,
    beta and build metadata.
  - `v0.8.0` (a plain version, which is unaffected) keeps its tag; the fix ships with the next
    release.
- **Pre-release forms beyond `-label.N`** (second review). `0.9.0-alpha` and `0.9.0-rc1` were
  not converted, so the package would refuse its own engine, and `release-build.sh` would crash
  on them.
  - The conversion now lives once, in `python/behavior/_versions.py`. The package uses it, and
    `release-build.sh` loads the same file by path.
  - It accepts `alpha`/`a`, `beta`/`b` and `rc`/`c`, in any case, with or without a separator or
    number.
  - Any other label (for example `dev`, whose PEP 440 form maturin may write differently) is
    refused by name. A release with such a version fails at build time instead of producing an
    unimportable wheel.
- **The determinism gate did not require the smoke scenario to succeed.** `run_twice` compares
  two runs, so a scenario that failed the same way twice passed. `determinism-check.sh` now also
  requires exit 0 and the `smoke: OK` line.
