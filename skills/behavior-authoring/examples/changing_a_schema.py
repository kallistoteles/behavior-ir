"""Changing a schema: a migration between two modules, with automatic copies, explicit
conversions, an acknowledged drop, a retirement, and a narrowing proven by a requirement."""

from decimal import Decimal
from enum import Enum
from types import SimpleNamespace

from behavior import (
    BehaviorModule, Input, Migration, Option, action, all_, entity, enum_map, field, select, set_,
    strict_unwrap,
)


def generation_1() -> SimpleNamespace:
    class Medium(Enum):
        MS = "MS"
        WPM = "WPM"

    @entity
    class Culture:
        medium = field(Medium)
        ph = field(int)
        legacy_code = field(str)

    @entity
    class Order:
        region = field(Option[str])

    @entity
    class AuditNote:
        text = field(str)

    model = BehaviorModule(entities=[Culture, Order, AuditNote])
    return SimpleNamespace(Medium=Medium, Culture=Culture, Order=Order, AuditNote=AuditNote,
                           model=model)


def generation_2() -> SimpleNamespace:
    class Medium(Enum):
        MS = "MS"
        WPM = "WPM"
        B5 = "B5"

    @entity
    class Culture:
        medium_type = field(Medium)
        ph = field(Decimal)
        notes = field(Option[str])

    @entity
    class Order:
        region = field(Option[str])

    @action
    def set_region(order: Order, *, region: Input[str]):
        set_(order.region, region)

    model = BehaviorModule(entities=[Culture, Order], actions=[set_region])
    return SimpleNamespace(Medium=Medium, Culture=Culture, Order=Order, model=model)


def generation_3() -> SimpleNamespace:
    @entity
    class Order:
        region = field(str)

    g2 = generation_2()
    model = BehaviorModule(entities=[g2.Culture, Order])
    return SimpleNamespace(Order=Order, model=model)


v1, v2, v3 = generation_1(), generation_2(), generation_3()

# Broaden: every changed type gets a transform; unchanged fields are copied automatically.
transforms = {
    v1.Culture: lambda old: {
        "medium_type": enum_map(old.medium, {
            v1.Medium.MS: v2.Medium.MS, v1.Medium.WPM: v2.Medium.WPM,
        }),
        "ph": old.ph,
        "notes": None,
    },
}
broaden = Migration(
    source=v1.model,
    target=v2.model,
    transforms=transforms,
    drops={v1.Culture: ["legacy_code"]},
    retire=[v1.AuditNote],
)
assert broaden.admit().ok
assert broaden.summary()["types"]["Culture"] == {
    "copied": ["id"], "transformed": ["medium_type", "ph"], "new": ["notes"],
    "dropped": ["legacy_code"],
}

# Narrow: only under a source requirement that proves every order already has a region.
narrow = Migration(
    source=v2.model,
    target=v3.model,
    requires={
        "every_order_has_region": lambda: all_(select(v2.Order), lambda o: o.region.is_some()),
    },
    transforms={v2.Order: lambda old: {"region": strict_unwrap(old.region)}},
)
assert narrow.admit().ok

# A removed field nobody reads must be dropped explicitly: information loss is never silent.
no_drop = Migration(source=v1.model, target=v2.model, transforms=transforms, retire=[v1.AuditNote])
assert [e.code for e in no_drop.admit().errors] == ["UNACKNOWLEDGED_FIELD_DROP"]
print("ok")
