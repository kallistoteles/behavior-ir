# Data Model: Core and Ecosystem Repositories

This feature adds no runtime entities. Its "data" is the set of release and boundary documents
that make the split checkable. Every JSON document below is written canonically: sorted keys,
no insignificant whitespace, and a trailing newline. This matches today's `release-manifest.json`.

## Core Release

An immutable annotated tag `v<version>` on one commit of `behavior-ir-core`, plus its GitHub
Release assets.

| Field | Source | Rule |
|---|---|---|
| version | `[workspace.package] version` | equals the tag without `v` (checked before building) |
| tag | `v<version>` | annotated, never moved or deleted (FR-028) |
| commit | `git rev-parse HEAD` | reachable from `main` |
| assets | `dist/v<version>/` | `release-manifest.json`, `SHA256SUMS`, `behavior-conformance-<v>.tar.gz`, `behavior-<v>-x86_64-linux-manylinux_2_28` |

### Core `release-manifest.json` (`behavior.core_release_manifest.v1`)

```json
{
  "format": "behavior.core_release_manifest.v1",
  "release": "0.10.1",
  "tag": "v0.10.1",
  "commit": "<40 hex>",
  "versions": { "engine": "0.10.1", "read_records": ["behavior.read_record.v1"],
                "records": ["0.4", "0.5", "0.6"], "store_documents": ["behavior.entity_content.v1", "…"],
                "verifier": "0.6.0", "wire_ir": ["0.1", "…", "0.7"] },
  "public_surface": { "crate": "behavior-engine", "items_sha256": "<sha256 of api/engine-surface.txt>" },
  "artifacts": [ { "file": "…", "sha256": "<64 hex>" } ],
  "solver": { "name": "z3", "version": "4.16.0" }
}
```

- `versions` is exactly `behavior engine-info` output, the same function the binding calls.
  The migration IR version is not in `engine-info` today. Adding it changes a contract, so it is
  a follow-up core feature, not part of this extraction (FR-021).
- `artifacts` lists every asset except the manifest and `SHA256SUMS`, sorted by `file`.

## Ecosystem pin: `core-release.json`

Tracked at the ecosystem root. It is the single human-edited statement of the core the ecosystem
requires (FR-017).

```json
{
  "format": "behavior.core_pin.v1",
  "version": "0.10.1",
  "tag": "v0.10.1",
  "commit": "<40 hex>",
  "repository": "https://github.com/kallistoteles/behavior-ir-core",
  "assets": {
    "conformance": { "file": "behavior-conformance-0.10.1.tar.gz", "sha256": "<64 hex>" },
    "cli":         { "file": "behavior-0.10.1-x86_64-linux-manylinux_2_28", "sha256": "<64 hex>" }
  }
}
```

**Invariants** (`scripts/check-core-pin.sh`):

1. `commit` equals the `rev` of `behavior-engine` in `Cargo.toml` and the source in `Cargo.lock`.
2. `cargo metadata` resolves `behavior-engine` from `git+<repository>?rev=<commit>`. No path or
   `[patch]` override is active, except when `BEHAVIOR_DEV_CORE_PATH` is explicitly set, and
   never in CI or release.
3. The `cli` binary's `engine-info` reports `engine == version`.
4. The fetched assets match their `sha256`.

## Ecosystem Release

An annotated tag `v<version>` on `behavior-ir`.

### Ecosystem `release-manifest.json` (`behavior.release_manifest.v1`, extended)

The existing manifest keeps every field. One field is added: `core`, a copy of
`core-release.json` without `assets`. This is an additive field in the release tooling's
document, not an engine format.

```json
{ "format": "behavior.release_manifest.v1", "release": "0.10.1", "tag": "v0.10.1",
  "commit": "<ecosystem sha>", "versions": { … engine-info of the bundled core … },
  "core": { "version": "0.10.1", "tag": "v0.10.1", "commit": "<core sha>",
            "repository": "https://github.com/kallistoteles/behavior-ir-core" },
  "bindings": { "python": { "version": "0.10.1", "requires_python": ">=3.13" } },
  "platforms": ["manylinux_2_28_x86_64"], "artifacts": [ … ], "solver": { … } }
```

`behavior.versions()` gains `core: {version, commit}`. Today's keys are unchanged, and
`test_versions.py` pins the new shape. US4, scenario 2: the CLI's `engine-info` `engine` value
equals `versions()["core"]["version"]`.

## Public surface list: `api/engine-surface.txt` (core)

- One exported path per line, sorted, for example `read::ReadSource` and `engine_info`.
- It is generated from and checked against `crates/behavior-engine/src/lib.rs`.
- Rule: no line may end in `*`.
- A change to this file is a public API change and is classified by `docs/versioning.md`.

## Ownership manifest ([contracts/ownership.md](contracts/ownership.md))

An ordered list of `(glob, owner, note)` rules. The owner is `core` or `ecosystem`.

**Validation** (preflight): every path in `git ls-files` matches exactly one rule. A path that
matches no rule, or rules with different owners, fails and is named.

## Conformance digest

`scripts/conformance-digest.sh` writes `{path: sha256}` over:

- `tests/fixtures/**` (goldens, hash vectors, records, store documents);
- the stdout of every CLI call in the core determinism check.

The digests taken before the move (stage 1) and after it (stages 2 and 3) must be equal
(SC-003, SC-009). A difference is listed path by path.

## Spec numbering state

There is no stored state. The next number is derived by Spec Kit from `specs/`:

| Repository | Existing | Range | First new feature |
|---|---|---|---|
| core | 001–007, 009, 010, 011 (copy) | 012+ | `--number 012` (automatic anyway) |
| ecosystem | 008, 011 | 500+ | `--number 500` |
