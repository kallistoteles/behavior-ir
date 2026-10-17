# Feature 509 implementation record

## Scope and status

Starting commit: `de06480` (Feature 508 Core adoption and its snapshot review fixes).
Preparation T001–T019 is authorized. Publication T020–T024 requires merged source and a
separate explicit request (FR-015); no product tag will be created during preparation.
Status: preparing; Feature 509 is not yet complete.

Preserved unrelated work: `.specify/init-options.json`, `.specify/integration.json`,
`.specify/integrations/speckit.manifest.json`, `.specify/integrations/codex.manifest.json`
and `.agents/`. These are excluded from release changes.

Every task is assessed against: “Is this necessary to release 0.13.0 correctly, or can
it wait until 0.13.1?” Only existing release capabilities and essential guidance are in scope.

## Release identity (T001–T002)

Cargo's existing workspace version is Behavior **0.13.0**; no duplicate version source or
version bump is needed. Relative to published 0.12.0, the existing 0.x minor policy applies:
complete snapshot contracts, record 0.8 and changed verifier outcomes require consumer
adaptation. Additive Models and Skills alone would not justify this bump.

Feature 508 (`ab75237`, ancestor of the starting commit) already adopted exact Core
**0.15.0**, commit `6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier **0.10.0**.
`core-release.json` and dependency metadata stay unchanged, including CLI SHA256
`89668836823d9be2999acb60864788aea4f09515772d292fa91ff87831f8891b` and conformance SHA256
`d1e50c2c6c87b7ac2a3d10b40101a048d24de989c55514fceeb1400edec5d8f7`.
The pin check and runtime identity will be recorded below. Historical compatibility fixtures,
records, receipts and attestations are not modified.

## Validation

Results below describe observed checks, not promised success. Candidate acceptance is pending.

## Findings and suggestions

| Issue | Concrete impact | Evidence/location | Classification | Disposition |
|---|---|---|---|---|
| Authored release notes absent from generated body | Consumers receive identity only, no adaptation guidance | scripts/release.sh NOTES generation | Blocking release documentation | T003–T008; necessary for this release; benefit: useful exact release notes |
| Stale current documentation and Skills | Misstates Core/verifier identity and supported Models/commands | README, docs/versioning.md, skills/ | Blocking release documentation | T009–T017; necessary for correct usage; benefit: executable public guidance |
| release-verify compares checksum text only | Alone it cannot validate actual downloaded bytes/manifest | scripts/release-verify.sh; quickstart verification section | Nonblocking with existing checksum/manifest verification commands | Deferred script improvement to 0.13.1; benefit: consolidate existing verification steps; no new process now |

Initial disk availability: 8.6 GiB. Build disk use is an environment concern; only verified
generated artifacts may be removed if needed. Source and uncommitted user work are preserved.

### T002–T008: identity and release content

`nix develop -c scripts/check-core-pin.sh`: OK, exact Core 0.15.0 commit.
Runtime `behavior.versions()` reports Python 0.13.0 / engine 0.15.0 / verifier 0.10.0.
The focused notes regression failed first at the missing authored suffix (1 failed, 4 deselected);
generated exact identity already passed. The fix appends one versioned prose file in the existing
NOTES generation. README and versioning now agree with the pin and distinguish historical
policy from current capabilities. No compatibility fixture or Core dependency changed.

The notes regression is green: 1 passed, 4 deselected (Nix, BEHAVIOR_RELEASE_TESTS=1).
T009–T016 add four standalone examples and preserve all existing Skill sections, the exact
semantic-gap rule and seven gap-log fields. Verification distinguishes evidence classes and
retains unrelated precision-debt examples. Required example/API checks are running.

Initial example execution exposed authoring mistakes in the new examples (all nine WorkQueue
capabilities are mandatory; ModelBuildError exposes `.errors`). Corrected the examples, with
administrative queue operations explicitly denied for this minimal host. No API or Model change
was needed. The existing Skill-reference checker interprets document format names as Python
references, so the prose excerpt omits the stream-request construction; the full executable
example retains and exercises the public request document. No checker was weakened.

Standalone Approval postcondition example and WorkQueue Store lifecycle both pass on the pin.
The post-edit Skills-only check passes: 23 tests. Public API/equivalence combined run follows.

### T017: focused acceptance

`nix develop -c bash -c 'scripts/fetch-core.sh >/dev/null && maturin develop -q'` built and
installed the development package against the exact pin. After correcting the new examples,
`nix develop -c python -m pytest -q python/tests/test_skills.py python/tests/test_public_api.py
python/tests/test_binding_equivalence.py`: 49 passed. All standalone examples, release fields,
root-public references and verbatim excerpts pass; existing precision-debt outcomes remain
as asserted. T001–T017 complete; candidate acceptance T018–T019 pending.
