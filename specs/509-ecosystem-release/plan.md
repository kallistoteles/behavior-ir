# Implementation Plan: Ecosystem Release and Skills Update

**Branch**: `509-ecosystem-release` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: `specs/509-ecosystem-release/spec.md`; clarification completed with no critical ambiguities.

## Summary

Release the existing capabilities as Behavior 0.13.0, bundling the unchanged Core 0.15.0 pin
and verifier 0.10.0. Update current release documentation and the three existing consumer
Skills, add four compact essential examples, and use the existing gates and publishing path.
No Core/Model API, runtime semantics, dependency or distribution platform changes are planned.

Research confirmed that the workspace already declares 0.13.0. The required code change is
small: append committed release prose to the existing release.sh NOTES generation, preserving
its generated exact version/Core identity header. No new release process or example runner.

There are two delivery milestones: **ready for publication** after implementation/validation,
and **complete** only after merge, separate explicit publication authorization, publication and
verification. No release tag is pushed during implementation.

## Technical Context

**Language/Version**: Existing Rust 1.98.1 / edition 2024, Python >=3.13, Bash and Markdown.
Most changes are documentation and executable Python examples; one existing Bash release script
needs a small notes integration with a reproducing test first.

**Primary Dependencies**: Existing public behavior-engine pin at Core 0.15.0 commit
`6b93898a9375113f7a8946f23ab5535319b6f5d6`; verifier 0.10.0, Z3 4.16.0. Existing Nix,
maturin, Zig, pytest, mypy, auditwheel, git and gh tooling. No added dependency or service.

**Storage**: Versioned Markdown/examples and existing manifest/checksum files. Build output
stays in dist/ and target/. A short implementation log holds results/findings; no new schema,
database, evidence archive or issue-management framework.

**Testing**: Existing test_skills.py, public API/equivalence checks, release-script regressions,
scripts/gates.sh, complete conformance for 506 and 507, release-check.sh and release-verify.sh.
Hash actual downloaded wheel bytes and compare manifest content using existing standard tools.

**Target Platform**: Existing manylinux_2_28_x86_64 wheel, Python >=3.13; existing GitHub Release
distribution. Contributor toolchain through nix develop. No new platforms or registries.

**Project Type**: Existing library/package, CLI and consumer Skills release.

**Performance Goals**: Preserve existing budgets, including the 507 warm quick gate (35 s)
and release-check's clean-install/smoke interval (600 s). No new benchmark or optimization work.

**Constraints**: Exact Core pin; immutable historical evidence/tags; current public API only;
three established Skills; all existing required checks; complete 506/507; no external application
reviews or manual agent evaluation. Protect unrelated Spec Kit changes and uncommitted work.
Use a clean isolated committed checkout for release validation if the working tree is dirty.

**Scale/Scope**: Three SKILL.md files plus skills/README.md, README.md, docs/versioning.md,
one authored notes file, four self-contained examples and a small tested notes integration.
Existing examples are preserved unless an actual release mismatch requires correction.
All technical unknowns were resolved in [research.md](research.md).

## Constitution Check

*Design gate evaluated before research and re-evaluated after Phase 1. Both PASS. This is a
planning assessment; release execution gates have not been run by this planning command.*

| Rule | Design compliance before and after research |
|---|---|
| I. Deterministic Core | Existing Models lower to Core. Examples assert Core decisions; no host business-rule implementation or new AI execution. |
| II. Validated outputs | Guidance preserves untrusted caller inputs, trusted host context and Core admission/evaluation boundaries. No new parser or raw AI output path. |
| III. Test-first | Before changing notes generation, add and observe a focused failing regression in the existing release-script tests using a disposable repo. Author examples with assertions and validate with existing Skill checks. Any necessary code fix begins with a reproducer. |
| IV. Reproducibility | Preserve pinned source/toolchain and release-build/check/verify. Keep artifact formats and historical identities unchanged. Record the commit each validation covers. |
| V. Explicit auditability | Manifest, complete reports, release URL and checksum/manifest comparisons provide release evidence. Findings retain their actual classification; no secrets in logs. |
| VI. Simplicity | No new dependency, Model, example runner, publisher, platform or evaluation system. Four focused examples cover the required capabilities. |
| Bindings and Packaging | Core pin and asset digests unchanged; examples use public root exports. No repository helpers, engine internals, copied Core fixtures or hand-authored IR. |
| Technology constraints | Existing fmt/clippy/mypy/full gates remain required. No unsafe code, arithmetic change or new runtime code. |
| Workflow and quality | Existing release-check invokes the same gates.sh. Independent complete conformance remains required; full integration alone can tolerate blockers and is insufficient. |
| Governance | No constitutional amendment or historical-evidence rewrite. Optional work follows the explicit scope/approval rules. |

No unjustified violation or unresolved planning gate remains. If implementation discovers a
blocking problem that requires new semantics or broader scope, record it and obtain a scope
decision rather than expanding this plan automatically.

## Project Structure

### Documentation (this feature)

```text
specs/509-ecosystem-release/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── release-contract.md
│   └── skills-contract.md
└── checklists/requirements.md
```

Tasks are generated next by speckit-tasks; this planning command does not create tasks.md.
The implementation log is created during implementation, not as another planning subsystem.

### Source Code (repository root)

```text
README.md                                      # current release, capabilities, correct tag sequence
Cargo.toml, core-release.json                   # verify existing values; preserve pin and version source
api/public-api.json                            # existing reference, no API expansion
docs/versioning.md                             # current 0.13.0 policy rationale and exact Core/verifier
release/notes/0.13.0.md                         # authored change notes
scripts/release.sh                             # append those notes in the existing NOTES generation
scripts/release-{build,check,verify}.sh          # existing mechanisms, no redesign
.github/workflows/ecosystem-{ci,release}.yml    # existing orchestration, no new release workflow
skills/README.md
skills/behavior-authoring/{SKILL.md,examples/}  # models_composition.py, work_queue.py
skills/behavior-verification/{SKILL.md,examples/} # approval_verification.py
skills/behavior-application/{SKILL.md,examples/} # work_queue_store.py; reuse conflict.py
python/tests/test_skills.py                     # existing checks, not a new runner
python/tests/test_release_scripts.py            # focused notes regression
```

**Structure Decision**: Extend the current files/directories. The Skills contract maps the four
examples to essential requirements. No source changes under crates/ or new public facade are
needed. Historical specifications are context, not current release instructions to rewrite.

## Implementation Outline for Task Generation

Apply to **every** proposed task: “Is this necessary to release 0.13.0 correctly, or can it wait
until 0.13.1?” Keep only necessary release work; record optional suggestions separately.

1. **Record identity and scope.** Confirm 0.13.0 against the existing policy, preserve the 508
   pin and note the merged-source dependency. Start a short findings/results log. Do not redo
   Core adoption or regenerate historical semantic evidence.
2. **Refresh release content.** Correct README/versioning guidance, unsupported-feature claims
   and the obsolete tag sequence. Write release/notes/0.13.0.md covering changes since 0.12.0.
   Test the notes integration red first, then append that prose in existing release.sh.
3. **Update the three Skills in place.** Preserve sections, frontmatter and the gap rule.
   Cover Model choice/composition/ownership/generated entities; correct command authoring;
   explain verifier 0.10.0, actual evidence classes and remaining assumptions. Reuse existing
   conflict.py for both retry cases; keep precision-debt examples that remain inconclusive.
4. **Add essential executable coverage.** Use the four examples in contracts/skills-contract.md,
   or combine them if shorter without losing coverage. Keep them independently copyable and
   assert meaningful outcomes. No full application, shared example framework or extra API.
5. **Validate the candidate.** Run focused changed-file checks, then release-check.sh with its
   existing full gates and installed-package/Skill/reproducibility checks. Run independent
   complete 506 and 507 commands and record their reports/commit. Preserve 507 quick timing.
   Use existing failure reproduction tests when necessary; fix only release blockers.
6. **Finish implementation at ready for publication.** Review and merge the release changes
   through the existing process. Keep publication pending. A task list must not mark Feature
   509 complete merely because implementation tasks have finished.
7. **Separately authorized publication.** Select the merged clean source, verify successful
   required CI, revalidate the final candidate, annotate/check/build/push through existing
   scripts/workflow and verify actual published bytes/manifest and reproducibility.
   Record the resulting URL and evidence; only then mark the feature complete.

## Validation and Evidence

| Requirement group | Existing validation and acceptance |
|---|---|
| FR-001–FR-005 | Version-policy review, generated/installed version checks, current docs and authored notes; exact pin guard |
| FR-006–FR-012 | Existing test_skills.py plus installed-wheel copied examples, existing public API/equivalence checks; Skills contract coverage review |
| FR-013 | release-check.sh full gates; separate 506 complete and 507 complete with no non-passed dependency |
| FR-014–FR-016 | Existing release workflow/tag checks, repeat builds, release-verify; actual downloaded checksum and manifest comparison |
| FR-017–FR-020 | Short finding/suggestion table with impact and necessary/approval/deferred disposition; no mandatory failure waived |
| FR-021 | Published release URL and successful verification on the actual merged tagged source |

Release-check already invokes full gates. Do not create a second gate list, add broad new tests
or repeat unchanged successful checks gratuitously. Re-run affected checks after changes and
ensure final source is covered by the existing required checks. Failures name their step and
remain blocking where required. The plan and quickstart are instructions, not claims of release
success.

## Significant Suggestions and Scope Decisions

| Suggestion | Benefit | Disposition |
|---|---|---|
| Correct current docs' tag sequence and stale command/verifier claims | Prevent incorrect release operation or supported features being treated as gaps | Necessary for 0.13.0 |
| Append committed rich notes to existing generated NOTES | Publish the real changes from the merged source while retaining exact identity | Necessary; minimal existing-script change |
| Four compact public examples | Exercise essential coverage in existing package checks | Necessary; no external application |
| Check downloaded wheel digest and manifest with existing tools | Verify actual published assets, beyond comparing checksum text | Necessary publication validation; no new framework |
| Additional domain examples, exhaustive Model combinations or prose polish | Broader teaching coverage | Deferred to 0.13.1 or later |
| General release-verifier hardening, archive cleanup or new tooling | Potential future automation improvements | Deferred; standard current tools meet this release's checks |
| Extra platforms, registry publishing or an external agent evaluation campaign | Wider reach or additional feedback | Deferred; outside current release scope |

No optional addition currently requires approval. If one arises, record its benefit and explicit
approval requirement before adding it to the release work.

## Complexity Tracking

No constitution violations or extra architectural components are introduced. The small notes
integration is required release content, covered by existing-style regression tests; it is not
a new release process.
