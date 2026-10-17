# Contract: WorkQueue model (feature 507)

## 1. Meaning

`Meaning(WQ) := Meaning_Core(L_WQ(WQ))`. The lowering is given in research R2. The generated
items are listed in data-model §2–§3. There is no WorkQueue runtime: every decision is Core's
evaluation of the lowered actions, and every commit is the Store's.

## 2. Authoring (Python)

```python
from behavior import WorkQueue, Id, nominal

Time = nominal("Time", int, ops={"order"})
q = WorkQueue("document_work", worker=Worker, time=Time,
              payload={"document": Id[Document]}, max_attempts=3)
q.enqueue(authorized=lambda *, actor, **_: ..., context={...}, state={...}, effects=[...],
          emits=[...])
q.claim(eligible=lambda w, *, worker, **_: ..., requires=[...], emits=[...])
q.renew(requires=[...], emits=[...])
q.complete(requires=[...], state={...}, effects=[...], emits=[...])
q.retry(...); q.fail(...)
q.recover(authorized=..., context={"actor": Id[User]}, ...)
q.exhaust(authorized=..., ...); q.cancel(authorized=..., ...)
module = BehaviorModule(entities=[Worker, Document], models=[q], ...)
q.work_entity      # the generated entity type, for annotations, reads and guards
q.terminal(w)      # the derived value
```

**How callables are called.** A callable receives the subject (the work item) positionally,
except for enqueue, which has none. Every other parameter arrives **by keyword**, under its
lowered name: the generated ones (`worker`, `now`, `lease_until`, `available_at`, `generation`,
`expected_generation`, enqueue's `id` and payload fields) and the author's inputs, state bindings
and context. A callable receives only the keywords it declares, or all of them with `**kwargs`.
The lowered parameter order is still data-model §3; the keywords only spare authors from counting
up to five generated positions. The earlier model kinds keep their positional convention.

The binding records the callables and traces them in the scopes the model crate opens.

**Authorization, the eligibility predicate and guards** are ordinary Core expressions. An
explicit `lambda *_, **__: True` is how an author allows everyone (Q8). Unknown capability
options raise `TypeError`, so a misspelled `requires` cannot silently discard a guard.

**Rust**: the same through `WorkQueueInput` (data-model §1). W20 compares both paths.

## 3. Invocation (host)

| Capability | `bindings` | `input` | `context` (trusted host facts) |
|---|---|---|---|
| enqueue | authored state | `id`, payload fields, `available_at` | authored |
| claim | `{"work": id}` | `expected_generation` | `worker`, `now`, `lease_until` + authored |
| renew | `{"work": id}` + authored | `generation` | `worker`, `now`, `lease_until` + authored |
| complete, fail | `{"work": id}` + authored | `generation` | `worker`, `now` + authored |
| retry | `{"work": id}` + authored | `generation` | `worker`, `now`, `available_at` + authored |
| recover | `{"work": id}` + authored | — | `now`, `available_at` + authored |
| exhaust | `{"work": id}` + authored | — | `now` + authored |
| cancel | `{"work": id}` + authored | — | authored |
| read `ready`, `expired` | — | — | `now` |

The subject parameter is named after the entity in snake case. For `DocumentWork` it is
`document_work`; the table writes `work`.

**Outcomes** are Core's:
- ALLOW;
- DENY with the failing requirement;
- `EVALUATION_ERROR`;
- Store refusals: `STATE_CONFLICT`, `ENTITY_ID_ALREADY_USED`.

**Lost acknowledgement.** Resubmitting the original bundle returns `already: true` (W23a). A
re-evaluation is a new attempt (W23b).

## 4. Guarantees and their evidence

This is the VO table of the spec, with the evidence of research R10 and the arguments of
`safety-arguments.md`. All time results are relative to the recorded `now` (spec, Time). All
history results cover ordinary transitions (spec, Migration contract).

## 5. Scenarios → checks

| Scenario | Check | Level |
|---|---|---|
| W01–W03, W05 | the reference history steps 1–3, 9 and the US1 scenarios | test |
| W04 | two claims from one head: one commits, the other gets `STATE_CONFLICT` | test, contract |
| W06–W08, W10, W12 | denial cases from constructed states | test |
| W09, W11, W13–W15 | positive cases from constructed states | test |
| W16, W22, W25 | build-error cases: each forbidden writer and binding, a name collision, two queues | structural |
| W17, W18 | data and behavior replay of the reference history; altered context or lease evidence rejected | test, contract |
| W19 | ModelHash / BehaviorHash change with M, the payload, a guard and a referenced declaration; unchanged with display metadata | test |
| W20 | Python vs independent Rust authoring: identical BehaviorHash and ModelHash | test |
| W21 | evaluation with a parent snapshot missing `Worker` → Core rejects | test |
| W23a, W23b | lost-ack recovery and re-evaluation | test, contract |
| W24 | complete with an effect on a bound `Document` and a command: exact candidate, atomic commit, nothing executed | test |
| NC-* | the negative controls of `safety-arguments.md` | structural |
| VO-* | `obligations` in the report (data-model §7) | report |

## 6. Versions and compatibility

- Model kind `work_queue` 1/1, with a new tuple in `models/fixtures/compat.json`.
- `work_queue` 1/1 has one tuple per embedded machine version: 2 without transition bindings,
  3 with them (research R13).
- The D1(a) extension adds `state_machine` 3/1 and `request_flow` 3/1 tuples.
- Every established tuple is byte-identical and nothing is superseded.
