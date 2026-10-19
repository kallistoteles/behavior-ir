# Safety Models v1 — Release Readiness

**Feature/branch**: `510-safety-models` · **Date**: 2026-10-18 · **Status**: Implemented and locally validated for candidate 0.13.1; publication is a separate post-merge step.

Complete the existing v1 release by addressing the supplied DeclarativeHost integration findings.
The authoritative requirements remain in [spec.md](spec.md): US5–US7, FR-039–FR-046 and
SC-010–SC-014 extend the existing contract. This document is a concise scope guide.
See the [current plan](plan.md#current-plan--release-readiness-2026-10-18) and
[release contract](contracts/release-readiness.md) for decisions and validation boundaries.

## Required corrections

1. **Membership order**: Supported typed enum membership sets have the same certification
   eligibility in every literal order. Preserve the exact set, type, binding, polarity and
   mandatory guard placement. Preserve each authored Core module and bind evidence to its own
   identities. Establish sound, bounded recognition through the pinned public Core interface;
   report a precise blocker if unavailable. No two-value workaround or general factorial search.
2. **Diagnostics**: Distinguish missing parent State binding, incorrect owner association and
   missing permitted-parent guard with stable codes and actionable context. Use deterministic
   precedence or independently established findings without guessing downstream failures.
3. **Authoring guidance**: Explain the complete lifecycle contract, including transitions to a
   nonterminal child state with unchanged ownership. Record Core-verifier and Safety-certifier
   outcomes separately for the six guard cases required by FR-044.

### Lifecycle obligations to explain

| Operation | Safety condition within the established v1 pattern |
| --- | --- |
| Create nonterminal child | Correctly associated existing parent in a permitted state |
| Transition child to nonterminal | Owner matches bound parent; parent is permitted, even if owner is unchanged |
| Transition child to terminal | No new permitted-parent guard under existing v1 assumptions |
| Transition parent to prohibited | All dependent children are terminal |
| Create parent | Existing isolated-creation and freshness conditions |
| Remove child | Ownership permissions and Core integrity |
| Remove parent | No surviving references, including terminal children |
| Assign child owner after creation | Forbidden throughout the child's lifetime, including same-value writes |

## Evidence and release boundary

Use a minimal local C2 reproduction, membership permutations and adversarial controls, plus the
existing proof, extraction, snapshot, replay and conformance checks. Reuse `scripts/gates.sh`
and the full `scripts/release-check.sh` process, including installed-package certification and
reproducibility. Exact implementation choices and task ordering belong to planning.

The existing mathematical statements, concrete obligations, axiom policy and fresh proof
checking remain mandatory. Keep extraction correspondence, initial validity, migrations and
host authorization explicit. A successful ordinary verification result cannot replace Safety
certification; an inconclusive ordinary result is not automatically a failed Safety proof.

Apply existing Model/checker/evidence versioning and package-version policy; preserve published
records and fixtures. Update the relevant public documentation and release notes. Prior
implementation/review evidence covers the previous scope and must not establish readiness of
the corrected candidate. Publication is a separate explicit step after merge and release checks.

## Scope boundary

Keep Core 0.15.0, state transparency and the restricted v1 operation language. No new runtime
state, event stream, replay dependency, owner transfer, optional ownership, Safety Model family,
generated lifecycle API, governance mechanism or production-domain redesign is included.
The VM/Resource fixture remains a restricted reference; I-5b/I-6 remain unmapped.

**Planning rule:** Is this necessary to release Safety Models v1 correctly, or can it wait?

## Completed candidate acceptance

The corrected candidate passed the full existing release process, without `--skip-gates`,
followed by separate `506 complete` and `507 complete` runs. T086–T113 extend the original
T001–T085 implementation; historical runs are not the evidence for these corrections.

| Acceptance | Actual result / local evidence |
| --- | --- |
| Public-Core feasibility | [Preflight](../../target/safety-models/release-readiness/preflight.json); only bounded order discovery uses the scoped public serializer exception |
| Membership recognition | [82 exact-module certificates](../../target/safety-models/release-readiness/membership-current.json): 64 small permutations, 12 duplicate cases and 6 larger representatives, across both guard families; twelve audited roots per certificate |
| Diagnostic causes | [Diagnostic milestone](../../target/safety-models/release-readiness/diagnostics.json): distinct binding, association and parent-guard codes, staged over all candidates; resource exhaustion remains Inconclusive |
| Authoring and verification | [Contract milestone](../../target/safety-models/release-readiness/contract.json) and [fresh six-case comparison](../../target/safety-models/release-readiness/comparison-current.json) |
| Full existing gates | [Current gates](../../target/safety-models/release-readiness/gates-current.json): 320 Rust tests, 662 Python/model tests with no skips, mandatory proofs, compatibility and script controls |
| Complete conformance | [Fresh reports](../../target/safety-models/release-readiness/conformance-complete.json): 506 has 39 passed checks; 507 has 61 passed checks and 12 passed obligations; no blocked, failed, inconclusive or unsupported results |
| Existing quick budget | [32.56s / 35s](../../target/safety-models/release-readiness/507-quick-budget.json); unchanged reviewed budget; quick coverage is explicitly incomplete |
| Installed package and artifacts | [Full release check](../../target/safety-models/release-readiness/release.json): clean installation, all 23 Skill examples, 49 drift/equivalence checks, Core-only replay, packaged certification/reverification and identical repeated wheel builds |
| Baseline and final review | [Review](../../target/safety-models/release-readiness/review.json): 32 protected files unchanged, legacy fixtures retained, all three historical Safety tuples preserved, 22 pinned receipts unchanged |

Behavior 0.13.1 retains Core 0.15.0 / verifier 0.10.0 and uses Lean 4.30.0. Safety checker/lowerer
2 records the changed recognition and diagnostics; policy semantics remain 1. Skills changes
are version metadata only. The [source journal](../../target/safety-models/release-readiness/release-source.json)
records the uncommitted trial candidate, the README anchor correction before packaging and the
release-test version correction before pytest. The two earlier failed attempts (formatting and
disk exhaustion) remain recorded and are not counted as successful release checks.

The generic and concrete Lean proofs, axiom audit and fresh replay passed. Rust extraction,
probe/encoding correspondence and snapshot evaluation remain trusted boundaries. Genesis,
migrations and host-enforced approved modules/policies at an exact fresh activation anchor are
separate obligations. Finite tests and conformance do not turn these assumptions into proofs.
The permuted guard's local Core check is Inconclusive while its independent Safety proof
certifies conditionally; the report does not relabel that local result as proven.

Remaining nonblocking limits are the documented canonical v1 language, fresh-proof/build cost
and host/correspondence obligations. Build/temp/cache work uses the separate data volume;
only generated task caches were removed or losslessly archived. Root-disk capacity still
needs observation. No user work, published tag, record or attestation was removed or changed.

These links identify ignored local build evidence, reproducible through [quickstart §7](quickstart.md#7-release-readiness-validation).
The archived wheel is an unpublished validation artifact. Publication must rebuild the approved
merged source through the existing process and remains a separate explicit action.
