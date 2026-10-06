# Versioning

A Behavior release has one version number, but it carries several independent versions. They
change for different reasons, and every one of them is reported, never implied
(`behavior.versions()`, `behavior engine-info`, `release-manifest.json`).

| Kind | Example | What it identifies | Where it lives |
|---|---|---|---|
| **Release** | `0.12.0` | this repository's release: the Python package and everything it bundles | `[workspace.package] version` in `Cargo.toml`, the single source |
| **Core** | `0.12.0` | the exact Core Release the package is built against and bundles (feature 011) | `core-release.json`; `behavior.versions()["core"]` reports the full immutable commit |
| **Binding** | `python 0.12.0` | the installable package of the Python binding | the release version in Python's form (PEP 440); the package requires *exactly* the core in `core-release.json` (checked at import) |
| **Formats and verifier** | accepted wire IR `0.1` … `0.8`, records `0.4` … `0.7`, verifier `0.8.0` | the document formats and the verification encoding | defined by the core: [its versioning](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/versioning.md); reported by `behavior engine-info` |

A module document, a record or a store document says which format version it is written in.
That is what a reader checks, not the release number.

## Release version policy (0.x)

While the release is below 1.0, the **minor** number marks anything a consumer must adapt to:

- **Minor bump** (`0.8.x` → `0.9.0`):
  - any change to behavior identities (item hashes, behavior versions);
  - a new wire IR, record or store document version, or a change to an existing one;
  - removing or changing a public API element (see `api/public-api.json`);
  - a verifier change that can alter outcomes, together with a `VERIFIER_VERSION` bump.
- **Patch bump** (`0.8.0` → `0.8.1`):
  - additive public API;
  - fixes that change no identity, format or verification outcome;
  - documentation and skills.

After 1.0, the same rules apply with **major** in place of minor.

Release 0.12.0 aligns the package with Core Release 0.12.0 (ecosystem feature 500). Its minor
bump reflects current document formats, verifier outcomes and governance refusal behavior:

- `BehaviorModule.from_wire_json` admits current documents explicitly. The existing Python DSL
  retains its legacy profile, semantic identities and replay paths.
- Unified invocation handles reads and actions with typed bindings, explicit snapshots and
  host context. The package exposes canonical invocation records, candidates and replay.
- Wire IR 0.8 introduces command intents and decision record 0.7. Durable occurrences are
  streamed using exact history anchors; source diagnostics remain outside record identity.
- Current reads on Wire 0.8 produce `behavior.read_record.v2`. Historical v1 records retain
  their replay path. Verifier 0.8.0 corrects read-filter assumptions and entity-role validity;
  fresh verification outcomes can therefore differ from the older release.
- Store-v2 and trusted governance are explicit. Fresh writes to required-governance v1
  histories refuse with `TRUSTED_GOVERNANCE_UPGRADE_REQUIRED`, including migrations. Exporting
  validated state into a new v2 genesis preserves content without authenticating old history.
- The bundled CLI exposes `invoke`, `invoke-intent`, `invoke-replay` and `governance`. Signed
  proof and authorization generation use that CLI; Python commits independently supplied
  authorization context through the public core contract.

Core's historical `wire_ir` version-report array remains 0.1–0.7. The additional
`accepted_wire_ir` array reports decoder support through 0.8. `records`, `read_records`,
`command_stream` and `command_occurrence_domain` report the actual current formats/domains.
The ecosystem public API manifest lists all accepted wire formats, including the explicit
current profile; it does not imply the legacy Python DSL emits that profile.

See the pinned core documentation for
[invocation](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/invocation.md),
[commands](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/commands.md) and
[governance](https://github.com/kallistoteles/behavior-ir-core/blob/v0.12.0/docs/governance.md).

Release 0.10.4 makes the package reproducible across machines (SC-011): maturin's generated
Rust SBOM recorded the absolute build directory, so the wheel built in CI and one built locally
from the same commit differed in that file alone. The SBOM is off until a path-independent one is
produced, and the release check refuses any wheel member that embeds a build directory. Still
bundles Core Release 0.10.2.

Release 0.10.3 is the first published release of the ecosystem as its own repository, bundling
Core Release 0.10.2. The tag `v0.10.2` exists but was never released: its release check failed
on a test that inherited the workflow's required-check list from the environment. Under the tag
rule (a tag is never moved) the fixed test ships as 0.10.3. From here the ecosystem's release
version and the core's differ; the package names its core exactly.

Release 0.10.2 is a patch release: the repository split (feature 011). Behavior Core moved to
behavior-ir-core and is consumed as Core Release 0.10.2, pinned by commit, through its public
`behavior-engine` crate. The package bundles the core's `behavior` CLI and reports the core it
contains. No identity, format or verification outcome changes. The ecosystem's release version
follows the core's for now; the two may diverge later, and the package always names its core
exactly.

Release 0.10.0 was a minor bump for three reasons:

- **Wire IR 0.7:** a module's `reads` section (declared reads, a new behavior item kind) and the
  read document (an ad-hoc read). A module with declared reads has a new behavior version, never a
  new schema; a module without them keeps its bytes and identity.
- **A new document kind:** the read record `behavior.read_record.v1`, with its identity
  `read:sha256:…`.
- **`VERIFIER_VERSION` 0.6.0**, for the `evaluation_error` checks of declared reads.

Every existing module, decision record, store document, hash vector and attestation format keeps
its bytes.

Release 0.9.0 was a minor bump for three reasons:

- **Exact store-schema binding** replaces the touched-types comparison. A module that differs from
  the store only in an entity type its action does not touch used to evaluate and now gets
  `SCHEMA_MISMATCH`, and the old code `ENTITY_DECLARATION_MISMATCH` is no longer reported for it.
- **A new document kind:** the migration IR 0.1, plus optional fields that existing documents
  never carry: `Head.schema`, the record `kind` and `migration` fields, and
  `EvidencePolicy.migration`.
- **`VERIFIER_VERSION` 0.5.0**, for migration verification and the narrowing obligation.

Every existing module, record, genesis, store head (of a store that never migrated), evidence
policy and attestation format keeps its bytes.

## Compatibility promise

- Documents written by any release of a minor line (modules, records, stores, attestations) are
  read and replayed by every later release of the same line with identical results.
- A release that cannot read a document refuses it explicitly: the wire, record and store
  version gates report the unsupported version. It never produces different results
  silently.
- An existing document format never changes its bytes. New forms get a new version; documents
  without them keep their old version and bytes.

## Releases

A release is the annotated tag `v<version>` on a commit that passed `scripts/release-check.sh`,
together with the artifacts `scripts/release.sh` built into `dist/v<version>/`. Tags are never
moved; a fix is a new patch release.
