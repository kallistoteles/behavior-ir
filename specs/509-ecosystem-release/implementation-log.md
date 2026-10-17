# Feature 509 implementation record

## Scope and status

Starting commit: `de06480` (Feature 508 Core adoption and its snapshot review fixes).
Preparation T001–T019 and publication T020–T024 are complete. The user explicitly authorized
publication after merge, as recorded below (FR-015).
Status: **published and verified — Feature 509 complete**.

Release: https://github.com/kallistoteles/behavior-ir/releases/tag/v0.13.0
Tagged merged commit: `63f916cbd9d999270f476e33267200e0aacd8696`.
Published at: `2026-10-17T17:42:34Z`.
The chronological sections below retain intermediate preparation/publication states as history.

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

Results below describe observed checks. Preparation, merged-source validation, publication
and actual downloaded-artifact verification are complete; see the final acceptance record.

## Findings and suggestions

| Issue | Concrete impact | Evidence/location | Classification | Disposition |
|---|---|---|---|---|
| Authored release notes absent from generated body | Consumers receive identity only, no adaptation guidance | scripts/release.sh NOTES generation | Blocking release documentation | Resolved by T003–T008; necessary for this release; benefit: useful exact release notes |
| Stale current documentation and Skills | Misstates Core/verifier identity and supported Models/commands | README, docs/versioning.md, skills/ | Blocking release documentation | Resolved by T009–T017; necessary for correct usage; benefit: executable public guidance |
| release-verify compares checksum text only | Alone it cannot validate actual downloaded bytes/manifest | scripts/release-verify.sh; quickstart verification section | Nonblocking with existing checksum/manifest verification commands | Deferred script improvement to 0.13.1; benefit: consolidate existing verification steps; no new process now |
| Linker optimization-setting deprecation | Diagnostic noise; artifacts pass compliance, install and reproducibility | Local release/rebuild logs | Nonblocking | Deferred to 0.13.1; benefit: clearer toolchain diagnostics |
| Pinned Actions target deprecated Node 20 | GitHub reports deprecation and runs the actions on Node 24; workflow succeeds | Workflow 39053833380 annotation and archived log | Nonblocking | Action-pin review deferred to 0.13.1; benefit: supported runner compatibility |

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

### T018: isolated committed candidate (in progress)

Candidate: `1be17ac` (the commit contains the release content, examples and planning artifacts).
Clean detached worktree: `/tmp/behavior-509-check.zrc3Xv`. Validation logs are under
`target/validation-509/` there; final conformance reports will remain under `target/conformance/`.
The initial `release-build.sh 0.13.0 dist/v0.13.0` is running. Subsequent checks use the
isolated checkout's own Nix environment and leave the maintainer worktree's user files alone.
No required gate, profile, runtime budget, pin or historical fixture has been changed.

Initial release build passed (optimized profile 1m22s); wheel has the required manylinux_2_28
CPython 3.13 abi3 tag. The linker emitted its existing deprecated optimization-setting warning;
it did not prevent the build. Full `scripts/release-check.sh dist/v0.13.0` is now running
in the isolated checkout's Nix environment, including gates and reproducibility.

All Rust tests, fmt and clippy passed. After compilation, free disk reached 2.4 GB. With no
active compiler using this checkout, removed only its generated `target/debug/incremental`
cache (809 MB); kept binaries, source, reports and every user file. This is environment cleanup,
not a gate/profile/budget change. The Python/model gate is running.

### T018: full release check passed

`nix develop -c scripts/release-check.sh dist/v0.13.0`: exit 0, release-check OK.
It ran all required gates: Rust fmt/clippy/tests (full properties), Python/model tests
**582 passed**, 506 full **39 passed**, 507 quick **56 passed / 5 unsupported**,
507 full **61 passed / all 12 obligations passed**, mypy, Model compatibility, determinism,
ownership, terms, workflow and script checks. Quick runtime: **32.12s / existing 35s**.
The budget and gates are unchanged. Baseline self-test: **22 receipts unchanged, 0 new**;
semantic-snapshot and compatibility self-tests also passed.

507 evidence summary: contract 5, SMT 7, structural 5, test 13; assumptions named by 9/12
obligations. Passing all obligations does not turn this mixed evidence into universal SMT proof.

Package acceptance passed: manylinux compliance and exact bundled CLI, clean no-Rust install,
smoke and missing-solver handling, installed versions equal manifest, repository/installed output
identity, all copied standalone Skill examples, **49 drift/equivalence tests**, and two repeat
builds matching the original wheel. Wheel SHA256:
`43184ecdec3348b696b00573511d3c0d8f912bb6970a5284a840d2b117b767fe`.
The separate `506 complete` then `507 complete` runs are in progress. No product tag exists
as a result of this work; publication remains pending.

### T018–T019: independent complete runs and acceptance

Both commands exited 0 on candidate `1be17ac0601a331f432f92e3985495285ea85449`:

| Command (inside isolated Nix checkout) | Observed result |
|---|---|
| `scripts/conformance.sh 506 complete` | 39 passed (43.40s), complete true, all other statuses zero |
| `scripts/conformance.sh 507 complete` | 61 passed (125.51s), 12/12 obligations passed, complete true, all other statuses zero |

The JSON reports use the full test profile; the separate `complete` CLI gate strictly required
all checks and obligations to pass. C19 attests unchanged module
`sha256:a04ba37ab89e051e448abb760bd754bbb0955125fd22ba8da927b8529a531779`; both Approval
uniqueness postconditions are proven. Core #35 is resolved in the pinned Core 0.15.0, so 506
is complete too. 507 remains independently complete. The 12 WorkQueue obligations retain
all four evidence classes and explicit assumptions (9/12); no structural/test/contract evidence
has been relabeled as SMT proof.

Reviewed the release and Skills contracts against the candidate and observed evidence:
versions/pin agree; four Models, composition, ownership/generated entities and commands have
public executable coverage; callable conventions match the binding; time, imported genesis,
migration continuity, atomicity, replay and original-bundle recovery remain explicit.
StateMachine/RequestFlow versions 2 and 3 are covered by the unchanged compatibility manifest.
No Core or Model implementation, public API manifest, established fixture, receipt or historical
attestation was modified. Required gates, profiles and runtime budgets were retained.

Evidence retained in the maintainer worktree (generated, ignored):

- `target/validation-509/1be17ac/logs/`: build, full release check and both complete-run logs.
- `target/validation-509/1be17ac/dist/`: verified wheel, SHA256SUMS and release-manifest.json.
- `target/conformance/509-1be17ac/`: both final reports and Rust/Python supporting evidence.

The manifest commit equals the candidate, its Core identity equals core-release.json and
installed versions match the manifest. The retained wheel bytes were hashed again against
SHA256SUMS. Both reports have complete true and every required entry passed. The candidate
checkout stayed clean. Product files in the maintainer worktree equal the tested candidate;
only this implementation record and task checkboxes are updated after validation. The final
merged publication source must be revalidated under T020, as already required.

After preserving evidence, removed only the temporary validation checkout's generated
`target/debug`, `target/release-build` and `target/tmp` compilation caches. Source, wheel and
reports remain available. Free disk is **7.6 GB**; no unrelated user data was removed.
Checksums confirm all previously recorded unrelated `.specify/` and `.agents/` files are intact.

Additional nonblocking finding: the release linker warns that optimization setting `1` is
deprecated. Artifacts pass compliance, clean install and reproducibility. Toolchain-warning
cleanup is **deferred to 0.13.1** (benefit: less diagnostic noise); no flags or dependencies
were changed to suppress it. The release-verify helper consolidation remains deferred to
0.13.1; the existing run guide explicitly requires actual downloaded-wheel hashing and manifest
comparison, so its present limitation is covered at publication.

**Preparation outcome: ready for publication.** T001–T019 are complete with no unresolved
release blocker. The existing review/merge process can proceed with the scoped release commit.
No release tag was created or pushed, no merge or publication was performed. T020–T024 remain
unchecked: FR-015 and the task execution boundary require merge plus a later explicit
publication request. Only actual published-artifact verification will complete Feature 509.

## Publication authorization and selected source (T020 in progress)

The user explicitly requested **“publish”** after confirming PR #27 was merged. Selected merged
source: `63f916cbd9d999270f476e33267200e0aacd8696` on origin/main. It includes Feature 508 and
both Feature 509 preparation commits; its tree equals preparation head `ff6b674`.
Refreshed origin/main and tags; v0.13.0 is unused locally/remotely and latest published release
is v0.12.0. Required merged-source CI checks pin/surface passed; gates/package are running.
No tag will be created until required CI and complete-conformance revalidation pass.
Use a separate clean selected-source checkout; keep this record and task updates in the
maintainer worktree, outside the tagged source. Unrelated user files remain intact.

### T020: merged source accepted for publication

Merged-source CI run `39051411412` passed all required checks: pin, surface, gates and package.
CI exercised the existing full gate and installed-package/reproducibility checks on exact
commit `63f916cbd9d999270f476e33267200e0aacd8696`. Independent local merged-source runs passed:
506 complete **39 checks** (43.89s); 507 complete **61 checks / 12 obligations** (127.82s),
all other statuses zero. Complete reports preserved in
`target/conformance/509-63f916c-before-tag/`. This revalidates the selected source rather than
assuming the pre-merge candidate's evidence applies to its new commit identity.

The canonical `release.sh` invocation now performs the local full release-check again, with
release-script regressions enabled, on the tagged merged source before any tag push. This
reuses required CI and the existing release process; no gate or runtime budget is waived.

### T021: tagged local release passed

Created annotated `v0.13.0` on selected merged commit `63f916cbd9d999270f476e33267200e0aacd8696`;
annotation object `9cce4240c7ee743bf059d529dd61dc7b2175ff30`. Authenticated `check-tag.sh` passed all four required CI
checks plus version and main ancestry. Canonical `scripts/release.sh 0.13.0` exited 0:
full gates OK, **582 Python/model tests**, 506 full 39, 507 full 61 / 12 obligations,
507 quick **31.59s / 35s**, all installed Skill examples and **49 API/equivalence tests**,
clean install, bundled CLI/version identity and identical repeat builds.

Current tagged-source wheel SHA256:
`4f9944347e6c88cceea9fb93fb88f8069365c4b231fa70cc5930f3d0200e05ae`.
Generated NOTES contains the exact release/Core header and all authored release prose.
Manifest commit equals the selected source; wheel bytes were hashed again. Clean checkout
preserved. Copies of local artifacts/logs are under `target/publication-013/63f916c/`.
T022 next: push only the immutable tag and wait for the existing ecosystem-release workflow.

### T022: immutable tag pushed; workflow in progress

Pushed only `refs/tags/v0.13.0`. Remote annotation is
`9cce4240c7ee743bf059d529dd61dc7b2175ff30`, peeled commit
`63f916cbd9d999270f476e33267200e0aacd8696`, matching the selected source.
Existing ecosystem-release workflow run:
https://github.com/kallistoteles/behavior-ir/actions/runs/39053833380
(event push, head v0.13.0, selected source). Waiting for its check/build/publication sequence.
After the completed local release check, removed only this temporary checkout's compilation
caches. Source, local wheel, NOTES, logs and conformance evidence remain; free disk restored
to about 7 GB. Actual publication and downloaded-artifact verification are still pending.

### T022: publication succeeded

Existing workflow `39053833380` completed successfully on selected commit
`63f916cbd9d999270f476e33267200e0aacd8696`. Check/build and Publish both succeeded.
Release: https://github.com/kallistoteles/behavior-ir/releases/tag/v0.13.0
Published at: `2026-10-17T17:42:34Z`. Release is public, not draft and not prerelease.

### T023: downloaded assets passed; independent rebuild in progress

Downloaded all actual release assets to the selected checkout's
`target/publication-013/downloaded/`. `sha256sum -c SHA256SUMS` passed on the downloaded wheel;
`cmp` confirmed the downloaded canonical manifest equals the local tagged build byte for byte.
The wheel also matches the manifest digest and local wheel bytes:
`4f9944347e6c88cceea9fb93fb88f8069365c4b231fa70cc5930f3d0200e05ae`.
Tag, manifest and selected merged commit agree; manifest identifies Behavior 0.13.0,
Core 0.15.0 exact pin and verifier 0.10.0. Published body contains the generated exact
identity and the entire authored notes source. `scripts/release-verify.sh` is now independently
rebuilding from the tag in its own temporary checkout.

Additional nonblocking workflow annotation: pinned checkout/Nix-installer actions target Node
20 and GitHub ran them on Node 24. The workflow succeeded. Reviewing those action pins is
**deferred to 0.13.1** (benefit: supported runner compatibility and fewer deprecation notices).
No release-process change is needed for the verified current artifacts.

### T023–T024: published verification and completion

`nix develop -c scripts/release-verify.sh v0.13.0 kallistoteles/behavior-ir` exited 0 and
printed **release-verify: identical (v0.13.0)**. It downloaded the published release and built
from the immutable tag in a fresh temporary checkout (optimized build 1m16s). Combined with
the actual downloaded-wheel hashing, byte-identical manifest/local-wheel checks and published
body inspection above, this satisfies the complete artifact verification contract.

Final identities:

| Item | Verified value |
|---|---|
| Release | Behavior 0.13.0, https://github.com/kallistoteles/behavior-ir/releases/tag/v0.13.0 |
| Published | 2026-10-17T17:42:34Z |
| Annotated tag object | `9cce4240c7ee743bf059d529dd61dc7b2175ff30` |
| Tagged / manifest / selected merged source | `63f916cbd9d999270f476e33267200e0aacd8696` |
| Core | 0.15.0, commit `6b93898a9375113f7a8946f23ab5535319b6f5d6` |
| Verifier / solver | 0.10.0 / Z3 4.16.0 |
| Downloaded wheel SHA256 | `4f9944347e6c88cceea9fb93fb88f8069365c4b231fa70cc5930f3d0200e05ae` |
| Publication workflow | [39053833380 — success](https://github.com/kallistoteles/behavior-ir/actions/runs/39053833380) |

A final remote-ref check confirms the annotation and peeled commit remain unchanged. The clean
selected-source checkout is intact; no historical release or tag was replaced. Source and
complete conformance evidence match the selected merged commit. All three consumer Skills and
essential public examples passed both required gates and installed-wheel checks.

Maintainer evidence archive: `target/publication-013/63f916c/` contains local artifacts plus
`evidence/` with the actual downloaded wheel/checksums/manifest, published release metadata,
workflow metadata and complete logs, independent rebuild log and verified remote refs.
Independent complete-conformance reports remain under `target/conformance/509-63f916c-before-tag/`.
The archive uses existing artifacts and logs, not a new release or evidence format.

Every remaining nonblocking finding has a 0.13.1 disposition above. No required check failed,
no gate/profile/budget was weakened and no new Core or Model semantics were introduced.
All originally recorded unrelated user files remain byte-identical. Only generated caches in
agent-created temporary checkouts were removed; free disk remains above 7 GB.

**Feature 509 is complete.** T001–T024 are marked complete from observed results. The published
tag/source remains immutable; this post-publication record is maintained outside that checkout.
