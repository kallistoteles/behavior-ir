# Contract: RequestFlow (semantic version 1)

**Feature**: 502 | The author-facing contract. Implementation and Python syntax are bound by it.

## 1. Meaning

```text
RF = (f, A, E, Q, Π, 𝒞, 𝒯)      E = EncodeEntityName(f)      Q := Cases(T_s), T_s authored
c  = (id_c, q_c, S_c, I_c, C_c, G_c, V_c, E_c, P_c, K_c)       q_c ∈ Q
Q_entry := { q_c | c ∈ 𝒞 }        R_T := { (q, q_t) | t ∈ 𝒯, q ∈ F_t }
SM(RF)  := (E, status, Q_entry, 𝒯)                             lowered by the 501 StateMachine lowerer

L_RF(RF) = Compose( Decls(RF), Creates(RF), L_SM(SM(RF)) )     Decls(RF) = { E }
Meaning_RF(RF) := Meaning_Core(L_RF(RF))
```

**Lifecycle theorem.** Every ordinary request status trace is in `Q_entry · R_T*`. By
induction: creation sets `q_c ∈ Q_entry`, the embedded machine owns every later status
write, so every change is an `R_T` edge.

`⊥ → q_c` is notation only. Nonexistence is never a StateMachine state, and creation is the
Core creation operation.

## 2. Runtime behavior (all Core behavior of the generated actions)

| Situation | Outcome |
|---|---|
| creation with valid `S_c` bindings, a fresh `request_id` and true guards | one atomic commit: one request with `status = q_c` and `V_c` values, together with `E_c` over `S_c`; `K_c` emitted |
| two state bindings bound to the same entity where Core forbids it | Core's ordinary binding error; no RequestFlow alias rule |
| `request_id` already used | Core's ordinary `ENTITY_ID_ALREADY_USED` |
| a creation or transition guard is false | Core's ordinary denial |
| a guard fails to evaluate | Core's ordinary evaluation error |
| a transition from a status outside `F_t` | Core's ordinary denial (generated `In` requirement) |
| a requester or assignee that is not an existing actor | allowed; they are identities (`Id`), not references |
| a creation's postconditions `P_c` or command payloads `K_c` referring to the new request | only through `request_id` (for example `exists(request_id)`) and the creation's inputs, context and state bindings; there is no state binding for the new request |

No RequestFlow-specific outcome, authorization, clock, identity generation, scheduling,
notification default, read or approval semantics exist.

## 3. Build-time rules

Blocking:
- resolution (data model §3);
- initializer completeness (§5 of the data model);
- capability id collisions;
- declaration collisions and mismatches;
- the ownership matrix (research R7);
- all of 501's StateMachine errors for the embedded machine.

Non-blocking: 501's graph diagnostics for the embedded machine, reported under the flow.

## 4. Python authoring API (thin layer; illustrative)

```python
flow = RequestFlow("purchase_request", actor=User, status=PurchaseRequestStatus,
                   payload={"title": str, "amount": Money}, assignee=True)
flow.create("create", to=S.DRAFT,
            inputs={"title": str, "amount": Money}, context={"actor": Id[User]},
            state={"budget": Budget},                  # S_c: existing entities, changed atomically
            values=lambda request_id, amount, title, budget, actor: {
                "requester": actor, "title": title, "amount": amount, "assignee": none},
            effects=[lambda request_id, amount, title, budget, actor:
                     set_(budget.reserved, budget.reserved + amount)],
            emits=[lambda request_id, amount, title, budget, actor: emit(Notify, request=request_id)])
flow.transition("approve", from_={S.IN_REVIEW}, to=S.APPROVED, context={"actor": Id[User]},
                requires=[lambda r, actor: actor == r.assignee])   # authority is explicit
flow.request_entity                     # typed handle: Id[flow.request_entity], parameter types
BehaviorModule(entities=[User], enums=[PurchaseRequestStatus], models=[flow], actions=[...])
```

Callables receive parameters by position in the data-model §4 order. `values` returns the
initializer mapping. Python checks nothing semantic and passes everything to Rust.

## 5. Reference flows (compatibility manifest)

| Case | Purpose |
|---|---|
| `rf_purchase_request` | full lifecycle, assignee, payload, two creations (Draft, Submitted), a command on creation and on approve, `cancel` guarded by the requester |
| `rf_two_flows_local_submit` | `purchase_request` and `leave_request`, both with `submit` |
| `rf_with_standalone_machine` | a flow plus a standalone StateMachine on another entity, plus hand-written behavior referencing the request entity |
