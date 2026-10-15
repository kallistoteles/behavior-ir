# Research: Approval Model

**Feature**: 503-approval-model | **Date**: 2026-10-13 | **Builds on**: feature 502 (branch
`502-request-flow`, PR #17)

All Core facts were read from the pinned Core Release 0.12.0 (`b525da4…`) in Cargo's checkout; since the
rebase onto feature 504 (2026-10-15) the pin is Core 0.13.0 (`1509497…`), see R13.
Nothing in behavior-ir-core was changed. Paths are relative to that checkout.

**Status:** every planning gate passes. R4 stopped planning, was classified, and was decided by
the user on 2026-10-13: action-scoped child-state checks, proved as inductive preservation.

## R1. Concurrency contract (spec FR-029 gate 6): PASS

**Stated contract.**
- `specs/005-persistence-contract/spec.md:45-46`: "Whole-state optimistic concurrency. A commit
  is accepted only when its expected parent is still the store's current state; otherwise it is
  `STATE_CONFLICT`".
- Acceptance scenario 2 (`spec.md:119-122`) is exactly our case: two transitions evaluated
  against the same state; the first succeeds and the second gets `STATE_CONFLICT`. This holds
  even when they touch disjoint entities (`:266-268`).
- `FR-007` (`:325-330`): nothing is applied on a conflict.
- `docs/persistence.md:37-39, 58`: `commit(expected_parent = S, bundle)` is the only concurrency
  primitive, and it is atomic.
- Rebase is excluded by contract. Entity-level rebasing "was clarified away"
  (`005/research.md:282`), and a record satisfies `evaluated_against == committed_on`
  (`persistence.md:47`).

**Implementation (consistent with the contract).** The head check is at
`behavior-store/src/store.rs:1935`. A lost backend race (`CasOutcome::HeadMoved`) becomes
`STATE_CONFLICT` at `:2249`.

**Consequence.** For candidates `C₁`, `C₂` evaluated from the same parent `S`:
`Commit(C₁, S) = Success ⇒ Head ≠ S ⇒ Commit(C₂, S) = STATE_CONFLICT`. This is Core's stated
contract, not an implementation effect.

## R2. Observed evidence of queries (gate 11): PASS

**Stated contract.**
- Evaluation records the facts it observed: existence, identity use, query memberships and
  member field values (`persistence.md:109-117`).
- "a query fact records the instance's complete membership … Records list full memberships
  even for `count`", each with a `result_hash` (`persistence.md:132-137`;
  `specs/007-relational-queries/spec.md` FR-014).
- At commit, the store re-derives the facts at the parent and refuses a bundle whose facts
  differ (`persistence.md:109-117`; `store.rs:2644` `rederive`, `:2716` byte-identical record).

**Consequence.** `HasDecision(s, a)`, `ApproveCount(s)` and `RejectCount(s)` are query facts
with complete memberships, so `Pending(s)` is too. Read-set facts do not decide conflicts today
(007 FR-015). Whole-state concurrency is strictly stronger: every fact is a function of the
parent state, and the parent must be the head. So no commit can rest on stale query evidence:
- if the head moved, the commit gets `STATE_CONFLICT`;
- if it did not, the facts are re-derived at the parent and must match.

## R3. Query representation (gate 4) and derived predicates (gate 9): PASS

Core constructs (public `Builder`):

```text
D(s)         = lambda("where", select(XDecision), d, d.subject == s.id)
D_A(s)       = lambda("where", select(XDecision), d, d.subject == s.id ∧ d.choice == lit(APPROVE))
A(s)         = op("count", [D_A(s)])                  (R(s) likewise with REJECT)
HasDecision  = lambda("any", select(XDecision), d, d.subject == subject.id ∧ d.actor == <actor>)
```

- Queries are allowed in action pre- and postconditions, in derived values and in module
  invariants (`admit/typecheck.rs:1453, 2232, 1344`).
- A lambda body reads the candidate and captured outer values (007 FR-013). Captured values are
  evaluated in the enclosing state.
- Named reusable definitions: `add_derived(name, DerivedKind::Rule | Derived, params, body, …)`.
  Their parameters must be entity types (`admit/resolve.rs:628, 659-675`), so the generated
  definitions take the subject entity, for example `<id>__approved(s: Subject)`. They are
  callable from any scope where a parameter of type `Subject` is bound, which includes a
  RequestFlow transition's `request` parameter.
- `HasDecision` depends on the per-capability actor expression, so it is generated inline in
  each decision action, not as a derived value.
- There is no Approval-specific query engine.
- Indexes are semantically transparent: 007 FR-016, and `Backend::keys_by_field_at` returns
  `None` without an index and the store re-checks every candidate
  (`behavior-store/src/lib.rs:75-90`). So `Semantics_indexed = Semantics_scan` holds by Core
  contract.

## R4. Resulting-state validity of uniqueness and threshold consistency (FR-007, FR-008): DECIDED

**Finding (verified in source).**
- Module invariants (`add_global_invariant`) may use queries, but a query predicate **cannot
  contain another query** (`admit/typecheck.rs:918-925`, `QUERY_NOT_ALLOWED`) and cannot use
  `exists` (`:697-707`).
- Per-entity invariants and constraints may not use queries at all (`:1316`, `queries: false`).
- `unique(q, key)` takes one scalar key (`:1251`). Core has no tuple type and no concatenation
  operator (`wire.rs:260-290`).

So neither law can be a module invariant:

```text
∀ s, a: |{ d | subject_d = s ∧ actor_d = a }| ≤ 1     needs a pair key or a nested count
∀ s: ¬(A(s) ≥ k_A ∧ R(s) ≥ k_R)                       needs a count grouped per subject
```

Core states this limit as deliberate: "Grouped rules such as 'every customer has at most 3 open
orders' are out of scope … an explicit grouping operator may come later"
(`specs/007-relational-queries/spec.md:88-92`, FR-004, FR-024).

**Classification: genuinely unrepresentable semantic requirement** for a *module-wide*
validity rule. This is not a facade gap: Core's semantics has no grouped quantification in
module invariants and no composite keys for `unique`. It is not "existing Core semantics but
missing public API".

**What Core can express today (no Core change).** Action-scoped resulting-state validity:
generated **postconditions** of each decision action. Postconditions are evaluated on `S′` and
may query, with captured values from the parent state:

```text
post_unique   = count(where(select(XDecision), d, d.subject == subject.id ∧ d.actor == <actor>)) ≤ 1
post_terminal = ¬(A(subject) ≥ k_A ∧ R(subject) ≥ k_R)          (only when k_R ≠ ⊥)
```

Together with the structural ownership rules (R5), these give an inductive guarantee:
- Only the generated decision actions create a Decision.
- No action writes a Decision field, and no action removes a Decision.
- A decision action adds exactly one decision, to `D(subject)` of its own bound subject.
- So every ordinary transition leaves every other group `(s', ·)` unchanged, and its own group
  is checked on `S′`.

Hence every ordinary transition preserves both laws. This is not a convention: the
admitted-module ownership pass makes it structural, and the postconditions are evaluated on
the child state by Core.

**What this does not cover.** Paths that are not ordinary actions: a genesis seed
(`Store::create` validates module invariants, not action postconditions) and migrations. A
module invariant would have covered these. A seed or migration could therefore introduce
duplicate decisions or a both-thresholds state. Spec FR-021-adjacent text ("Migration semantics
remain separate"; immutability "covers ordinary behavior only") already scopes Approval to
ordinary behavior, but the clarified principle "no execution path may commit a semantically
impossible Approval state" is stronger than that.

**Decision (user, 2026-10-13).** Use action-scoped child-state checks, stated as an
**inductive preservation proof, not a workaround**:

```text
P_U(S) ≡ ∀ s, a: Count_S(s, a) ≤ 1          P_T(S) ≡ ∀ s: ¬(A_S(s) ≥ k_A ∧ R_S(s) ≥ k_R)
decision action δ for (s*, a*):  (s, a) ≠ (s*, a*) ⇒ Count_S′(s, a) = Count_S(s, a)
                                 s ≠ s*            ⇒ A_S′(s) = A_S(s) ∧ R_S′(s) = R_S(s)
⇒ Count_S′(s*, a*) ≤ 1  ∧  ¬(A_S′(s*) ≥ k_A ∧ R_S′(s*) ≥ k_R)   suffice for P(S) ⇒ P(S′)
any other ordinary action: D unchanged (structural ownership)  ⇒ P(S) ⇒ P(S′)
P(S₀) ⇒ ∀ n: P(S_n) over ordinary reachable states
```

- The base state is explicit. An empty decision relation satisfies `P`. Seeds, imports and
  migrations that introduce decisions are separate semantics, must establish `P` themselves, and
  are not valid Approval states by default.
- No Core proposal is opened. Grouped or module-wide quantification is recorded only as a
  possible future generalization, if further consumers show the same need ("Core changes
  require semantic necessity, not model convenience").

**Alternatives rejected.**
- Deriving the decision identity from `(subject, actor)` so that `unique(by = id)` works. Spec §8
  forbids identity derived from actor or subject.
- A Core feature first (composite `unique` keys or nested queries in module invariants). It is
  not semantically necessary, because the proof above holds without it.
- A per-entity invariant on the decision. Per-entity invariants cannot query.

## R5. Ownership is structural (gate 8): PASS

- The admitted `Module::actions()` lists every action. For each action, `creates()`,
  `effects()` and `removes()` give every creation, write and removal (`semantic/module.rs:359-390`).
- An effect or removal names a parameter. `params()` → `ty().to_wire_type()` gives
  `WType::Entity(XDecision)`, the same technique as 502's requester rule
  (`request_flow/ownership.rs`).
- Rules (final composition, common pass):

| Operation on `XDecision` | Allowed for |
|---|---|
| create | the approval's two generated actions |
| write any field | nobody |
| remove | nobody (ordinary behavior) |
| read, query | anyone |

Completeness: an `ActionItem` changes state only through `effects`, `creates` and `removes`
(`semantic/module.rs:347-357`; `wire.rs:163-174` has exactly `Create` and `Remove`), and no
cascading removal exists. So the check above sees every ordinary way to create, change or
remove a decision. The check covers model-generated actions too: no action is exempt because
of where it came from.

## R6. Declarations through the public engine (gate 1): PASS

- `declare_enum` (`builder.rs:433`) is idempotent for an identical re-declaration.
- `declare_entity` (`:487`) adds the implicit `id` field.
- `Id<T>` needs `T` to be a declared entity (`admit/resolve.rs:112-126`), so the decision entity
  must be declared after the subject and the actor. This is the dependency edge of `G_D`.
- An entity's hash covers its fields in the order given (502 R2), so the decision fields are
  declared in field-name order.

## R7. Model-declaration DAG (gate 2): PASS (design in data-model, pending R4)

502's P2 already declares contributions in model-id order and supports authored forward
references (D5). It generalizes to a topological pass over `G_D`:
- the edge `M_i → M_j` exists iff a type `M_j` references is produced by `M_i`;
- ready nodes are taken in model-id order;
- a cycle gives a blocking `DECLARATION_CYCLE` naming the models (sorted).

For StateMachine-only and RequestFlow-only sets the graph has no edges, so the topological
order equals model-id order. The builder call sequence is therefore unchanged and the 501/502
pinned hashes cannot move (asserted against every manifest case).

## R8. No raw Wire JSON (gate 7): PASS with the 502 exception

The creation input is `WLifecycle::Create { fields: Vec<(String, WExpr)> }`, which needs
`Node::into_wire()`, exactly as in 502 (research R4 there). The guard allow-list gains one file
(`approval/lower.rs`). No JSON document is built or parsed.

## R9. No Core change (gate 10): PASS for every gate except R4

R4 has a Core-free alternative (action-scoped validity). Only a module-wide invariant would need
Core work.

## R10. Generated derived definitions and when they are added (FR-009, FR-010)

**Finding.**
- `add_derived` types its body against the current declarations (`builder.rs:965-1000`).
- `derived_ref` to a derived value that has not been added yet gives an untyped node, which
  `finish` reports as a cycle (`builder.rs:798-812`).
- Adding a declaration or a command resets the declaration cache, which is recomputed lazily
  (`builder.rs:499, 172-180`). Typed derived values persist.
- Rules and postconditions evaluated after the effects query the resulting state `S′`
  (`specs/007-relational-queries/spec.md` FR-013; `eval.rs:5`).

**Decision.**
- Approval's five derived definitions are added in **P2**, right after all model declarations
  of the topological pass, so they are typed before P3 traces any hand-written reference. They
  are added in approval-id order, and within an approval in the order `approve_count`,
  `reject_count`, `approved`, `rejected`, `pending`, the dependency order of their bodies.
- P2b (commands) may follow: it does not disturb typed derived values.
- P4 re-derives them and requires an identical item by Core hash, extending law D4 to
  definitions (`DEFINITION_MISMATCH`).
- The decision actions use the same derived values:
  - in preconditions: `<id>__pending(subject)`, evaluated on `S`;
  - in postconditions: `¬(<id>__approve_count(subject) ≥ k_A ∧ <id>__reject_count(subject) ≥ k_R)`,
    evaluated on `S′`.

  One definition therefore serves the guard, the backstop and the external consumers.

**Alternatives rejected.**
- Inlining the queries everywhere: consumers would get no named provenance, and the outcome
  would have several copies.
- Adding the derived values in P4: hand-written references traced in P3 would be untyped.

## R11. Decision action layout and evaluation order (FR-012–FR-014)

```text
action   encode(id, approve | reject)
params   decision_id: Input Id<XDecision>, <inputs sorted>, subject: State S, <context sorted>
pre      [ Open, <id>__pending(subject), Eligible, ¬any(XDecision, d → d.subject == subject.id ∧ d.actor == actor), G_c… ]
create   XDecision { id: decision_id, actor: <actor>, choice: lit(APPROVE | REJECT), subject: subject.id, <payload> }
effects  none          lifecycle: exactly one Create
post     [ count(where(XDecision, d → d.subject == subject.id ∧ d.actor == actor)) ≤ 1,
           ¬(<id>__approve_count(subject) ≥ k_A ∧ <id>__reject_count(subject) ≥ k_R)   (only if k_R ≠ ⊥) ]
commands K_c (command profile only)
```

- The precondition order is fixed, which fixes the hashes and the reported denial. `G_c` keeps
  author order.
- The actor expression is captured in query lambdas. Captured values are evaluated in `S`
  (007 FR-013), which is correct: the actor is a parameter-derived value, not state.
- A Core-assumptions test pins that an actor expression over input and context parameters can
  be captured. An actor expression Core cannot capture is a `FRAGMENT_INVALID` from Core
  admission, never a model workaround.

## R12. Concurrency proof inputs

- R1: `Commit(C₁, S) = Success ⇒ Commit(C₂, S) = STATE_CONFLICT` (stated contract).
- R2: decision guards observe complete query memberships, re-derived at the parent.
- R10, R11: the terminal guard is evaluated on `S`, and the backstop on `S′` of the same
  transition.

The proof sketch is in plan.md.

## R13. Query facts under the command profile (found in implementation, 2026-10-14; corrected same day)

**Finding (verified by test at the pin).** At the pinned Core release v0.12.0 (`b525da4`), under
the command profile (Wire 0.8, v2 store genesis), Core refuses its own candidate for every action
whose evaluation observes a query fact, with `BundleInvalid("INVALID_CANDIDATE: invalid observed
facts")`: the store adds `result_hash` to query facts, and the v2 candidate validator decoded
action `read_facts` with the ordinary `Facts` decoder, which rejects `result_hash`. Under the
legacy profile the same actions commit. Pinned by
`approval::lower::tests::core_v2_candidates_refuse_query_facts` (now flipped, see Resolution).

**Correction.** The defect is **fixed on Core `main`** by Core 023 (`4f44fb8`, strict v2 bundle
evidence validation): action candidates are decoded by `bundle_evidence::ValidatedBundleEvidence`
(`behavior-verify/src/governance/trusted.rs:157`); ordinary `Facts` still rejects `result_hash`,
by design. The first version of this note, and
[behavior-ir-core#24](https://github.com/kallistoteles/behavior-ir-core/issues/24), wrongly said
`main` (`634023a`) was affected; that came from reading `parse_queries`, not from running the
reproduction there. Verified on `main` `1c8191b` (all ten public reproduction cases and a signed
command-intent test pass). **No tagged release contains the fix**: v0.12.1 (`1e286eb`) does not
include `4f44fb8`.

**Resolution.** Core Release 0.13.0 includes 023. Feature 504 (merged, `42777c1`) moved the
pin to 0.13.0 and added engine- and binding-level regressions; 503 was rebased onto it
(2026-10-15). The pinning test is now
`approval::lower::tests::decisions_observing_queries_commit_under_both_profiles`: a decision
commits under the legacy and the command profile. Runtime decisions under the command profile
are testable without any workaround (FR-028).

**Also from 504 (affects 503 tests):** fresh plain evaluation requires a complete parent
snapshot (Core 017; `evaluate(..., facts=...)` is required), fresh decision records are 0.8, and
the project has no backward compatibility before 1.0 (`docs/versioning.md`).
