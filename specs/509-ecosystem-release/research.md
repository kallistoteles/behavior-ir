# Research: Ecosystem Release and Skills Update

Research is limited to the repository's existing contracts, scripts and public authoring
surface. Two read-only research agents inspected release mechanics and consumer Skills.
No builds, release gates, tags or publication were performed during planning.

## R1. Release identity and dependency

**Decision:** Keep Behavior 0.13.0, Core 0.15.0 at commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`, verifier 0.10.0. Preserve `core-release.json`
including asset digests. No version bump is needed in the current Cargo workspace: it
already declares 0.13.0. Confirm all generated and human-authored references during implementation.

**Rationale:** `docs/versioning.md` requires a minor release for changed verifier outcomes or
consumer contracts. Since published 0.12.0, the ecosystem accumulated both, including the
Core 0.15.0 verifier improvement. Core and ecosystem versions identify different products.
Feature 508 is already in the working ancestry and must be included in the merged release.
Local tags contain 0.12.0 but no 0.13.0; remote tag availability is rechecked at publication.

**Alternatives considered:** Matching the ecosystem number to Core 0.15.0; treating the
release as documentation-only patch 0.12.1; revising the policy. None fits the requested release
or the existing policy. Historical records, attestations and release tags are unchanged.

**Sources:** `Cargo.toml`, `core-release.json`, `docs/versioning.md`,
`specs/508-core-release-0-15/spec.md` and its archived verification comparison.

## R2. Preparation and publication use one existing process

**Decision:** Prepare with `release-build.sh` and `release-check.sh` without a tag. Require
506 and 507 `complete` explicitly. After merge and explicit authorization, annotate the chosen
merged commit, check the tag, use `release.sh`, and push the tag to trigger the existing workflow.

**Rationale:** `release-check.sh` already runs full `gates.sh` with release tests enabled,
checks the installed wheel and all Skill examples, and performs reproducibility comparisons.
Full conformance permits registered blocked checks, whereas complete requires every dependency
passed. `release.sh` requires a clean tree and an annotated tag on HEAD; `check-tag.sh` requires
its commit on `origin/main` and, with a token, successful `pin surface gates package` checks.
The current README's claim that release.sh creates a tag is wrong and must be corrected.
Only a pushed `v*` tag triggers `.github/workflows/ecosystem-release.yml`.

**Alternatives considered:** A second release workflow, extra consumer application review,
manual agent evaluation, or treating a successful build as feature completion. All are excluded.
Do not copy obsolete signatures from historical specs into the current run guide.

**Sources:** `scripts/{release-build,release-check,release,check-tag,conformance,gates}.sh`,
`.github/workflows/{ecosystem-ci,ecosystem-release}.yml`, `README.md`.

## R3. Artifact shape, release notes and published verification

**Decision:** Retain the current Linux x86_64 manylinux 2.28 wheel, Python >=3.13,
SHA256SUMS and `behavior.release_manifest.v1`. Verification requires Z3 4.16.0; other
runtime operations do not. Add authored notes at `release/notes/0.13.0.md` and append them to
the generated version/Core header in the existing release.sh NOTES output. Add only the focused
regression needed for that small script change, before implementing it.

**Rationale:** Existing release.sh currently generates only identity/install notes. A committed
prose source lets the existing workflow publish the actual changes since 0.12.0 using its
unchanged `--notes-file` step. The build already uses the pinned Core asset, locked dependencies,
fixed source timestamp, path remapping and repeat-build comparisons. Consumer Skills are
obtained from the same repository tag, as documented in skills/README.md; adding Skill assets
to the wheel or introducing an installer is unnecessary.

For published verification, use release-verify.sh, then hash the downloaded wheel with
`sha256sum -c SHA256SUMS` and compare the downloaded manifest with the local tagged build.
The existing verifier compares published and rebuilt checksum files; by itself it does not
hash the downloaded wheel or compare the published manifest. These additional inspections use
standard existing tools and satisfy artifact acceptance without creating a new framework.

**Alternatives considered:** A release-note generator, hand-edited remote notes, new artifact
formats/platforms, or redesigning release-verify. Defer these; the existing workflow and ordinary
checksum/manifest inspection are sufficient. Existing historical release specifications stay intact.

**Sources:** `scripts/{release-build,release-check,release,release-verify}.sh`,
`pyproject.toml`, `skills/README.md`.

## R4. Keep the three Skills and their existing automated checks

**Decision:** Edit the existing three SKILL.md files and skills/README.md in place. Preserve
frontmatter, established sections, the verbatim semantic-gap rule and example citation format.
Add a compact set of self-contained public-API examples to the existing examples directories.

**Rationale:** Frontmatter already names release 0.13.0, but verifier guidance still says 0.9.0;
command authoring is incorrectly described as future work, and the four Models are missing from
consumer guidance. test_skills.py checks exactly three Skills, release matching, public names,
verbatim excerpts and execution without repository imports. release-check.sh automatically
copies and executes each example against the installed wheel from an empty directory.
No registration mechanism, new example runner or external application is required.

Keep callable conventions accurate: SM/RF transition fragments are positional (subject,
name-sorted state bindings, inputs, context); Approval has its established positional order;
WorkQueue takes a positional subject except enqueue and named remaining parameters. Do not
change or unify these APIs. Compose through `BehaviorModule(models=[...])`, generated entity
handles and derived values; bindings never grant authority over owned fields.

**Alternatives considered:** Reorganizing Skills, adding a fourth Skill, importing repository
reference applications into consumer examples, or building a shared example framework. These
add unnecessary scope and can break the copied-example check.

**Sources:** `skills/*/SKILL.md`, `skills/README.md`, `python/tests/test_skills.py`,
`api/public-api.json`, `python/behavior/{models,request_flow,approval,work_queue}.py`.

## R5. Evidence, retries and semantic gaps

**Decision:** Document verifier 0.10.0's resolved Approval query-congruence limitation without
claiming all query reasoning is solved. Keep other precision-debt entries unless actual
pinned outcomes show they are obsolete. Local SMT, structural/inductive arguments, tests and
host assumptions retain their different meanings; INCONCLUSIVE never counts as proof.

**Rationale:** The unchanged 506 Approval reference is established evidence of the improvement.
507 already records its local proofs, global arguments and trusted-time, backend, genesis and
migration assumptions. Existing application `examples/conflict.py` already demonstrates both
re-evaluation after conflict and original-bundle recovery after lost acknowledgement; reuse it.
A WorkQueue host example needs to demonstrate the new Model boundary and command intents,
not introduce a scheduler or guarantee physical execution/exactly-once external effects.

Update the current semantic-gap guidance for supported Model and command capabilities, while
preserving the rule and format for genuinely inexpressible requirements. Do not rewrite a
historical gap record merely because the current release adds a capability.

**Alternatives considered:** Deleting the entire precision-debt catalogue, treating every passed
VO as an SMT theorem, adding host business rules, or exercising an external agent application.
These would misrepresent evidence or broaden the release.

**Sources:** `models/examples/{purchase_approval,work_queue}/`, 508 archived evidence,
`skills/behavior-application/examples/conflict.py`, `python/behavior/store.py`.

## R6. Scope and findings

**Decision:** Keep findings and validation outcomes in a short implementation log, not a new
issue-management system. Significant suggestions list benefit and disposition: necessary,
explicit approval, or deferred. Task generation must apply the spec's 0.13.0-versus-0.13.1 test.

**Rationale:** Current-version accuracy, valid examples, rich release notes and verified artifacts
are release necessities. New features, extra platforms, general release hardening and polish
can wait. A failed existing mandatory gate remains blocking; it cannot be relabeled as polish.
Publication infrastructure failure leaves the feature incomplete. A defective immutable release
needs an explicitly decided replacement patch, never tag movement or silent target substitution.

**Alternatives considered:** Fixing every discovered defect or adopting a new tracking format.
Both would turn release preparation into an engineering project.

All planning unknowns are resolved by the existing repository contracts. No new dependency,
service, protocol, API or unresolved product decision is introduced.
