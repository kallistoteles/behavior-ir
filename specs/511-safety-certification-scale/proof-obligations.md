# Safety 511 proof obligations

Status: baseline inventory and candidate acceptance contract. No replacement representation or
algorithm is selected. Generic/concrete proofs still need to be checked for the selected route.

## Baseline semantic obligations

The source of truth is `crates/behavior-models/proofs/safety/` and the exact expected types generated
in `crates/behavior-models/src/safety/certify.rs`. This list maps semantic obligations, not merely
names that could accidentally refer to weaker propositions.

| Baseline audited root | Required meaning / expected contract |
| --- | --- |
| `Safety.FieldOwnership.sound` | `OwnershipGuarantee`: complete effect projection and per-operation authorization imply an authorized witness for each protected change, for arbitrary finite typed states. |
| `Safety.LifecycleDependency.sound` | `LifecycleGuarantee`: the restricted ordinary operation preserves reference existence and nonterminal-child/permitted-parent safety from a valid source state under the stated permitted-operation premises. |
| `Safety.LifecycleDependency.conjunction` | `LifecycleConjunctionGuarantee`: every configured dependency is preserved independently, including dependencies sharing a parent. |
| `Safety.LifecycleDependency.projection_frame` | `ProjectionFrameGuarantee`: mechanically irrelevant operations leave the dependency projection unchanged. |
| `Safety.membership_order_equivalent` | `MembershipOrderGuarantee`: accepted typed membership order/duplicate evidence has the declared set meaning without rewriting authored Core expressions. |
| `Safety.checkLifecycle_sound` | `LifecycleCheckerGuarantee`: all relevant operations, binding/guard premises and the union operation restriction imply the finite lifecycle premises. |
| `Safety.classify_permitted` | `ClassificationGuarantee`: each accepted projected operation is permitted under actual source-guard semantics and Core side conditions. |
| `Safety.lifecycle_module_preserves` | `LifecycleModuleSoundness`: checked finite module input implies linked lifecycle premises and conditional preservation for every action/dependency. |
| `Safety.checkModule_sound` | `CheckedAuthority`: every operation of every action satisfies every applicable ownership policy. |
| `Safety.module_preserves` | `ModuleSoundness`: checked finite input establishes the complete generic ownership guarantee. |
| `Concrete.checked` | The exact complete concrete request satisfies the finite module checker; a theorem about `True` or a subset is insufficient. |
| `Concrete.certified` | Exact module/inventory/policy and payload identities plus complete ownership and lifecycle guarantees, under explicit correspondence assumptions. |

## Candidate obligations

1. A changed representation must have a defined interpretation and a machine-checked bridge to
   the preserved guarantees. Current nested payload chunking and exact concrete target syntax may
   change; exact semantic binding and every obligation above may not disappear.
2. A checking relation must be sound for arbitrary proposed data: accepted data implies the full
   required guarantee. The proof contains no assumption that the producer/search algorithm is correct.
3. Independently establish complete admitted action/assignment/create/remove occurrence coverage and
   the full caller-expected policy set. Neither the producer's inventory nor a prior success flag
   defines the expected request. Auxiliary action indices must preserve original operation bindings.
4. A uniqueness method checks original full keys and multiplicities. Non-strict ordering, set
   equality, equal length or a truncated/digest-only comparison is insufficient. A permutation
   theorem and checked uniqueness can transfer the original premise; no hash collision-freedom
   axiom establishes the concrete uniqueness check.
5. Permission changes must imply the original `allowed` rule independently for each operation kind
   and every policy. Overlapping restrictions intersect. Authoring duplicates/empty permissions and
   no-op/unreachable assignments retain their v1 rules.
6. Preserve every relevant guard, typed owner association and effect projection. All affected
   dependencies are checked even when the same single operation affects multiple footprints.
   Source-state guards cannot justify unaccounted simultaneous relevant effects.
7. Concrete type checks and transitive axiom audit cover all mapped obligations plus selected
   checking/refinement bridges. Only `propext`, `Classical.choice`, `Quot.sound` are permitted.
   Fresh pinned-kernel replay covers the complete actual dependency closure. No omitted row,
   native-evaluation axiom, custom axiom, proof hole or cached approval can satisfy a check.

The selected design records exact new root/type mappings and checker/statement/evidence versions
before integration. Its producer algorithm may remain unverified; an independent valid witness is
accepted on its properties, not its producer's identity.

## Explicit trusted boundaries and exclusions

- Core admission, ordinary atomic execution, freshness and complete resulting-state Ref integrity.
- Rust extraction, its separate Core inventory comparison, policy resolution, canonical guard
  probes and encoding into the interpreted finite model. These are tested implementation bridges,
  not established as formally verified merely by Lean identity equalities.
- Pinned kernel/auditor/toolchain/build infrastructure and cryptographic binding of Core identities.
- Host/backend completeness and consistency. Genesis validity, migration safety, approved modules
  and exact activation continuity are separate obligations, never discharged by transition proofs.

A checksum identifies evidence; it is not a proof of correctness. A failed or exhausted obligation
never certifies. An invalid proposed witness does not prove the module unsafe or that no witness
exists. A completed over-budget proof can be valid while practical acceptance fails.

## Required correspondence and integrity controls

Pair valid controls with omitted action/operation/policy, duplicated/missing keys, wrong comparator
law, stale identity, wrong index/binding, unauthorized role, one-denying overlapping policy and
missing/weakened lifecycle guards. Mutate real compiled declarations for proof holes, forbidden
axioms and weakened final types; confirm the auditor imports the mutation rather than the parent.
All existing valid v1 constructions remain target coverage. Record which completeness properties
are proved and which are only empirically demonstrated under a fixed capacity profile.

## Integrated statement-2 root mapping

All twelve baseline semantic obligations retain their original predicates and theorem types.
`Concrete.checked` is assembled from independently checked uniqueness, bounds, authority,
lifecycle link and lifecycle conditions; `Concrete.certified` still includes exact identity/payload
coordinates plus both original module guarantees. Only payload layout changes to an exact UTF-8
interpretation, with independent expected anchors regenerated by the verifier.

Four additional audited roots:

| Root | Intended obligation |
| --- | --- |
| `Safety.Candidate.traceChecked_sound` | For any original full keys and any trace, acceptance implies original multiplicity-sensitive uniqueness. |
| `Safety.Candidate.compactPayload_flatten` | The new payload's flattening is the exact UTF-8 byte sequence. |
| `Safety.Candidate.literalBytes_injective` | Equality of interpreted bytes implies equality of Lean strings. |
| `Safety.Candidate.checked_from_parts` | All original finite premises reconstruct the unchanged `checkModule` condition. |

Actual merge replay/permutation and strict comparator/transitivity lemmas are transitive proof
dependencies of the trace root and therefore axiom-audited and fresh-replayed too. Their producer
is not assumed correct. The existing trusted Core/Rust/extraction/encoding/probe boundary remains.
