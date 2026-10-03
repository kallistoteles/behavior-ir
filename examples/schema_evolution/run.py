"""Schema evolution on a living store (feature 009): python -m examples.schema_evolution.run

Broaden, try to narrow too early, backfill with an ordinary action, narrow; then show the schema
history and replay it all.
"""

from decimal import Decimal

from behavior import (
    CommitRefused, InMemoryBackend, Store, replay_behavior, replay_data, verify_migration,
)

from . import v1, v2, v3
from .migrations import broaden, narrow

now = "2026-10-02T12:00:00Z"
seed = [
    {"entity": "Customer", "value": {"id": "c1", "name": "Ada"}},
    {"entity": "Culture", "value": {"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                                    "legacy_code": "L1", "price": Decimal("12.50")}},
    {"entity": "Order", "value": {"id": "o1", "customer": "c1", "region": None, "qty": 3}},
    {"entity": "Order", "value": {"id": "o2", "customer": "c1", "region": "north", "qty": 1}},
]
store = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, seed))
print("V1 store at", store.current().position)

# 1. Broaden (review the summary first).
print("broaden:", broaden.summary())
store.migrate(broaden, commit_time=now)
print("now V2:", store.load("Culture", "k1")["value"])

# 2. Narrowing too early is refused: o1 has no region yet.
try:
    store.migrate(narrow, commit_time=now)
except CommitRefused as e:
    print("refused:", e.code, "-", e)

# 3. Backfill with an ordinary, verified, replayable action.
ev = store.evaluate(v2.model, "set_region", bindings={"order": "o1"}, input={"region": "south"},
                    commit_time=now)
assert ev.bundle is not None
store.commit(v2.model, ev.bundle)

# 4. Narrow; the requirement now holds.
store.migrate(narrow, commit_time=now)
print("now V3:", store.load("Order", "o1")["value"])

# 5. Old behavior is refused; history is intact and replays across both migrations.
try:
    store.evaluate(v2.model, "set_region", bindings={"order": "o1"}, input={"region": "x"},
                   commit_time=now)
except CommitRefused as e:
    print("V2 behavior now:", e.code)
print("schema history:", [(s.since, s.hash[:15]) for s in store.schema_history()])
print("replay:", replay_data(store).ok,
      replay_behavior(store, [v1.model, v2.model, v3.model], migrations=[broaden, narrow]).ok)
if __import__("shutil").which("z3") or __import__("os").environ.get("BEHAVIOR_Z3"):
    print("narrow verified:", verify_migration(narrow).verified)
