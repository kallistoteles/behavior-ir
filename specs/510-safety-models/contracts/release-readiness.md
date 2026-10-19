# Contract: Safety Models Release Corrections

**Status**: Implemented and locally validated for candidate 0.13.1; publication remains separate. **Applies to**: FR-039–FR-046 / SC-010–SC-014.
This supplements the existing proof, authoring and deployment contracts.

## 1. Recognition authority

1. Obtain candidate literal orders from public Core serialization of the actual admitted module.
   Generic traversal collects only string arrays with the exact required distinct-member set;
   preserve order and duplicates. Never infer meaning from serialized expression keys/operators.
2. Reconstruct the entire expected guard through the public Core Builder with the resolved
   enum/entity/field, State bindings, captures, polarity and existing canonical query structure.
3. Require exact equality to an actual mandatory precondition-root hash. A nested subexpression,
   matching unrelated array or normalized policy hash cannot satisfy this check.
4. Select matching roots deterministically by hash, preserving the complete original inventory.
   Share the validated selection between analysis and concrete proof encoding.
5. Kernel-check ordered-vector set agreement/membership equivalence and all existing concrete
   premises. Generic FO/LD conclusions, axiom audits and fresh replay remain mandatory.

Only the private order-discovery helper may call public Core serialization in the Model crate.
The public-surface gate's named exception and controls must retain rejection of other Model
serialization sites/private dependencies. This revises the original blanket serializer ban;
it authorizes no Wire-expression semantic interpreter or Core hash implementation.

The internal work budgets in plan.md apply cumulatively. Exhaustion blocks certification as
Inconclusive with a resource diagnostic; malformed/unrecognized semantic shapes remain Unsupported.
Do not truncate a scan and present it as complete. Retain explicit false/true empty-set guards;
Core rejects authored empty membership. Duplicates are admitted and truth-preserving, but remain
in the reconstructed vector because their multiplicity/order contributes to Core identity.

## 2. Lifecycle diagnostic contract

| Code | Established condition | Outcome |
| --- | --- | --- |
| SAFETY_PARENT_BINDING_REQUIRED | No same-type State parent candidate | Unsupported |
| SAFETY_OWNER_ASSOCIATION_REQUIRED | State candidates exist; none has the required owner association | Unsupported |
| SAFETY_PARENT_GUARD_REQUIRED | Associated candidates exist; none also has the mandatory permitted-parent guard | Unsupported |
| SAFETY_GUARD_SCAN_EXHAUSTED | The bounded recognition operation could not complete | Inconclusive |

Evaluate the first three stages over all candidates; do not conjoin facts about different parents.
One earliest established failure is emitted per action/dependency/operation, with action,
dependency ModelHash, entity/field where known and actionable detail. Retain the diagnostic order
by action identity, dependency identity and operation occurrence; author labels are display only.
Serialize the structured issues in analysis and assessment diagnostics. Keep structural ownership
errors, unsupported patterns, resource failures and proven premise/identity failures distinct.

Terminal child assignment retains its existing exemption. Terminal child creation still needs
the bound existing parent and exact owner initializer, but not a permitted-lifecycle-state guard.
Public guidance covers every operation in the release overview and explicitly includes both
Planned → Created and Terminal → Active when their destination is nonterminal. These labels
are examples, not lifecycle values built into the checker.

## 3. Independent verification matrix

Use one minimal local parent-prohibition/query fixture with a relevant local postcondition,
plus the existing child-side controls. Public Core verification must run a declared nonempty
profile, including its query-sensitive postcondition. Record actual per-check outcomes and
their property subjects; do not infer a global relation from the attestation's overall status.

| Variant | Safety expectation | Core expectation |
| --- | --- | --- |
| Existing canonical root | Certified under assumptions | Record actual local-check outcomes |
| Same set in another literal order | Certified under assumptions for its exact module | Record independently |
| Equivalent form outside the profile | Unsupported, no certificate | Record independently |
| Required guard absent | Unsupported, no certificate | Record independently |
| Guard bound to another parent | Unsupported, no certificate | Record independently |
| Matching expression under insufficient OR | Unsupported, no certificate | Record independently |

An observed Core Inconclusive result is retained, not relabeled as proven or automatically
treated as failed Safety certification. Missing tools, empty check inventories, missing rows
or errors preventing the comparison are failures of the required comparison run.

Extend the existing reference runner with `--guard-contract --certify --output PATH`; the mode
requires explicit certification and writes `guard-contract.json` with six rows and overall
completeness/result. The mandatory proof gate runs it alongside the existing five cases.
Expected negative controls are successful tests only when they actually refuse certification;
unexpected certificates, absent rows and supported-case proof failures make the gate nonzero.
Existing five-case interfaces and ordinary lowering mode retain their meanings.

## 4. Versions and release acceptance

Safety checker/lowerer 2 distinguishes changed certification inputs/diagnostics; policy semantics
remain 1. Bind the new helper/checker/proof sources and exact witnessed roots in evidence.
Keep v1 artifact formats while their meaning remains compatible; reject stale checker/source
bundles. New version-2 compatibility tuples retain all existing tuple records and legacy fixtures.

Selected package 0.13.1, the first additive Safety API release after published 0.13.0. Document the
unpublished-outcomes rationale; reassess if another release intervenes. Core remains 0.15.0 and
its verifier remains 0.10.0. Update metadata/docs/notes using the existing version authority.

Reproduce C2 before fixes, exhaust the specified small permutations and targeted negative cases,
and retain full existing regression/proof checks. Run full `release-check.sh` with its mandatory
gates and installed certification/reverification; `--skip-gates` alone is insufficient evidence
for this corrected candidate. Record actual complete 506/507 outcomes and repeated semantic
evidence/artifact identities. Previous completed T001–T085 evidence remains historical.

This work creates no runtime state/replay dependency and no publication authorization. Tagging
and publication occur only through the existing separate explicit post-merge step.
