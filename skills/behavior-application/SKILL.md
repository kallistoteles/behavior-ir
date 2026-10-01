---
name: behavior-application
description: Build the host application around a Behavior model (stores, evaluate and commit, conflicts, replay, custom storage backends, supplying facts), keeping every business rule in behavior. Use when writing services, handlers, jobs or storage code that use a behavior model.
release: 0.8.0
---

# Building an application on Behavior

This skill describes **Behavior release 0.8.0**. Use only what is described here or listed in the
release's public API.

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

1. **Every business decision is a behavior action.** A condition checked only in host code (an
   `if` before calling the engine, a filter in a SQL query) is a hidden rule. Either move it into
   the model or record it as a semantic gap.
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

Committing an identical transition again, for example a retry after a lost acknowledgement, is
recognized (`.already` is true) and never applied twice. Other refusals raise `CommitRefused`
with a `code`.

## Custom storage backends

A backend stores Behavior's documents and has nothing else to do. It provides:
- `genesis`, `head` and `record`;
- `version_at(key, position)` and `version(key, revision)`;
- `removed_at(key)`;
- `incoming_at(target, position)`;
- `keys_at(entity_type, position)`;
- `create(...)`, the genesis write;
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

`data_version` names the snapshot the data came from. `facts` supplies what the model reads
beyond the bound entities:
- `existence`;
- `identities` (whether an identity was ever used);
- `references` (incoming references);
- for queries, `queries` and `fields`, or a whole `universe` of a type.

A missing fact is `UNKNOWN_FACT`, never guessed. Facts that contradict each other are
`INCONSISTENT_FACTS`.

**Value formats.**
- `state`, `input` and `context` accept Python values (`Decimal`, `Enum` members, `None`) or
  their canonical JSON form.
- `facts`, stored values and records always use the canonical JSON form. Decimals are strings
  such as `"5.00"`; enums are their value strings.

**Plain evaluation assumes you supplied a valid state**, as a store would: every constraint
holds, and so does every module invariant.

```python
# from examples/plain_evaluation.py
universe = {"universe": [{"entity": "Order", "members": [
    {"id": "o1", "customer": "c2", "amount": "5.00"}]}]}
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
| `INVALID_INPUT` | the request does not fit the model (types, missing fields, bad facts) | a host or client error |
| `INVALID_STATE` | an incoming entity breaks a declared rule | the supplied state is corrupt: investigate, never commit around it |
| `ERROR` | evaluation failed (overflow, division by zero, a missing fact) | a model or data problem; the reason says which |

## Anti-patterns and their corrections

| Anti-pattern | Correction |
|---|---|
| `if customer.has_debt: return error` in a handler | a `requires` in the action (or a gap entry if it cannot be expressed) |
| updating a row directly "because it is only a status" | an action that `set_`s it, committed through the store |
| retrying `commit` with the same bundle after `StateConflict` | re-evaluate on `store.current()`, then commit the new bundle |
| generating ids inside the model or deriving them from time | the host chooses ids and passes them as inputs |
| filtering entities in the database before evaluation | a query in the model (the store answers it as of the evaluated state) |
| importing the package's private modules (any name starting with `_`) or anything from the Behavior repository | only `from behavior import …` of the pinned release |
