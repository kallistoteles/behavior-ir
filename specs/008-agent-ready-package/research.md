# Research: Agent-Ready Package and Skills Library

Phase 0 of [plan.md](plan.md). Each decision resolves an open technical question of the spec.

## Current state

The facts the decisions build on:

- **Python binding.** The binding (`behavior` package, extension `behavior._engine`) is built with
  maturin from `crates/behavior-py`. It has no Python runtime dependencies and requires
  Python ≥ 3.13.
- **Versions.** The version is `0.1.0` in both `Cargo.toml` (workspace) and `pyproject.toml`,
  written twice. Supported format versions are separate constants: wire IR `0.1`–`0.6`
  (`wire.rs`), records `0.4`–`0.6` (`eval.rs`), store document tags `behavior.*.v1`
  (`documents.rs`). `VERIFIER_VERSION` (`0.4.0`) is also separate, and it is part of attestations
  and check keys. **No hashed document contains the crate version**, so a release version bump
  changes no golden.
- **CLI.** It is a clap binary (`crates/behavior-cli/src/main.rs`) with no library target.
- **Solver.** It is found through `BEHAVIOR_Z3`, else `z3` on `PATH`. When it is missing, the
  error is only "cannot run the solver at z3: …", with no hint of what to install.
- **Build environment.** The dev shell (`flake.nix`) provides Rust 1.98.1, Python 3.13, maturin
  and Z3. There is no zig, docker, auditwheel or gh.
- **Remote.** The remote is a self-hosted git server over SSH (`git@10.10.2.4:…`). There is no
  known release-asset API.
- **Precedent for binding equivalence.** `python/tests/test_wire.py` compares the invoice and
  project-margin DSL modules byte for byte with independently generated wire fixtures
  (`tests/fixtures/wire/build_fixtures.py`). `test_lifecycle.py` does the same for accounts.
- **Skills location.** Agent skills in this repo live in `.claude/skills/<name>/SKILL.md` (YAML
  frontmatter `name`, `description`, then markdown). Only the Spec Kit skills exist today.

## R1. Release identity and the single version source

- **Decision:**
  - One version source: `[workspace.package] version` in `Cargo.toml`. `pyproject.toml` declares
    `dynamic = ["version"]`, so maturin takes the binding version from the crate.
  - The first release is **0.8.0**, the engine release after feature 008.
  - A release is the annotated git tag `v0.8.0` on a commit that passes the release check
    (R9), plus the artifacts in `dist/v0.8.0/` (R3).
- **Rationale:** FR-001, FR-004. Exact engine–binding matching is automatic when both come from
  one number, which removes a class of mismatch.
- **Alternatives considered:**
  - Separate binding versioning now: premature (spec: exact matching at first).
  - Keeping 0.1.0: hides seven features of change.

## R2. What a release reports (three versions)

- **Decision:**
  - `behavior_cli::engine_info()` (the CLI library depends on every engine crate) returns a
    canonical JSON object assembled from each crate's own constants. The core contributes
    `behavior_core::format_versions()` (engine, wire IR, records), the store its document tags,
    and the verifier `VERIFIER_VERSION`. The object has:
    - `engine`: the crate version;
    - `wire_ir`: all supported IR versions, in order;
    - `records`: the supported record versions;
    - `store_documents`: the store document tags;
    - `verifier`: `VERIFIER_VERSION`.
  - The Python binding exposes `behavior.__version__` (the binding version) and
    `behavior.versions()`, which is the engine's object plus `"binding": {"python": "<version>"}`.
  - At import time the binding checks that its own version equals the engine version it loaded,
    and raises `ImportError` otherwise.
  - The CLI prints the same object for the new `behavior engine-info`. The existing
    `behavior version <wire>` prints a module's behavior version and is unchanged.
- **Rationale:** FR-004. One reporting function keeps the lists from drifting from the constants
  that implement them.
- **Alternatives considered:** version strings scattered per module — rejected because they
  drift.

## R3. Distribution channel (host-agnostic tagged artifacts)

- **Decision:**
  - `scripts/release.sh <version>` builds everything into `dist/v<version>/`:
    - the binding wheel;
    - `SHA256SUMS`;
    - `release-manifest.json` (the versions from R2, file names and hashes, the supported
      platforms, the solver prerequisite).
  - The script then runs the release check (R9) and creates the annotated tag.
  - Uploading the directory to the git host's release page, or to any file server, is a manual,
    host-specific step, because the host's release API is unknown.
  - Consumers install **by exact file and hash**, for example a requirements line
    `behavior @ file:///…/behavior-0.8.0-….whl --hash=sha256:…` or an https URL to the uploaded
    asset. `pip install --require-hashes` makes the pin immutable even if the host is not.
  - `dist/` is git-ignored.
- **Rationale:** FR-001 says artifacts attached to the tag are enough for the experiment phase.
  A hash pin gives immutability without trusting the host, which fits the project's
  content-addressing.
- **Alternatives considered:**
  - Committing wheels to the repository: this bloats history.
  - A private index: new infrastructure (spec option B, not chosen).
  - Building from git source in the consumer: violates FR-003.

## R4. Portable wheel without an engine toolchain for the consumer

- **Decision:**
  - Build a **manylinux_2_28 x86-64** wheel with `maturin build --release --zig --compatibility
    manylinux_2_28`, adding `zig` to the dev shell.
  - Check it with `auditwheel show`, adding `auditwheel` to the dev shell's Python.
  - Use PyO3's **`abi3-py313`** feature, so one `cp313-abi3` wheel serves CPython ≥ 3.13. If the
    stable ABI refuses an API the binding uses, fall back to a `cp313`-only wheel and record
    that as a deviation.
- **Rationale:**
  - FR-003 and SC-010: a wheel built normally in the Nix shell links against Nix store paths and
    loads only on hosts with that store. zig cross-links against an old glibc, which gives a
    standard manylinux wheel.
  - abi3 keeps the artifact count at one per platform.
- **Alternatives considered:**
  - manylinux Docker images: no container runtime is available.
  - Unmarked `linux_x86_64` wheels: not portable, which would silently weaken the clean-install
    check.
  - Bundling a static musl build: Python extension modules cannot use musl on glibc hosts.

## R5. The command-line tool in a binding-neutral release

- **Decision:**
  - `behavior-cli` becomes library plus binary: `pub fn run(args: Vec<OsString>) -> u8` holds
    today's `main`, and `main` calls it.
  - The Python binding exposes `_engine.cli(args) -> int`, and the wheel declares the console
    script `behavior = "behavior._cli:main"`. `pip install` therefore puts `behavior` on the
    environment's `PATH` with identical behavior, because it is the same code.
  - A standalone CLI binary artifact is **deferred** until the first non-Python binding needs it.
    It is recorded as a follow-up.
- **Rationale:**
  - FR-002 requires the release to provide the tool.
  - For the only consumer (Python) the console script is the least moving parts.
  - A second native artifact would need its own portability work (R4) with no current user.
- **Alternatives considered:**
  - Packaging the binary inside the wheel (`bindings = "bin"`): needs a second wheel.
  - A Python reimplementation of the CLI: violates FR-003a.

## R6. The solver prerequisite

- **Decision:**
  - When the solver cannot be started, the error names the prerequisite and both ways to supply
    it: "verification needs the Z3 SMT solver (supported: 4.16.0); install `z3` on PATH or set
    BEHAVIOR_Z3". The same message appears in the CLI, in Python (`BehaviorError`) and in the
    docs.
  - The attestation keeps recording the solver version it ran with, as today.
  - A different solver version is allowed but reported: the CLI prints a warning to stderr, and
    Python emits `warnings.warn`.
  - Nothing else requires the solver: admission, evaluation, stores and replay work without it.
- **Rationale:** FR-005 and the missing-solver edge case.
- **Alternatives considered:**
  - Bundling Z3: large, licence-neutral but platform-heavy, and the spec keeps it external.
  - Refusing other versions: too strict. Results may differ, but the attestation says which
    version produced them.

## R7. Public API manifest and its checks

- **Decision:** `api/public-api.json` is a checked-in, canonical JSON document for the release.
  - **`python`:** the names exported by `behavior` (equal to `behavior.__all__`), each with its
    kind (function, class, decorator, value).
  - **`cli`:** the subcommands and their flags, taken from clap's command tree.
  - **`formats`:** wire IR versions with their JSON schema files, record versions, store
    document tags, and the attestation and authorization formats.
  - **`backend`:** the backend contract methods, with required or optional marked.
  - **`solver`:** name and supported version.

  Three tests keep it honest:
  1. `behavior.__all__` equals the manifest's Python names.
  2. The CLI command tree equals the manifest's CLI section.
  3. The backend methods called by the store equal the manifest's backend list.
- **Rationale:** FR-006, FR-021. It gives the skill checker one authoritative list, and a change
  to the public surface becomes a visible diff.
- **Alternatives considered:** deriving the API purely from `__all__` at check time — rejected
  because it does not cover the CLI, formats or backend, and gives no reviewable diff.

## R8. Skills: layout, format and drift control

- **Decision:**
  - **Consumer skills** live in `skills/` at the repository root:
    `skills/behavior-authoring/`, `skills/behavior-verification/`,
    `skills/behavior-application/`. Each contains:
    - `SKILL.md`: agent-skill frontmatter (`name`, `description`) plus
      `release: 0.8.0` (FR-018);
    - `examples/*.py`: self-checking scripts that `assert` their stated outcome.
  - `skills/README.md` explains how an agent is pointed at the skills of a pinned release (the
    maintainer's manual step: copy `skills/*` of tag `vX.Y.Z` into the consumer's agent skills
    directory). It also defines the gap log (R10).
  - **The engine skill** lives in `.claude/skills/behavior-engine-development/SKILL.md`. It is
    active for agents in this repository, and it is outside `skills/` so it is never handed to
    consumers (FR-019, SC-007).
  - **Drift control** (FR-017, FR-020, FR-021):
    1. Every `examples/*.py` runs against the installed package and must exit 0. Its assertions
       are its stated outcome.
    2. Every fenced `python` block in a consumer `SKILL.md` must start with a comment
       `# from examples/<file>.py`, and its lines must appear verbatim, in order, in that file.
       Prose never shows code that is not tested.
    3. Every `behavior.<name>` import or reference and every `behavior <command>` in consumer
       skills must be in the public API manifest (R7).
    4. Each skill's `release` equals the release version.
  - The examples reuse the existing example domains, mainly `examples/orders`, `accounts`,
    `invoice` and `ledger`, and never invent new language features.
- **Rationale:**
  - A separate top-level directory makes "exactly three consumer skills" a directory listing.
  - Verbatim excerpts plus runnable files mean skill prose cannot silently drift from the
    package.
- **Alternatives considered:**
  - Executing SKILL.md snippets directly: snippets are fragments and cannot run alone.
  - Consumer skills under `.claude/skills`: they would load for engine agents in this repo and
    blur the boundary.

## R9. The release check

- **Decision:** `scripts/release-check.sh` runs these steps; any failure stops the release.
  1. Run the normal gates (fmt, clippy, tests, pytest, mypy, determinism).
  2. Build the wheel (R4) and run `auditwheel show`.
  3. Create a temporary directory **outside the repository**. Make a venv from a bare Python
     3.13 with no site packages. Install only the wheel, with `pip install --no-index
     --require-hashes`, and with `PATH` stripped of `cargo`/`rustc` (asserted absent; SC-010).
  4. In that venv:
     - run the consumer smoke scenario `release/smoke.py`: author, admit, evaluate, verify,
       store, commit, replay, CLI (SC-001);
     - run every skill example (SC-002);
     - run the missing-solver case with `BEHAVIOR_Z3=/nonexistent` (explicit error; evaluation
       still works);
     - run the version check (`behavior.versions()` equals `release-manifest.json`).
  5. Run the same smoke scenario inside the repository. The canonical outputs (records, state
     identities, attestation contents) must be byte-identical (SC-003, FR-007).
  6. Run the cross-binding equivalence test (R11) and the skill drift checks (R8).
- **Rationale:** FR-020–FR-023; the constitution's quality gates.
- **Alternatives considered:** CI-only checks — there is no CI configured, so a script that CI
  can call later is the portable form.

## R10. Semantic gap log format

- **Decision:** A consumer project keeps `SEMANTIC_GAPS.md`. It has one `## GAP-NNN: <title>`
  section per gap, with fixed fields:
  - `Date`;
  - `Behavior release`;
  - `Requirement` (the business rule in plain words);
  - `Why inexpressible` (the missing construct, e.g. grouped invariant, cross-entity filter,
    bulk effect);
  - `What was done instead` (`nothing`, or an explicit, reviewed host-side interim with its
    risk);
  - `Severity` (`blocking` | `degraded` | `cosmetic`);
  - `Evidence` (the attempted model or error).

  The template and a filled example live in `skills/README.md`, and each consumer skill links
  to it.
- **Rationale:** FR-016. Plain markdown with fixed headings can be read by humans and collected
  mechanically across repositories.
- **Alternatives considered:** a JSON log — harder for agents to append to correctly, and no
  better for a handful of entries.

## R11. Cross-binding equivalence while one binding exists

- **Decision:**
  - The canonical wire form is the binding-neutral reference. `build_fixtures.py`, which writes
    JSON directly and never goes through the DSL, gains one generator per example domain that
    lacks one: `orders_example`, `ledger` and `accounts` where they differ from existing
    fixtures. The fixtures go to `tests/fixtures/bindings/`.
  - `python/tests/test_binding_equivalence.py` builds every example module through the Python
    binding. It asserts, per module, the same behavior version and the same item hashes as the
    admitted canonical fixture. It compares hashes, not bytes: source locations are metadata and
    not part of the typed IR's identity.
  - Future bindings add the same test against the same fixtures (SC-009).
- **Rationale:** FR-003b. Two independent authoring paths (DSL and a direct JSON generator) that
  meet at the same hashes is the property a second binding will have to meet.
- **Alternatives considered:** fixtures generated by the DSL itself — circular, since they would
  prove nothing.

## R12. Agent-facing acceptance (SC-004–SC-006) and Principle III

- **Decision:**
  - SC-004–SC-006 are **manual acceptance evaluations**, not automated tests: the constitution
    forbids tests that call a live model.
  - `skills/evals/` holds the reference sets:
    - expressible requirements drawn from the example domains;
    - inexpressible requirements (a grouped invariant, a cross-entity filter, a bulk effect);
    - three seeded verification situations (a real counterexample, a precision-debt
      inconclusive, a missing-guarantee inconclusive).
  - A rubric per set scores the agent's output. The maintainer runs an agent in a scratch
    consumer repository against a pinned release and records the results in
    `specs/008-agent-ready-package/evals/results.md`.
- **Rationale:**
  - The spec's agent outcomes need a real agent.
  - The constitution keeps the automated suite deterministic.
  - The deterministic parts (every model the rubric expects must be admitted; gap entries must
    have all fields) are checked by scripts over the agent's output files.
- **Alternatives considered:** recorded agent transcripts as replay fixtures — they would test
  the transcript, not the skills.

## R13. Versioning policy

- **Decision:** `docs/versioning.md` defines the policy (0.x.y while pre-1.0).
  - **Minor bump** (0.8 → 0.9) for:
    - any change to behavior identities (item hashes, behavior versions);
    - a new wire, record or store document version, or a change to one;
    - removing or changing a public API element;
    - a verifier change that can alter outcomes, together with a `VERIFIER_VERSION` bump.
  - **Patch** for additive public API, fixes that change no identity or format, and docs or
    skills.
  - Old records and stores within a minor line must replay. A release that cannot read an older
    document refuses with an explicit error (FR-009); the existing version gates already do
    this.
  - After 1.0, "minor" in this policy becomes "major".
- **Rationale:** FR-008, FR-009. It names the three version kinds (FR-004) and how each moves.
- **Alternatives considered:** strict SemVer before 1.0 — the conventional 0.x "minor may break"
  reading is clearer for an experiment.
