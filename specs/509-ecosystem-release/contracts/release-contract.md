# Contract: Behavior 0.13.0 Release

## Identity and deliverables

- Ecosystem and Python binding: 0.13.0, from Cargo workspace version; dynamic package version
  remains the existing mechanism. Core: exactly the existing 0.15.0 pin; verifier: 0.10.0.
- Existing distribution: GitHub Release of the manylinux_2_28_x86_64 wheel, Python >=3.13,
  SHA256SUMS and release-manifest.json. Skills remain accessible at the same repository tag.
- Existing `behavior.release_manifest.v1` and version reporting stay unchanged.
- Committed `release/notes/0.13.0.md` supplies consumer-facing changes. Existing release.sh
  generates exact identity/install text and appends the authored notes into NOTES.md;
  the existing workflow uses that file as the published release body.

## Preparation acceptance

Use existing scripts, without creating/pushing a product release tag during preparation:

| Check | Required result |
|---|---|
| scripts/gates.sh through the existing full release check | All required gates passed, including full property tests and 507 quick runtime gate |
| scripts/conformance.sh 506 complete | Every check passed; complete true; C19 proven on the unchanged reference module |
| scripts/conformance.sh 507 complete | Every check and all 12 obligations passed; evidence classes and assumptions retained |
| scripts/release-check.sh | Wheel compliance, clean install, smoke, missing-solver handling, version identity, Skill examples, public API/equivalence and reproducibility passed |

Full integration conformance is not a substitute for complete conformance. Gate failures are
blocking. No check/profile is weakened, no baseline is rewritten and no new external
application or manual agent evaluation is required. The release check already invokes full
gates; avoid gratuitous repeat runs on unchanged source.

## Merge and explicitly authorized publication

The release changes, including Core adoption, must be on the selected merged commit. Confirm
successful existing required CI checks `pin`, `surface`, `gates`, `package` and revalidate
complete conformance for the selected source before publication. Record the commit explicitly:
ancestry on main alone does not establish that it includes the intended release changes.

Only after explicit publication authorization:

1. Fetch current main/tags, select the clean merged release commit and record its identity.
2. Create annotated v0.13.0 on that commit, if the name is unused; never move/overwrite it.
3. Run check-tag.sh with authenticated access and the existing required CI check names.
4. Run release.sh 0.13.0 on that tag's commit. It requires the tag first, builds/checks and
   generates NOTES; it does not create tags or publish.
5. Push only that tag. The existing ecosystem-release workflow checks/builds again and publishes.
6. Verify published artifacts before declaring Feature 509 complete.

Preparation scripts and generated tasks must label this phase as requiring later authorization.
No new publishing destination, workflow, permanent branch or distribution target is introduced.

## Published verification

Use release-verify.sh to rebuild from the tag and compare published/rebuilt SHA256SUMS.
Additionally, hash the downloaded wheel against its checksum file and compare the downloaded
manifest with the local tagged build. Check the version/Core/verifier fields, commit and artifact
hashes; standard sha256sum, git and Python inspection suffice. The current release-verify script
alone does not validate downloaded wheel bytes or the published manifest.

Record the release URL, selected commit, workflow result and verification outcomes. Checks must
pass on actual assets; a matching filename or checksum text alone does not prove asset identity.

## Failure and immutability

Stop at the failed step and retain its evidence. Missing solver, missing authorization,
unsuccessful required CI or a partial publication does not count as complete.
A transient failure may use the existing workflow's recovery only without replacing published
content. A defective tagged release requires an explicit replacement-version decision under
the existing policy, never a moved/deleted tag. Historical artifacts and attestations are not
regenerated in place. Noncritical findings are recorded for subsequent releases.
