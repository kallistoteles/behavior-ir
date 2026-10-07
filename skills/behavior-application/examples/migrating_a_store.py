"""Migrating a living store: apply a migration in place, handle SCHEMA_MISMATCH, backfill
between migrations, and require evidence for migrations."""

from enum import Enum
from types import SimpleNamespace

from behavior import (
    BehaviorModule, CommitRefused, InMemoryBackend, Input, Migration, Option, Store, action, all_,
    entity, enum_map, field, replay_behavior, replay_data, select, set_, strict_unwrap,
)

NOW = "2026-10-02T12:00:00Z"


def generation(values: tuple[str, ...], region_required: bool) -> SimpleNamespace:
    status_enum = Enum("Status", {v: v for v in values})  # type: ignore[misc]

    @entity
    class Ticket:
        status = field(status_enum)
        region = field(str if region_required else Option[str])

    @action
    def set_region(ticket: Ticket, *, region: Input[str]):
        set_(ticket.region, region)

    model = BehaviorModule(entities=[Ticket], actions=[set_region])
    return SimpleNamespace(Status=status_enum, Ticket=Ticket, model=model)


v1 = generation(("OPEN", "DONE"), region_required=False)
v2 = generation(("OPEN", "DONE", "HELD"), region_required=False)
v3 = generation(("OPEN", "DONE", "HELD"), region_required=True)

broaden = Migration(source=v1.model, target=v2.model, transforms={
    v1.Ticket: lambda old: {"status": enum_map(old.status, {
        v1.Status.OPEN: v2.Status.OPEN, v1.Status.DONE: v2.Status.DONE})},
})
narrow = Migration(
    source=v2.model, target=v3.model,
    requires={"every_ticket_has_region": lambda: all_(select(v2.Ticket),
                                                       lambda t: t.region.is_some())},
    transforms={v2.Ticket: lambda old: {"region": strict_unwrap(old.region)}},
)

seed = [{"entity": "Ticket", "value": {"id": "t1", "status": "OPEN", "region": None}}]
store = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, seed))

# 1. Apply a migration in place: one atomic transition at the next position.
store.migrate(broaden, commit_time=NOW)
assert [s.since for s in store.schema_history()] == [0, 1]

# 2. Behavior of another schema is refused before it runs: pick the module of the store's schema.
try:
    store.evaluate(v1.model, "set_region", bindings={"ticket": "t1"}, input={"region": "x"},
                   commit_time=NOW)
except CommitRefused as e:
    assert e.code == "SCHEMA_MISMATCH" and e.details is not None
current = next(m for m in (v1.model, v2.model, v3.model)
               if m.schema_hash == store.schema_at().hash)
assert current is v2.model

# 3. Narrowing waits for the data: backfill with an ordinary action, then narrow.
try:
    store.migrate(narrow, commit_time=NOW)
except CommitRefused as e:
    assert e.code == "MIGRATION_REQUIREMENT_FAILED" and "Ticket#t1" in str(e)
ev = store.evaluate(current, "set_region", bindings={"ticket": "t1"}, input={"region": "north"},
                    commit_time=NOW)
assert ev.bundle is not None
store.commit(current, ev.bundle)
store.migrate(narrow, commit_time=NOW)
assert store.load("Ticket", "t1")["value"]["region"] == "north"

# 4. History replays across both migrations.
assert replay_data(store).ok
assert replay_behavior(store, [v1.model, v2.model, v3.model], migrations=[broaden, narrow]).ok

# 5. Fresh required-governance writes in legacy history require explicit v2 adoption.
policy = {"format": "behavior.evidence_policy.v1", "require": "none",
          "migration": {"require": "commit_authorization"}}
strict = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, seed, policy))
try:
    strict.migrate(broaden, commit_time=NOW)
except CommitRefused as e:
    assert e.code == "TRUSTED_GOVERNANCE_UPGRADE_REQUIRED"
print("ok")
