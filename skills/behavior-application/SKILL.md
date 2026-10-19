---
name: behavior-application
description: Build the host application around a Behavior model (stores, evaluate and commit, reads and read intents for agents, conflicts, replay, custom storage backends, supplying facts), keeping every business rule in behavior. Use when writing services, handlers, jobs or storage code that use a behavior model.
release: 0.13.1
---

# Building an application on Behavior

This skill describes **Behavior release 0.13.1**. Use only what is described here or listed in the
release's public API.

This release bundles Core 0.15.0 (verifier 0.10.0). Public Models and command declarations
lower to ordinary Core behavior. Command modules use explicit Store-v2 genesis and durable
command history; command-free modules retain their established profile. Trusted proof and authorization
generation use the bundled `behavior governance` commands; a fresh trusted commit receives
independent host context through `Store.commit_with_context`.

> If the public Behavior API cannot express a requirement, record a semantic gap in
> `SEMANTIC_GAPS.md` (format: skills/README.md). Do not work around it: no engine internals,
> no hand-written IR, no moving the rule into host code, no silent approximation.

## The host boundary

The behavior model decides and the host serves it.

| The model owns | The host owns |
|---|---|
| every business rule: what is allowed, what changes, what must hold | identities (it chooses `id`s), time (`commit_time`), inputs, who is acting (context) |
| creation and removal of entities | storage (a backend), transport, presentation, scheduling |
| the meaning of every value (types, exact arithmetic, rounding) | choosing which action to run and binding the entities it acts on |

The rules of the boundary:

1. **Every business decision is a behavior action, and every question a read.** A condition
   checked only in host code (an `if` before calling the engine, a filter in a SQL query) is a
   hidden rule. Either move it into the model or record it as a semantic gap. A question the
   application answers is a declared read, never an action without effects.
2. **Every state change goes evaluate → commit.** Never write entity state any other way.
3. **Supply identities and time.** Never ask the engine to invent them. Never reuse an identity:
   it is used forever, even after removal.
4. **On a conflict, re-evaluate on the current state.** Never retry or force a stale bundle.
5. **Trust a custom backend only once it passes the conformance suite.**
6. **Audit by replay.** The history replays without trusting stored results.
7. **Use only the released package.** Import `behavior` and nothing else from Behavior. Pin the
   release by file and hash; never depend on engine crates or paths inside the Behavior
   repository.

## Stores: evaluate, commit, read, replay

A store is created once, from a genesis (the model's declarations plus seed entities). After that
it changes only through commits. After a restart, reopen it from its backend with
`Store.open(backend)`; `Store.create` is only for a new store.

```python
# from examples/store_lifecycle.py
store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, []))
start = store.current()
now = "2026-09-30T12:00:00Z"  # time comes from the host, never from the engine

opened = store.evaluate(model, "open_account", bindings={}, input={"account_id": "a1"},
                        commit_time=now)
assert opened.decision.result == "ALLOW" and opened.bundle is not None
store.commit(model, opened.bundle)
```

- **`store.evaluate(model, action, bindings=…, input=…, context=…, commit_time=…)`** evaluates
  against one consistent snapshot of the current state. `bindings` names the entity id of each
  state parameter. It returns an `Evaluation`:
  - `decision`: result, reasons, changes, trace and the full record;
  - `bundle`: present only when the decision allows.
- **`store.commit(model, bundle)`** applies the bundle atomically onto the state it was evaluated
  against, and returns the new state.
- **`commit_time`** is UTC in exactly the form `YYYY-MM-DDTHH:MM:SSZ`, with whole seconds.
  Fractional seconds and offsets such as `+00:00` are refused (`BUNDLE_INVALID`). Format the
  host clock that way, e.g. with `strftime("%Y-%m-%dT%H:%M:%SZ")`.
- **Unknown ids.** Binding an id that does not exist makes `store.evaluate` itself raise
  `CommitRefused` with `ENTITY_NOT_FOUND`, before any decision.
- **Reading.** `store.load(entity, id, at=state)` reads any past state, and `store.history()`
  lists the transition records.
- **Replay.** `replay_data(store)` recomputes every state from the recorded writes.
  `replay_behavior(store, [model])` re-evaluates every decision.

```python
# from examples/store_lifecycle.py
assert store.load("Account", "a1")["value"]["balance"] == "25.00"
assert store.current().position == 2
assert len(list(store.history())) == 2
assert replay_data(store).ok
assert replay_behavior(store, [model]).ok
```

A denied decision has no bundle: report its `decision.reasons` to the user. There is nothing to
commit.

### Models and a work lifecycle

StateMachine transitions, RequestFlow creation/status actions and Approval decision actions
are ordinary Store evaluations and commits. Approval derives outcomes from immutable decisions;
a host invokes a separately guarded flow transition when appropriate. WorkQueue generates work
state, validity and ready/expired reads. The host selects work, invokes capabilities and handles
transport; the Model governs whether each state change is allowed. It provides no scheduling,
ordering, worker execution or exactly-once physical effects.

The standalone example declares an active-worker eligibility query and a completion binding
whose document id must match the work payload. It explicitly selects Store v2 with a `none`
evidence policy for demonstration; choose the deployment's real governance policy separately.
A claim's worker and time come from the trusted host. Completion uses the current generation:

```python
# from examples/work_queue_store.py
commit("claim", bindings={"document_work": "w1"}, input={"expected_generation": 0},
       context={"worker": "worker-1", "now": 10, "lease_until": 20})
assert store.load("DocumentWork", "w1")["value"]["attempts"] == 1
commit("complete", bindings={"document_work": "w1", "doc": "doc-1"}, input={"generation": 1},
       context={"worker": "worker-1", "now": 11})
assert store.current().position == 3
assert store.load("DocumentWork", "w1")["value"]["status"] == "SUCCEEDED"
assert store.load("Document", "doc-1")["value"]["processed"] is True
assert len(stream.items) == 1
assert stream.items[0]["intent"]["payload"] == {"document": "doc-1"}
assert replay_data(store).ok and replay_behavior(store, [model]).ok
```

Completion, document mutation and the command intent commit atomically. `commands_since` reads
committed occurrences; the adapter owns delivery, deduplication and physical effects. Stable
occurrence identities support deduplication but cannot guarantee exactly-once external execution.
RequestFlow can emit an intent that an adapter turns into enqueue in a separate commit; that
asynchronous handoff is not one cross-module transaction.

**Time and genesis.** Supply recorded `now` in the queue's nominal time domain and canonical
UTC `commit_time` separately. The host owns freshness at commit and monotonic time; a recorded
live lease is no physical deadline guarantee. Core validates genesis work state but does not
establish imported provenance: a seeded LEASED item need not have a recorded claim.

## Reads: asking without changing

A question about the state is a **read**: the model declares it (`@read`, see the authoring
skill) and the host evaluates it. A read changes nothing: no commit, no record in the store's
history, and nothing to bind except what the question is about.

```python
# from examples/reading_state.py
total = store.read(model, "open_total", bindings={"customer": "k1"})
assert total.value == 120 and store.current().position == 2
rows = store.read(model, "open_orders")
assert [r["id"] for r in rows.value] == ["o1", "o2"]  # identity order
```

- **`store.read(model, read, bindings=…, input=…, context=…, at=…)`** returns a `ReadResult`:
  - `result`: `VALUE`, `EVALUATION_ERROR`, `INVALID_INPUT` or `INVALID_BINDING` (an id that does
    not exist at that state);
  - `value`: a value, a list of records (a query projection, in identity order) or one record
    (an entity projection); an absent field is `None`, never left out;
  - `record`: the full evidence, and `record_id`, its identity.
- **The past.** `at=store.state_at(p)` (or any earlier `StateRef`) reads that state exactly as it
  was, under its own schema.
- **A schema mismatch** raises `CommitRefused` with `SCHEMA_MISMATCH`, as for evaluation.
- **Evidence.** A read record names the exact state, the inputs, everything the read observed,
  and the result. `store.replay_read(model, record)` checks it against the store;
  `replay_read(model, record)` checks it on its own. The store never keeps read records: keep the
  ones you need.

```python
# from examples/reading_state.py
assert store.read(model, "open_total", bindings={"customer": "k1"}, at=before).value == 30

# The record is evidence: it replays against the store and on its own.
assert store.replay_read(model, total.record).matches
assert replay_read(model, total.record).matches
```

### Agents and other untrusted callers

An agent asks through **`store.read_intent(model, {"capability", "targets", "input"})`**. It may
name only a declared read, and every problem is listed at once (`IntentRejected`), including
targets that do not exist. The result is a `ReadExecution`:

- **`response`** holds the declared result and the record identity. Forward only this.
- **`record`** holds the evidence, including what derived values read internally. Keep it on the
  host side.

```python
# from examples/reading_state.py
x = store.read_intent(model, {"capability": "customer_summary", "targets": {"customer": "k1"}})
assert x.response.value == {"id": "k1", "name": "Ada", "in_good_standing": False}
assert "credit_limit" not in x.response.to_json()  # what the read observed stays in x.record
```

Asking is a read capability and changing is a transition capability. An action name is not a
read capability (`UNKNOWN_CAPABILITY`), and an agent can never send an expression of its own:
ad-hoc reads (`store.read(model, some_read_function)`) are for trusted host code only.

## Conflicts

Concurrency is whole-state and optimistic. A bundle commits only if the store is still at the
state it was evaluated against; otherwise `StateConflict` is raised. The answer is always to
**evaluate again on the current state**, because the rules may now decide differently.

```python
# from examples/conflict.py
first, second = withdraw_("60.00"), withdraw_("50.00")  # both against the same state
assert first.bundle is not None and second.bundle is not None
store.commit(model, first.bundle)
try:
    store.commit(model, second.bundle)
except StateConflict as conflict:
    assert conflict.current["position"] == 1
else:
    raise AssertionError("a stale bundle must not commit")
```

An ordinary conflict retry reevaluates against the new state and may now be denied.
After a lost acknowledgement, recover by resubmitting the **original bundle**: an equivalent
already committed transition is recognized (`.already` is true), without applying it again.
Reevaluating instead creates a new candidate and does not answer whether the original committed.
Other refusals raise `CommitRefused` with a `code`.

```python
# from examples/conflict.py
retried = withdraw_("50.00")
assert retried.decision.result == "DENY"
assert store.load("Account", "a1")["value"]["balance"] == "40.00"

# Committing the same transition twice (a retry after a lost acknowledgement) is recognized and
# never applied twice.
assert store.commit(model, first.bundle).already
assert store.current().position == 1
```


## Changing the schema of a living store

A store is bound to exactly one schema at a time, and each history position keeps its own. When
the model's declarations change, write a `Migration` (behavior-authoring), verify it
(behavior-verification), and apply it with `store.migrate(migration, commit_time=...)`. It is
one atomic transition at the next position. Every entity of a changed type gets a new version;
history, identities and references carry over, and old states still load exactly as written.

- **`SCHEMA_MISMATCH`** (a `CommitRefused` with `.details`) means the module is not the store's
  current schema. Pick the module whose `schema_hash` is `store.schema_at().hash`; never convert
  data in host code.
- **`MIGRATION_REQUIREMENT_FAILED`** means the data does not fit yet. Backfill with an ordinary
  action, then apply the same migration again. The other refusals (`MIGRATION_SOURCE_INVALID`,
  `MIGRATION_TRANSFORM_ERROR`, `MIGRATION_INVALID_RESULT`, `RETIRED_TYPE_NOT_EMPTY`) name the rule
  and the entities, and the store is unchanged.
- **Evidence**: an evidence policy can demand more for migrations than for actions (its
  `migration` section). Required-governance v1 histories refuse fresh migrations with
  `TRUSTED_GOVERNANCE_UPGRADE_REQUIRED`; structural `authorize_migration` evidence does not
  upgrade that trust. Adopt validated state into an explicit v2 genesis for new trusted
  history. Trusted migrations follow the
  [public core migration contract](https://github.com/kallistoteles/behavior-ir-core/blob/v0.15.0/specs/013-durable-command-intents/contracts/governance.md);
  the legacy Python `store.migrate` helper supplies no independent v2 authorization context.
- **Replay**: `replay_behavior(store, models, migrations=[...])` re-runs every migration.

```python
# from examples/migrating_a_store.py
store.migrate(broaden, commit_time=NOW)
assert [s.since for s in store.schema_history()] == [0, 1]
current = next(m for m in (v1.model, v2.model, v3.model)
               if m.schema_hash == store.schema_at().hash)
try:
    store.migrate(narrow, commit_time=NOW)
except CommitRefused as e:
    assert e.code == "MIGRATION_REQUIREMENT_FAILED" and "Ticket#t1" in str(e)
store.commit(current, ev.bundle)
store.migrate(narrow, commit_time=NOW)
assert replay_behavior(store, [v1.model, v2.model, v3.model], migrations=[broaden, narrow]).ok
```

### WorkQueue migration continuity

Core checks target validity, not the queue's historical safety assumptions. A WorkQueue migration
must not decrease or reuse attempts, revive terminal work, manufacture a lease or reinterpret
claim generations/time. Keep a LEASED item's lease or move it to READY with unchanged attempts
and attempts below the maximum. Preserve terminality. Review these obligations explicitly;
a successful migration or local SMT proof alone does not discharge them.

## Custom storage backends

A backend stores Behavior's documents and has nothing else to do. It provides:
- `genesis`, `head` and `record`;
- `version_at(key, position)` and `version(key, revision)`;
- `removed_at(key)`;
- `incoming_at(target, position)`;
- `keys_at(entity_type, position)`;
- `create(...)`, the genesis write. A retry of the **same** original creation (same genesis,
  seed versions, references and original head) must be a no-op that leaves later history
  intact; any other creation on a created backend must be refused;
- `commit(...)`: **one atomic compare-and-set** that writes the versions, removals, reference
  changes, record and head together, or nothing.

`used_at` and `keys_by_field_at` (a field index; it only speeds queries up) are optional. Reads
at a position must be as-of that position.

The documents a backend handles are plain dicts. Store them as given, and read back exactly what
was written. A backend reads only a few fields:

| Field | Meaning |
|---|---|
| `head["last_record"]` | the value the compare-and-set compares |
| `head["state_ref"]["position"]` | the position a commit creates |
| a version's `entity`, `id`, `revision`, `created_at` | the key, and when the version became current |
| a reference change's `op` (`add`/`drop`), `target`, `source`, `field` | how the incoming-reference index changes |

Key order from `keys_at` and `keys_by_field_at` never matters. The example backend
(`examples/custom_backend.py`) is a complete reference.

```python
# from examples/custom_backend.py
report = run_conformance(DictBackend)
assert report.ok, report.failed()
```

Run `run_conformance(factory)` in the application's test suite, and do not use a backend until
every case passes.

## Evaluation without a store

`evaluate(model, action, state=…, input=…, context=…, data_version=…, facts=…)` decides from
explicit data. The engine never looks anything up, so everything a decision reads must be
supplied.

`data_version` names the snapshot the data came from. `facts` is required and holds the
**complete parent snapshot**: `universe` lists every declared entity type as
`{"entity": T, "members": [...]}`, an empty type as `"members": []`, and the bound `state` must
agree with it. Nothing is inferred from the bound entities: an incomplete, inconsistent or
invalid snapshot is `INVALID_INPUT` with `INVALID_STATE_SNAPSHOT`. Historical facts that a
snapshot cannot show, such as `identities` (whether an identity was ever used), go alongside.
A missing historical fact is `UNKNOWN_FACT`, never guessed. Facts that contradict each other
are `INCONSISTENT_FACTS`.

**Value formats.**
- `state`, `input` and `context` accept Python values (`Decimal`, `Enum` members, `None`) or
  their canonical JSON form.
- `facts`, stored values and records always use the canonical JSON form. Decimals are strings
  such as `"5.00"`; enums are their value strings.

**Plain evaluation checks the snapshot you supplied**, as a store's state would be: an entity
that breaks a constraint or a module invariant makes it `INVALID_STATE_SNAPSHOT`, naming the
rule.

```python
# from examples/plain_evaluation.py
universe = {"universe": [
    {"entity": "Customer", "members": [state["customer"]]},
    {"entity": "Order", "members": [{"id": "o1", "customer": "c2", "amount": "5.00"}]},
]}
decision = evaluate(model, "close_customer", state=state, data_version="snapshot-17",
                    facts=universe)
assert decision.result == "ALLOW"
```

The decision record (`decision.record_json`) is the audit artifact. It holds the observed facts,
and `replay(model, record_json)` reproduces the decision without any store. A store supplies all
of this automatically, so prefer a store for anything that changes state.

## Decision results

| Result | Meaning | Host response |
|---|---|---|
| `ALLOW` | the transition is allowed; a store evaluation has a bundle | commit it |
| `DENY` | a rule refused it (`reasons` name the rule) | show the reason; nothing to commit |
| `ENTITY_ID_ALREADY_USED`, `LIFECYCLE_CONFLICT` | a creation or removal is not possible | choose another id, or fix the request |
| `INVALID_INPUT` | the request does not fit the model (types, missing fields, an incomplete or invalid snapshot) | a host or client error; with `INVALID_STATE_SNAPSHOT`, the supplied state is incomplete or corrupt: investigate, never commit around it |
| `ERROR` | evaluation failed (overflow, division by zero, a missing fact) | a model or data problem; the reason says which |

## Anti-patterns and their corrections

| Anti-pattern | Correction |
|---|---|
| `if customer.has_debt: return error` in a handler | a `requires` in the action (or a gap entry if it cannot be expressed) |
| updating a row directly "because it is only a status" | an action that `set_`s it, committed through the store |
| retrying `commit` with the same bundle after `StateConflict` | re-evaluate on `store.current()`, then commit the new bundle |
| generating ids inside the model or deriving them from time | the host chooses ids and passes them as inputs |
| filtering entities in the database before evaluation | a query in the model (the store answers it as of the evaluated state) |
| an action without effects, and a placeholder entity, just to answer a question | a declared read (`store.read`), with no binding beyond what the question is about |
| handing an agent a read record, or letting it send an expression | `store.read_intent`, forwarding only `response` |
| importing the package's private modules (any name starting with `_`) or anything from the Behavior repository | only `from behavior import …` of the pinned release |

## Where does it belong?

When a requirement seems to need a new concept, such as a state machine, a workflow, an approval
flow or a convenience API, ask one question: **must the evaluator understand this construct for
its semantics to be correct?**

- **No:** it is a model, a library or application code. Build it from what this skill describes,
  so that it lowers completely to ordinary Behavior: preconditions, effects, postconditions,
  queries and reads.
- **Yes:** it is a gap in the language. Record a semantic gap (see `skills/README.md`); do not
  emulate it in host code.

The rules for models, and a worked state-machine lowering, are in the `models/` area of the
Behavior repository (behavior-ir).
