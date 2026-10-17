# Data Model: Ecosystem Release and Skills Update

This feature changes release content and guidance, not runtime entities or persistence schemas.
The objects below describe existing files and evidence; no new database, public schema or
release-manifest format is introduced.

## Release candidate

- **Identity:** Behavior version 0.13.0 plus the exact source commit.
- **Dependencies:** Core 0.15.0, commit `6b93898a9375113f7a8946f23ab5535319b6f5d6`,
  verifier 0.10.0 and the unchanged asset digests in core-release.json.
- **Content:** Three consumer Skills and their examples, README, versioning guidance,
  authored release notes, package and existing release metadata.
- **Validation:** Existing gate and release-check results plus independent 506/507 complete
  reports associated with the candidate commit.

Changing candidate source invalidates claims that older evidence covers that exact commit.
Re-run affected checks; final publication checks must cover the merged tagged source.
Version claims distinguish ecosystem/binding version from Core/verifier version.

## Consumer Skill

- **Identity:** Existing skill name and release frontmatter, matching workspace version.
- **Content:** Established sections, semantic-gap paragraph and executable examples.
- **Relationships:** Python excerpts cite a local examples file; examples depend only on the
  installed public package and standard library, never on this repository's fixtures/helpers.
- **Validation:** Existing layout, release, reference, excerpt and executable-example checks.

No fourth Skill, example registry, common example service or external evaluation dataset is needed.

## Release artifacts and evidence

Reuse the current `behavior.release_manifest.v1` fields: release, tag, commit, versions,
core, bindings, platforms, artifacts and solver. SHA256SUMS names the wheel digest; the
manifest records that wheel and its digest. NOTES.md is the existing generated release body:
exact generated identity header followed by committed 0.13.0 release prose.

Required relationships:

1. Tagged commit, manifest commit and verified merged release commit are identical.
2. Manifest Core identity equals core-release.json; installed version reports equal the manifest.
3. Downloaded wheel bytes satisfy published SHA256SUMS; published checksum and manifest
   content agree with the local tagged build.
4. Both conformance reports say complete; all required checks and obligations passed.
5. A reproducibility comparison says identical bytes for builds of the same source/toolchain.

The implementation log records candidate commit, commands, outcomes, CI/release URLs and
published verification results. Keep large generated build output in existing artifact locations;
do not create another archival evidence system or copy Core fixtures into the repository.

## Publication state

`preparing → ready for publication → merged and revalidated → explicitly authorized →
published → verified`

- Preparation may build and check without a release tag; ready does not mean feature complete.
- Publication requires merged source, successful required CI and explicit authorization.
- The annotated v0.13.0 tag points to that selected commit. Pushing it starts the existing workflow.
- Feature completion requires the published assets to pass verification.
- A failed prerequisite or publication/verification step leaves completion pending; it cannot
  be hidden by moving a tag or silently substituting 0.13.1 for the requested target.

These are reporting states, not a new release automation state machine.

## Release finding or suggestion

Record in a short implementation-log table: issue, concrete impact, evidence/location,
blocking/nonblocking classification and disposition. For suggestions also record benefit and
necessary/approval/deferred decision; name a later release when deferred.

Required-gate failures and incorrect artifacts remain blocking. A nonblocking finding is
recorded, not automatically fixed. Historical published tags, records and attestations stay immutable.
