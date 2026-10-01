# Implementation Plan: Agent-Ready Package and Skills Library

**Branch**: `008-agent-ready-package` | **Date**: 2026-09-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/008-agent-ready-package/spec.md`

## Summary

Turn features 001–007 into a **binding-neutral, tagged release (0.8.0)** that a separate project
installs without the engine toolchain, together with a **skills library** that teaches agents to
use exactly what the release implements.

- **Release and version source.** One version source (the workspace crate version) and a
  version report covering engine, wire IR, records, store documents, verifier and binding. The
  binding checks at import that its version matches the engine exactly.
- **Wheel.** A manylinux_2_28 abi3 Python wheel, built with zig and checked with auditwheel.
- **CLI.** Shipped as a console script calling the same Rust CLI code.
- **Solver error.** A clear message when the solver is missing.
- **Public API manifest.** A checked-in manifest that tests hold equal to the package, the CLI
  and the backend contract.
- **Skills.** Three consumer skills in `skills/`, with runnable, self-checking examples and
  excerpt, reference and version drift checks. The engine skill lives in `.claude/skills/`.
- **Gap log.** A semantic gap log format.
- **Cross-binding equivalence.** DSL modules must match independently generated wire fixtures by
  hash.
- **Release scripts.** A release script and a release check that install the wheel into a clean
  environment outside the repository.
- **Agent acceptance.** Manual agent evaluations with rubrics. No automated test calls a model.

No behavior semantics change.

## Technical Context

**Language/Version**: Rust 1.98.1 (pinned), Python ≥ 3.13 (binding, CPython stable ABI 3.13).

**Primary Dependencies**:
- existing crates;
- PyO3 0.26 with the `abi3-py313` feature added;
- maturin;
- new dev-shell tools **zig** (manylinux cross-linking) and **auditwheel** (wheel compliance
  check).

No new runtime dependencies for consumers.

**Storage**: N/A. Release artifacts are files in `dist/v<version>/` (git-ignored).

**Testing**:
- `cargo test`: `format_versions()`, `engine_info()` and the CLI library entry, the solver error text.
- `pytest`: `test_public_api.py`, `test_skills.py` (examples, excerpts, references, release
  field, layout, gap rule), `test_binding_equivalence.py` (identities and decisions),
  `test_versions.py`, `test_release_scripts.py` (release script refusals and failing-step names;
  slow, opt-in).
- `scripts/release-check.sh`: clean-venv install, smoke scenario, byte-identity, missing solver.
- The determinism check is extended to the smoke scenario.

**Target Platform**: Linux x86-64 (manylinux_2_28), CPython ≥ 3.13.

**Project Type**: library + CLI + language binding + documentation (skills).

**Performance Goals**: The consumer install plus smoke scenario take under 10 minutes (SC-001;
expected to be under 1 minute once the wheel is built).

**Constraints**:
- no engine toolchain for consumers;
- bindings carry no semantics (FR-003a);
- exact engine–binding version match;
- no automated test calls a live model (constitution III);
- existing identities and bytes unchanged (SC-008).

**Scale/Scope**:
- 1 binding and 1 platform;
- 3 consumer skills, 1 engine skill, about 5 example domains;
- 1 manifest, 2 release scripts, 3 evaluation sets.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How the plan complies |
|-----------|--------|-----------------------|
| I. Deterministic core | Pass | No semantic change. Agents only *propose* models and edits, which the engine admits, evaluates and verifies. Skills tell agents never to bypass the engine |
| II. AI output validated | Pass | Agent-authored models pass admission (typed parse) before anything uses them. Agent-proposed gap entries have fixed fields, checked by the evaluation scripts |
| III. Test-first | Pass (process) | Manifest, skill-drift, equivalence and version tests are written and seen failing first. Agent evaluations are manual acceptance, **not** automated tests, so no test calls a live model |
| IV. Reproducibility | Pass | Byte-identity between the installed package and the repo (FR-007, SC-003); tags are immutable; artifacts are pinned by SHA-256; solver version recorded |
| V. Explicit state and auditability | Pass | Release manifest, public API manifest and gap log are explicit, versioned documents |
| VI. Simplicity | Pass with justification | No new crates. The CLI becomes lib+bin instead of a second artifact. Two dev-shell tools (zig, auditwheel), justified below |
| Tech constraints | Pass | Rust gates unchanged; `Cargo.lock` committed; no `unsafe` |

**Post-design re-check (after Phase 1)**: all rows pass. Complexity Tracking lists the
justified additions.

## Project Structure

### Documentation (this feature)

```text
specs/008-agent-ready-package/
├── plan.md
├── research.md            # R1–R13
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── release.md         # layout, manifest, install, version reporting, solver, scripts
│   ├── public-api.md      # api/public-api.json and its checks
│   ├── skills.md          # layout, frontmatter, mandatory content, drift checks
│   └── gap-log.md         # SEMANTIC_GAPS.md format
├── evals/results.md       # manual agent acceptance results (written during implementation)
└── tasks.md               # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                             # workspace version 0.8.0 (single version source)
pyproject.toml                         # dynamic version, console script `behavior`
flake.nix                              # + zig, + auditwheel
api/public-api.json                    # the release's public surface
crates/
├── behavior-core/src/lib.rs           # format_versions()
├── behavior-verify/src/solver.rs      # named-prerequisite error, version warning
├── behavior-cli/src/{lib.rs,main.rs}  # CLI as library (run, engine_info) + thin binary; `engine-info`
└── behavior-py/src/lib.rs             # abi3; engine_info(), ENGINE_VERSION, cli()
python/behavior/
├── __init__.py                        # __version__, versions(), import-time version check
└── _cli.py                            # console-script entry
python/tests/
├── test_public_api.py
├── test_skills.py
├── test_binding_equivalence.py
├── test_release_scripts.py
└── test_versions.py
tests/fixtures/bindings/               # canonical wire modules of the example domains (independent generator)
tests/fixtures/wire/build_fixtures.py  # + example-domain generators
skills/
├── README.md                          # using a pinned release's skills; gap log format
├── behavior-authoring/{SKILL.md,examples/}
├── behavior-verification/{SKILL.md,examples/}
├── behavior-application/{SKILL.md,examples/}
└── evals/                             # manual acceptance sets and rubrics
.claude/skills/behavior-engine-development/SKILL.md
release/smoke.py                       # consumer smoke scenario
scripts/{release.sh,release-check.sh}
docs/versioning.md
```

**Structure Decision**:
- No new crate.
- The consumer skills get a top-level `skills/` directory so that the consumer delivery is exactly
  one directory.
- The engine skill joins the existing `.claude/skills/`, which loads only for agents working in
  this repository.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| zig in the dev shell | Builds a manylinux wheel that loads outside Nix (FR-003, SC-010) | A plain Nix-built wheel links to Nix store paths and would pass only on this machine. No container runtime is available for manylinux images |
| auditwheel in the dev shell | Proves wheel compliance in the release check | Trusting the platform tag unchecked could publish a non-portable wheel |
| `behavior-cli` becomes lib + bin | One CLI implementation reachable from the binding's console script | A second native artifact needs its own portability work with no current user; a Python CLI would duplicate semantics (FR-003a) |
| Public API manifest (a new checked-in document) | One authoritative list for skill checks and API review (FR-006, FR-021) | `__all__` alone does not cover the CLI, formats or backend, and gives no reviewable diff |
