# Interface and versioning research

Date: 2026-10-22. Source inspection only; no proof execution, tests or production changes.
Authority: revised Feature 511 specification, including settled Q1–Q10.

## Decision — Retain public entry points and opaque certificate authority

**Decision:** Keep the Rust/Python authoring, certification, bundle-loading and verification
entry points as the integration seam. Separate construction from checking internally if the
investigation supports it; do not introduce a second certification API merely to carry a witness.

**Rationale:** Rust already owns `analyze_with_dependencies`, `certify`, `bundle` and
`verify_safety_evidence`. Python delegates through `certify_safety`, `load_safety_bundle` and
`verify_safety_evidence`. `SafetyCertificate` has no public constructor or deserializer;
`UnverifiedSafetyBundle` is transport only and always reports `is_verified() == false`.
The consumer supplies an independently expected module and Safety policy set, not merely a
producer-selected subset. This distinction must survive any internal refactoring.

Sources: [Rust entry points](../../../crates/behavior-models/src/safety/mod.rs),
[certificate and bundle types](../../../crates/behavior-models/src/safety/evidence.rs),
[consumer verification](../../../crates/behavior-models/src/safety/certify.rs),
[Python API](../../../python/behavior/safety.py),
[thin binding](../../../crates/behavior-py/src/safety.rs).

**Alternatives considered:** A public witness-construction API or separate verifier service is
not presently required. Preserve those possibilities only if selected experiments demonstrate a
need that the current library seam cannot satisfy.

## Decision — A witness is versioned, untrusted data; its payload is not yet selected

**Decision:** If the witness path is selected, define a strict versioned transport and checker
contract before accepting it. The contract must identify the admitted BehaviorHash, complete
policy identity, full inventory identity, intended theorem/statement version, checker and proof
environment, and the witness interpretation. The final field names, nesting, encoding and witness
algorithm remain a design-gate outcome, not a schema already chosen here.

**Rationale:** Current `behavior.safety_bundle.v1` permits exactly five top-level members:
`format`, `module`, `inventory`, `policies`, `evidence`. Adding a witness member would violate
that parser; placing new semantics inside existing objects would still require an explicit
compatibility decision. Current consumer checks re-admit transported Core IR, independently
compare inventory and expected policies, validate evidence identity, compare exact proof/source/
dependency identities, and then rerun certification. This last step is the replaceable
construction/checking seam, not permission to bypass independent input coverage.

A future verifier must parse bounded typed data, reject unsupported contract versions, and
regenerate any executable Lean wrappers exclusively from shipped trusted templates plus safely
encoded data. Supplied Lean source, compiled objects, scripts, commands, paths to executable
plugins or arbitrary expressions must not become executable authority. Generated identifiers and
literal encodings must not permit source injection. Resource/parse bounds are operational
controls; their exhaustion cannot establish a mathematical counterexample.

Sources: [fixed bundle parser](../../../crates/behavior-models/src/safety/evidence.rs),
[`prepare_bundle` and `verify_safety_evidence`](../../../crates/behavior-models/src/safety/certify.rs),
[current inventory/policy identities](../../../crates/behavior-models/src/safety/identity.rs).

**Alternatives considered:** Re-running all construction remains the baseline and fallback for
comparison, not a mandatory future architecture. Trusting supplied success flags or checking
producer-supplied code would violate Q7/Q10. No particular sorting/permutation witness is selected.

## Decision — Preserve outcome meaning and independent state-validation boundaries

**Decision:** Preserve `Certified under assumptions`, `Inconclusive`, `Unsupported` and `Failed`,
with certificate construction only after all required checks pass. Keep performance observations
outside canonical semantic evidence and distinguish whole-chain budget failure from safety
violation. Initial-state validation, migrations and host deployment authorization remain separate.

**Rationale:** Current assessments explicitly enumerate structural, generic/concrete proof,
type/axiom and fresh-replay obligations. Existing code classifies proof exhaustion and unavailable
tools as Inconclusive; unsupported patterns separately. Certificate records already identify
trusted extraction/encoding, Core/backend/toolchain and cryptographic boundaries. Those trusted
boundaries are not removed simply by making the producer untrusted. Snapshot records have a
separate type and exact-history anchor; witness verification must not grant Store authority.
Current proof limits are per process; a whole-chain measurement/profile is an additional
operational contract and need not change the public function signature without demonstrated need.

Sources: [outcomes, assumptions and snapshot authority](../../../crates/behavior-models/src/safety/evidence.rs),
[`outcome_for`, `ProofLimits` and persistence](../../../crates/behavior-models/src/safety/certify.rs),
[Python proof configuration](../../../crates/behavior-py/src/safety.rs).

**Alternatives considered:** A new public outcome or treating timeout as Failed would conflate
computation with the mathematical assertion. A new outcome requires a demonstrated unrepresentable
case and explicit public-contract review, not an incidental implementation choice.

## Decision — Version the selected contract; preserve historical verification conditions

**Decision:** Review statement, checker, transport/evidence and release versions after the
representation/checking experiments select a solution and before implementation is accepted.
Declare supported verification contracts explicitly. Preserve published 0.13.1 artifacts and use
their original pinned verifier for historical checks; do not promise universal legacy dispatch or
automatic download/execution of historical verifiers.

**Rationale:** Baseline policy semantics is 1, checker 2, evidence/bundle domains v1 and proof
`statement_version` 1. Evidence binds embedded Lean sources, generated concrete/audit source,
Rust implementation sources, Cargo lock, Rust toolchain, Core pin and proof-toolchain manifest.
Thus even equivalent representation changes invalidate exact source identities under the current
contract. Identical mathematical meaning does not justify silently relabeling a changed contract
as the old one. Conversely, a representation change alone need not change policy meaning or
Core BehaviorHash. A failure to read old evidence is not its mathematical refutation; reproducing
acceptance by an old verifier is not a soundness guarantee if that verifier is later found unsound.

The 0.x release policy requires a minor bump for changed existing formats/public APIs/verifier
outcomes; 0.13.1 was additive because 0.13.0 had no published Safety API. Feature 511 cannot
reuse that historical rationale automatically. Record the exact applicable release decision;
do not choose a patch number merely because Core identities remain unchanged.

Sources: [checker and semantic constants](../../../crates/behavior-models/src/safety/mod.rs),
[`proof_identity`, `implementation_identity`](../../../crates/behavior-models/src/safety/certify.rs),
[dependency identities](../../../crates/behavior-models/src/safety/evidence.rs),
[release policy](../../../docs/versioning.md).

**Alternatives considered:** Universal backward compatibility and automatic historical-verifier
management add functionality outside Q5. Keeping the exact current chunk representation would
impose an unjustified architectural constraint rejected by Q3.

## Decision — Reuse installed-package and proof gates

**Decision:** Extend the current proof runner and installed-package smoke path with the selected
contract and independent checking cases. Retain ordinary Core execution/replay without proof
tools and reproducible release verification. Do not require a source checkout for consumers.

**Rationale:** The Rust binary embeds eight Lean source modules and `proofs/toolchain.json`.
`ProofToolchain` verifies immutable Nix output and derivation identity, rejects executable
overrides, and supports pinned x86_64-linux/aarch64-linux profiles. The flake exposes
`safety-proof-toolchain`. `scripts/gates.sh` invokes `check-safety-proofs.sh`; release-check
installs a consumer wheel, runs ordinary replay without Lean on PATH, then certification and
bundle re-verification outside the checkout, followed by reproducible builds. Preserve these
seams and include any selected new verifier source/data in the same package/identity machinery.

Sources: [embedded source list and toolchain validation](../../../crates/behavior-models/src/safety/certify.rs),
[locked manifest](../../../crates/behavior-models/proofs/toolchain.json),
[flake](../../../flake.nix), [proof gate](../../../scripts/check-safety-proofs.sh),
[release-check](../../../scripts/release-check.sh),
[installed consumer smoke](../../../release/safety_smoke.py),
[package configuration](../../../pyproject.toml).

**Alternatives considered:** A separate witness distribution service, new proof toolchain manager
or parallel release process is unnecessary. Optional precompiled shipped dependencies must be
documented in cold-run conditions and cannot substitute for required concrete fresh checks.

## Required contract-selection gate

Before production implementation is finalized, record: selected proof interpretation and its
machine-checked correspondence; producer/verifier responsibilities; independent inventory and
complete expected-policy comparison; bounded data grammar and injection rejection; required
proof/audit/fresh checks; exact identity inputs and version transitions; supported contract
versions and historical verification instructions; package source inclusion; and separate cold
certification/re-verification profiles. This gate selects the witness format only if evidence
justifies using a witness. It must not silently turn a candidate into an implemented API.
