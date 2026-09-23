"""Admission of DSL modules by the engine, and content-addressed identity."""

from __future__ import annotations

import json

from behavior import admit

from .conftest import FIXTURES
from .fixtures import (
    cycles, invoice_model, invoice_model_shifted, invoice_model_swapped, margin_model,
    margin_model_renamed,
)

VERSIONS = json.loads((FIXTURES / "versions.json").read_text())


def test_models_are_admitted() -> None:
    inv = admit(invoice_model.model)
    assert inv.ok, inv.errors
    mar = admit(margin_model.model)
    assert mar.ok, mar.errors
    assert mar.evaluation_order == ["margin", "high_risk"]


def test_cycle_is_reported_with_all_locations() -> None:
    result = admit(cycles.model)
    assert not result.ok
    assert result.behavior_version is None
    assert [e.code for e in result.errors] == ["CYCLE"]
    err = result.errors[0]
    assert err.message == "derived values form a cycle: a → b → c → a"
    assert len(err.related_locs) == 3


def test_versions_match_golden() -> None:
    assert admit(invoice_model.model).behavior_version == VERSIONS["invoice"]["behavior_version"]
    assert admit(margin_model.model).behavior_version == VERSIONS["project_margin"]["behavior_version"]
    assert admit(margin_model.model).items == VERSIONS["project_margin"]["items"]


def test_comments_and_lines_do_not_change_identity() -> None:
    base = admit(invoice_model.model).behavior_version
    assert admit(invoice_model_shifted.model).behavior_version == base


def test_declaration_order_does_not_change_identity() -> None:
    base = admit(invoice_model.model).behavior_version
    reordered = invoice_model.build_model(
        entities=[invoice_model.Invoice, invoice_model.User],
        actions=[invoice_model.apply_discount, invoice_model.approve_invoice],
    )
    assert admit(reordered).behavior_version == base


def test_rename_keeps_item_hash_but_changes_version() -> None:
    base = admit(margin_model.model)
    renamed = admit(margin_model_renamed.model)
    assert renamed.items["derived:gross_margin"] == base.items["derived:margin"]
    assert renamed.items["derived:high_risk"] == base.items["derived:high_risk"]
    assert renamed.behavior_version != base.behavior_version


def test_swapped_preconditions_change_identity() -> None:
    base = admit(invoice_model.model)
    swapped = admit(invoice_model_swapped.model)
    assert swapped.items["action:approve_invoice"] != base.items["action:approve_invoice"]
    assert swapped.behavior_version != base.behavior_version
