# Versioning

A Behavior release has one version number, but it carries several independent versions. They
change for different reasons, and every one of them is reported, never implied
(`behavior.versions()`, `behavior engine-info`, `release-manifest.json`).

| Kind | Example | What it identifies | Where it lives |
|---|---|---|---|
| **Release / engine** | `0.8.0` | the implementation release: the engine crates and the command-line tool | `[workspace.package] version` in `Cargo.toml`, the single source |
| **Binding** | `python 0.8.0` | the installable package of one language binding | taken from the release version; a binding requires *exactly* its engine version (checked at import) |
| **Wire IR** | `0.1` … `0.6` | the module document format; a new semantic form needs a new IR version | `crates/behavior-core/src/wire.rs` |
| **Records** | `0.4` … `0.6` | the decision record format | `crates/behavior-core/src/eval.rs` |
| **Store documents** | `behavior.commit_bundle.v1`, … | persistence documents (genesis, versions, records, bundles, reports) | `crates/behavior-store/src/documents.rs` |
| **Verifier** | `0.4.0` | the verification encoding; part of every attestation and cache key | `VERIFIER_VERSION` in `crates/behavior-verify/src/lib.rs` |

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
