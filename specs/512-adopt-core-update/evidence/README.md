# Feature 512 execution record

Status: T001–T014 complete; current policy/Safety controls, Skills documentation and final
acceptance are in progress. Behavior 0.15.0 now pins published Core 0.16.0/verifier 0.11.0.
The full release check has not yet passed; this is not a publication or readiness claim.

## Checkout and environment

Feature branch: `512-adopt-core-update`, checkout `/home/kalle/repos/behavior-ir-512`, based on
published Behavior 0.14.0 commit `1cba9e16ff73f927ae08e70a7bc74a6ca52355e4`.
The existing dirty 510/511 checkout was not repurposed or overwritten. Its pre-existing modified
files retain their recorded hashes. A temporary unused plan-template copy created there during
setup was removed only after confirming exact template bytes. Active Spec Kit selection points
to this feature checkout. Existing target/cache/environment ignores cover all generated files;
`.core/0.15.0` links to the already fetched released assets in the original ignored cache.

Pinned tools are exposed by the existing local environment at
`/home/kalle/repos/behaviour-ir/target/safety-models/release-readiness/env.sh`.
The original developer venv reports Behavior 0.13.1 and is not the package baseline.
A retained local 0.14.0 candidate wheel also had a different checksum, so it was rejected for
published-baseline use. The actual published 0.14.0 wheel was downloaded and checksum-verified:
`794d2dd5ddf8613c52f524a1b4b74c0246d86a5ea52414424ab89ef104e86513`.
It is installed separately at `target/core-adoption/baseline`; installation/download are outside
runtime measurements. No Rust build or development dependency override was used.

[Baseline environment](baseline-environment.json) records the artifact, versions, source,
machine and protected file hashes. [Preparation identity](preparation.json) binds the new probes
and gate correction. Low disk space was handled by using the published wheel and existing
released assets; no unrelated cache or user work was removed.

## Executed preparation

| Task | Evidence and result |
|---|---|
| T001 | Isolated published-source worktree, active feature selection, ignore and unchanged-user-work checks above |
| T002 | [Exact published wheel/environment](baseline-environment.json), binding 0.14.0/Core 0.15.0/verifier 0.10.0 |
| T003 | [Complete semantic snapshot](baseline-semantic.json): 15 reference cases, 24 purchase-approval steps and 10 WorkQueue steps; original protected metadata/baseline hashes retained |
| T004 | [Six actual red support controls](removal-support-red.txt), [complete reports and runtime controls](baseline-removals.json) |
| T005 | [Nine growing-state observations](baseline-growth.json), [raw execution](baseline-growth.txt), [frozen profile](growth-profile.json); real 1000/1001 rule and unrelated update/replay pass |
| T006 | [Red metadata-only failure](baseline-gate-red.txt), [nine passing regression controls](baseline-gate-green.txt), [actual baseline gate](pinned-baseline-green.txt): 22 historical receipts unchanged |

The receipt gate now removes only the top-level `core_release` metadata when comparing.
It retains hashes, generated declarations, source maps, nested fields and receipt multiplicity.
The immutable original 507 baseline file was not edited.

All six removal fixtures have independent allowed commits/replays, and extra-referrer guards
refuse without advancing history. Four unsafe unguarded/new-dangling-Ref controls refuse and
remain unverified. The baseline verifier produces no successful required reference-world proof;
all six strict support assertions fail for that actual limitation, after report collection.
That is test-first support evidence, not candidate acceptance.

Two setup failures are retained separately: the clean installed environment does not contain
pytest ([setup log](baseline-gate-setup.txt)), and a module without a declared read initially
selected an older wire format ([profile setup](removal-profile-setup.txt)). Neither counts as a
red defect reproduction. The gate-only regressions ran under the existing pytest environment;
the public runtime/proof/growth baselines ran under the exact released 0.14.0 installation.
Adding an inert declared read supplies actual profile 0.7; an inert command supplies 0.8.

Baseline median transaction windows (eight complete evaluate/commit cycles) were 58.970ms at
population 64, 171.302ms at 256 and 677.440ms at 1024. Each point has three repetitions and eight
real creations. Setup/replay and metric serialization are separate; full public return conversion
is included in each transaction timing. Output sizes and total window time are retained.
These are observed baseline costs, not claims that the new Core has improved them.

The frozen candidate criteria require at least 15% lower median transaction cost at the largest
point and 10% lower largest-minus-smallest growth increment, with no more than 10% regression at
other points and all semantic controls passing. The operational ceiling is 60 seconds per
measured repetition. Profile/environment and baseline digests are recorded before candidate use;
a failure must not be hidden by changing thresholds after measuring a candidate.

## Release dependency: initial absence and subsequent publication

[Published release discovery](core-release-availability.json), checked 2026-10-27, lists Core
v0.15.0 as latest and no release containing 029/030. Merged PRs do not supply released assets.
Task T007 was initially incomplete. The separately authorized Core release has now
been published and verified; [selected release provenance](core-release.json) records
Core 0.16.0/verifier 0.11.0 at exact annotated-tag commit
`63a8c164523275e06ac1d7dc5330db4a899c9d99`. All five asset hashes match published metadata,
checksummed artifacts match the independent reproducible local build, and the downloaded
CLI proves all six mandatory reference cases. Git ancestry confirms both 029/030.
Task T007 is now complete. No unreleased branch or invented checksum was used. The following
adoption work uses that exact published release; historical fixtures remain unchanged.

Core publication was separately authorized and completed. Behavior publication remains a
separate explicit action after merge. Full-gate results are recorded below when executed.

## Remaining scope and findings

- Remaining at this checkpoint: T015–T021 evidence/documentation and final acceptance.
- Nonblocking: no new capability or optional improvement is included. Existing Lean toolchain
  discoverability feedback and unrelated `unique(Option)` work remain outside this feature.
- No new Core/runtime semantics, proof theorem, Safety policy or host-governance mechanism.

## Candidate adoption and consumer evidence

[Candidate build](candidate-build.txt), [exact installed identities](candidate-environment.json)
and [pin/version controls](version-and-pin-green.txt) establish Behavior 0.15.0 with the exact
published Core 0.16.0 CLI. The initial tested release wheel is
`a362ccf94abdf3f0009d71fafb44fe1331e0f1f5cea08ec1062fc463cceaf965`.
Its manifest records base HEAD, not the then-uncommitted working source; the environment record
states that distinction explicitly. Final acceptance will identify exact tested content separately.

[Public adoption tests](adoption-tests-green.txt) establish six positive removal/profile cases,
unsafe removals, complete additional incoming fields/classes, truthful solver exhaustion and
unsupported safe shapes. Actual guarded execution, history continuity and replay are independent
controls. Existing Store tests cover conflict/refusal, re-evaluation and replay. No binding
implementation change was needed: Rust Core remains authoritative.

[Candidate semantic snapshot](candidate-semantic.json) has [zero semantic differences](semantic-comparison.txt)
from the complete baseline. [Compatibility preservation](compat-preservation.json) independently
compares every prior manifest tuple byte-for-canonical-byte, including multiplicity: all nine
ordinary and nine Safety tuples survive. The existing writers append nine ordinary and three
Safety tuples for the new Core. Safety case differences are exclusively `inventory_hash`:
these bind the new dependency; Model/policy identities and BehaviorHashes remain equal.
The 22-receipt 507 historical baseline is unchanged apart from comparison-time normalization
of top-level Core release metadata.

### Growing-state result and limits

[Candidate measurements](candidate-growth.json) use the same frozen public probe and comparable
installed release wheel, CPU affinity, native backend and retained Store as the published baseline.
[Comparison](growth-comparison.json) passes every [predeclared criterion](growth-profile.json):

| Initial population | Baseline median / eight cycles | Candidate median / eight cycles | Candidate / baseline |
|---|---:|---:|---:|
| 64 | 58.970 ms | 40.949 ms | 0.6944 |
| 256 | 171.302 ms | 112.182 ms | 0.6549 |
| 1024 | 677.440 ms | 393.393 ms | 0.5807 |

The largest-minus-smallest growth increment ratio is 0.5699. All repetitions stay below the
frozen 60-second ceiling. Creation 1000 allows, creation 1001 denies without head advance;
unaffected update and both replay modes pass. Authored wire/hash and every observed record/bundle
output size match the baseline. Setup, evaluation, commit, whole-window conversion and replay
are recorded separately. This materially improves the original growing-creation workload while
preserving its genuinely affected global rule.

Residual costs remain: complete records/snapshots and public conversion are population-dependent.
No constant-time evaluate/commit claim is made. Unsupported global dependency shapes, changed
continuity, reopened Store instances and external-writer invalidation can require reference
validation. These fallback paths retain Core semantics; an entity-returning query naturally
costs at least its output size. Measurements cover finite Linux populations, not all globals,
backends or platforms. Conflicts still require re-evaluation; lost acknowledgements require
establishing whether the original commit happened before producing another invocation.

## Current evidence contracts and guidance

The baseline Safety bundle was produced with the checksummed published 0.14.0 package and
pinned Lean toolchain, not forged by changing a current bundle. Its
[production record](baseline-safety-certification.txt) and [unchanged archive](baseline-safety-bundle.json)
retain Core 0.15.0. The new package [rejects it before proof work](stale-core-safety.txt):
`Failed`, `SAFETY_INVALID_DATA: wrong Core release`, no certificate and `NotRun` obligations.

[Policy/cache/Safety run](current-policy-cache-safety.txt) passed 58 controls, including fresh
FieldOwnership and shared LifecycleDependency certification and independent verification.
Its last new control had an incorrect expected diagnostic code; the actual refusal already
correctly named the old Core. [The corrected isolated control](stale-core-safety.txt) passes.
The full gate reruns the final assertions. [Initial current-policy red](current-policy-red.txt)
reproduces the stale 0.10-policy selection; explicit released 0.11 policies now authorize the
correct context. Selecting an old execution policy under the new immutable evidence policy
refuses with `EXECUTION_POLICY_NOT_ALLOWED`, exit 2 and no output evidence. No old policy
fixture was rewritten. The initial expectation of exit 1 is retained separately as
[current-policy-exit-expectation.txt](current-policy-exit-expectation.txt).

Cache tests independently cover equal-body/differently named actions, incoming schema changes,
profile changes and cold/warm content identities. [29 focused Skills/verifier/Store tests](skills-verifier-store.txt)
pass, including the existing precision-debt examples. The three Skills and release guidance
now identify Behavior 0.15.0/Core 0.16.0/verifier 0.11.0; the new public removal example runs
from an empty consumer directory. Existing local-SMT/global-Safety/test/host boundaries and
LifecycleDependency v1 restrictions are preserved. Full clean-install/reproducibility results
remain pending the release-check record below.

Two additional current-contract checks were discovered during full-gate preparation:
[Core alignment](core-alignment-red.txt) still selected 0.15/0.10, and the
[Safety guard-report validator](safety-report-version-red.txt) rejected genuine 0.11 attestations.
[Eleven focused controls](current-contracts-green.txt) pass after adapting those live selections.
The aggregate report now also checks the complete installed Core identity and verifier identity;
mutation controls retain stale engine/verifier/commit rejection. Historical versioning text is
kept separate from these current selections. No theorem or Safety meaning changed.

The first full [release-check attempt](release-check-attempt-1.txt) stopped honestly in
`cargo test`: the successful Safety proof reached a test assertion still expecting Core 0.15.0.
That current Rust evidence assertion now expects 0.16.0. The full script is rerun rather than
bypassing the failed gate. [37 current policy/cache tests](current-policy-cache-green.txt) pass;
this includes a real old-verifier cache produced by the published baseline, a mandatory fresh
0.11 check with identical BehaviorHash, and subsequent current warm-cache hits. The
[old cache archive](baseline-verifier-cache.json) retains its original report and bytes.
The [current Core guard comparison](current-guard-core-results.json) confirms the six local
outcomes independently of Safety; the documented precision limits still hold.

## Complete conformance

[506 complete](conformance-506-complete.txt): 39 passed, zero blocked/failed/inconclusive/
unsupported/unverifiable. [507 complete](conformance-507-complete.txt): 61 passed plus all
12 obligations passed, likewise zero nonpassing outcomes. The [raw 506 report](conformance-506-complete.json),
[raw 507 report](conformance-507-complete.json) and [commands/exits](conformance-complete.json)
are retained before subsequent gate runs can overwrite target reports.

507 reports contract 5, SMT 7, structural 5 and test 13 evidence items, with assumptions named
by 9 of 12 obligations. Passing obligations are not relabeled universal SMT proofs.
The separate mandatory warm quick-mode 35-second budget remains active in the full gates.

The [second full attempt](release-check-attempt-2.txt) passed exhaustive correspondence:
38,006 attempted transitions, 30,218 successful commits, zero unsafe certified commits;
[the coverage artifact](safety-exhaustive-coverage.json) is byte-identical to the published
source's historical coverage record. It then exposed a stale cross-Core assumption in the Rust
manifest test: whole case equality incorrectly required dependency-bound inventory hashes to
stay equal between Core releases. The corrected test retains exact whole-case comparison within
one exact Core identity and compares all semantic fields across Core identities. Its
[positive and mutation controls](safety-identity-green.txt) reject Model/policy/behavior/generated
identity changes, same-Core inventory changes and missing hashes. No historical tuple changed.
The full required script is rerun with this correction; prior attempts are not called passing.

[An exact rebuild](source-rebuild.json) from the current packaged sources matches the measured
candidate wheel, manifest and SHA256SUMS byte-for-byte. [The source index](tested-source.json)
records actual working-tree file hashes independently of base HEAD. These are nonpublished
review artifacts; the normal build must be rerun from the approved merge commit for publication.

## Requirement and scenario evidence map

Final acceptance of rows involving the release check remains pending until that script exits 0.
The map separates the observations from the final release decision.

| Requirements | Evidence |
|---|---|
| FR-001, FR-002; SC-001; US1.1 | Published release/asset provenance, declared pin, candidate environment, pin/version controls and installed release steps 2–7 |
| FR-003; SC-002; US2.1 | Baseline six red cases and candidate six nonempty proven reference inventories with independent allowed commits/replay |
| FR-004; US2.2 | Public unsafe/extra-field controls, real solver exhaustion and unsupported-form controls in `current-policy-cache-green.txt`; full reports retained |
| FR-005; SC-004; US2.4 | Frozen growth profile, nine baseline/candidate observations, complete transaction/output accounting and passing comparison |
| FR-006; SC-005; US2.3 | Real 1000/1001 boundary, unchanged refused head, unrelated update, both replay modes and Core fallback limits above |
| FR-007, FR-008; SC-003; US3.1 | Zero-difference snapshot, all old tuples preserved, 22-receipt baseline, within-Core exact inventories and cross-Core semantic mutation controls |
| FR-009; US3.2 | Explicit verifier-0.11 governance, old-policy refusal, genuine old-cache miss under 0.11, action/schema/profile cache separation and existing archival tests |
| FR-010; SC-006; US3.3 | Fresh module/policy-bound Lean certificates and independent checks; authentic old-Core bundle rejected; full proof/integrity gate and installed package proof checks |
| FR-011 | No changes to Model/Safety meaning or theorem sources; unchanged native runtime source, Core-only replay and transparency tests; complete exhaustive coverage preserved |
| FR-012; US4.1 | Three versioned Skills, approval/removal examples, observed precision-debt matrix, atomic Store/retry/lost-ack guidance and installed example tests |
| FR-013 | Cargo/API/pin declarations, README, versioning and 0.15.0 notes; preserved historical documentation |
| FR-014; SC-007; US4.2 | Full release-check log, all existing gates, separately retained complete 506/507 reports and mandatory quick runtime limit |
| FR-015; US1.3 | Clean installed wheel, ordinary replay without Lean, separate installed consumer proof checking, exact rebuild and indexed working source |
| FR-016 | Honest failed attempts retained, remaining limitations/deferred findings recorded, no additional production integration or Core work |
| US1.2 | Existing graph/override/corrupt-asset/wrong-CLI controls, unchanged and required in the gates |

This covers all 16 FRs, seven SCs and 12 acceptance scenarios. A mapped but unpassed release
step is not acceptance; final outcome is recorded only after execution completes.

## Final metadata-only report correction and CI acceptance route

The [third local full release attempt](release-check-attempt-3.txt) passed 329 Rust tests,
749 Python/model tests (including slow package tests), all five Safety reference certificates,
the six-case Core/Safety comparison, full 506/507, the mandatory 507 quick budget (9.95/35s),
mypy, compatibility, determinism, ownership, terminology and workflow checks. Its final script
stage exposed one additional historical report comparison still requiring old release metadata.
Only Core version/commit and engine/verifier version differed; the published baseline is intact.

The corrected comparison first requires the current pinned Core/verifier context (regenerating
stale quick reports), retains canonical byte-format checks, then substitutes only those four
metadata leaves for historical comparison. [Two meaningful red cases](conformance-baseline-red.txt)
preceded the correction. [Thirteen controls](conformance-baseline-green.txt) now pass, including
actual result/Model/format/type/nested-field differences, wrong versions, missing fields,
noncanonical bytes and stale report regeneration. [All script self-tests](script-tests-green.txt)
pass after the correction. This does not grant archived verification evidence fresh authority.

Final acceptance now uses the repository's existing `ecosystem-ci` split: mandatory `gates`
and `package` jobs, the latter invoking `release-check.sh --skip-gates`. No workflow, gate or
proof requirement is changed. A draft PR runs these existing jobs on the exact corrected commit;
CI success and clean final-package checks remain pending. Local completed component results are
retained honestly rather than claiming the earlier orchestration exited zero.
