# Contract: StateMachine Model (semantic version 1)

**Feature**: 501 | The author-facing contract. Implementation and Python syntax are bound by it.

## 1. Meaning

```text
SM = (E, f, Q_entry, 𝒯)        Q := Cases(T_f)        Q_entry ⊆ Q, non-empty
t  = (F_t, q_t, G_t, E_t, P_t, K_t)  with  F_t ⊆ Q non-empty, q_t ∈ Q
G_t, P_t ∈ Predicate_Core*    E_t ∈ Effect_Core*    K_t ∈ CommandIntent_Core*
```

The enum defines which values the field can hold. The machine defines which changes between
them are permitted. Only `(F_t, q_t)` is model semantics; the other components are Core
constructs placed into the generated action (lowering contract §3). `Meaning(SM) :=
Meaning_Core(L(SM))`.

A StateMachine:
- **does**: govern every ordinary write of `f` on existing `E` entities; constrain every
  ordinary creation of `E` to a literal entry state; lower each transition to one ordinary
  action `encode(machine_id, transition_id)`;
- **does not**: create or remove entities, represent nonexistence, schedule, trigger, queue,
  retry, time or orchestrate transitions, keep an event log, or add any runtime error or
  outcome. A transition happens only when its action is invoked externally.

## 2. Runtime behavior (all of it is Core behavior of the generated action)

| Situation | Outcome |
|---|---|
| state ∈ F_t, all guards true | ordinary commit: `f := q_t`, `E_t` applied, `K_t` emitted, postconditions checked |
| state ∉ F_t | Core's ordinary denial: first precondition false |
| a guard is false | Core's ordinary denial |
| a guard raises (for example `DIVISION_BY_ZERO`) | Core's ordinary evaluation error |
| entity removed | Core lifecycle; not a transition |

There is no `INVALID_STATE_MACHINE_TRANSITION` or any other model-specific outcome.

## 3. Build-time rules

Blocking errors (lowering contract §6) cover all of spec FR-030. Specifically:
- ids follow the semantic-id grammar `[a-z][a-z0-9]*(_[a-z0-9]+)*`;
- every state named is a case of `T_f`, and there is no separate state declaration;
- source sets and entry states are non-empty, and transition ids are unique per machine;
- a pure self-transition (`F_t = {q} = {q_t}` with no extra effect and no command) is
  rejected;
- extra effects never write `f`;
- the composed module passes the ownership pass (contract composition-and-ownership).

Non-blocking diagnostics (warnings only, never verification claims):
- `UNREACHABLE_STATE`: not reachable from `Q_entry` through the transition graph, ignoring
  guards;
- `NO_OUTGOING_TRANSITIONS`: informational, because terminal states are valid;
- `NO_INCOMING_TRANSITIONS`: a state that is not an entry state and has no incoming
  transition;
- `UNUSED_STATE`: a case of `T_f` in no entry set and no transition, which only historical
  entities can hold.
- `UNREACHABLE_TRANSITION`: a transition none of whose source states is reachable from
  `Q_entry` (`F_t ∩ Reach(Q_entry) = ∅`), even if its target is reachable some other way.

## 4. Python authoring API (thin layer; first binding)

Illustrative shape, binding-level and not normative semantics:

```python
from behavior import BehaviorModule, StateMachine, Context, Input

machine = StateMachine(
    "purchase_request",
    subject=PurchaseRequest,
    state_field="status",                      # T_f = RequestStatus; Q derived
    entry_states={RequestStatus.DRAFT},
    display_name="Purchase request",           # metadata
)
machine.transition("submit", from_={RequestStatus.DRAFT}, to=RequestStatus.SUBMITTED)
machine.transition(
    "approve",
    from_={RequestStatus.IN_REVIEW},
    to=RequestStatus.APPROVED,
    context={"context": Actor},
    requires=[lambda r, ctx: ctx.actor == r.approver],   # bound by position
    effects=[lambda r, ctx: set_(r.approved_by, ctx.actor)],
    emits=[lambda r, ctx: emit(NotifyRequester, approver=ctx.actor)],   # K_t
    display_name="Approve request",
)

module = BehaviorModule(entities=[...], enums=[...], actions=[...], commands=[NotifyRequester],
                        models=[machine])   # declaring commands selects Core's command profile
module.model_receipts      # [LoweringReceipt]: typed, read-only views of the Rust receipts
module.model_diagnostics   # [Diagnostic]
```

Binding obligations (lowering contract §5): `StateMachine` and `transition()` only record
`ModelInput`. A `set`, `dict` or `list` from Python is passed as-is (no sorting). Lambdas are
traced only inside the scope Rust opens, with proxies named by Rust. Build errors surface as
`behavior.ModelBuildError` carrying the Rust error list unchanged.

## 5. Reference machines (fixtures)

| Case | Purpose |
|---|---|
| `purchase_request` | Draft → Submitted → InReview → {Approved, Rejected}, guarded approve with context, extra effect and a command |
| `order` | the former worked example `submit: DRAFT → SUBMITTED` over `Order.status` (FR-036 equivalence) |
| `request_flow_reference` | spec US5: adds `return → NeedsChanges`, `resubmit → Submitted`, with creation as a separate ordinary action |
| `two_machines_same_local_ids` | two entities, both with `submit` (composition) |
