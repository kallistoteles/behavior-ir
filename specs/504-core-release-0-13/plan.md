# Implementation Plan: Core Release 0.13.0 alignment

**Branch**: `504-core-release-0-13` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/504-core-release-0-13/spec.md`

## Summary

Move the ecosystem pin from Core Release 0.12.0 (`b525da4`) to the published Core Release
0.13.0 (`1509497`), bump the ecosystem to 0.13.0, and make every place that names the core
agree. The `behavior-engine` public surface is byte-identical between the two releases
(same `items_sha256`), so no binding signature changes are expected; the visible changes are
fresh decision record 0.8, verifier 0.9.0 and the new governance fixtures for it. Add the
query-under-command-profile regression (US2) that unblocks feature 503. Core is not changed.

## Technical Context

**Language/Version**: pinned Rust toolchain; Python 3.13 (abi3); PyO3 0.26.
**Primary Dependencies**: `behavior-engine` at Core Release 0.13.0, exact git rev
`15094972191aa0ae79705df0397c551a18745399`; no new dependencies.
**Storage**: unchanged (`Backend` protocol, in-memory backend in tests).
**Testing**: `cargo test`, pytest, mypy, fmt/clippy, `scripts/gates.sh`,
`scripts/release-check.sh` under `nix develop`.
**Target Platform**: CPython abi3 manylinux x86_64 wheel with the bundled musl CLI.
**Project Type**: library + binding (Rust workspace, Python package).
**Performance Goals**: none new.
**Constraints**: no Core changes, no override (path/patch) in committed metadata or gates;
disk is limited until the new drive arrives (see research R6): one shared cargo target dir.
**Scale/Scope**: pin and metadata files, ~6 test modules, docs and the three skills.

## Constitution Check

| Principle | Design evidence | Result |
| --- | --- | --- |
| I. Deterministic core | All semantics stay in `behavior-engine`; the binding only re-pins. | PASS |
| II. Validated input | Assets are accepted only on checksum match (`fetch-core.sh`). | PASS |
| III. Test-first | Version/pin tests and the US2 regression are updated and seen red before the pin moves. | PASS |
| IV. Reproducibility | Module bytes and identities must not change; any changed expected output is listed with its Core reason (FR-006). | PASS |
| V. Auditability | `implementation-log.md` records red/green evidence and tested revisions. | PASS |
| VI. Simplicity | No new crates, APIs or DSL. | PASS |
| Bindings & packaging | Exact published release; `check-core-pin.sh` unchanged and passing. | PASS |
| Quality gates | Full `gates.sh` and `release-check.sh` before delivery. | PASS |

Pre-research and post-design: PASS. No deviation.

## Project Structure

### Documentation (this feature)

```text
specs/504-core-release-0-13/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/versions.md
├── checklists/requirements.md
└── tasks.md              # /speckit-tasks
```

### Source Code (repository root)

```text
core-release.json                     # pin: version, tag, commit, asset sha256
Cargo.toml / Cargo.lock               # workspace version 0.13.0; behavior-engine rev
models/fixtures/compat.json           # core_release tuple of the pinned model cases
api/public-api.json                   # "release": "0.13.0"
crates/behavior-py/                   # rebuild only; no signature change expected
python/tests/
├── test_core_alignment.py            # CURRENT_CORE, verifier 0.9.0, records 0.4–0.8
├── test_versions.py                  # package 0.13.0
├── test_command_documents.py         # governance fixtures for verifier 0.9
├── test_release_scripts.py, test_release_dependency_guard.py
└── test_query_command_profile.py     # new: US2 through the binding
crates/behavior-models/tests/query_command_profile.rs   # new: US2 through the engine
README.md, models/README.md, docs/versioning.md, skills/*/SKILL.md
```

**Structure Decision**: existing layout; one new Rust and one new Python test module for US2.

## Delivery

Work in the worktree `~/repos/behaviour-ir-504` from `main` `47b4231`. After merge, feature
503 is rebased onto it and its R13 consequence is lifted (spec SC-004). Publishing an
ecosystem package release is outside this feature.

## Complexity Tracking

None.
