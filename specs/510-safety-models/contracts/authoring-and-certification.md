# Contract: Authoring and Explicit Certification

This is the planned public contract, not an assertion that these APIs already exist.
All policy meaning, normalization, resolution and checking resides in Rust.

## 1. Authoring inputs

Add `safety=` to BehaviorModule, independent of its existing `models=` argument. Defaults to no
policies. Existing module construction, Core output and receipt interfaces retain their behavior.

Planned recording objects:

| Object | Required inputs | Optional inputs / defaults |
|--------|-----------------|----------------------------|
| FieldOwnership | id, entity, writers (field → action selectors) | creators/removers default empty; omitted permissions never mean unrestricted. |
| LifecycleDependency | id, parent, child, owner field, parent/child state fields, prohibited/terminal members, applicable ownership policy references | None that weaken the required Ref/immutability/control contract. |
| GeneratedAction | model_id, local_action_id | No display-name or prefix wildcard authority. |

Action selectors accept existing authored action handles/exact names or GeneratedAction pairs.
Resolve against the complete final admitted module and reject missing, stale or ambiguous
selectors. Canonical permission sets contain actual action identities, not selector spellings.

LifecycleDependency requires explicit FieldOwnership controls for relevant lifecycle fields,
owner assignments and entity creation/removal, intersecting existing generating Model ownership.
The owner field's effective writer set is empty. Multiple restrictions never broaden permissions.

Parent/child lifecycle fields must be enum-valued. The owner must be a nonoptional Ref to the
configured parent, checked using the reference marker as well as normalized type. A plain ID
or optional/polymorphic owner is unsupported by this initial dependency pattern.

No lifecycle actions, hidden guards or runtime capabilities are generated. Scratch expressions
used to recognize existing guards never enter the emitted module.

## 2. Lowering boundary

Keep Rust `compose_models` and BuildOutput unchanged. Add a safety-aware composition entry
point sharing the same implementation and returning the existing behavior output plus a
separate UncertifiedSafetyAnalysis. Run:

1. Existing resolve/lower/compose and final Core admission.
2. Every existing generating Model ownership check.
3. Safety policy resolution, complete inventory extraction and structural ownership checking.
4. Uncertified analysis and unchanged existing lowering receipts.

FieldOwnership violations, owner reassignment and malformed/ambiguous policy declarations
are deterministic build errors naming the action/protected state. Unsupported LD guard/value/
mixed-effect shapes can be represented in an uncertified analysis; they block certification.
Successful lowering never implies a proof was checked.

The emitted admitted Core module must be identical to lowering the same authored/generating
inputs without the additional Safety policies. Policies may reject inputs, not rewrite them.

## 3. Explicit operations

Implemented Python-facing operations delegate to Rust:

- `certify_safety(module, *, output, toolchain=None, limits=None)` produces an assessment and
  evidence bundle. It extracts/checks the exact final module/policies afresh. Only successful
  generic/concrete proof, type, axiom and fresh replay checks produce a SafetyCertificate.
- `verify_safety_evidence(module, bundle, *, output, toolchain=None, limits=None)` treats the bundle as
  unverified data, checks identities against the actual module/policy set/inventory, and reruns
  the required proof-integrity checks. JSON status or file hashes are insufficient.
- `validate_safety_snapshot(store, module, *, at=None)` acquires the public Core snapshot at the
  requested exact HistoryRef (or captures current history when omitted) and delegates original-property evaluation to Rust. It returns a separate
  StateValidationRecord; it neither activates a module nor certifies transition preservation.

These operations reuse the existing Core module/Store wrappers without creating another Store
reader. Validation is anchored to the acquired history; it does not assert that the head remains
there. Activation additionally requires the enforced continuity protocol.

The Rust snapshot evaluator consumes the public SnapshotExport product. The Store adapter
records actual acquisition; a detached user-created JSON object must not be relabeled as a
verified export. Backend completeness and export/validator correspondence remain explicit.

Certification uses a pinned System proof runner and controlled proof sources/data. Resource
limits are explicit, recorded diagnostically and never cause successful fallback. Existing
Nix package identity resolves the expected Lean/compiler/rechecker, rejecting PATH-only
substitution or mismatched toolchain selection. Library execution does not silently download
or install tools. Missing tools return a noncertifying error/assessment.
An empty Safety Policy set is ordinary authoring without a Safety claim; requesting certification
of it must not produce a misleading certificate purporting to establish an unspecified property.

Core invocation, commit and replay use ordinary Core APIs and require no Lean process. A host
must enforce its approved module/policy segment; importing this library supplies no authority.

## 4. Outcome contract

| Outcome | Meaning |
|---------|---------|
| Lowered / uncertified analysis | Structural build completed; certification not established. Not a certificate status. |
| Certified under assumptions | All mandatory generic/concrete and proof-integrity checks pass, with enumerated external assumptions. |
| Inconclusive | Required proof/check could not finish or prerequisites were unavailable; no certificate. |
| Unsupported | A requested guard/value/effect/configuration falls outside the declared pattern; no certificate. |
| Failed | A concrete premise, policy, identity or proof-integrity requirement is violated; no certificate. |

Do not call a failed build Lowered. Do not reinterpret absent guards as proven safe, Unsupported
as Failed semantics, or a successful kernel proof as verification of unproved extraction code.
Return deterministic typed diagnostics; subprocess logs and resource measurements are separate.

## 5. Identity and compatibility

Each new kind starts at semantic version 1 and checker/lowerer version 1. Reuse existing
ModelHash canonical infrastructure with kind-specific documents. Policy-set/inventory/evidence
identities use distinct versioned tags. Existing receipt lists/schemas are unchanged.

Extend the existing compatibility checker to validate new sidecar mappings from a separate
safety-compat manifest using established tuple/history rules. Preserve old compat.json, cases,
hashes and receipt bytes. No automated rewrite of established fixtures.

Names/order changes must preserve decisions when meaning is unchanged. Byte identity is
required only when all semantic identities, including Core hashes and proof inputs, are
unchanged; parameter names already included by Core hashing are not declared nonsemantic here.

## 6. Reference runner

Add one example runner, `python -m models.examples.safety_models.run`, supporting:

- `--case fo_field|fo_state_machine|ld_parent_child|ld_shared_parent|ld_vm_resource|all`
- `--output PATH`
- `--certify` for explicit certification; absence means Lowered/uncertified output only.

Use existing public authoring/Store APIs and Rust certification. The runner is a reference
entry point, not a second compiler, release process or production application framework.
Mandatory proof gates exercise all required cases and fail on noncertifying outcomes.
