# Safety Model reference cases

Behavior 0.13.1 introduces these APIs on exact Core 0.15.0 (verifier 0.10.0).
All examples use the public Python API and lower to ordinary Core IR.

| Case | Acceptance contract |
| --- | --- |
| `fo_field` | Exclusive structural field permissions; creation/removal checked separately. |
| `fo_state_machine` | FieldOwnership composes with existing StateMachine ownership and preserves IR/receipts. |
| `ld_parent_child` | Immutable required Ref, literal lifecycle transitions, original R ∧ P. |
| `ld_shared_parent` | Two child types, one shared parent operation, independent obligations for both. |
| `ld_vm_resource` | Restricted VM-owned Resource fixture; nonterminal Resource needs a VM neither Removed nor Abandoned. |

```bash
nix develop -c maturin develop
# Lowering and ordinary Core execution require no proof tools:
nix develop -c python -m models.examples.safety_models.run --output target/safety-models/examples-lowered
# Explicit proof checking: five cases, Rust preflight and six-case comparison:
nix develop -c scripts/check-safety-proofs.sh
```

The runner compares IR and existing receipts with Safety policies omitted, rejects an extra
unauthorized writer, executes actual Core transitions, and writes `summary.json`. For each LD
case it separately reports valid exact-history state validation, a known invalid base and an
unavailable base (Unestablished). Requesting `--certify` issues conditional module certificates
only after generic/concrete proofs, axiom audit and fresh rechecking pass. A module certificate
continues to say initial validity Unestablished: the separate state record does not mutate it.
Missing tools, failures, Unsupported or Inconclusive required cases fail the mandatory gate.
All cases are reported, including NotRun when an earlier preflight prevents their execution.

## What the evidence means

| Evidence | Establishes | Boundary |
| --- | --- | --- |
| Generic Lean FO/LD theorems | Abstract ownership and inductive lifecycle preservation for arbitrary finite states | Versioned definitions/theorem statements must express intended property. |
| Concrete Lean checking | Extracted finite module satisfies theorem premises | Rust must faithfully extract every actual action/effect, binding and guard. |
| Structural ownership checks | Unauthorized writes/creation/removal rejected in final module | Trusted Rust correspondence, including generated actions. |
| Exhaustive small-state and mutation tests | Observed agreement with actual Core transitions; essential premises matter | Finite implementation evidence, not a universal theorem. |
| Snapshot validation | Original R ∧ P holds at exactly one acquired full HistoryRef | Trusted backend completeness/provenance and Rust predicate evaluator. |
| Host protocol | Approved module/policy segment begins at validated state without an intervening write | All writers, migrations and activation must obey host coordination. |

Only standard logical axioms are allowed. Hashes establish identity, not mathematical validity.
A fresh bundle verification regenerates fixed trusted sources and proofs; imported status flags,
supplied Lean source and compiled objects grant no authority. `leanchecker --fresh` uses Lean's
kernel in a fresh process. This is not an independently implemented kernel or full verification
of Rust-to-Core correspondence. INCONCLUSIVE is never proven.

## Runtime and deployment

[deployment.py](deployment.py) gives a single-coordinator host example. Verify an incoming
certificate, acquire the host's exclusive boundary, validate the exact current full HistoryRef
under the incoming BehaviorHash and complete policy set, recheck that anchor and activate while
all writers remain excluded. Core conflict handling still requires re-evaluation. An earlier
snapshot is insufficient, even if both old/new modules certify independently. Command-only
commits may preserve StateId but advance history. An unlocked full-head comparison also races.

The host example is ephemeral authorization/configuration, not Safety Model runtime semantics.
It cannot constrain uncoordinated backend handles; real multi-process hosts need equivalent
transactional/distributed enforcement. The Core Store accepts other same-schema admitted modules
unless the host/trusted authorizer restricts them. Migrations deactivate the example segment and
require fresh validation/reactivation. Core migration validity alone does not preserve R ∧ P.
A segment's argument starts at its anchor and makes no retroactive safety claim.

The example consumes a process-local checked certificate to demonstrate the boundary. Production
hosts can obtain the approved module/policy tuple from a trusted offline build/authorization
channel instead; that channel's integrity is an explicit host responsibility. This example
does not implement distribution, signing or a new governance protocol. Loading unsigned JSON
never establishes mathematical proof or approval. Store execution/replay need no Lean process.

Safety Models add no persistent business state, ownership/relationship registry, event stream,
execution semantics or replay dependency. All five histories replay with policy-free Core IR,
without invoking Lean or Safety analysis. Missing evidence may prevent an audit of guarantees;
it never prevents deterministic Core reconstruction.

## Deliberate v1 limits

- Required nonoptional Ref owner, immutable for the child's entire existence. Terminal children
  still need an existing parent; physical deletion with surviving references is blocked by Core.
- Literal enum destinations and documented canonical mandatory roots only.
- At most one relevant operation across all configured dependencies; mixed parent/child roles
  and simultaneous relevant effects remain Unsupported, even when apparently safe.
- Genesis validity, migration safety, approved-module enforcement, physical execution, liveness
  and complete end-to-end proof correspondence are not supplied by the transition certificate.
- The VM fixture is not a production data-model prescription. Pool-owned resources and other
  ownership schemes remain valid application requirements outside this restricted pattern.
- **I-5b and I-6 remain unmapped** until authoritative definitions and a formal implication or
  equivalence analysis exist. `live` is not assumed equivalent to `nonterminal`. E3 scoped
  cardinality and E4 cleanup obligations remain outside Feature 510.

Proof checking is intentionally a separate, comparatively expensive build step. It stays outside
the existing WorkQueue quick-mode runtime budget and outside Store execution/replay.


## Core verifier and Safety certifier

Run the comparison through the existing reference runner:

```bash
nix develop -c python -m models.examples.safety_models.run \
  --guard-contract --certify --output target/safety-models/guard-contract
```

It writes `guard-contract.json` with all six exact BehaviorHashes, a nonempty query-sensitive
postcondition profile, canonical Core attestations/check subjects, actual solver outcomes and
separate Safety assessments/evidence identities. The mandatory proof gate requires this complete
comparison alongside the original five certificates. Early failures retain explicit unrun rows.
Ordinary lowering mode remains available without proof tools.

With pinned Core 0.15.0 / verifier 0.10.0 and profile `postcondition`, the reference has these
observed local results:

| Guard variant | Core local postcondition | Safety v1 |
| --- | --- | --- |
| Canonical whole root | proven | Certified under assumptions |
| Same set, reversed members | inconclusive (`counterexample_not_reproduced`) | Certified under assumptions |
| Equivalent `all_` form outside profile | inconclusive (`counterexample_not_reproduced`) | Unsupported |
| Guard absent | counterexample | Unsupported |
| Guard on another parent | counterexample | Unsupported |
| Required expression under insufficient OR | counterexample | Unsupported |

The Core check asks whether the declared source conditions establish a particular local
postcondition. Safety checks the restricted generic inductive pattern and complete transition
inventory under named assumptions. A Core counterexample is to that local check; the runner
also records its evaluation trace, including postcondition refusal. INCONCLUSIVE remains
unproved. Equivalent membership order can therefore certify even while this Core query profile
is inconclusive. The canonical form above is the demonstrated recommendation for both tools.

For every lifecycle operation, diagnostic stage and raw member-order rule, use the
[authoring contract](../../README.md#lifecycle-guard-authoring-contract-checker-2). In particular,
**Planned → Created** and **Terminal → Active** both require owner association and the permitted
parent guard when their destination is nonterminal, even though owner is unchanged. The executable
`child_transition_examples()` in [guard_contract.py](guard_contract.py) builds guarded and
unguarded versions and reports the exact analysis issues.

Repetitions are truth-preserving under Core membership but retain their authored hashes.
Certification checks exact typed sets and whole roots; it does not accept arbitrary equivalent
expressions or strengthen the mathematical claims beyond the documented trust boundaries.
