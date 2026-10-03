"""The two migrations of the schema evolution example (feature 009).

`broaden` (V1 → V2) only widens: a new enum value, a renamed field, a new optional field, a decimal
`ph`, a four-decimal `Money`, a dropped field, a retired type. `narrow` (V2 → V3) makes `region`
required, which is safe only once every order has one: its source requirement proves it, and
the migration is refused until the data fits.
"""

from __future__ import annotations

from behavior import Migration, Rounding, all_, enum_map, rescale, select, strict_unwrap, underlying

from . import v1, v2, v3

broaden = Migration(
    source=v1.model,
    target=v2.model,
    name="broaden",
    transforms={
        v1.Culture: lambda old: {
            "medium_type": enum_map(old.medium, {
                v1.MediumKind.MS: v2.MediumKind.MS, v1.MediumKind.WPM: v2.MediumKind.WPM,
            }),
            "ph": old.ph,
            "notes": None,
            "price": rescale(underlying(old.price), v2.Money, Rounding.HALF_EVEN),
        },
    },
    drops={v1.Culture: ["legacy_code"]},
    retire=[v1.AuditNote],
)

narrow = Migration(
    source=v2.model,
    target=v3.model,
    name="narrow",
    requires={
        "every_order_has_region": lambda: all_(select(v2.Order), lambda o: o.region.is_some()),
    },
    transforms={v2.Order: lambda old: {"region": strict_unwrap(old.region)}},
)
