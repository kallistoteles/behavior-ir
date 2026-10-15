# Data model: Core Release 0.13.0 alignment

No new runtime entities. The changed documents:

- **Core pin** (`core-release.json`, `behavior.core_pin.v1`): `version`, `tag`, `commit`,
  `repository`, `assets.cli/conformance.{file,sha256}` — values in research R1. Invariant
  (gate): Cargo.toml `rev` and Cargo.lock source equal `commit`; no override.
- **Ecosystem version**: `[workspace.package] version = "0.13.0"`; `behavior.__version__`,
  `api/public-api.json` `release`, skill front matter `release` all equal it.
- **Model compatibility fixture** (`models/fixtures/compat.json`): each tuple's `core_release`
  becomes `{version: "0.13.0", commit: "1509497…"}`; model semantic and lowerer versions and
  the pinned case outputs are unchanged.
- **Version report** (`behavior.versions()`): see [contracts/versions.md](contracts/versions.md).
