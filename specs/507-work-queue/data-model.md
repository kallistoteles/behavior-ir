# Data Model: WorkQueue (feature 507)

## 1. Model input (`WorkQueueInput`, Rust; recorded by the Python binding)

| Field | Type | Rule |
|---|---|---|
| `queue_id` | identifier | encodes the entity name `EncodeEntityName(id)`; unique among models |
| `worker` | entity name | referenced as `Id<worker>` only |
| `time` | nominal name | must be declared with `order` (`TIME_NOT_ORDERED`) |
| `payload` | list of (field, type), any order | names must not be reserved (`PAYLOAD_FIELD_RESERVED`); sorted by name in the entity |
| `max_attempts` | Int | 1 ≤ M ≤ i64::MAX − 1 (`INVALID_MAX_ATTEMPTS`) |
| `capabilities` | one `CapabilityInput` per capability id | all nine required (`MISSING_CAPABILITY`) |
| `display`, `source` | metadata | not in the ModelHash |

`CapabilityInput`:
- authored `inputs`, `context` and `state` bindings (`state` only on enqueue and, under D1(a),
  on the transitions; never of the queue's own entity type);
- `authorization` (mandatory for enqueue, claim, cancel, recover and exhaust; absent on the
  others), `guards`, `effects` (never on the subject), `commands`;
- `display` and `source`.

## 2. Generated declarations

```text
enum   <E>Status   READY | LEASED | SUCCEEDED | FAILED | CANCELLED       (case order fixed)
entity <E>         attempts: Int
                   available_at: T
                   lease_until: Option<T>
                   owner: Option<Id<A>>
                   status: <E>Status
                   <payload fields…>                                     (all fields sorted by name)
invariant <id>__valid_work(w: <E>)                                       ValidWork, spec Context
derived   <id>__terminal(w: <E>) : Bool      status in {SUCCEEDED, FAILED, CANCELLED}
read      <id>__ready(now: Context<T>)       project(select(<E>).where(status = READY ∧ available_at ≤ now ∧ attempts < M), [])
read      <id>__expired(now: Context<T>)     project(select(<E>).where(status = LEASED ∧ is_some(lease_until) ∧ now ≥ value_or(lease_until, now)), [])
```

Each read row is `{"id": …}` (Core adds `id`; the item list is empty).

## 3. Generated actions

Parameter order, as in RequestFlow:
- **enqueue**: `id`, then inputs, state bindings and context, each group sorted by name;
- **transitions**: the subject, then state bindings (D1(a)), inputs and context, each group
  sorted by name.

| Action | Generated parameters | Generated requires (in this order, before authored guards) | Generated effects | Generated ensures |
|---|---|---|---|---|
| `<id>__enqueue` | input `id: Id<E>`, payload fields, `available_at: T` | — | create `E`{status READY, attempts 0, owner none, lease_until none, available_at, payload} | — |
| `<id>__claim` | ctx `worker: Id<A>`, `now: T`, `lease_until: T`; in `expected_generation: Int` | status ∈ {READY}; `available_at ≤ now`; `attempts < M`; `attempts = expected_generation`; `lease_until > now` | status := LEASED; attempts := attempts + 1; owner := worker; lease_until := lease_until | status = LEASED; `attempts = expected_generation + 1` |
| `<id>__renew` | ctx `worker`, `now`, `lease_until`; in `generation: Int` | status ∈ {LEASED}; Current; `is_some(w.lease_until) ∧ lease_until > value_or(w.lease_until, lease_until)` | lease_until := lease_until | status = LEASED; `attempts = generation` |
| `<id>__complete` | ctx `worker`, `now`; in `generation` | status ∈ {LEASED}; Current | status := SUCCEEDED; owner := none; lease_until := none | status = SUCCEEDED; `attempts = generation` |
| `<id>__retry` | ctx `worker`, `now`, `available_at`; in `generation` | status ∈ {LEASED}; Current; `attempts < M`; `available_at ≥ now` | status := READY; owner, lease_until := none; available_at := available_at | status = READY; `attempts = generation` |
| `<id>__fail` | ctx `worker`, `now`; in `generation` | status ∈ {LEASED}; Current | status := FAILED; owner, lease_until := none | status = FAILED; `attempts = generation` |
| `<id>__recover` | ctx `now`, `available_at` | status ∈ {LEASED}; Expired; `attempts < M`; `available_at ≥ now` | status := READY; owner, lease_until := none; available_at := available_at | status = READY |
| `<id>__exhaust` | ctx `now` | status ∈ {LEASED}; Expired; `attempts = M` | status := FAILED; owner, lease_until := none | status = FAILED |
| `<id>__cancel` | — | status ∈ {READY, LEASED} | status := CANCELLED; owner, lease_until := none | status = CANCELLED |

The definitions used in the table:
- `Current := owner = worker ∧ attempts = generation ∧ is_some(lease_until) ∧ now < value_or(lease_until, now)`;
- `Expired := is_some(lease_until) ∧ now ≥ value_or(lease_until, now)`.

The status requirement, the status effect and the status postcondition are the StateMachine
lowerer's. The other generated requirements, effects and postconditions are passed to it as
the transition's fragments, ahead of the authored ones. Authored guards, effects and commands
follow the generated ones.

## 4. State transitions

```text
          enqueue
             │
             ▼
   ┌──────► READY ──────────── cancel ──────────────┐
   │         │                                      │
   │       claim (attempts+1)                       │
   │         ▼                                      ▼
   ├────── LEASED ◄─ renew                      CANCELLED
   │ retry / recover (attempts < M)                 ▲
   │         │                                      │
   │         ├── complete ──► SUCCEEDED             │
   │         ├── fail ──────► FAILED                │
   │         ├── exhaust ───► FAILED                │
   │         └── cancel ────────────────────────────┘
```

Terminal: SUCCEEDED, FAILED, CANCELLED. No transition has a terminal source state.

## 5. Ownership table

See research R4. Errors are typed `ModelError` variants with codes `UNAUTHORIZED_WORK_*`,
`AUTHORED_WORK_EFFECT` and `WORK_ITEM_BINDING`.

## 6. Receipt

This is the existing `LoweringReceipt`:
- model kind `work_queue`, semantic version 1, lowerer version 1, the Core release;
- ModelHash, BehaviorHash, the canonical document;
- the generated declarations: the enum, the entity, the invariant, the definition, the reads
  and the actions;
- the embedded StateMachine identity;
- the source map from capability fragments to Core items.

## 7. Conformance report (`target/conformance/507-report.json`)

It has the shape of 506 (canonical JSON, the six statuses, readiness, `complete`), and adds:

```json
"obligations": {
  "VO-005": {
    "status": "passed",
    "dependencies": [
      {"class": "structural", "check": "SA-FENCE", "status": "passed"},
      {"class": "smt", "check": "work__complete/postcondition", "status": "passed"},
      {"class": "test", "check": "W07", "status": "passed"},
      {"class": "test", "check": "NC-generation", "status": "passed"}
    ],
    "assumptions": ["ordinary transitions only (migration contract)",
                    "now is the trusted evaluation time"]
  }
}
```

A VO's `status` is the weakest of its dependencies. `assumptions` never contribute a status.
