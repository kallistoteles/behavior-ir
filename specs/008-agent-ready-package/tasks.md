---
description: "Task list for feature 008: agent-ready package and skills library"
---

# Tasks: Agent-Ready Package and Skills Library

**Input**: Design documents from `/specs/008-agent-ready-package/`: plan.md, spec.md,
research.md (R1–R13), data-model.md, contracts/ (release, public-api, skills, gap-log),
quickstart.md.

**Tests**: The constitution requires test-first (Principle III), so test tasks come before their
implementation and must be seen failing. Agent evaluations (SC-004–SC-006) are **manual
acceptance**, never automated tests, because no automated test may call a live model.

**Organization**: tasks are grouped by user story. Paths are repository-relative.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6 from spec.md

## Standing rules

These apply to every task:
- **Consumer boundary.** Skill examples and `release/smoke.py` must not import `examples.*` or
  use repository paths. They must run in a venv that only has the wheel installed (FR-009a).
- **No semantic change.** Existing identities, records and goldens stay byte-identical (SC-008).

---

## Phase 1: Setup

- [X] T001 Add `pkgs.zig` and the Python package `auditwheel` to the dev shell in `flake.nix` (research R4), and add `dist/` to `.gitignore`. Verify with `nix develop -c zig version` and `nix develop -c auditwheel --version`.
- [X] T002 Make the workspace crate version the single version source (research R1):
  - set `[workspace.package] version = "0.8.0"` in `Cargo.toml`;
  - in `pyproject.toml`, replace `version = "0.1.0"` with `dynamic = ["version"]` and add `[project.scripts] behavior = "behavior._cli:main"`;
  - grep the repository for other hard-coded `0.1.0` release versions and fix them;
  - confirm `cargo test --workspace` and `pytest` still pass, i.e. that no golden changes (SC-008).
- [X] T003 [P] Write `docs/versioning.md` per research R13:
  - the three version kinds (engine/binding, wire IR/record/store documents, verifier);
  - minor bumps for identity, format, public-API removal or verifier-outcome changes, and patch bumps for additive changes;
  - readability of older records and stores within a minor line, with explicit refusal otherwise.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: version reporting, the CLI as a library, and the public API manifest. Every story
depends on these.

### Tests first

- [X] T004 [P] Write `crates/behavior-core/tests/versions.rs`: `behavior_core::format_versions()` returns engine equal to `env!("CARGO_PKG_VERSION")`, `wire_ir == ["0.1","0.2","0.3","0.4","0.5","0.6"]` and `records == ["0.4","0.5","0.6"]`, each taken from the constants in `wire.rs` and `eval.rs` rather than retyped.
- [X] T005 [P] Write `crates/behavior-cli/tests/cli_engine_info.rs`: `behavior engine-info` exits 0 and prints canonical JSON with keys `engine`, `wire_ir`, `records`, `store_documents`, `verifier`. `store_documents` contains every `TAG_*` of `crates/behavior-store/src/documents.rs`, and `verifier` equals `behavior_verify::VERIFIER_VERSION`. `behavior version <wire>` is unchanged: it still prints a module's behavior version.
- [X] T006 [P] Write `python/tests/test_versions.py`:
  - `behavior.__version__ == "0.8.0"`;
  - `behavior.versions()` equals `json.loads(<behavior engine-info output>)` plus `"binding": {"python": "0.8.0"}`;
  - `behavior._check_versions("0.8.0", "0.9.0")` raises `ImportError` naming both versions.
- [X] T007 [P] Write `python/tests/test_public_api.py` per contracts/public-api.md:
  - `set(behavior.__all__) == set(manifest["python"]["names"])`;
  - the CLI subcommands and flags equal `manifest["cli"]`;
  - the backend methods equal `manifest["backend"]`. Take `required` from the methods `PyBackend` calls unconditionally in `crates/behavior-py/src/lib.rs`, and `optional` from the `hasattr`-guarded ones, and compare them with the `Backend` trait in `crates/behavior-store/src/lib.rs`.

### Implementation

- [X] T008 Add `pub fn format_versions() -> serde_json::Value` to `crates/behavior-core/src/lib.rs`, built from the IR and record version constants. Make T004 pass.
- [X] T009 Turn `crates/behavior-cli` into a library plus a binary (research R5):
  - `src/lib.rs` holds today's `main.rs` logic as `pub fn run(args: Vec<std::ffi::OsString>) -> u8`, using clap `try_parse_from`, and `pub fn engine_info() -> serde_json::Value`. `engine_info` merges `format_versions()`, the store document tags and `VERIFIER_VERSION`;
  - add the `engine-info` subcommand;
  - `src/main.rs` becomes `std::process::ExitCode::from(behavior_cli::run(std::env::args_os().collect()))`;
  - flush stdout before returning;
  - make T005 pass, and keep every existing CLI test passing.
- [X] T010 In `crates/behavior-py`:
  - enable PyO3 `abi3-py313` in `Cargo.toml`. If an API is refused, fall back to a cp313 wheel and note it for the review;
  - add the dependency on `behavior-cli`;
  - expose `_engine.engine_info() -> dict`, `_engine.ENGINE_VERSION` and `_engine.cli(args: list[str]) -> int` in `src/lib.rs`;
  - add them to `python/behavior/_engine.pyi`.
- [X] T011 In `python/behavior/__init__.py`:
  - set `__version__` from `importlib.metadata.version("behavior")`;
  - add `_check_versions(binding, engine)` and call it at import with `_engine.ENGINE_VERSION`;
  - add `versions()`, returning the engine info plus `{"binding": {"python": __version__}}`;
  - export `versions` and `__version__` in `__all__`.

  Make T006 pass.
- [X] T012 Create `api/public-api.json` (canonical JSON, `format: "behavior.public_api.v1"`, `release: "0.8.0"`) per contracts/public-api.md:
  - every `behavior.__all__` name with its kind;
  - every CLI subcommand with its flags, including `engine-info`;
  - the formats (wire IR versions → `schema/wire-ir-*.schema.json`, record versions, store document tags);
  - the backend methods with `required`/`optional`;
  - the solver `{"name": "z3", "version": "4.16.0"}`.

  Make T007 pass.

**Checkpoint**: versions are reported identically by the core, the CLI and Python. The public
surface is a checked document.

---

## Phase 3: User Story 1 — Install and use the library in a fresh project (P1) 🎯 MVP

**Goal**: a pinned wheel installs into a clean environment outside the repository, with no
engine toolchain, and supports authoring, evaluation, verification, persistence, replay and the
CLI, with byte-identical results.

**Independent Test**: `scripts/release-check.sh` passes its install, smoke, missing-solver,
version and byte-identity steps (quickstart §2 items 1–3, 5 and 6).

### Tests first

- [X] T013 [P] [US1] Extend `crates/behavior-verify/tests/solver.rs`: `Z3Process::from_env()` with `BEHAVIOR_Z3=/nonexistent` errors with exactly `verification needs the Z3 SMT solver (supported: 4.16.0); install z3 on PATH or set BEHAVIOR_Z3` (plus the cause). The running solver's version is reported. A fake solver script that prints `Z3 version 4.15.0 - 64 bit` yields a mismatch notice against `SUPPORTED_Z3 = "4.16.0"`.
- [X] T014 [P] [US1] Write `python/tests/test_cli_entry.py`:
  - `python -m behavior._cli admit <tests/fixtures/wire/valid/orders.json>` exits 0 with the same stdout as `target/debug/behavior admit`;
  - the Python `verify()` with `BEHAVIOR_Z3=/nonexistent` raises `BehaviorError` with the prerequisite text;
  - evaluation still works without the solver.
- [X] T015 [US1] Write `release/smoke.py`, the consumer smoke scenario. It is self-contained and imports only `behavior`, so it can run in the clean venv. It:
  1. defines a small module inline (a customer, orders with a query precondition, a module invariant, `Money`);
  2. prints canonical JSON of the admission hashes;
  3. evaluates an allowed action and a denied action;
  4. verifies with the default profile;
  5. creates an `InMemoryBackend` store, commits two transitions and prints the records and state identities;
  6. runs `replay_data` and `replay_behavior` and asserts they succeed;
  7. runs `behavior admit`, `behavior eval`, `behavior replay` (on the record `eval` produced) and `behavior verify` through `subprocess` on the console script, printing each stdout.

  It prints one canonical JSON line per artifact and exits non-zero on any failed assertion. With `--expect-missing-solver`, it runs every step except verification. For verification it asserts that the `BehaviorError` text (and the CLI's stderr) contains `verification needs the Z3 SMT solver`, and it omits the attestation lines.

- [X] T016 [P] [US1] Write `python/tests/test_release_scripts.py`. It runs the scripts in a temporary `git clone` of the repository, so the real tree is never touched. It asserts:
  - `scripts/release.sh 0.8.0` refuses with a non-zero exit and names the reason for (a) a dirty tree (an untracked file) and (b) a version argument that differs from the workspace version (`0.9.0`). In both cases no tag is created;
  - `scripts/release-check.sh <dist>` fails and names the failing step for (a) a `SHA256SUMS` entry altered by one character (the install step) and (b) a skill example whose assertion was flipped in the clone (the skill-examples step).

  It is marked slow and skipped unless `BEHAVIOR_RELEASE_TESTS=1`, because it builds a wheel. It is seen failing before T019/T020 exist.

### Implementation

- [X] T017 [US1] In `crates/behavior-verify/src/solver.rs`:
  - add `pub const SUPPORTED_Z3: &str = "4.16.0"`;
  - reword `SolverError::Spawn` to the prerequisite message;
  - add `Z3Process::version_mismatch() -> Option<String>`;
  - print the mismatch on stderr in `crates/behavior-cli/src/lib.rs`, and raise it as `warnings.warn` in `python/behavior/verify.py`.

  Make T013 pass.
- [X] T018 [US1] Add `python/behavior/_cli.py`, whose `main()` calls `sys.exit(_engine.cli(sys.argv[1:]))`, and confirm the console script is declared (T002). Make T014 pass.
- [X] T019 [US1] Write `scripts/release.sh <version>` per contracts/release.md:
  1. refuse unless the tree is clean and the workspace version equals `<version>`;
  2. run `maturin build --release --zig --compatibility manylinux_2_28 --out dist/v<version>/`;
  3. write `SHA256SUMS`;
  4. write `release-manifest.json` (`format: "behavior.release_manifest.v1"`, the versions from `behavior engine-info`, `bindings.python`, the platforms, the artifacts with sha256, the solver, the commit);
  5. run `scripts/release-check.sh dist/v<version>`;
  6. on success, create the annotated tag `v<version>`. Never push or upload.
- [X] T020 [US1] Write `scripts/release-check.sh [dist-dir]` per research R9. Build into a temporary dist directory if none is given. Each numbered step prints its name and exits non-zero naming the failing step:
  1. the gates, including `python/tests/test_release_scripts.py` with `BEHAVIOR_RELEASE_TESTS=1`;
  2. `auditwheel show`, which must report manylinux_2_28;
  3. a temporary directory outside the repository with a venv made with `python3.13 -m venv` (pip bootstrapped offline by ensurepip, no system site packages), and `PATH` reduced so that `command -v cargo rustc` both fail (asserted, SC-010);
  4. `pip install --no-index --require-hashes -r <generated requirements with the wheel's sha256>`;
  5. `release/smoke.py` in that venv;
  6. `release/smoke.py --expect-missing-solver` with `BEHAVIOR_Z3=/nonexistent`;
  7. `behavior.versions()` compared with `release-manifest.json`;
  8. `release/smoke.py` inside the repository, with the same four CLI commands, whose stdout must be byte-identical to step 5 (SC-003, FR-007).

  Leave named placeholders for the step 9 skill examples (T041), the step 10 drift and equivalence checks (T042), and a step 0 timer that fails if steps 3–8 take longer than 10 minutes (SC-001).
- [X] T021 [US1] Extend `scripts/determinism-check.sh` to run `release/smoke.py` twice and compare the outputs (in-repo).
- [X] T022 [US1] Run `scripts/release-check.sh` and fix everything it finds. This includes the abi3 fallback, missing wheel content (the engine and verifier driver, the reference store with `run_conformance`, the console script), and any byte differences. **Checkpoint**: US1 is independently demonstrable.

---

## Phase 4: User Story 2 — An agent authors a correct model from the skills alone (P1)

**Goal**: the authoring skill, with runnable examples that cover FR-012, the gap rule and the gap
log format.

**Independent Test**: `pytest python/tests/test_skills.py -k authoring` passes, and the manual
authoring evaluation (T028) meets its rubric.

### Tests first

- [X] T023 [US2] Write `python/tests/test_skills.py` implementing the checks of contracts/skills.md for every skill directory under `skills/` except `evals/`, one test per check so failures name the skill and the file:
  1. **examples**: every `skills/<skill>/examples/*.py` runs with `sys.executable` in a temp cwd, with `PYTHONPATH` unset and the repo not on `sys.path`, and must exit 0;
  2. **excerpts**: every fenced `python` block in `SKILL.md` starts with `# from examples/<file>.py`, and its remaining lines appear verbatim and in order in that file;
  3. **references**: every `from behavior import …` name, every `behavior.<name>` and every `behavior <subcommand>` in the skill is in `api/public-api.json`;
  4. **release**: the frontmatter `release` equals the workspace version;
  5. **gap rule**: the verbatim gap rule of contracts/skills.md is present;
  6. **layout**: `skills/` contains exactly `README.md`, `evals/`, `behavior-authoring/`, `behavior-verification/` and `behavior-application/`.

  Add a self-test that copies one example to a temp skill directory, breaks an assertion, and checks that the runner reports that file by name (SC-002).
- [X] T024 [P] [US2] Write `skills/README.md`:
  - how to use the skills of a pinned release: copy `skills/behavior-*` from tag `vX.Y.Z` into the consumer's agent skills directory, and install the wheel of the same release;
  - the verbatim gap rule;
  - the `SEMANTIC_GAPS.md` format and the filled example from contracts/gap-log.md;
  - the consumer boundary (FR-009a).

### Implementation

- [X] T025 [P] [US2] Write `skills/behavior-authoring/examples/`. Each script is self-contained, imports only `behavior`, models are copied from the example domains, and each asserts its outcome:
  - `entities_and_types.py`: entities, `field`, `Option`, `Id`, `Ref`, enums, `nominal` with `scale`;
  - `actions_and_lifecycle.py`: `requires`/`set_`/`ensures`, `create`/`remove`, `exists`/`referenced`, from the accounts domain;
  - `exact_arithmetic.py`: fixed-scale money, `Exact`, `rescale` with `Rounding`, and the `LOSSY_CONVERSION` fix;
  - `queries_and_module_invariants.py`: `select`/`where`/set algebra, `count`/`any_`/`all_`/`sum_`/`min_`/`max_`/`unique`, a module invariant, a derived value as reuse, and a `@rule` (a Bool derived value) used in a precondition, from the orders domain;
  - `placement_errors.py`: asserts `QUERY_NOT_ALLOWED` (a query in a constraint), `NON_LOCAL_PREDICATE`, `TYPE_MISMATCH` (mixed set algebra), and `BehaviorDefinitionError` (iterating a query), each with the corrected model admitted.
- [X] T026 [US2] Write `skills/behavior-authoring/SKILL.md` per contracts/skills.md:
  - frontmatter (`name`, `description`, `release: 0.8.0`) and the scope line;
  - sections covering the FR-012 list (including derived values versus rules, and the verifier's `vacuity` check on rules), with excerpts only from T025's files;
  - the placement rules, with an error code → fix table;
  - the gap rule, with typical inexpressible requirements as recognition cues: grouped invariants, cross-entity filters, bulk effects, ordering or pagination, joins.

  Make T023 pass for this skill.
- [X] T027 [P] [US2] Write `skills/evals/authoring/`:
  - `requirements.md`: at least 8 expressible requirements drawn from the invoice, accounts, ledger and orders domains, and at least 4 inexpressible ones (at most three open orders per customer, orders of customers in a region, close all open orders of a customer, the newest order);
  - `rubric.md`: every expressible requirement admitted and meeting its acceptance check using only manifest names; every inexpressible one has a complete `SEMANTIC_GAPS.md` entry; zero hand-written IR or internal imports;
  - `check_output.py`: runs the agent's model files and checks the gap-log fields mechanically.
- [X] T028 [US2] Manual acceptance (SC-004, SC-005):
  1. in a scratch consumer repository with only the pinned wheel and `skills/behavior-*` copied in, run an agent on `skills/evals/authoring/requirements.md`;
  2. score it with `rubric.md` and `check_output.py`;
  3. record the results in `specs/008-agent-ready-package/evals/results.md`;
  4. fix the skill (not the rubric) for any failure the skill caused, and re-run.

---

## Phase 5: User Story 3 — An agent runs and interprets verification correctly (P2)

**Goal**: the verification skill, with its decision procedure and precision-debt catalogue.

**Independent Test**: `pytest python/tests/test_skills.py -k verification` passes, and the manual
verification evaluation (T032) meets its rubric.

- [X] T029 [P] [US3] Write `skills/behavior-verification/examples/`, each asserting its outcome. The solver must be available; the release check provides it:
  - `outcomes.py`: a small orders model with one proven, one counterexample and one inconclusive check. It asserts each `outcome` and that the counterexample's record re-evaluates to the same DENY;
  - `fix_the_model.py`: an unguarded action with a counterexample. Adding the guard (`requires`) makes the same check proven. The violated rule is never removed;
  - `precision_debt.py`: a changed capture (`raise_limit`-style) and `min` after removal, both inconclusive;
  - `missing_guarantee.py`: an inconclusive result caused by an unstated expectation, resolved by adding a constraint to the model.
- [X] T030 [US3] Write `skills/behavior-verification/SKILL.md`:
  - frontmatter, the scope line and the gap rule;
  - the profile and check kinds (preservation, postcondition, evaluation_error, referential_integrity, dead_action, redundant_precondition, vacuity);
  - the three outcomes, with "inconclusive is blocking";
  - the decision procedure: confirmed counterexample → fix the model (guard, constraint or type) or record an accepted finding; inconclusive → classify it as known precision debt (catalogue) or an unstated expectation, and add the guarantee to the model. Never weaken or remove a check to pass;
  - the precision-debt catalogue from `docs/verification.md` and the 007 implementation review (changed captures, extrema after a removal or change, partial-sum range checks), including the stated-guarantee versus unstated-expectation distinction;
  - excerpts only from T029's files.

  Make T023 pass for this skill.
- [X] T031 [P] [US3] Write `skills/evals/verification/`:
  - three seeded situations, each a model plus its attestation: a real counterexample, a precision-debt inconclusive, and a missing-guarantee inconclusive;
  - `rubric.md`: the expected response class per situation, and zero weakened rules.
- [X] T032 [US3] Manual acceptance (SC-006): run an agent on `skills/evals/verification/`, score it with the rubric, record the results in `specs/008-agent-ready-package/evals/results.md`, fix the skill if needed, and re-run.

---

## Phase 6: User Story 4 — An agent builds an application around the library (P2)

**Goal**: the application skill: the host boundary, stores, conflicts, custom backends and
replay.

**Independent Test**: `pytest python/tests/test_skills.py -k application` passes, and the manual
application review (T036) meets its rubric.

- [X] T033 [P] [US4] Write `skills/behavior-application/examples/`, each asserting its outcome:
  - `store_lifecycle.py`: `Store.create` with `genesis_for`, `evaluate` and `commit` with host-supplied identities and `commit_time`, `load` at a past state, `replay_data`/`replay_behavior`;
  - `conflict.py`: two evaluations on the same state. The second commit raises `StateConflict`; re-evaluating on `current()` succeeds;
  - `custom_backend.py`: a dict-based backend (from `python/tests/test_store_conformance.py`'s `DictBackend`, with `keys_at`) that passes `run_conformance`;
  - `plain_evaluation.py`: `evaluate(..., facts=...)` with `universe`/`queries` facts, `UNKNOWN_FACT` when a fact is missing, and `replay` of the record.
- [X] T034 [US4] Write `skills/behavior-application/SKILL.md`:
  - frontmatter, the scope line and the gap rule;
  - the host boundary (FR-014): business rules in behavior; every state change through evaluate → commit; identities and time from the host; re-evaluation on conflict; the conformance suite for custom backends; replay as audit;
  - the consumer boundary (FR-009a): only the released binding, no engine crates or repository paths;
  - anti-patterns with their corrections: a rule duplicated in host code, writing state outside commit, retrying a stale bundle, generating identities in the engine;
  - excerpts only from T033's files.

  Make T023 pass for this skill.
- [X] T035 [P] [US4] Write `skills/evals/application/`:
  - `task.md`: a small application feature, for example an order intake service with a custom backend;
  - `rubric.md`: a review checklist mirroring US4's Independent Test.
- [X] T036 [US4] Manual acceptance: run an agent on `skills/evals/application/task.md` in the scratch consumer repository, review it against the rubric, record the results in `specs/008-agent-ready-package/evals/results.md`, fix the skill if needed, and re-run.

---

## Phase 7: User Story 5 — Keep the package and the skills in sync with the engine (P3)

**Goal**: the release fails whenever a skill, the public API or a binding's semantics drift.

**Independent Test**: breaking a skill example, adding an unlisted API name to a skill, or changing
an example model's semantics each make `scripts/release-check.sh` fail and name the cause.

- [X] T037 [P] [US5] Extend `tests/fixtures/wire/build_fixtures.py` (the independent JSON generator, never the DSL) with canonical modules for the example domains `examples/invoice`, `project_margin`, `accounts`, `ledger` and `orders`, written to `tests/fixtures/bindings/<domain>.json`. Add `tests/fixtures/bindings/<domain>.request.json` with one allowed and one denied request per domain. Regenerate the fixtures and assert that existing fixtures are unchanged.
- [X] T038 [P] [US5] Write `python/tests/test_binding_equivalence.py`: for each domain, build `examples/<domain>/behavior.py`'s `model` through the Python binding and admit `tests/fixtures/bindings/<domain>.json`. Assert an equal `behavior_version` and equal item hashes by comparing the `behavior hashes` output (via the console script) for the DSL module's wire JSON and for the fixture. Source locations are ignored (FR-003b, SC-009). Also evaluate each domain's requests two ways: through the Python binding's module (`behavior.evaluate`), and through `behavior eval` on the canonical fixture. Assert byte-identical records, so FR-003a is checked for decisions, not only identities. Seen failing before T037 exists.
- [X] T039 [US5] Make T038 pass by aligning the generator with the example models. Where they differ, the example model is authoritative only if its semantics is intended; record every alignment for the review.
- [X] T040 [P] [US5] Add to `python/tests/test_skills.py` a unit test that the references check fails for a synthetic skill naming `behavior.not_a_real_name`, and that the release check fails for a mismatched `release:` (SC-002, FR-021).
- [X] T041 [US5] Fill step 9 of `scripts/release-check.sh`: run every `skills/*/examples/*.py` with the clean venv's Python, from a temp cwd, with `BEHAVIOR_Z3` set, and fail naming the file.
- [X] T042 [US5] Fill step 10 of `scripts/release-check.sh`: run in-repo `pytest python/tests/test_skills.py python/tests/test_public_api.py python/tests/test_binding_equivalence.py`.

---

## Phase 8: User Story 6 — Engine development guidance, kept separate (P3)

**Goal**: an engine skill for agents changing this repository, never part of the consumer skills.

**Independent Test**: the layout check passes (the engine skill exists only under
`.claude/skills/`), and every command and path the engine skill names exists.

- [X] T043 [P] [US6] Add to `python/tests/test_skills.py` the check that `.claude/skills/behavior-engine-development/SKILL.md` exists, is not under `skills/`, and that every repository path and script in it exists (e.g. `scripts/determinism-check.sh`, `tests/fixtures/frozen_versions_007.json`, `.specify/memory/constitution.md`).
- [X] T044 [US6] Write `.claude/skills/behavior-engine-development/SKILL.md`:
  - the constitution's non-negotiables: test-first with failing tests seen, determinism with `BTreeMap`, no unwrap/expect outside tests, typed errors, no `unsafe`;
  - the Spec Kit flow (specify → clarify → plan → tasks → analyze → implement) and the implementation review checklist;
  - the gates: fmt, clippy `-D warnings`, `cargo test --workspace`, pytest, mypy, `scripts/determinism-check.sh`, the ignored release tests, `scripts/release-check.sh`;
  - the compatibility rules: frozen identity snapshots and wire version gates (a new form needs a new IR version, and old documents keep their bytes), record versions, and store document tags;
  - the verifier rules: soundness first, and counterexamples confirmed by evaluation;
  - the conformance mutant pattern;
  - "never change semantics to satisfy a consumer: record it as a feature".

  Make T043 pass.

---

## Phase 9: Polish & cross-cutting

- [X] T045 [P] Add a "Using a release" section to `README.md`: install by file and hash (contracts/release.md), `behavior.versions()` / `behavior engine-info`, the solver prerequisite, where the skills are and how to hand them to an agent, unsupported platforms (pip refuses the wheel by its platform tag), and a link to `docs/versioning.md`.
- [X] T046 [P] Update `docs/verification.md` to reference the solver prerequisite message and `SUPPORTED_Z3`.
- [X] T047 Run all gates (fmt, clippy `-D warnings`, `cargo test --workspace`, pytest, mypy, `scripts/determinism-check.sh`, `cargo test --release --workspace -- --ignored`) and `scripts/release-check.sh`. Fix all findings.
- [X] T048 Run quickstart.md §1–§4 and fix any failures.
- [X] T049 Write `specs/008-agent-ready-package/checklists/implementation-review.md`:
  - a review against every FR and SC, the constitution and the principles "bindings carry no semantics" and "consumers use only the released binding";
  - deviations: the abi3 fallback if taken, the deferred standalone CLI binary, the host-agnostic upload step, and the T039 alignments;
  - follow-ups: a second binding, more platforms, a standalone CLI artifact, publishing to package indexes, and having the next release's check replay the `dist/v0.8.0` smoke records (FR-009).
- [X] T050 Cut the release: `scripts/release.sh 0.8.0` builds `dist/v0.8.0/`, passes the release check and creates the annotated tag `v0.8.0` locally. Pushing the tag and uploading the artifacts are left to the maintainer.

---

## Dependencies & Execution Order

- **Setup (T001–T003)**: first. T002 changes the version, so every later version assertion uses 0.8.0.
- **Foundational (T004–T012)**: blocks all stories. It is needed for `engine-info`, `versions()`, `cli()` and the manifest that the skill checks use.
- **US1 (T013–T022)**: needs Foundational. It is the MVP.
- **US2, US3, US4**: need Foundational (the manifest). Their examples also run in the release check, so they need US1's T020 before T041.
  - US2's T023 (the checker) comes before T026, T030 and T034.
  - US3 and US4 can proceed in parallel with each other once T023 exists.
- **US5 (T037–T042)**: T037/T038 can start after Foundational. T041/T042 need T020 and the skills.
- **US6 (T043–T044)**: independent after Setup, apart from T043's addition to `test_skills.py`.
- **Polish (T045–T050)**: last. T050 only after T047 and T048 pass.
- **Manual acceptance (T028, T032, T036)**: after the pinned wheel exists (T022) and the skill is written. The results feed back into the skills.

## Parallel Opportunities

- T003 runs alongside T001/T002.
- T004–T007 (four test files) run in parallel.
- T013 and T014 run in parallel.
- T024, T025 and T027 run in parallel.
- US3 (T029, T031) and US4 (T033, T035) run in parallel with each other.
- T037, T038 and T040 run in parallel.
- T043 runs alongside any US2–US5 task.
- T045 and T046 run in parallel.

## Implementation Strategy

1. **MVP**: Setup, then Foundational, then US1. The result is a verified, installable, versioned wheel with byte-identical results. Stop and demo with `scripts/release-check.sh`.
2. **Next**: the authoring skill (US2), which is the first thing a consumer agent needs. Run its manual evaluation before writing the other skills, since lessons carry over.
3. **Then**: US3 and US4, in parallel.
4. **Then**: US5 hardens drift detection across everything, and US6 adds the engine skill.
5. **Finally**: Polish and the 0.8.0 tag.
