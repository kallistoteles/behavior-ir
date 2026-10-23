# Remediation assessment — complete

## Historical G2 integration results

Exact published A256/A512 requests that exhausted the old concrete stage now pass complete
installed-package certification and separate-process checking, without larger limits:

| Case | Cold certification | Independent cold check | Complete proof | Separate 300 s budgets |
| --- | ---: | ---: | --- | --- |
| A256 | 106.70 s | 105.54 s | Passed | Passed |
| A512 | 196.10 s | 190.55 s | Passed | Passed |

See [scale smoke](evidence/scale-smoke/). These use the provisional Core-codec wheel, not a final
frozen candidate. Every mapped baseline obligation and new trace/codec/assembly obligation was
checked, followed by intended types, permitted transitive axioms and fresh kernel replay. No
certificate was inferred from a fake worker, cached acceptance or a passing data-only prototype.
A separately constructed nonbalanced trace passed the full checker; deliberately malformed,
incomplete/truncated/reordered traces and actual proof-hole/custom/native-axiom/weak-statement
mutations could not certify. See [independent checking](evidence/independent-checking/).

## Guarantees and trust

Lean establishes arbitrary-input trace soundness and the bridge to the *unchanged* ownership and
lifecycle guarantees. Producer correctness is not a premise. Rust/Core extraction, probe meanings,
exact encoding/expected wrapper generation, pinned build/kernel/toolchain integrity and backend
contracts remain explicit trusted correspondence. Hashes establish identity under their stated
assumptions; they do not prove that extraction is correct. Full end-to-end formal verification is
not claimed. Initial validity, migrations and approved module/policy/activation continuity remain
separate obligations, not silently upgraded into theorem conclusions.

Language compatibility, producer completeness and practical capacity are distinct. Supported v1
language is unchanged; producer completeness/runtime complexity is not claimed to have been
universally proved. Complete inventory/policy comparison and kernel checking fail closed. Invalid
witnesses do not prove a module unsafe. Resource exhaustion is Inconclusive. A completed proof over
a soft performance budget may stay mathematically valid while failing feature acceptance.

## Final frozen capacity result

The exact 0.14.0 wheel in [final-01](evidence/candidate-builds/final-01.json) completed all
**60 cold pairs / 120 roles**, covering every frozen request and all six dimensions/material
interactions. The independent collector accepted all **1,440 tool-stage records**. Every selected
obligation, intended-type comparison, permitted-axiom audit and fresh replay passed, with each
whole role below its unchanged 300 s budget. No case, repeat, action, operation or policy was removed.
See [complete assessment](evidence/final-runs/assessment.json) and [growth analysis](scalability-results.md).

| Evaluation dimension | Result and scope |
| --- | --- |
| Mathematical soundness | Arbitrary-trace soundness and unchanged safety implications machine-checked under explicit Core/Rust/probe/build assumptions; no producer-correctness premise |
| Language coverage | Existing v1 predicates/grammar retained; full 328 Rust / 719 Python tests and 82 membership proof instances passed; final installed references and all 86 public Safety tests passed |
| Mathematical completeness | No proof of producer completeness or all true domain predicates is claimed |
| Practical capacity | All 20 frozen cases × 3 pairs passed on the declared x86_64 reference environment; A256/A512 are floors, not a maximum |
| Reproducibility | All 20 requests reproduce exact semantic evidence across 3 repeats and producer/consumer; operational noise is excluded |
| Core state transparency | Ordinary Core IR/receipts/state/replay remain independent of Safety policies and Lean; source and installed smoke checks passed |

A256 certification was 100.94–106.61 s, checking 101.86–103.49 s. A512 certification was
183.96–195.66 s, checking 180.83–188.61 s. These are descriptive finite measurements, not an
unbounded runtime promise. Authority-list work remains quadratic; no resource limit was raised.

## Final reconciliation

Frozen transport, actual proof-integrity and finite-premise controls passed. Installed references,
the six-case Core/Safety comparison, all 86 public Safety tests, quickstart execution, immutability
and full requirement coverage passed. Feature 511 implementation is complete for the declared
contract and capacity profile. Publication is a separate action and was not performed. ARM
execution/performance was unavailable and is not claimed.
[Remaining boundaries](remaining-limitations.md) retain the explicit host and computational duties.

## Invocation corrections retained

Initial installed pytest attempts used default prepend mode and imported the old editable source
extension. Green-named logs from those attempts were collection failures, not passes. Corrected
installed tests use `--import-mode=importlib`; seven focused version/transparency cases passed.
Cold workers already removed source paths and genuinely passed against installed wheels.
The first M4 attempt failed reference-CPU comparison because `/proc/cpuinfo` preserves spacing
that `lscpu` normalizes. Raw model text is now retained and only whitespace is normalized for the
same CPU identity; no profile, hardware condition or budget was changed. That invocation did not
execute a proof and is not a failed mathematical case.

The first final extracted-premise diagnostic attempt rejected a correctly failed Lean claim because
its error-message parser expected different quotation/capitalization. It is quarantined as invalid
control evidence; fresh attempt 02 passed all 7 controls, their exact-type/axiom audit and fresh replay.
Candidate sources, the profile and proof obligations were unchanged. See
[control provenance](evidence/final-controls/invalid-driver-attempt.json).

## Delivery, compatibility and platform reconciliation

The default full release check passed all existing gates, 328 Rust tests, 719 Python tests and
49 installed Skill/API/equivalence tests. Complete 506 passed 39 scenarios; complete 507 passed 61
with 12 obligations supported by their actual mixed evidence classes. Quick 507 separately met
its mandatory 35 s budget in 26.36 s. The twelve obligations are not relabeled universal SMT proofs.
The installed package executed/replayed without Lean or Rust on PATH, then explicitly certified
and separately rechecked supplied data. See [delivery logs](evidence/delivery-gates/release-check.json).

The original candidate and an explicit additional reproducible build have identical SHA256SUMS.
All 155 declared source inputs, evaluated tooling, the artifact and profile still match the frozen
identities after all 60 pairs. Source validation used an isolated authorized snapshot to preserve
unrelated original Spec Kit/docs edits. No active Core override was used and no tag was published.

The ordinary Model compatibility manifest and exact Core pin are unchanged; all 22 established
Model receipts and six historical Safety tuples are retained. Checker 3 adds three tuples without
editing prior rows. Policy semantics stay v1; evidence/bundle v2 and statement 2 are explicit contracts.
Historical reproducibility is not continued trust in an old implementation if a soundness flaw
is later found; no published artifact was rewritten or implicitly reinterpreted.

Available x86_64-linux correctness/package/reproducibility checks passed. ARM execution was
unavailable; its pinned profile remains intact and no ARM performance or test pass is claimed.
Genesis validity, migration safety, approved exact BehaviorHash/complete policy-set authorization
and fresh enforced activation anchors remain separate obligations. No persistent runtime state,
new event stream or replay dependency was introduced. See [platform record](evidence/platform-checks/environment.json).

Final evidence: [installed corpus](evidence/v1-regression/reconciliation.json),
[quickstart execution](evidence/quickstart-validation/execution.json),
[41-row requirement matrix](acceptance-evidence.md),
[unchanged user work/history](evidence/delivery-gates/final-immutability.json) and
[exact artifact binding](evidence/delivery-gates/final-artifact-binding.json).
