"""The Python form of the core's migration conformance document `cultures_v1_to_v2`
(`tests/fixtures/migration/valid/cultures_v1_to_v2.json` of the pinned Core Release), for the
equivalence test (feature 011, SC-004)."""

from __future__ import annotations

from behavior import Migration, Rounding, enum_map, rescale, underlying

from . import cultures_v1 as v1
from . import cultures_v2 as v2

migration = Migration(
    source=v1.model,
    target=v2.model,
    name="cultures_v1_to_v2",
    transforms={
        v1.Culture: lambda old: {
            "medium_type": enum_map(old.medium, {
                v1.MediumKind.MS: v2.MediumKind.MS, v1.MediumKind.WPM: v2.MediumKind.WPM,
            }),
            "ph": old.ph,
            "notes": None,
            "price": rescale(underlying(old.price), v2.Money, Rounding.HALF_EVEN),
            "fee": rescale(underlying(old.fee), v2.Fine, Rounding.HALF_EVEN),
        },
        v1.Order: lambda old: {"buyer": old.customer},
    },
    drops={v1.Culture: ["legacy_code"]},
    retire=[v1.AuditNote],
)
