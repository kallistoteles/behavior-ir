# Upstream feedback drafts — Behavior 0.13.1

**Status: for review only.** No issue filing, feature approval, implementation or release change
is authorized by this document. No earlier feedback draft was found in this workspace; this
consolidates the supplied feedback against the published release. Examples marked **unexecuted**
are investigation inputs, not reproduced defects. No application-specific code is included.

Baseline: Behavior **0.13.1**, commit `21ba441299998ca0c6baf53a5178d1b591216b1f`;
Core **0.15.0**, commit `6b93898a9375113f7a8946f23ab5535319b6f5d6`;
Core verifier **0.10.0**, Safety checker **2**, policy semantics **1**, Lean **4.30.0**.
The [published manifest][manifest] identifies the release and Core; the separate
[proof-toolchain definition][tools] supplies Lean identities.

## Decisions

| Draft | Disposition | Proposed layer |
| --- | --- | --- |
| D1 — Proof-toolchain release metadata | Confirmed gap; propose for the next release | Release contract / documentation |
| D2 — Certification cost | Document observations; obtain controlled measurements | Documentation / measurement |
| R1 — Certification cache | Research only; do not implement | Model certification tooling |
| D3 — Optional-value uniqueness | Clarify existing semantics; no semantic change | Documentation |
| C1 — Parent diagnostics | Resolved; retain positive feedback | No new work |
| R2 — Approved modules | Evaluate existing Approval + trusted authorization first | Host / existing governance |
| R3 — Relational properties | Establish an unmet mathematical property first | Model research; Core only if necessary |
| R4 — Composite uniqueness | Formalize the use case and assess existing alternatives | Modeling research |
| R5 — Mutual references | Reproduce the exact lifecycle obstacle first | Modeling / documentation research |
| R6 — Same-type binding and creation | Minimal public-Core reproduction required | Core verifier investigation |
| R7 — Equivalent guards | Separate documented incompleteness from unsoundness | Verifier / Safety / documentation |

All open drafts below remain proposals. Selecting a next-release version requires the existing
versioning policy; these items do not automatically become separate features. Published 0.13.1
tags, manifests, wheels, proof records and attestations remain unchanged.

## D1 — Include the locked proof toolchain in future release manifests

- **Property:** a consumer can identify the exact supported proof environment from the release
  contract. Its Lean version, lock identity and per-platform derivations agree with the
  authoritative release sources. This is a provenance/discoverability property, not a theorem.
- **Minimal reproduction:** inspect the published `release-manifest.json`: it contains `core`,
  `versions` and `solver`, but no Lean/proof-toolchain identity. Compare it with the released
  [toolchain JSON][tools], [flake lock][lock] and [flake outputs][flake].
- **Expected / actual:** expected discoverable proof-tool requirements alongside the package;
  actual release metadata omits them. Individual Safety assessments already record
  `proof.toolchain`; this is not evidence that the certificates lack toolchain binding.
- **Affected / impact:** Behavior 0.13.1 consumers preparing certification must consult source
  metadata separately. Ordinary Core execution and replay do not require Lean.
- **Workaround:** use the existing public flake output, rather than searching nixpkgs manually:

  ```sh
  nix build --no-link --print-out-paths \
    'github:kallistoteles/behavior-ir/v0.13.1#safety-proof-toolchain'
  ```

  The explicit output is `packages.<system>.safety-proof-toolchain`; the released platforms are
  `x86_64-linux` and `aarch64-linux`. Pass the resulting store path as `toolchain=` to the public
  `certify_safety` / `verify_safety_evidence` API. These commands are source-checked guidance,
  not a new installation measurement.
- **Proposal / layer:** extend a future release contract and its documentation with Lean version,
  nixpkgs revision, flake-lock digest and per-platform `drvPath` / `outPath`. Derive values from
  `flake.lock` and `crates/behavior-models/proofs/toolchain.json`, and check their agreement with
  the actual derivations. Avoid an independently maintained version list. Select any schema/version
  change under the existing policy; do not retrofit the published manifest.
- **Current authority:** nixpkgs revision `6774f7bc253789b113a4f39285dc0fa100abeacc`;
  flake-lock SHA-256 `ed16bd3f98b17e19e07d6ecafecdd20897024a170f7f3727ed6565c582a01bbe`.
  Per-platform derivations are already present in [the released toolchain definition][tools].

Sources: [manifest][manifest], [manifest generator][build], [tool selection and proof identity][certifier].

## D2 — Document certification cost without inventing a cold/warm comparison

- **Property:** a timing claim identifies the exact module, complete policy set, proof/checker/
  toolchain identities, obligations, platform, resource limits and cache state. Performance must
  not change what successful certification establishes.
- **Minimal reproduction:** the released [installed-package smoke][smoke] certifies the one-field
  `SafetyNote` ownership policy and independently re-verifies its transported bundle. Each call
  performs fresh proof checking. Its module is
  `sha256:d52b8efe792b2f2dbf39356bfa1af4358c2f16c7e8e2222493a3c6166644fb5c`; its policy set is
  `sha256:f33de7bcda8d25a142475dd97c404d221745622a9491146f8d640ecc3550687e`.
- **Expected / actual:** expected documented, reproducible cold and warm costs; actual records
  provide the observations below, but not a controlled cold/warm experiment. No new benchmark
  was run for this triage.

| Observation | Measured scope | Result | Limit on interpretation |
| --- | --- | --- | --- |
| Pre-release local installed-wheel certification | Sum of 11 recorded subprocess phases, including audits and fresh replay | 133.795s; fresh replay 113.996s | Not end-to-end wall time; initial cache temperature and CPU model unrecorded |
| Immediate local bundle re-verification | Same phase sum, regenerated proofs and fresh replay again | 132.187s; fresh replay 112.946s | A repeated check, not a cache hit or controlled warm benchmark |
| Published-release CI installed-package step | Whole step 10a: ordinary smoke, certification and bundle re-verification | 118.832s, 2026-10-19 17:23:21.042–17:25:19.874 UTC | Combined wall time; per-phase CI resource records unavailable in the published assets |

Local platform: `x86_64-linux`, pinned Lean 4.30.0. CI: Ubuntu 24.04.5, image
`20261012.334.1`, x86-64 wheel build. Do not infer an exact CPU, CPU quota, load or cache state
from the runner label. Local maximum sampled subprocess RSS was **1,530,896 KiB** in each pass;
fresh replay sampled **539,808 / 542,072 KiB**. Sampling is every 20ms, not exact peak accounting;
it does not measure the whole process tree or CPU time. CI RSS/CPU measurements are missing.

The local limits were 200,000 Lean heartbeats, 4,096 MiB Lean heap, 8,192 MiB process virtual
address space and **180s per subprocess**, with elaboration `-j1` and bounded reduction depth.
All six recorded obligations passed: inventory comparison, generic proof, concrete proof,
expected theorem types, transitive axiom audit and fresh replay. There were twelve audited roots,
including the generic FO/LD theorems, checker/frame/membership results and `Concrete.checked` /
`Concrete.certified`. The FO example does not claim an instantiated lifecycle property.

- **Affected / impact:** Behavior 0.13.1 authors may underestimate build-time proof cost. The
  observed local/release difference remains unexplained; these differing scopes are not a
  performance regression measurement. Do not attribute it to caching, hardware or compiler
  optimization without evidence.
- **Workaround:** keep certification as an explicit build-time operation; retain its logs and
  `*.resources.json`. Ordinary Store operation and replay remain independent of proof tools.
- **Proposal / layer:** document the observations and measure a matched pair using the exact
  published wheel, same module/policies/toolchain and same host. Define **cold** precisely
  (for example fresh worker with a declared artifact/page-cache state) and **warm** as repetition
  on that host into a fresh output directory; both must still execute `leanchecker --fresh`.
  Record end-to-end monotonic wall time separately from every phase, CPU time, sampled versus
  true peak RSS, CPU/memory constraints, attempts and failures, all identities and obligations.
  Toolchain download/build time is a separate metric. Do not clear shared host caches to stage
  the experiment. Publish a redacted measurement bundle before filing a performance claim.

Evidence: [public release run][release-run] and [smoke source][smoke]. Local review records are
`target/safety-models/release-readiness/installed-safety-evidence/{assessment.json,proof-*/}` and
`verified/{assessment.json,proof-*/}`; they are **not published artifacts**. The existing runner's
[resource accounting and fresh-check call][certifier] define their scope. Earlier checker-1
milestones and whole test-suite times are not interchangeable with this checker-2 module timing.

## R1 — Investigate proof-artifact reuse only after the measurement contract

- **Property:** a reused artifact must establish the same exact theorem and concrete premises for
  the same admitted module and complete policies, under an explicitly reviewed trust contract.
  Content identity alone never establishes mathematical validity.
- **Minimal case:** repeat D2's exact `SafetyNote` certification in a new output directory.
  Source inspection shows each invocation regenerates proofs and runs fresh kernel checking;
  no cache-speedup result was measured here.
- **Expected / actual:** current expected and actual behavior includes `leanchecker --fresh`.
  Skipping it would change the certification contract, not merely accelerate a build.
- **Affected / impact:** Behavior 0.13.1 repeated certification can be expensive; no cache defect
  or semantic defect is established.
- **Workaround:** existing ordinary build caches may be reused without treating them as proof
  authority; retain all mandatory fresh verification. A transported bundle remains unverified
  until the public verifier succeeds.
- **Proposal / layer:** research only, after D2. Analyze artifact reuse separately from trusting
  a cached success result. Any design needs exact BehaviorHash, policy set, inventory, theorem
  statements, proof sources, checker, toolchain and dependency identities, corruption/stale-data
  refusal and explicit kernel-recheck semantics. Those keys are necessary, not sufficient. No
  cache implementation, skipped audit or new certificate mode is approved.

Sources: [certifier and bundle verifier][certifier], [public certification API][safety-api].

## D3 — Explain mathematical uniqueness over optional values

- **Property:** `Unique(Q, f) iff forall x != y in Q, f(x) != f(y)`.
  Under ordinary option equality, `None = None`.
- **Minimal example:** two records with distinct IDs and both optional keys absent violate
  `unique(Q, by=key)`; filtering both records out yields an empty query, which is unique.
- **Expected / actual:** Core 0.15.0 compares evaluated key values, including absent values.
  Two absent keys collide. This is consistent with the mathematical contract, not an established
  bug or an implicit request for SQL NULL semantics.
- **Affected / impact:** Behavior 0.13.1 / Core 0.15.0 applications that intend uniqueness only
  among present keys must state that predicate.
- **Workaround / proposed documentation:** show the public API pattern below in the context of
  an existing entity `Item` with an optional `key`. This is a source-checked authoring fragment,
  not a newly executed test:

  ```python
  import behavior as b

  present = b.select(Item).where(lambda item: item.key.is_some())
  present_keys_are_unique = b.unique(present, by=lambda item: item.key)
  ```

- **Layer / disposition:** documentation clarification. A different optional-value equality or
  uniqueness rule requires a separate semantic argument and compatibility decision.

Pinned evaluator/API references and equality scope are in the [Core evidence appendix][core-evidence].

## C1 — Retain positive diagnostics feedback; close the old combined-message proposal

The three parent prerequisites now have separate structured diagnostics, including deterministic
precedence and action/dependency context. The published authoring contract also requires parent
checks for nonterminal child transitions even when the owner is unchanged. Record that this
addresses the reported authoring problem. No new diagnostic feature is proposed.

Sources: [published lifecycle checker][lifecycle], [authoring contract][authoring],
[diagnostic regression controls][diagnostics].

## R2 — Evaluate Approval plus trusted authorization before proposing a module allowlist

- **Property:** for a deployment segment with approved module/policy set A, every successful
  commit uses an authorized `(BehaviorHash, complete safety-policy-set identity)` in A, starting
  at a valid exact-history activation anchor and preserving continuity across switches/migrations.
- **Minimal case:** admit two same-schema modules; A has a protected-state restriction and B
  lacks it. Evaluate whether a candidate under B can reach each direct/intent commit entry point
  without a trusted authorization approving B. Also try changing an approval after issuance.
  This integration reproduction is **not yet supplied or executed**.
- **Expected / actual:** authorization is bound to a particular BehaviorHash, but this is not
  itself a persistent Store-wide approved-module allowlist. An application Approval decision is
  ordinary domain state; it does not automatically become trusted commit authorization.
- **Affected / impact:** conditional Safety deployment under Behavior 0.13.1 / Core 0.15.0;
  an unmediated commit path can bypass a host's intended module restrictions.
- **Workaround:** existing coordinated host protocol, with one enforced boundary for all writers,
  incoming-state validation and trusted authorization; do not claim an in-process example lock
  constrains other backend handles.
- **Proposal / layer:** demonstrate Approval -> trusted authorization -> actual Store refusal
  for unapproved modules, including invalidation and all commit/migration paths. Identify which
  policy facts the trusted issuer checks and how revocation works. Only a demonstrated gap
  justifies a Core extension; no new Core field is approved now.

Sources: [governance and authorization analysis][core-evidence], [reference host][deployment].

## R3 — Establish a missing relational property before proposing general relations

- **Property candidate:** `forall c, active(c) -> exists p, c.parent = p.id and allowed(p)`;
  a separate stronger candidate is scoped cardinality
  `forall p, count({c | c.parent = p.id and active(c)}) <= capacity(p)`.
- **Minimal cases:** one parent and one active child distinguish absence/prohibition; one parent
  with capacity one and two active children distinguishes scoped cardinality. Specify initial
  validity and every creating/updating/removing transition; cardinality is not lifecycle safety.
- **Expected / actual:** the first restricted relationship is already addressed by
  LifecycleDependency's immutable required-Ref pattern, under its stated assumptions. The
  candidate-local query language does not provide arbitrary joins or grouped universal rules;
  global scalar uniqueness and aggregate module invariants already exist. These language limits
  do not prove every relational property impossible with guarded transitions or other modeling.
  No demonstrated unmet preservation theorem is supplied for the stronger candidate.
- **Affected / impact:** requirements outside Behavior 0.13.1's supported Safety patterns need
  investigation; no new runtime relational semantics are justified by the label alone.
- **Workaround:** assess explicit domain transitions, immutable references, FieldOwnership,
  existing lifecycle restrictions and alternative state representations against the exact
  predicate. Host enforcement is an explicit trust boundary, not an equivalent Core proof.
- **Proposal / layer:** document a neutral property, allowed state sizes/effects and a precise
  expressibility/preservation obstacle. Explore a narrowly justified Model pattern before any
  general Core extension. Liveness/eventual cleanup remains a different claim.

Sources: [v1 contract][authoring], [existing limitations and alternatives][core-evidence].

## R4 — Define composite uniqueness and compare modeling alternatives

- **Property:** `forall x != y in Q, (x.scope, x.key) != (y.scope, y.key)` under product
  equality. Separately define optional components, mutable scope, reassignment and lifetime.
- **Minimal case:** `(scope=A,key=k)` twice must collide; `(A,k)` and `(B,k)` must not. An absent
  component follows option equality unless explicitly filtered, as in D3.
- **Expected / actual:** a direct tuple-key public primitive is not established by the current
  scalar uniqueness API. This does not establish that scoped uniqueness cannot be enforced
  through domain modeling or controlled transitions. No complete verifier reproducer is attached.
- **Affected / impact:** Behavior 0.13.1 / Core 0.15.0 domains needing scoped keys; exact practical
  impact depends on mutation and creation rules.
- **Workaround:** evaluate a relation entity, separate namespaces, or a demonstrably injective
  canonical key representation; prove that representation and enforce it on every relevant
  operation. Plain string concatenation is not automatically injective, and a hash requires a
  collision assumption. A relation entity alone does not prove uniqueness.
- **Proposal / layer:** modeling research with formal definition and rejected alternatives first;
  do not request tuple/composite primitives until a concrete unsatisfied contract remains.

Sources: [unique API and current boundaries][core-evidence].

## R5 — Distinguish mutual declarations from mutual creation obligations

- **Property:** every required reference targets an existing entity in the complete resulting
  state. If both directions are required, define whether creation/removal is atomic and whether
  the relationship is one-to-one; two references alone do not establish a bijection.
- **Minimal case:** entities A and B reference each other. Evaluate separately forward type-name
  resolution, valid complete genesis, creating the pair in one atomic action, staged creation and
  deleting the pair. A sequential-construction obstacle is not a declaration-level prohibition.
- **Expected / actual:** public forward-reference authoring and named reference identities exist;
  no minimal failing mutual-reference example has been provided. Do not claim all mutual
  references are unsupported. Actual admission/commit outcomes still need reproduction.
- **Affected / impact:** proposed Behavior 0.13.1 / Core 0.15.0 use cases, not a confirmed defect.
- **Workaround:** assess an explicit relation entity or one-directional required reference against
  the original invariant and construction protocol. Optional intermediate state changes the
  invariant and must be explicitly permitted, not silently introduced.
- **Proposal / layer:** modeling/lifecycle investigation first; no new Core primitive approved.

Sources: [reference identity, integrity and authoring evidence][core-evidence].

## R6 — Reproduce same-type surviving bindings plus creation precisely

- **Property:** verification must establish the stated invariant/postcondition for actual Core
  source bindings and the complete resulting state, including both the surviving bound entity
  and the newly created entity of the same type.
- **Minimal reproduction needed:** ordinary entity T with an entity constraint/invariant; an
  action binds an existing State T and creates a fresh T that also satisfies the constraint.
  Keep one nontrivial relevant check, record the exact module/profile and actual invocation.
  Contrast removal of the constraint and removal of the surviving same-type binding. No
  executed public-Core reproduction or observed attestation is supplied in this triage.
- **Expected / actual:** source inspection identifies a narrower encoder restriction involving
  same-type child identity ordering **when entity constraints/invariants exist**. It is not a
  blanket prohibition on binding and creating the same type. Source inspection alone does not
  determine an observed top-level verifier result.
- **Affected / impact:** candidate Core verifier 0.10.0 precision limitation under Core 0.15.0 /
  Behavior 0.13.1. Runtime validity and SMT support must be reported independently.
- **Workaround:** preserve the property; accept a noncertifying verifier result where necessary
  or investigate a different atomic domain operation. Do not remove constraints to obtain green
  verification or split an operation in a way that changes its invariant.
- **Proposal / layer:** minimize through public Core APIs and independently replay any candidate
  counterexample. A sound `inconclusive` is incompleteness. A reported proof contradicted by an
  admitted, valid-source actual Core transition is a soundness defect and must be escalated with
  its exact check subject and assumptions. No such unsound result is established here.

Pinned encoder references and a source-checked scaffold: [Core evidence appendix][core-evidence].

## R7 — Classify equivalent-guard sensitivity by the property each checker establishes

- **Property:** for the same bindings and typed source universe, G1(S) iff G2(S). Verify both
  directions; membership permutations and De Morgan rewrites must retain set, capture, polarity
  and mandatory guard placement. A guard hidden under an insufficient OR is not equivalent.
- **Existing minimal reproduction:** the public [guard-contract fixture][guard-fixture] has
  canonical and permuted terminal membership guards, an equivalent `all_` form, and three unsafe
  controls. The local postcondition concerns the stopped parent's children; Safety establishes
  its separately stated conditional inductive property.
- **Observed on checker 2 / verifier 0.10.0:**

  | Variant | Local Core check | Safety outcome |
  | --- | --- | --- |
  | Canonical | Proven | Certified under assumptions |
  | Same-set membership permutation | Inconclusive (`counterexample_not_reproduced`) | Certified under assumptions |
  | Equivalent `all_` form outside Safety's canonical profile | Inconclusive (`counterexample_not_reproduced`) | Unsupported |
  | Missing guard / wrong parent / insufficient OR | Counterexample | Unsupported |

  These are the already recorded results, not new runs. The original Safety membership-order
  defect is **resolved** and must be removed from open-bug drafts. The remaining Core outcome
  difference is an incompleteness observation, not evidence of an unsound proof.
- **Additional documentation finding:** the 0.13.1 verification Skill still says only `not(any)`
  can discharge a uniqueness guard and recommends replacing `count(where(...)) == 0`. Core
  0.15.0 documents typed parent-count congruence for both forms. Remove that blanket old
  limitation from proposed feedback; reconcile the Skill against the documented supported
  fragment and reproduce any residual case before claiming it persists. Do not infer that all
  logically equivalent expressions are within the verifier's supported fragment.
- **Affected / impact:** Behavior 0.13.1 authors may encounter noncertifying verifier outcomes
  for equivalent guards or follow stale advice. No new incorrect `Verified` result is supplied.
- **Workaround:** use a demonstrated supported form for the exact check; otherwise preserve the
  requirement and report Inconclusive. A Safety certificate does not turn a local Inconclusive
  into Proven, and local SMT success does not authorize an unsupported Safety pattern.
- **Proposal / layer:** documentation correction plus minimized verifier investigation. Record
  exact IR, BehaviorHash, verifier/profile/subject, solver, both expressions, actual outcomes and
  independently evaluated counterexamples. Escalate genuine unsoundness; do not expand Core or
  Safety's expression language merely to equalize green results.

Sources: [existing public authoring/results][authoring], [guard fixture][guard-fixture],
[pinned Core congruence contract and stale Skill reference][core-evidence].

## Review and sharing boundary

Before filing any open draft, attach a minimal public, domain-neutral reproducer and the actual
expected/observed results for its stated property. Keep hypotheses and unexecuted scaffolds
labelled. Include affected release, impact, workaround and intended layer as above. Do not copy
private repositories, application internals, infrastructure identifiers or complete private logs.

D1 and D2 are the confirmed next-release improvement proposals; D3 and the bounded stale-Skill
correction are documentation proposals. C1 and the Safety membership-order defect are resolved.
R1–R6 require research/reproduction, and R7's existing evidence establishes a precision difference,
not a soundness defect. No issue, new feature or implementation is approved automatically.

[manifest]: https://github.com/kallistoteles/behavior-ir/releases/download/v0.13.1/release-manifest.json
[build]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/scripts/release-build.sh
[flake]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/flake.nix
[lock]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/flake.lock
[tools]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/crates/behavior-models/proofs/toolchain.json
[certifier]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/crates/behavior-models/src/safety/certify.rs
[safety-api]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/python/behavior/safety.py
[smoke]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/release/safety_smoke.py
[release-run]: https://github.com/kallistoteles/behavior-ir/actions/runs/39279668897
[lifecycle]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/crates/behavior-models/src/safety/lifecycle_dependency.rs
[authoring]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/models/examples/safety_models/README.md
[diagnostics]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/python/tests/test_safety_lifecycle.py
[deployment]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/models/examples/safety_models/deployment.py
[guard-fixture]: https://github.com/kallistoteles/behavior-ir/blob/v0.13.1/models/examples/safety_models/guard_contract.py
[core-evidence]: research/0.13.1-core-triage.md
