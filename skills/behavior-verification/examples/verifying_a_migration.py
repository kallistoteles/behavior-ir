"""Verifying a migration: narrowing proven under a named requirement, a counterexample when the
requirement is missing, and a module invariant over migrated values left inconclusive."""

from types import SimpleNamespace

from behavior import (
    BehaviorModule, Migration, Option, all_, entity, field, invariant, select, strict_unwrap,
    unique, verify_migration,
)


def generation(region_required: bool, with_invariant: bool) -> SimpleNamespace:
    @entity
    class Ticket:
        region = field(str if region_required else Option[str])
        code = field(str)

    @invariant
    def codes_unique():
        return unique(select(Ticket), by=lambda t: t.code)

    model = BehaviorModule(entities=[Ticket], invariants=[codes_unique] if with_invariant else [])
    return SimpleNamespace(Ticket=Ticket, model=model)


v2 = generation(region_required=False, with_invariant=False)
v3 = generation(region_required=True, with_invariant=False)

proven = verify_migration(Migration(
    source=v2.model, target=v3.model,
    requires={"every_ticket_has_region": lambda: all_(select(v2.Ticket),
                                                       lambda t: t.region.is_some())},
    transforms={v2.Ticket: lambda old: {"region": strict_unwrap(old.region)}},
))
assert proven.verified
narrowing = next(c for c in proven.checks if c["kind"] == "migration_narrowing")
assert narrowing["outcome"] == "proven" and narrowing["under"] == ["every_ticket_has_region"]

unproven = verify_migration(Migration(
    source=v2.model, target=v3.model,
    transforms={v2.Ticket: lambda old: {"region": strict_unwrap(old.region)}},
))
assert not unproven.verified
cx = next(f for f in unproven.findings if f["kind"] == "migration_narrowing")["counterexample"]
assert cx["value"]["region"] is None and cx["refusal"]["code"] == "MIGRATION_TRANSFORM_ERROR"

# A module invariant the source did not state, over a migrated type: precision debt, never assumed.
v3_unique = generation(region_required=True, with_invariant=True)
debt = verify_migration(Migration(
    source=v2.model, target=v3_unique.model,
    requires={"every_ticket_has_region": lambda: all_(select(v2.Ticket),
                                                       lambda t: t.region.is_some())},
    transforms={v2.Ticket: lambda old: {"region": strict_unwrap(old.region)}},
))
check = next(c for c in debt.checks if c["kind"] == "module_invariant")
assert check["outcome"] == "inconclusive" and not debt.verified
print("ok")
