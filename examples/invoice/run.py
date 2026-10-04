"""Runs the quickstart.md §2 invoice cases and prints each decision and record."""

from __future__ import annotations

from decimal import Decimal

from behavior import evaluate
from examples.invoice.behavior import model

ANNA = {"id": "anna", "role": "manager", "approval_limit": 50000}


def invoice(amount: object) -> dict[str, object]:
    return {"id": "1042", "amount": amount, "status": "pending", "approved_by": None}


CASES = [
    ("allowed", invoice(Decimal("43200")), ANNA),
    ("above limit", invoice(Decimal("60000")), ANNA),
    ("wrong role", invoice(Decimal("43200")), {**ANNA, "role": "clerk"}),
    ("exact boundary", invoice(Decimal("50000.00")), ANNA),
    ("invalid start", invoice(Decimal("-1")), ANNA),
]

for label, inv, actor in CASES:
    d = evaluate(model, "approve_invoice", state={"invoice": inv}, context={"actor": actor},
                 data_version="18342")
    print(f"== {label}: {d.result}")
    for r in d.reasons:
        print(f"   reason: {r['code']}: {r['message']}")
    for c in d.changes:
        print(f"   change: {c.param}.{c.field}: {c.old!r} -> {c.new!r}")
    print(f"   record: {d.record_json}")

try:
    evaluate(model, "approve_invoice", state={"invoice": invoice(43200.0)},
             context={"actor": ANNA}, data_version="18342")
except TypeError as e:
    print(f"== float input: TypeError: {e}")

print(__import__("time").time_ns())
