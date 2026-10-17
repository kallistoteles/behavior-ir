# Validation Guide: Behavior 0.13.0 Release

This is a run guide for implementation/publication, not evidence that the commands have already
passed. See contracts/release-contract.md for acceptance and contracts/skills-contract.md for
essential example coverage. No release tag is needed for preparation.

## Prerequisites

- Core adoption from Feature 508 is included in the candidate; core-release.json still pins
  0.15.0 and its exact commit/assets. The workspace declares Behavior 0.13.0.
- Use the existing nix develop toolchain, supported Z3 and sufficient space for wheel/repeat
  builds. Do not delete sources or uncommitted work to make room.
- Use a committed candidate and clean isolated checkout for release validation if the current
  directory contains unrelated changes. For example, from the repository root:

```sh
candidate_commit="$(git rev-parse HEAD)"
validation_dir="$(mktemp -d /tmp/behavior-509-check.XXXXXX)"
git worktree add --detach "$validation_dir" "$candidate_commit"
cd "$validation_dir"
```

All following paths are relative to that checkout. Do not interpret results on an old committed
candidate as validation of newer uncommitted changes.

## Focused Skills and notes validation

During editing, use existing checks before the full release pass:

```sh
nix develop -c bash -c 'scripts/fetch-core.sh >/dev/null && maturin develop -q && python -m pytest -q python/tests/test_skills.py'
```

Expected: all three Skills match the release, public names/excerpts are valid, every standalone
example passes. The notes change starts with a failing regression in the existing release-script
tests; the full release check enables those tests with BEHAVIOR_RELEASE_TESTS=1.

## Prepare the candidate without tagging or publishing

```sh
nix develop -c scripts/release-build.sh 0.13.0 dist/v0.13.0
nix develop -c scripts/release-check.sh dist/v0.13.0
nix develop -c scripts/conformance.sh 506 complete
nix develop -c scripts/conformance.sh 507 complete
```

The build recreates only its dedicated dist/v0.13.0 output directory. The release check already
runs scripts/gates.sh with full properties and release tests, validates installed-wheel Skill
examples and checks reproducibility; a second identical gate run is unnecessary.

Expected: release-check OK; both suites complete with every required check/obligation passed.
Ordinary full-mode integration permits registered blockers, so it is insufficient on its own.
The existing full gates include the mandatory 507 quick runtime budget. Inspect current README,
versioning and Skills against the built release, and the authored notes source against the
notes integration regression. No product tag is needed to check that integration in a disposable
regression repository.

Record the candidate commit, results and remaining nonblocking findings in the implementation
log. At this point the candidate is **ready for publication**, not a completed Feature 509.

## Separate step: merged source and explicit publication authorization

Do not execute this section as part of feature implementation. Begin only after the release
changes have merged and publication has been explicitly requested. Use a clean checkout;
refresh main/tags and verify that the selected commit contains both 508 and 509. Required CI
checks pin, surface, gates and package must have succeeded for it. Revalidate the selected source
with the preparation checks above when prior evidence does not cover that commit.

Provide the existing GH_TOKEN or GITHUB_TOKEN through the normal credential mechanism so the
tag check verifies CI, not just ancestry. Do not write credentials into docs or logs.
If v0.13.0 already exists locally/remotely, stop and follow the immutable-release policy; do not
move or replace it.

From the clean selected merged checkout, with origin/main freshly fetched:

```sh
release_commit="$(git rev-parse HEAD)"
git tag -a v0.13.0 "$release_commit" -m 'Behavior 0.13.0'
GITHUB_REPOSITORY=kallistoteles/behavior-ir CHECK_TAG_REQUIRED='pin surface gates package' nix develop -c scripts/check-tag.sh v0.13.0
nix develop -c scripts/release.sh 0.13.0
git push origin refs/tags/v0.13.0
```

Expected: check-tag validates the annotated tag, version, main ancestry and required CI;
release.sh builds/checks and generates NOTES including the committed release prose.
Pushing the tag starts the existing ecosystem-release workflow; it creates the GitHub Release.
Wait for that workflow's successful publication before verifying assets. Do not add a second
manual publisher or a new registry.

## Verify the actual published release

From the same tagged checkout with the local release build available:

```sh
published_dir="$(mktemp -d /tmp/behavior-0.13.0-published.XXXXXX)"
gh release download v0.13.0 -R kallistoteles/behavior-ir -D "$published_dir"
(cd "$published_dir" && sha256sum -c SHA256SUMS)
cmp "$published_dir/release-manifest.json" dist/v0.13.0/release-manifest.json
nix develop -c scripts/release-verify.sh v0.13.0 kallistoteles/behavior-ir
```

Expected: the actual wheel passes its checksum, canonical manifest equals the local tagged
build, and release-verify prints identical. Confirm manifest commit equals release_commit and
its release/Core/verifier fields identify 0.13.0 / exact 0.15.0 / 0.10.0. Confirm the published
release body includes the authored changes and generated exact Core identity. The current
release-verify script alone compares checksum files; it does not hash the downloaded wheel
or compare the published manifest.

Record the release URL, commit, workflow result and verification outcomes. Only then is Feature
509 complete. On any failure retain the failed-step evidence, preserve immutable tags/records,
and resolve the blocker through the existing release policy. Do not silently replace the
requested target with a different version.
