# Contract: GitHub Actions workflows

The workflows only orchestrate. Every step that decides a verdict or builds an artifact is a
repository script that runs identically on a laptop inside `nix develop` (FR-030).

## Common rules

- Runner: `ubuntu-24.04`.
- Toolchain: nix, through a pinned installer action, then `nix develop -c <script>`. No
  `actions-rs`, `setup-python` or similar steps.
- Third-party actions are pinned by full commit SHA.
- `permissions`: CI has `contents: read`; release has `contents: write`.
- No workflow reads `.specify/`, `specs/` or any agent tooling (FR-035).
- Concurrency: CI cancels superseded runs per ref. Release never cancels.
- Caching (cargo registry and target, nix store) is an optimization only. A cold run must give
  the same verdict.

## `core-ci.yml` (behavior-ir-core)

**Triggers**: `pull_request`; `push` to `main` and `dev`.

| Job | Script(s) | Covers |
|---|---|---|
| gates | `scripts/gates.sh`: fmt, clippy, `cargo test --workspace`, `cargo build`, `determinism-check.sh`, `check-boundary.sh`, `check-public-surface.sh` | FR-005, FR-026 |
| consumer | `scripts/check-consumer.sh`: builds and tests `consumer/` against this commit | FR-006d |

Required status checks on `main`: `gates` and `consumer`.

## `core-release.yml`

**Trigger**: `push` of tags `v*` only. Merges never release (FR-028).

1. `scripts/check-tag.sh "$GITHUB_REF_NAME"`. It refuses, naming the values, if the tag is not
   annotated, its version ≠ the workspace version, the commit is not on `main`, or a required
   check of the commit is not green (FR-028). This runs before any build.
2. `scripts/release-check.sh <version>` (all gates, reproducibility).
3. `scripts/check-consumer.sh --rev <tag commit>`: build against the exact pushed Git revision
   (FR-006d). Failure stops before the final artifact build and publication. The revision is
   already available; a GitHub Release is not required.
4. `scripts/release-build.sh <version> dist/v<version>`.
5. `gh release create v<version> dist/v<version>/* --verify-tag --notes-file
   dist/v<version>/NOTES.md`.

Steps 1–4 live in `scripts/release.sh`, including temporary artifacts used by the full release
checks. The workflow only invokes that script and then publishes. Wrong-version refusals are
tested in disposable repositories; product release tags are never created or deleted for tests.

## `ecosystem-ci.yml` (behavior-ir)

**Triggers**: `pull_request`; `push` to `main` and `dev`.

| Job | Script(s) | Covers |
|---|---|---|
| pin | `scripts/check-core-pin.sh` (no path/patch override; rev = declared commit) | FR-024, US2-1, US2-3 |
| surface | `scripts/check-public-surface.sh --consumer` (only `behavior-engine`; no `behavior_core::` etc.) | FR-007, US2-2 |
| gates | `scripts/fetch-core.sh` then `scripts/gates.sh`: `maturin develop`, `python -m pytest -q python/tests models` (equivalence, model lowering, conformance against `$BEHAVIOR_CORE_DIR`, skills), mypy, fmt/clippy of `behavior-py`, determinism (ecosystem part) | FR-015, FR-016, FR-027 |
| package | `scripts/release-check.sh --skip-tag <version>`: wheel build, auditwheel, clean venv install, smoke, missing solver, versions, byte identity, skill examples | FR-018, FR-027, SC-005 |

Model lowering is included explicitly. Regression tests prove collection of the actual model
tests and failure of the shared gate for deliberately broken lowering. Required status checks on `main`:
all four jobs.

**Core access**: if the core repository is private, the secret `CORE_READ_TOKEN` (fine-grained,
read-only contents on `behavior-ir-core`) is exposed to `fetch-core.sh` and to cargo's git
fetch. Fork pull requests then cannot fetch, which is acceptable for a private project.

## `ecosystem-release.yml`

**Trigger**: tags `v*` only.

1. `scripts/check-tag.sh "$GITHUB_REF_NAME"` (annotated; equals the `Cargo.toml` workspace
   version; on `main`).
2. `scripts/fetch-core.sh`.
3. `scripts/release-check.sh <version>`.
4. `scripts/release-build.sh <version> dist/v<version>`.
5. `gh release create …`. The notes state the core version, tag and commit (FR-029).

## Local equivalence (SC-011)

`scripts/release-verify.sh v<version>` is the same in both repositories. It downloads the
published assets, rebuilds locally from the tag, and diffs `SHA256SUMS`. Its result for each
first release is recorded in the implementation review.

## Proving each gate (SC-010)

For each required gate, one throwaway pull request breaks exactly that gate:

- an unformatted line;
- a clippy warning;
- a failing test;
- a nondeterministic print;
- a core import of a binding path;
- a missing facade export;
- an internal-crate import in the binding;
- a path override;
- an equivalence drift;
- a broken skill example.

Each must turn its job red. The pull requests are closed unmerged, and their run URLs are
recorded.
