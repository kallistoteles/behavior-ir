# Contract: Reference workflow and its expected outcomes (feature 506)

The domain is data-model §1. The outcomes below are stated by hand from the workflow definition
(FR-023). `models/examples/purchase_approval/expected.json` encodes exactly this table. No
expected value is copied from a run.

## 1. Reference history H (one store, from S₀)

Positions count commits after genesis (position 0).

| Step | Operation | Expected | Position after |
|---|---|---|---|
| 1 | `purchase_request__create` r1 (alice; Laptop, 900) | ALLOW, status `Draft`, requester alice | 1 |
| 2 | `purchase_request__submit` r1 (alice) | ALLOW, `Submitted` | 2 |
| 3 | `purchase_request__start_review` r1 (reviewer dave) | ALLOW, `InReview` | 3 |
| 4 | approve r1 by bob (x1) | ALLOW; outcome PENDING (1 of 2) | 4 |
| 5 | approve r1 by bob again (x2) | DENY (no decision yet: false) | 4 |
| 6 | approve r1 by alice (x3) | DENY (not eligible) | 4 |
| 7 | approve r1 by carol (x4) and by dave (x5), both evaluated at position 4 | x4 commits; outcome APPROVED | 5 |
| 8 | commit x5 | `STATE_CONFLICT`; nothing published | 5 |
| 9 | re-evaluate x5 at position 5 | DENY (terminal: outcome APPROVED) | 5 |
| 10 | `purchase_request__finalize_approved` r1 (actor alice) | ALLOW, `Approved`; intent `RequestApproved{request: r1, by: alice}` | 6 |
| 11 | create r2, submit, start_review (alice, alice, reviewer bob) | ALLOW ×3; r2 `InReview` | 9 |
| 12 | approve r2 by dave (x6), evaluated at position 9 | ALLOW (candidate) | 9 |
| 13 | `set_role` dave → Requester | ALLOW | 10 |
| 14 | commit x6 | `STATE_CONFLICT` | 10 |
| 15 | re-evaluate x6 at position 10 | DENY (not eligible) | 10 |
| 16 | reject r2 by carol (x7) | ALLOW; outcome REJECTED (`k_R = 1`) | 11 |
| 17 | approve r2 by bob (x8) | DENY (terminal: outcome REJECTED) | 11 |
| 18 | `purchase_request__finalize_approved` r2 | DENY (`approved(r2)` false) | 11 |
| 19 | `purchase_request__finalize_rejected` r2 | ALLOW, `Rejected`; no intent | 12 |
| 20 | `budget__close` b1 | ALLOW, `Closed` | 13 |

**Final state at position 13**:
- r1 is `Approved` with decisions x1 (bob, APPROVE) and x4 (carol, APPROVE);
- r2 is `Rejected` with decision x7 (carol, REJECT);
- dave is a `Requester`;
- b1 is `Closed`;
- the command stream holds exactly one intent, `RequestApproved{r1, alice}`.

## 2. Invariants checked after every committed step

- **VO-003**: for every subject and actor, at most one decision.
- **VO-004**: for every subject, not both `approve_count ≥ 2` and `reject_count ≥ 1`.
- **Approval never moves a request**: after steps 4, 7 and 16, the request's status is
  unchanged.
- **Failed commits are atomic**: after steps 5, 6, 8, 9, 14, 15, 17 and 18, the position, state
  and command stream are unchanged.

## 3. Scenario table

| Id | Level | Operation | Expected | Verification |
|---|---|---|---|---|
| C01 | 1 | build the module | admitted; no Approval-specific construct (every item is an ordinary enum, entity, derived item, action or read) | `model.admission`, item kinds |
| C02 | 1 | build twice | equal BehaviorHash, item hashes and model hashes | the authoring identity (data-model §6) |
| C03 | 3 | H step 1 | committed at position 1; the state is valid | the commit result, `load` |
| C04 | 2 | H step 4 | ALLOW; PENDING | decision, `outcome` reads |
| C05 | 2 | H steps 7, 10 | APPROVED; `Approved`; one intent | reads, `commands_since` |
| C06 | 2 | H steps 16, 19 | REJECTED; `Rejected`; no intent | reads, `commands_since` |
| C07 | 2 | H step 5 | DENY; position unchanged | decision, `current()` |
| C08 | 2 | H step 6 | DENY; position unchanged | decision, `current()` |
| C09 | 2 | direct `evaluate` with the `User` universe missing from `facts` | refused as an invalid state snapshot (`INVALID_STATE_SNAPSHOT`) | the error code |
| C09.input | 2 | approve with `decision_id` missing | refused as invalid input, not as DENY | the error kind |
| C10 | 3 | H steps 7–9 | as §1 | `StateConflict`, `history()` |
| C11 | 3 | H steps 12–15 | as §1 | `StateConflict`, re-evaluation |
| C12 | 2 | direct evaluation of a decision on parents seeded invalid (the 503 T017 technique): a duplicate `(subject, actor)` row; both thresholds reachable | DENY at the postcondition (uniqueness `[0]`, or threshold consistency `[1]`); nothing committed | the trace position |
| C13a | 4 | `replay_data` over H | success; recomputed head identity = `current()` | `ReplayReport` |
| C13b | 4 | `replay_behavior(store, [model])`; `replay_invocation` for the recorded invocations | success for every transition and record | `ReplayReport`, `ReplayResult` |
| C13.history | 4 | commit history consistency | each entry's parent is the previous head; positions 1..13 contiguous; each entry's changes = its decision's effects | `history()` |
| C14.* | 4 | altered facts, input or result; wrong module; missing context; altered stored transition | each is a mismatch, refusal or replay failure; none passes | replay results |
| C15 | 4 | run H twice in fresh stores | equal record ids, positions, state identities and command streams | comparison |
| C16a | 1 | Python and Rust authoring identities | equal | `comparison.md` §1 |
| C16b | 2 | Python and CLI on the operations of steps 4, 5, 6, 10 and C09.input | byte-equal canonical records, or equal refusals | `comparison.md` §2 |
| C17 | 3 | `Store.create` twice with the same genesis; once with another genesis | same `store_id` and history; refused | `store_id`, the error |
| C18 | 1 | add a case to `Role` | the `purchase_request_approval` and `purchase_request` model hashes change (they reference `User`); the `budget` model hash is unchanged | receipts |
| C19 | 4 | `verify(model)` (full mode) | `verified`, with every supported check proven | `Attestation` |
| FR-018 | 3 | `run_conformance(InMemoryBackend)` | ok | `ConformanceReport` |
| VO-005 | 1 | the module plus a hand-written action writing a decision field | `UNAUTHORIZED_DECISION_WRITE`, no module | the build error |
| VO-001 | 3 | genesis with an invalid seed (a negative `Budget.limit` violating the invariant `limit >= 0`) | `GENESIS_INVALID` | the error |

`Budget` carries the invariant `limit >= 0`, so VO-001's negative control has something to
violate.
