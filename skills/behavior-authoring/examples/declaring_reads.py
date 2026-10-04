"""Declaring reads: the questions a model answers. A read returns a value or a projection
(named fields and derived values of a query's members, or of one bound entity) and changes
nothing. Declared reads are capabilities: entry points that nothing in the model calls."""

from behavior import (
    BehaviorDefinitionError, BehaviorModule, Id, Input, action, admit, count, derived, entity,
    evaluate_read, field, project, read, requires, select, set_,
)


@entity
class Culture:
    name = field(str)
    active = field(bool)
    measurements = field(int)
    ph_total = field(int)


@derived
def ph_avg(c: Culture):
    return c.ph_total / c.measurements


@read
def active_count():
    return count(select(Culture).where(lambda c: c.active))


@read
def measured_cultures(*, at_least: Input[int]):
    measured = select(Culture).where(lambda c: c.measurements >= at_least)
    return project(measured, lambda c: [c.name, ph_avg(c)])


@read
def culture_view(culture: Culture):
    return project(culture, lambda c: [c.name, c.active])


@action
def retire(culture: Culture):
    set_(culture.active, False)


model = BehaviorModule(entities=[Culture], derived=[ph_avg], actions=[retire],
                       reads=[active_count, measured_cultures, culture_view])

# Reads answer from one state; here the facts are supplied (plain evaluation).
facts = {"universe": [{"entity": "Culture", "members": [
    {"id": "c1", "name": "Basil", "active": True, "measurements": 2, "ph_total": 14},
    {"id": "c2", "name": "Mint", "active": False, "measurements": 0, "ph_total": 0}]}]}
assert evaluate_read(model, "active_count", data_version="1", facts=facts).value == 1
listed = evaluate_read(model, "measured_cultures", input={"at_least": 1}, data_version="1",
                       facts=facts)
assert listed.value == [{"id": "c1", "name": "Basil", "ph_avg": "7"}]

# Adding reads changes the behavior version, never the schema: no migration is needed.
plain = BehaviorModule(entities=[Culture], derived=[ph_avg], actions=[retire])
assert plain.schema_hash == model.schema_hash
assert plain.behavior_version != model.behavior_version


# A read is an entry point, not a building block: share the computation as a derived value.
@action
def retire_if_idle(culture: Culture):
    requires(active_count() > 1)
    set_(culture.active, False)


try:
    BehaviorModule(entities=[Culture], actions=[retire_if_idle], reads=[active_count])
except BehaviorDefinitionError as e:
    assert "READ_CALL_NOT_ALLOWED" in str(e)
else:
    raise AssertionError("expected READ_CALL_NOT_ALLOWED")


# A projection item is a field or a derived value of the member, never a computation.
@read
def doubled():
    return project(select(Culture), lambda c: [c.ph_total * 2])


try:
    BehaviorModule(entities=[Culture], reads=[doubled])
except BehaviorDefinitionError as e:
    assert "UNKNOWN_PROJECTION_ITEM" in str(e)
else:
    raise AssertionError("expected UNKNOWN_PROJECTION_ITEM")


# A read named like an action (or a derived value) is refused: capabilities need distinct names.
@read
def retire_count():
    return count(select(Culture))


retire_count.name = "retire"
clash = BehaviorModule(entities=[Culture], actions=[retire], reads=[retire_count])
assert [e.code for e in admit(clash).errors] == ["DUPLICATE_CAPABILITY"]
print("declaring_reads: OK")
