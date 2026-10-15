# Implementation Plan: Approval Model

**Branch**: `503-approval-model` (stacked on `502-request-flow`, PR #17) | **Date**: 2026-10-13 |
**Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/503-approval-model/spec.md`

## Summary

Approval is a new model kind in the existing `behavior-models` crate:

```text
L_Approval(AM) = Compose( Decl(AM), Defs(AM), Actions(AM) )
  Decl    = { C = E·"DecisionChoice", X = E·"Decision" }     E = EncodeEntityName(p)
  Defs    = { p__approve_count, p__reject_count, p__approved, p__rejected, p__pending }   (Core derived values)
  Actions = { p__approve, p__reject }:  pre = Open, Pending, Eligible, ¬HasDecision, G_c
                                        Δ   = CreateEntity(X) + commands
                                        post = child-state uniqueness and threshold consistency
```

This is the first model that combines all of these:
- generated declarations that depend on another model's generated declarations;
- generated queries;
- cross-entity laws;
- concurrency.

Three architectural results come from research:

1. **Concurrency** (R1, R2). Core's stated persistence contract gives whole-state optimistic
   concurrency (`STATE_CONFLICT` on a stale parent). Query facts with complete memberships are
   re-derived at the parent.
2. **Cross-entity validity** (R4). Core module invariants cannot express grouped laws; Core
   documents this as deliberate. Both laws are therefore child-state checks of the decision
   actions, and are proved globally by **inductive preservation** under structural ownership.
   They are not described as a workaround. The user decided this on 2026-10-13. The base-state
   assumption is explicit.
3. **Declaration DAG** (R7). 502's P2 becomes a topological pass over model declarations,
   which is common infrastructure. Generated derived values are added in P2 so that every
   later phase can reference them (R10).

**Planning acceptance criterion**: the concurrency proof sketch at the end of this plan holds
using only Core's stated contracts, and the declaration DAG and ownership are common,
structural and regression-free. Both are established below.

## Technical Context

**Language/Version**: Rust stable (edition 2024, pinned toolchain); Python 3.13 binding.

**Primary Dependencies**: `behavior-engine` (Core 0.13.0, pinned `1509497`, since the rebase onto 504), `serde_json`,
`thiserror`; dev only `proptest`; binding `pyo3` 0.26. No new dependency.

**Storage**: N/A for the model. The concurrency tests use Core's public `Store` with
`InMemoryBackend`.

**Testing**:
- `cargo test -p behavior-models`: unit, golden, property, regression and DAG tests;
- pytest: end-to-end through the public engine and Store, and the decision-sequence
  properties;
- the `models compat` gate;
- the determinism check.

**Target Platform / Project Type**: unchanged (a library plus a Python extension).

**Performance Goals**: none beyond the gate run. Property suites use ≥ 1,000 cases. Query cost
is Core's: indexes are transparent, so `Semantics_indexed = Semantics_scan` (R3).

**Constraints**:
- no Core change;
- `behavior-engine` only;
- no Wire JSON as an interface (`into_wire` additionally allowed only in `approval/lower.rs`);
- deterministic outputs;
- 501 and 502 pinned hashes unchanged (D9).

**Scale/Scope**:
- about 7 new Rust source files (`approval/`) and a generalized `declare_models`;
- `error.rs` codes, the Python glue, one new Python module, fixtures and the gate-script
  tuple.

## Constitution Check

*GATE: checked before Phase 0 and re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Deterministic Core | No AI. Lowering is deterministic, and the outcome is derived by Core expressions. ✅ |
| II. Validated outputs | Binding input is untrusted: typed resolution, typed errors. ✅ |
| III. Test-First | Tests first in every phase, with property tests for both laws, terminal stability and soundness (`proptest`, seeded Python sequences). ✅ |
| IV. Reproducibility | Canonical orders (fields, parameters, capabilities, derived definitions); the topological order is a function of the model set (D7); compat tuple for `approval`. ✅ |
| V. Explicit state, auditability | Decisions are explicit immutable records; the outcome is a named Core rule; receipts map predicates, guards and checks to Core items. ✅ |
| VI. Simplicity | No new crate or dependency. The only new common abstraction is the DAG pass, which the spec requires. No Approval runtime and no query engine: Core derived values. ✅ |
| Bindings: no core semantics | Python records input and holds typed handles. Thresholds, counts and names are all Rust's. ✅ |
| `behavior-engine` only | Unchanged. The `into_wire` exception is extended to one file for the creation input type (R8). ✅ |
| Gates = `scripts/gates.sh` | Unchanged set; `models compat` gains the `approval` tuple. ✅ |

**Result: PASS** before and after design. Complexity Tracking is empty.

## Planning gates (spec FR-029)

| # | Gate | Result | Evidence |
|---|---|---|---|
| 1 | enum and entity through the public engine | PASS | `declare_enum`, `declare_entity` (R6) |
| 2 | model declarations depend deterministically on model declarations | PASS | DAG pass (R7, contracts/declaration-dag.md) |
| 3 | uniqueness with existing query and validity semantics | PASS *as decided* | not a module invariant (Core has no grouping); child-state check plus inductive preservation (R4) |
| 4 | counts with Query semantics | PASS | `count(where(select(X), …))` (R3) |
| 5 | the guards observe exactly the needed query facts | PASS | complete memberships recorded and re-derived (R2) |
| 6 | stale concurrent commits cannot both succeed | PASS | stated contract: whole-state concurrency, `STATE_CONFLICT`, no rebase (R1) |
| 7 | no raw Wire JSON | PASS | `into_wire` only as the creation input type (R8) |
| 8 | ownership detectable in final composition | PASS | `creates()`, `effects()` and `removes()` over the admitted module (R5) |
| 9 | the predicates are usable as RequestFlow and StateMachine guards | PASS | Core derived values over the subject entity, added in P2 (R3, R10) |
| 10 | no Core change | PASS | none needed |
| 11 | every fact behind Pending, HasDecision, ApproveCount and RejectCount is in the evidence or concurrency validation | PASS | R2: query memberships are facts; whole-state CAS subsumes read-set validation |

The stop gate was triggered by gate 3, classified (genuinely unrepresentable as a *module-wide*
invariant: Core spec 007 makes grouping out of scope), and resolved by decision without a Core
change.

## Project Structure

### Documentation (this feature)

```text
specs/503-approval-model/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/
│   ├── approval-contract.md     # meaning, runtime behavior, rules, Python API, reference cases, guarantees
│   └── declaration-dag.md       # G_D, topological P2, definitions, laws D7–D9, D4′
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code

```text
crates/behavior-models/src/
├── composition.rs        # MODIFIED: Contribution = Enum | Entity; topological merge (DAG, cycle); definitions; D4′
├── kinds.rs              # MODIFIED: AnyModel::Approval; Needs/Contrib per kind; compose dispatch; receipts
├── error.rs              # MODIFIED: INVALID_THRESHOLD, DECLARATION_CYCLE, DEFINITION_MISMATCH, UNAUTHORIZED_DECISION_*
├── lib.rs                # MODIFIED: approval versions, re-exports
└── approval/             # NEW: all approval-specific code
    ├── mod.rs
    ├── input.rs          # ApprovalInput, CapabilityInput, DecisionFragments
    ├── resolve.rs        # → ResolvedApproval; names; schema; params; every resolution error
    ├── decls.rs          # Contrib (C, X), Needs ({S, A}), definitions (the five derived values)
    ├── lower.rs          # decision actions (the only approval file allowed into_wire); backstop unit test
    ├── ownership.rs      # create / write / remove rules over the admitted module
    └── identity.rs       # canonical document, receipt, source map
crates/behavior-models/tests/
│   approval_resolve.rs, approval_lower.rs, approval_ownership.rs, approval_identity.rs,
│   declaration_dag.rs, approval_laws.rs (proptest)
crates/behavior-py/src/models.rs   # MODIFIED: approval input, decision-scope tracer, definitions handles
python/behavior/approval.py        # NEW: Approval API, decision_entity handle, predicate handles
python/behavior/module.py          # MODIFIED: nothing new in order (P2 already exists); passes approvals
python/tests/test_approval*.py     # NEW: thresholds, terminality, store concurrency, composition, properties
python/tests/test_model_staging.py # MODIFIED: regression D9 over every pinned 501/502 case (builders are Python-only)
models/fixtures/reference_cases.py # MODIFIED: ap_* cases
models/fixtures/compat.json        # MODIFIED: new `approval` tuple (existing tuples byte-identical)
scripts/check-public-surface.sh    # MODIFIED: into_wire also allowed in approval/lower.rs
scripts/check-terms.sh             # MODIFIED if needed: DENIED/REJECT/REJECTED vocabulary check in approval tests
api/public-api.json, models/README.md  # MODIFIED: Approval
```

**Structure Decision**: a module inside the existing crate, like `request_flow/`. Approval
vocabulary stays out of kind-neutral files (`composition.rs`, `identity.rs`, `source_map.rs`,
`ids.rs`). The 502 neutrality test is extended to approval terms.

## Implementation phases (tests first in each)

1. **DAG infrastructure**:
   - `Contribution` gains `Enum`;
   - `Needs` and `Contrib` per kind;
   - the topological merge with `DECLARATION_CYCLE`;
   - definitions and D4′;
   - **the D9 regression test against every 501 and 502 manifest case comes first**.
2. **Approval resolve**: names, schema, parameters, thresholds, every resolution error.
3. **Declarations and definitions**: `C`, `X`, the five derived values; a golden test of their
   Core bodies; P2 visibility from hand-written behavior.
4. **Decision lowering**: scope protocol, precondition positions, the create, the postconditions,
   the initializer checks; the backstop unit test.
5. **Ownership**: the three `UNAUTHORIZED_DECISION_*` rules; reads and queries accepted.
6. **Identity and provenance**: canonical document, receipt, source map.
7. **Python**: `Approval`, handles, tracer; end-to-end tests including `Store` concurrency.
8. **Compat and determinism**: `ap_*` cases, the `approval` tuple, gate tests.
9. **Properties and docs**: decision-sequence properties, README, the terms check.

## Test plan (spec regression list → tests)

| Requirement / law | Test | Kind |
|---|---|---|
| 1–3, SC-002 | thresholds (1,⊥), (2,⊥), (2,1), (3,2), (1,1): the outcome after each step equals the definition | integration (Py) |
| 4–5 | the same actor: second approve; approve then reject → DENIED, counts unchanged | integration (Py) |
| 6–7 | `k_R = 1` veto; `k_R = ⊥`: rejections counted, outcome PENDING | integration (Py) |
| 8, SC-003 | ≥ 1,000 random sequences (actors, choices, thresholds, open and eligible): never both thresholds, never a duplicate pair, terminal stable, soundness | property (Py, seeded) |
| 9–10 | any decision after APPROVED or REJECTED → DENIED | integration (Py) |
| 11–13 | Open false, Eligible false → DENIED; Eligible evaluation error → Core `EVALUATION_ERROR` | integration (Py) |
| 14 | eligibility made false after a decision (the requester changes) → the decision still counts | integration (Py) |
| 15–17, SC-004 | hand-written write of each decision field, removal, creation → `UNAUTHORIZED_DECISION_*` | negative (Rust + Py) |
| 18 | a reused `decision_id` → Core's identity error | integration (Py) |
| 19 | an actor id with no entity → recorded | integration (Py) |
| 20 | different payloads, same choices → equal counts and outcome | integration (Py) |
| 21–22 | a command on approve: emitted only on commit; a DENIED attempt emits nothing | integration (Py) |
| 23–24 | RequestFlow `finalize_approved` / `finalize_rejected` guarded by the predicates; status IN_REVIEW while the outcome is APPROVED | integration (Py) |
| 25 | two approvals with `approve` / `reject` → four distinct capabilities, independent outcomes | composition |
| 26–27, SC-006 | payload, parameter and model-supply permutations (≥ 1,000) → equal ApprovalHash, declarations, module, BehaviorHash | property |
| 28 | approval over a RequestFlow entity in every supply order → the flow entity is declared first | DAG (Rust) |
| 29 | mutual subjects → `DECLARATION_CYCLE` with the same models in every order | DAG (Rust) |
| 30, SC-008 | the module has no Approval constructs; admitted by the public engine | golden + integration |
| Layer 2 | two decisions from one parent via `Store`: second → `STATE_CONFLICT`; retry → DENIED | integration (Py, Store) |
| Layer 3 | postconditions alone deny both-thresholds and duplicate children | unit (Rust, backstop) |
| Inductive step | for random valid `S` and a random decision action, `P(S) ∧ ALLOW ⇒ P(S′)` | property (Rust or Py) |
| SC-005 | subject fields equal before and after each successful decision | integration (Py) |
| Other direction | `ap_document_review` with no RequestFlow | integration + compat |
| D9, SC-007 | every 501 and 502 manifest case reproduces its pinned hashes | regression |
| D5 | hand-written action with `Id[approval.decision_entity]`, a read over `X`, a guard using `approval.approved(s)` | integration (Py) |
| Naming | `C(p)`, `X(p)` injective and disjoint (proptest over the id grammar); cross-kind collision → `DECLARATION_COLLISION` | property + negative |
| Terminology | approval test files and diagnostics use DENIED for refused attempts (terms check) | script |

## Anti-pattern audit

| Anti-pattern | Prevented by |
|---|---|
| Approval moves the subject's status | `Δ = CreateEntity(X)` only; no effects are expressible; SC-005 test |
| Stored, mutable outcome | the outcome exists only as derived rules; no field holds it |
| D(s) as "currently eligible decisions" | D(s) is a plain `where subject == s.id`; Eligible appears only in the preconditions |
| Implicit authorization (actor, requester, assignee, roles) | the actor is an authored expression; Eligible and guards are authored; a parameter named `actor` has no meaning; a test shows an unrestricted approval accepts anyone |
| An Approval query engine or runtime | Core derived values and queries only |
| A correctness dependency on indexes | Core FR-016: indexes are transparent |
| A model choosing its own declaration order or names | the DAG pass and merge are common; no rename |
| Python deciding semantics | thresholds, names and counts come from Rust; handles are drift-checked |
| Changing 501 or 502 outputs | the D9 regression test; the compat gate |
| A workaround for Core | none: the R4 decision uses ordinary Core postconditions, proved by induction |

## Semantic ownership

| Fact | Owner |
|---|---|
| Decision entity `X`: creation, every field, ordinary removal | Approval `p` |
| Choice enum `C` | Approval `p` (generated declaration) |
| Outcome, counts | derived: Core rules `p__*` (owned by none, written by none) |
| Subject entity, fields, lifecycle | not Approval (authored, or RequestFlow / StateMachine) |
| Actor entity | not Approval |
| Subject status transitions | the subject's StateMachine (for example RequestFlow's embedded machine) |
| Uniqueness and threshold consistency | Approval laws, enforced by its actions' child-state checks + ownership |
| Stale-commit prevention | Core Store (whole-state concurrency) |

## Phase ownership

| Phase | Owner |
|---|---|
| Authored enum and entity declarations (P1) | ordinary Behavior API |
| Model declaration DAG, topological merge, collisions, cycles (P2) | common model infrastructure |
| Choice enum and decision entity contribution | Approval (`decls.rs`), merged by common infrastructure |
| Derived outcome definitions (P2, after declarations) | Approval contributes, common infrastructure adds |
| Commands (P2b) | ordinary Behavior API |
| Hand-written tracing (P3) | Behavior authoring |
| Decision action lowering (P4) | Approval |
| Final ownership checks | common composition pass + Approval rules |
| Concurrency, evidence, child-state evaluation, runtime meaning | behavior-engine / Core |

## Concurrency proof sketch

**Setting.** `S₀` is a valid state (`P(S₀)`) with `Outcome_S₀(s) = PENDING`, for example
`A(s) = k_A − 1` and `R(s) = k_R − 1`. Two candidates are evaluated from `S₀`:
- `C_A`, actor `a₁` approving `s`;
- `C_R`, actor `a₂` rejecting `s`.

Both see `Pending(s)` and `¬HasDecision`, so both evaluate to ALLOW with children `S_A` and
`S_R`. Each candidate's record holds its query facts (complete memberships of `D_A(s)`,
`D_R(s)` and the `HasDecision` query) observed at `S₀`.

**Case 1: stale concurrent commit.** `C_A` commits first: `Commit(C_A, S₀) = Success`, and the
head becomes `S₁ = S_A` with `Outcome_S₁(s) = APPROVED`. Now `Commit(C_R, S₀)` is attempted.
- By Core's stated contract (005 FR-007 and acceptance scenario 2; persistence.md:37-39, 58), a
  commit is accepted only when its expected parent is the current head.
- `Head = S₁ ≠ S₀`, so `Commit(C_R, S₀) = STATE_CONFLICT`, and nothing is applied.
- There is no rebase: `evaluated_against == committed_on` by contract.
- The check depends neither on the candidates touching the same entities nor on a read set.

So no execution composes `C_A` and `C_R` onto one parent.

**Case 2: fresh evaluation on the new head.** The host retries `C_R` by re-evaluating from
`S₁`:
- `Outcome_S₁(s) = APPROVED`, so `p__pending(subject)` is false;
- the precondition fails, and `C_R` is DENIED;
- `CanReject(s, a₂) = false`, and no decision is created.

The same holds for any decision on `s` from `S₁`, by terminal stability.

**Case 3: any other ordinary execution path.** Suppose some ordinary transition `τ` from a
valid `S` would produce a child `S′` that violates `P`. Then:
- If `τ` is not an Approval decision action, structural ownership (final composition) says `τ`
  does not create, write or remove any decision. So `D_S′ = D_S`, and `P(S′) = P(S)` holds.
  That is a contradiction.
- If `τ` is a decision action for `(s*, a*)`, it changes `Count(·,·)` only at `(s*, a*)`, and
  `A`, `R` only at `s*`. Its postconditions evaluate exactly those groups on `S′`:
  `Count_S′(s*, a*) ≤ 1` and `¬(A_S′(s*) ≥ k_A ∧ R_S′(s*) ≥ k_R)`. If `S′` violated `P`, it would
  violate one of them, so Core DENIES `τ` (postcondition failure; only ALLOW is committable,
  store.rs:1790), and `S′` is never committed.

The validity check is independent of the guard. Case 3 does not use the preconditions, so it
holds even if a guard were wrong or bypassed. A unit test checks the postconditions alone.

**Uniqueness under a parallel same-actor submission** (`a₁` submits two approvals with
different `decision_id`s from `S₀`):
- Case 1 refuses the second commit;
- Case 2 DENIES it on the new head through `¬HasDecision`;
- Case 3 refuses any child with `Count(s, a₁) = 2` through the postcondition.

**Evidence.** Gate 11: the guards read `HasDecision`, `ApproveCount` and `RejectCount` as query
facts with complete memberships, and the commit re-derives them at the parent. Because
concurrency is whole-state, no commit can rest on a state other than the one its facts were
read from.

**Conclusion.**

```text
fresh second decision    → DENIED by the terminal guard (or ¬HasDecision)
stale concurrent commit  → STATE_CONFLICT by Core's whole-state concurrency contract
invalid child state      → DENIED by the action-scoped validity check; with ownership, P(S) ⇒ P(S′)
```

By induction from a valid base state: `P(S₀) ⇒ ∀ n: P(S_n)`. Two concurrent decisions therefore
cannot produce a state where both APPROVED and REJECTED hold, nor a duplicate decision.
Genesis seeds, imports and migrations that introduce decisions lie outside ordinary execution
and must establish `P` themselves (contract §6).
