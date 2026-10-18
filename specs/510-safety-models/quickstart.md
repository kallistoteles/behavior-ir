# Validation Quickstart: Safety Models

**Status**: Implemented and validated. All required gates, package checks and public API controls passed; results are recorded in the implementation matrix and `target/safety-models/final/`. No command publishes or tags a release.

See [authoring](contracts/authoring-and-certification.md),
[proof contract](contracts/proof-contract.md) and
[evidence/deployment](contracts/evidence-and-deployment.md) for the normative behavior.

## Prerequisites

Use the repository's locked Nix environment, exact Core 0.15.0 pin, Rust 1.98.1, Python 3.13 and
the added locked Lean 4.30.0 package. No elan or separately managed proof toolchain is needed.
Certification must verify actual selected Nix tool identities; a matching version banner is
only a diagnostic. Existing Core runtime use does not require Lean.

Use the existing locked package/tool output:

```bash
nix develop -c scripts/fetch-core.sh
nix develop -c maturin develop
nix develop -c lean --version
nix develop -c scripts/check-core-pin.sh
```

Keep free build space under observation; do not remove unrelated or uncommitted work. Tool
installation/build-space failures are environment failures, never successful proof evidence.

## 1. FieldOwnership milestone first

Write/observe failing controls before implementing them. Then run the focused tests:

```bash
nix develop -c cargo test -p behavior-models --test safety_ownership
nix develop -c cargo test -p behavior-models --test safety_inventory
nix develop -c cargo test -p behavior-models --test safety_identity
nix develop -c python -m models.examples.safety_models.run --case fo_field --certify --output target/safety-models/fo-field
nix develop -c python -m models.examples.safety_models.run --case fo_state_machine --certify --output target/safety-models/fo-state-machine
```

Expected: both examples certify under explicit assumptions; unauthorized/no-op/secondary-binding/
generated writers and unauthorized creators/removers fail. Concrete proof data includes every
admitted action. Proof-hole/custom/native-axiom and omitted-action controls block certification.
Measure proof time/memory. Do not start LD implementation until the FO gate passes.

## 2. Public-Core feasibility before LifecycleDependency

```bash
nix develop -c cargo test -p behavior-models --test safety_correspondence
```

Expected: declarations and Ref markers retain identities; canonical probe hashes match actual
mandatory precondition roots/initializers; required parent bindings are State. Wrong binding,
weakened/nested guard and supplied Context/Input Entity controls do not match. Probe construction
leaves the actual authored module unchanged. Report and stop on an essential public-facade gap.

## 3. Lifecycle proof and acceptance

```bash
nix develop -c cargo test -p behavior-models --test safety_lifecycle
nix develop -c python -m models.examples.safety_models.run --case ld_parent_child --certify --output target/safety-models/ld-parent-child
nix develop -c python -m models.examples.safety_models.run --case ld_shared_parent --certify --output target/safety-models/ld-shared-parent
nix develop -c python -m models.examples.safety_models.run --case ld_vm_resource --certify --output target/safety-models/ld-vm-resource
```

Expected: useful safe child creation/reactivation, parent transitions and isolated lifecycle
operations succeed under actual Core. Every affected dependency is checked. Unsafe transitions
refuse or their module cannot certify. Combined relevant operations/computed values/unrecognized
forms return Unsupported; immutable-owner assignments fail structurally, including same-value writes.

The VM output identifies only the restricted reference predicates; it claims no I-5b/I-6 mapping
or live/nonterminal equivalence and prescribes no mandatory production VM ownership.

## 4. Proof integrity and exhaustive correspondence

```bash
nix develop -c scripts/check-safety-proofs.sh
nix develop -c cargo test --release -p behavior-models --test safety_exhaustive
nix develop -c python -m pytest python/tests/test_safety_authoring.py python/tests/test_safety_evidence.py
```

The proof gate checks generic and all required concrete roots, their expected propositions,
structured transitive axiom allowlists and bundled `leanchecker --fresh`. Missing tools or a
noncertifying required case fail the gate. It must not silently skip proof checks.

The exhaustive coverage manifest records actual state/action counts and bounds from plan.md.
The independent oracle evaluates original Q before/after actual Core transitions. It is
correspondence evidence; finite enumeration is never reported as the generic theorem.

## 5. Exact state and activation boundary

```bash
nix develop -c cargo test -p behavior-models --test safety_snapshot
nix develop -c python -m pytest python/tests/test_safety_snapshot.py
```

Expected: valid full exports yield history-bound Valid records; missing/prohibited parents,
malformed/incomplete/untrusted inputs cannot establish state validity. A module certificate
alone leaves initial validity unestablished. An intervening history commit invalidates the
reference activation protocol's old anchor, even where StateId is unchanged.

## 6. Regression and reproducibility

```bash
nix develop -c python3 scripts/check-models-compat.py
nix develop -c scripts/determinism-check.sh
nix develop -c scripts/conformance.sh 506 complete
nix develop -c scripts/conformance.sh 507 complete
nix develop -c env BEHAVIOR_FULL_PROPERTIES=1 scripts/gates.sh
```

Expected: old receipts/reference hashes/version contracts remain unchanged; new Safety mappings
are checked by the existing compatibility gate. Identical pinned inputs yield identical canonical
evidence. Existing runtime budgets remain enforced; proof work is outside the 507 quick timer.

Compare lowering without `--certify`: it must say Lowered/uncertified and never issue a certificate.
Repeat certification with a missing/wrong tool, edited inventory, wrong theorem or changed policy:
all must fail closed. No release or publication step belongs to this feature's quickstart.

## Executed results and negative controls

The standalone mandatory proof gate passed for all five cases, including all eleven audited
roots per certificate, exact concrete anchors, the allowed transitive axioms and fresh kernel
replay (`target/safety-models/final/proof-preflight/summary.json`). All certificates remain
conditional on the named correspondence/host boundaries. The exhaustive release-profile run
covered 3,924 constructed typed states, 1,904 valid source states, 38,006 attempted transitions
and 30,218 commits, with zero unsafe certified commits (`target/safety-models/ld/coverage.json`).
The interrupted debug-profile attempt is not claimed as a passed test.

Both explicit complete conformance runs passed: 506 has 39 checks; 507 has 61 checks and 12
obligations. Their reports are retained under `target/safety-models/final/`. The existing
WorkQueue quick budget is the baseline's explicitly reviewed **35 seconds**; Feature 510
changes no budget. Proof work is outside that timer.

The final full-gate invocation additionally sets `CARGO_PROFILE_TEST_OPT_LEVEL=3`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_TEST_INCREMENTAL=false`, `CARGO_BUILD_JOBS=2`,
and a task-owned `TMPDIR` on the larger data volume. These optimize compilation/test execution
and bound disk use; they remove no test cases, assertions, proof obligations or required gates.
See `final/gates.json` for its actual result rather than inferring completion from this guide.

Lowering-only and missing-tool/tamper controls have run through the public API. To reproduce
those controls after the positive proof gate above:

```bash
nix develop -c python - <<'PYCODE'
import behavior as b
from models.examples.safety_models.fo_field import build
model = build()
assert model.safety_analysis.status == "Lowered"
assert not model.safety_analysis.is_certified
try:
    b.certify_safety(model, output="target/safety-models/negative/missing",
                     toolchain="/nonexistent/feature-510-proof-tools")
except b._engine.EngineError:
    pass
else:
    raise AssertionError("Missing proof tools were accepted")
data = b.load_safety_bundle("target/safety-models/proof-gate/fo_field/bundle.json").to_dict()
data["inventory"]["actions"].pop()
result = b.verify_safety_evidence(model, data, output="target/safety-models/negative/tampered")
assert result.outcome == "Failed" and result.certificate is None
PYCODE
```

The equivalent executed controls use the retained standalone `final/proof-preflight` bundle;
results are in `target/safety-models/final/quickstart-controls.json`. Reproducibility is also
checked by `safety_reproducibility.rs`: two fresh successful certificates must have identical
canonical bytes across different output paths and resource logs. Unverified bundle loading
never grants certificate authority.

The exact state adapter is `validate_safety_snapshot(store, model, at=history)`. It acquires the
public Core export directly; detached JSON cannot be passed as a Store. State records are
Valid/Invalid/Unestablished independently of proof outcomes. The host example consumes an
already checked process-local certificate; a production host can instead rely on a trusted
offline build-approval path. That host trust boundary does not require Lean during Store
execution/replay and does not make unsigned JSON into proof or Core authorization.

## Final integrated result

`target/safety-models/final/gates.json` records the successful complete gate run after resolving
API manifest/gate-test harness integration failures. It passed all 306 Rust tests and 621
Python/model tests (five existing release-only opt-in skips), the named five-case Safety proof
gate, full 506/507, compatibility, determinism, tooling and script checks. WorkQueue quick mode
measured 31.11 seconds against its unchanged 35-second budget. Both explicit `complete` modes
also passed independently.

`target/safety-models/final/package.json` records a fresh `release-check.sh --skip-gates` run:
installed ordinary execution and Core-only replay, explicit certification and fresh bundle
verification from an empty consumer directory, all existing Skill checks, 49 drift/equivalence
checks and matching reproducible wheel bytes. The recorded full gate was reused, not bypassed
or replaced. The final public API rerun is recorded separately in `package-api-controls.json`.

The final public API rerun passed all 11 tests (`package-api-controls.json` and its JUnit XML).
