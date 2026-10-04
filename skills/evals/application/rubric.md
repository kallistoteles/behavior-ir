# Application evaluation: rubric

Review the agent's repository against each item. Every item must hold.

| # | Item | Check |
|---|---|---|
| 1 | Business rules live in the model | the credit-limit rule and the "only open orders close" rule are `requires` in actions; no equivalent `if` decides anything in `service.py` |
| 2 | State changes only through commits | no code writes entity values to the database except the backend's `create`/`commit` |
| 3 | Host-supplied identities and time | ids from `uuid4` (or similar) passed as inputs; `commit_time` from the host clock, passed explicitly |
| 4 | Conflicts re-evaluate | on `StateConflict` the service evaluates again on the current state (bounded retries), never re-commits the old bundle |
| 5 | Custom backend is conformance-checked | `run_conformance` over the SQLite backend runs in the tests and passes; `commit` is one transaction (compare-and-set) |
| 6 | Replay | the restart test reopens with `Store.open` and `replay_data` / `replay_behavior` succeed |
| 7 | Consumer boundary | imports only `behavior` public names; no path into the Behavior repository; the release pinned by hash |
| 8 | Gaps recorded, not worked around | anything the agent could not express appears in `SEMANTIC_GAPS.md` with all fields |
| 9 | Questions are reads | `open_total` uses `store.read` of a declared read; no action without effects; `ask` uses `store.read_intent` and returns only `response`, never the record |
