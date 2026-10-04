# Contract: Core Release and the ecosystem pin

## Core Release

- **Identity**: an annotated tag `v<version>` on one commit of `behavior-ir-core`. The tag is
  never moved or deleted; a broken release is replaced by a new patch version.
- **Preconditions**, checked by `scripts/release-check.sh` in the core:
  1. The tag version equals `[workspace.package] version`, otherwise the check refuses and names
     both.
  2. fmt, clippy `-D warnings` and `cargo test --workspace` pass.
  3. The conformance fixtures, wire, verification, persistence and replay fixtures pass (part of
     the workspace tests).
  4. `scripts/determinism-check.sh` passes.
  5. `scripts/check-boundary.sh` passes (no binding, model or ecosystem reference).
  6. `scripts/check-public-surface.sh` and the external consumer build pass.
  7. A double build gives identical checksums (reproducibility).
- **Assets** (`dist/v<version>/`, built by `scripts/release-build.sh`):

  | File | Content |
  |---|---|
  | `release-manifest.json` | `behavior.core_release_manifest.v1` ([data-model.md](../data-model.md)) |
  | `SHA256SUMS` | `sha256  file` lines for every other asset, sorted |
  | `behavior-conformance-<v>.tar.gz` | `schema/`, `tests/fixtures/`, `README.md` (deterministic tar) |
  | `behavior-<v>-x86_64-linux-manylinux_2_28` | the `behavior` CLI, built with `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28 -p behavior-cli` |

- **CLI contract**: unchanged from 0.10.0. The commands, arguments, exit codes and output bytes
  of `behavior` are the same. The determinism check and the conformance digest prove it.

## Ecosystem pin

- `core-release.json` (see [data-model.md](../data-model.md)) is the declaration.
- The `Cargo.toml` git `rev` and the `Cargo.lock` source are the build pin.
- **Refusals** (US2, scenario 3):

  | Situation | Where | Message names |
  |---|---|---|
  | `Cargo.lock`/`Cargo.toml` rev ≠ `core-release.json` commit | `check-core-pin.sh` (CI, release-check) | both commits |
  | a `[patch]` or path override active in CI/release | `check-core-pin.sh` | the override source |
  | extension's engine version ≠ declared core version | `import behavior` (`ImportError`) | both versions |
  | bundled CLI's engine version ≠ declared core version | `release-check` and `fetch-core.sh` | both versions |
  | fetched asset checksum differs | `fetch-core.sh` | file, expected, actual |

- **Updating the pin** is one ecosystem change:
  1. Edit `core-release.json` and the `rev`.
  2. Run `cargo update -p behavior-engine`.
  3. Run `scripts/fetch-core.sh`.

  It is never done by a merge in the core.

## Ecosystem Release

- **Preconditions**, checked by `scripts/release-check.sh` in the ecosystem:
  1. The existing nine steps.
  2. `check-core-pin.sh`.
  3. The bundled CLI equals the pinned asset byte for byte.
  4. A double build gives identical checksums.
- **Assets**: the wheel, `SHA256SUMS` and `release-manifest.json` with its `core` section.
- **Statement**: the GitHub Release body names the core version, tag and commit it bundles
  (FR-029).
