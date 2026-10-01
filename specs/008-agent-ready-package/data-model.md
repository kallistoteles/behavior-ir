# Data Model: Agent-Ready Package and Skills Library

This feature adds no behavior semantics. Its entities are **release artifacts and documents**.
Formats are in [contracts/](contracts/).

## Behavior release

An immutable, tagged engine release.

| Field | Description | Rule |
|---|---|---|
| `version` | `MAJOR.MINOR.PATCH`, e.g. `0.8.0` | Equal to the workspace crate version and the git tag `v<version>` |
| `tag` | annotated git tag `v<version>` | Created only after the release check passes; never moved |
| `versions` | engine, wire IR list, record list, store document tags, verifier | From `behavior_cli::engine_info()`, built on `behavior_core::format_versions()` (R2) |
| `artifacts` | files in `dist/v<version>/` with SHA-256 | Listed in `release-manifest.json` and `SHA256SUMS` |
| `platforms` | supported targets, e.g. `manylinux_2_28_x86_64` | Only platforms with a built and checked artifact |
| `solver` | `{"name": "z3", "version": "4.16.0"}` | Documented prerequisite (FR-005) |

**Lifecycle:** `building → checked → tagged → published`.
- `published` means uploaded, which is a manual step.
- A failed release check stops at `building`, and no tag is created.
- A tag is never moved; a fix is a new patch release.

## Binding package

The installable package for one language. Only the Python binding exists in 008.

| Field | Description | Rule |
|---|---|---|
| `language` | e.g. `python` | One package per language |
| `binding_version` | package version | Equal to the release version (exact match at first, FR-004) |
| `engine_version` | engine it loads | Checked at import; a mismatch is `ImportError` |
| `platform_tag` | e.g. `cp313-abi3-manylinux_2_28_x86_64` | Installable without the engine toolchain (FR-003) |
| `public_api` | exported names | Equal to the `python` section of the public API manifest |
| `cli` | console script `behavior` | Same code as the engine CLI (R5) |

**Invariant (FR-003a):** the package contains no semantic logic outside the engine. Its Python
layer only builds nodes and calls the engine.

## Public API manifest

`api/public-api.json`: the stable surface of one release (R7). Sections: `release`, `python`,
`cli`, `formats`, `backend`, `solver`. Anything absent is internal.

**Rule:** checked-in and reviewed. Tests fail if the package, the CLI or the backend contract
disagree with it.

## Consumer skill

| Field | Description | Rule |
|---|---|---|
| `name` | `behavior-authoring` \| `behavior-verification` \| `behavior-application` | Exactly these three live in `skills/` |
| `description` | when an agent should use it | Frontmatter |
| `release` | release version described | Equal to the release version |
| `examples` | `examples/*.py` | Each runs against the installed package and exits 0 |
| `excerpts` | fenced `python` blocks in `SKILL.md` | Each begins `# from examples/<file>.py` and appears verbatim in that file |
| `api references` | `behavior.X` names, `behavior <cmd>` commands | All in the public API manifest |

**Rule (FR-015):** each consumer skill contains the semantic-gap rule and links to the gap log
format.

## Engine skill

`.claude/skills/behavior-engine-development/SKILL.md`: guidance for agents changing this
repository (the constitution, Spec Kit, gates, frozen identities, wire version gates). It is never
placed in `skills/`.

## Semantic gap entry (consumer project)

One `## GAP-NNN: <title>` section in the consumer's `SEMANTIC_GAPS.md` (R10).

| Field | Rule |
|---|---|
| `Date` | ISO date |
| `Behavior release` | the pinned release version |
| `Requirement` | the business rule, in domain words |
| `Why inexpressible` | the missing construct, naming the closest existing one |
| `What was done instead` | `nothing`, or a named, reviewed host-side interim with its risk |
| `Severity` | `blocking` \| `degraded` \| `cosmetic` |
| `Evidence` | the attempted model fragment or engine error |

## Evaluation set (manual acceptance)

`skills/evals/`: requirement sets and seeded verification situations with a rubric (R12).
Results are recorded in `specs/008-agent-ready-package/evals/results.md`.
