# Contract: Approval (semantic version 1)

**Feature**: 503 | The author-facing contract. Implementation and Python syntax are bound by it.

## 1. Meaning

```text
AM = (p, S, A, Π, k_A, k_R, Open, Eligible, {approve, reject})
D(s) = { d ∈ X | d.subject = id(s) }
A(s) = |{ d ∈ D(s) | d.choice = APPROVE }|          R(s) = |{ d ∈ D(s) | d.choice = REJECT }|
Outcome(s) = REJECTED if k_R ≠ ⊥ ∧ R(s) ≥ k_R;  APPROVED else if A(s) ≥ k_A;  PENDING otherwise

CanApprove(s, a) = Open(s) ∧ Pending(s) ∧ Eligible(s, a) ∧ ¬HasDecision(s, a) ∧ G_A(s, a)
CanReject(s, a)  = Open(s) ∧ Pending(s) ∧ Eligible(s, a) ∧ ¬HasDecision(s, a) ∧ G_R(s, a)
Δ_c = CreateEntity(X, (decision_id, id(s), a, choice(c), payload))  + command intents

L_Approval(AM) = Compose( Decl(AM), Defs(AM), Actions(AM) )
  Decl = { C, X }          Defs = { approve_count, reject_count, approved, rejected, pending }
  Actions = { p__approve, p__reject }, each carrying its child-state checks (Inv, data model §6)
Meaning_Approval(AM) := Meaning_Core(L_Approval(AM))
```

- **D(s) is every recorded decision.** It is never "currently eligible decisions": eligibility
  is checked at decision time only.
- **The outcome is never stored.** `Outcome(s) = F(D(s))`, exactly the derived definitions.
- **APPROVE and REJECT are records.** APPROVED and REJECTED are derived outcomes. DENIED is an
  attempt that was not permitted.

## 2. Runtime behavior (all Core behavior of the generated actions)

| Situation | Outcome |
|---|---|
| the subject binds, Open, Pending, Eligible, no decision yet, the guards hold, fresh `decision_id` | one commit: exactly one decision; command intents emitted |
| Open false, the outcome terminal, Eligible false, a guard false, or a decision already exists | DENIED (Core precondition denial); nothing created, nothing emitted |
| a guard or predicate fails to evaluate | Core's ordinary evaluation error; nothing created |
| `decision_id` already used | Core's ordinary identity error (`ENTITY_ID_ALREADY_USED`) |
| the child state would hold a duplicate `(subject, actor)` or both thresholds | DENIED (Core postcondition failure) |
| two decisions evaluated on the same parent `S` | the first commit succeeds; the second gets `STATE_CONFLICT`; a retry re-evaluates on the new head |
| an actor id with no actor entity | allowed; actors are identities |
| a later change of eligibility, the open window, or actor or subject existence | recorded decisions are unchanged and keep counting |
| outcome APPROVED while the subject's status is still IN_REVIEW | valid; Approval never changes the subject |

No Approval-specific outcome, authorization, clock, identity generation, scheduling,
notification default, delegation, stage, weight or withdrawal exists.

## 3. Build-time rules

Blocking:
- resolution (data model §3);
- initializer completeness (data model §6);
- the ownership matrix (data model §7);
- declaration collisions, mismatches and cycles (contracts/declaration-dag.md);
- generated-name collisions;
- duplicate model ids (one namespace across approvals, flows and state machines).

There are no non-blocking diagnostics in v1.

## 4. Python authoring API (thin layer; illustrative)

```python
approval = Approval(
    "purchase_request_approval",
    subject=flow.request_entity,                 # or any @entity class
    actor=User,
    approve_threshold=2, reject_threshold=1,     # reject_threshold=None for ⊥
    payload={"reason": Option[str], "decided_at": Instant},
    open_when=lambda request: request.status == S.IN_REVIEW,
    eligible=lambda request, actor: actor != request.requester,
)
approval.approve(context={"actor": Id[User], "now": Instant}, inputs={"reason": Option[str]},
                 actor=lambda decision_id, reason, request, actor, now: actor,
                 values=lambda decision_id, reason, request, actor, now:
                     {"reason": reason, "decided_at": now},
                 requires=[...], emits=[lambda ...: emit(NotifyRequester, request=request.id)])
approval.reject(...)                             # same shape; both must be configured

approval.decision_entity                         # typed handle (drift-checked like 502's)
approval.approved(request)                       # Expr: derived reference p__approved(request)
approval.approve_count(request)                  # also reject_count, rejected, pending

flow.transition("finalize_approved", from_={S.IN_REVIEW}, to=S.APPROVED,
                requires=[lambda request: approval.approved(request)])
BehaviorModule(entities=[User], enums=[S], models=[flow, approval], actions=[...])
```

- Callables receive parameters by position in the data model §4 order. `Open` receives
  `(subject, …)` and `Eligible` receives `(subject, actor, …)`, where `actor` is the
  capability's traced actor expression.
- `approval.<predicate>(x)` needs `x` to be a bound entity of type `S`, because a Core derived
  argument is a parameter name (research R3). It is usable in any guard, read or derived
  value, including those of other models.
- Python checks nothing semantic: no thresholds, no counts, no names.
- `context.actor` has no special meaning. The author writes the actor expression; a parameter
  named `actor` is not interpreted.

## 5. Reference approvals (compatibility manifest)

| Case | Purpose |
|---|---|
| `ap_document_review` | approval over an authored `Document` (no RequestFlow), `k_A = 2`, `k_R = ⊥`, payload `reason`, a command on approve |
| `ap_purchase_request` | RequestFlow `purchase_request` + approval over its request entity, `k_A = 2`, `k_R = 1` (veto), `open_when` on IN_REVIEW, `eligible` excludes the requester, `finalize_approved` / `finalize_rejected` guarded by the derived predicates |
| `ap_two_approvals` | `finance_approval` and `legal_review` over one subject, both with `approve` / `reject`, plus a standalone StateMachine |

## 6. Guarantees and their scope

For ordinary states reachable from a valid Approval base state `S₀`:

| Guarantee | Basis |
|---|---|
| Uniqueness `∀ s, a: Count(s, a) ≤ 1` | child-state check on `(s*, a*)` + structural ownership ⇒ inductive preservation |
| Threshold consistency | child-state check on `s*` + ownership ⇒ inductive preservation |
| Approval soundness: `approved(s) ⇒` at least `k_A` distinct actors approved | uniqueness |
| Terminal stability | the terminal guard; decisions are never written or removed |
| Veto (`k_R = 1`) | definition + terminal stability |
| No stale composition | Core's whole-state concurrency (`STATE_CONFLICT`) |
| Subject untouched | `Δ = CreateEntity(X)`; no effects are expressible |

**Base-state assumption.** An empty decision relation is a valid base state. Genesis seeds,
imports and migrations that introduce decisions are separate semantics. They must establish the
laws themselves, and a seeded decision is not a valid Approval state by default.
