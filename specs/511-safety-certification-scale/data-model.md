# Build-Time Data Model

**Authority**: [spec.md](spec.md), [plan.md](plan.md). These are conceptual typed contracts;
wire names/encoding are selected at G1. None of these records adds application runtime state.

## Entities and constraints

| Entity | Required content and invariants | Relationships |
| --- | --- | --- |
| Expected request | Exact admitted module/BehaviorHash and complete caller-expected policy set; producer data cannot redefine either. | One authoritative request per certification/check attempt. |
| Prepared input | Full action identities, every assignment/create/remove occurrence, entity/field/binding coordinates, resolved policies and guard premises. Independent comparison to admitted public Core inventory is mandatory. | Derived from request, never accepted solely from evidence. |
| Component/selection record | Alternatives, proved guarantees, laws, bridge obligations, measured costs, dependencies, trust effects, selected representation and version mapping. | G1 approves one route before production integration. |
| Proposed witness | Untrusted bounded data only; no producer approval flag has authority. Representation selected only after G1. Validity depends on the full expected request. | Zero or more proposals per claim; different valid proposals may have different artifact identities. |
| Unverified evidence | Declared contract, exact claim identities, proof/checker/toolchain/source identities, obligations and assumptions; loading never grants certificate authority. | Transport associated with expected request, not a replacement for it. |
| Checked certificate | Opaque authority issued only after independent binding/coverage, all mathematical obligations, intended types, axiom audit and fresh replay succeed. | Refers to exact claim and checked evidence. |
| Evaluation profile | Fixed acceptance conditions: platform/allocation, approved Core/proof-toolchain constraints, measurement version, semantic corpus/generator identities, finite stage limits and separate positive total cold budgets. No future candidate package/checker/source identity. | Draft -> Frozen; amendments create a reviewed revision retaining earlier results. |
| Build manifest | Exact artifact checksum, source tree, release/checker/statement/evidence versions, proof/dependency identities, actual Core/toolchain derivations and per-build expected request identities. | Separate immutable record for baseline/prototype/candidate builds; final candidate frozen before acceptance runs. |
| Workload case | Public neutral construction inputs, expected action/operation/policy counts and identities, six actual dimension values, expected language classification and mathematical premises. | Frozen corpus contains A256/A512 plus mixed/generated cases and controls. |
| Attempt observation | Request/config/witness identity, acceptance-profile hash and exact build-manifest hash, cold/warm conditions, commands, timestamps, whole-chain wall/CPU/memory scope, stage records and usability result. | Operational record separate from semantic evidence; every attempt identifies both conditions and actual build. |
| Stage observation | Stage/status, actual applied limits and scope, diagnostics, measured wall/CPU/peak-memory method; unavailable heartbeat/depth consumption explicitly absent with reason. | Includes failed and unrun stages, not just successful processes. |
| Final assessment | Soundness, language compatibility, proved completeness conditions and empirical capacity separately, residual trust and causal explanation. | References exact accepted/failed observations and formal obligations. |

## States and transitions

- Proposed evidence -> parsed unverified data -> independent expected-request comparison ->
  complete proof/integrity checking -> certificate. No transition skips an obligation.
- Operational obligation states: Pending, Passed, Failed, Unsupported, Exhausted, NotRun.
  These describe observations, not a mandated new public enum. Preserve current public outcomes
  unless an explicitly reviewed version change is necessary.
- Interrupted required checking yields no certificate, even if earlier components passed.
  Unsupported input, violated premise, invalid witness, resource exhaustion and established
  counterexample must remain distinguishable. An invalid witness does not prove module unsafety.
- Usability result is Passed, Failed or Unmeasured independently of mathematical completion.
  A completed proof over budget may remain valid while failing the feature's performance criterion.
- A Frozen profile is immutable by identity. A changed budget/corpus/environment creates a reviewed
  revision and forces complete final reevaluation; no overwritten failed run or passing-only corpus.
- Version and package changes precede candidate freezing. A later relevant artifact/source/proof/checker
  change creates a new build manifest and requires complete final reevaluation. A materially changed
  measurement method requires a reviewed profile revision and comparable baseline observations too.
- Delivery checks must match the evaluated candidate artifact. A new build manifest does not permit
  changing acceptance budgets or excluding frozen cases.

## Identity and reproducibility

Semantic claim identity binds exact module, complete policies, statement/interpretation, theorem,
checker and toolchain dependencies. Evidence identity additionally binds the checked representation
and proposed witness where applicable. Producer inputs/configurations necessary for reproduction
are recorded. Different valid witnesses need not have the same artifact identity.

Timing, process paths, sampled memory and environmental noise are operational fields and must not
contaminate semantic identities. Repeated identical complete inputs in the pinned environment must
reproduce semantic evidence. Established Core/Model identities and published artifacts are immutable.

## Validation before accepting records

A frozen profile cannot contain unknown or placeholder mandatory budgets, unbound fixture identities
or an unspecified machine allocation. No arbitrary maximum corpus size is implied by A512. Actual
finite sweep levels and budgets are pilot-derived deliverables, not values chosen in this document.

Typed parser bounds and exact witness fields are finalized by the G1 selection record; this document
is not a new bundle schema. Existing strict contracts remain strict until a versioned replacement is
implemented. No certificate can be deserialized or inferred from an evidence `status` string alone.

## G1 selected transport

See [design-decision.md](design-decision.md). `Proposed witness` becomes a flat postorder list of
Empty/Leaf/Branch nodes, with earlier child indices, Boolean merge choices and a final root index.
Each nonroot node has one parent. Bounds derive from original action count, not a fixed A512 ceiling.
Its semantic interpretation is Lean's recursive merge trace consuming independently expected keys.
Bundle/evidence v2, checker 3 and statement 2 explicitly separate this contract from historical v1.
