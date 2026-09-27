"""Entity lifecycle in the Python DSL (feature 006): create, remove, exists, referenced, Ref."""

from __future__ import annotations

import json
from decimal import Decimal

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, BehaviorTypeError, Id, Input, Ref, action, admit, create, entity,
    evaluate, field, remove, replay, requires, set_,
)
from .conftest import FIXTURES
from examples.accounts.behavior import model


def test_the_accounts_model_is_a_0_5_module_with_a_reference() -> None:
    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == "0.5"
    account = next(e for e in wire["entities"] if e["name"] == "Account")
    owner = next(f for f in account["fields"] if f["name"] == "owner")
    assert owner["type"] == {"t": "ref", "entity": "Customer"}
    effects = {a["name"]: a["effects"] for a in wire["actions"]}
    assert effects["open_account"][0]["create"] == "Account"
    assert effects["close_account"] == [{"remove": "account", "loc": effects["close_account"][0]["loc"]}]


def test_creation_needs_an_unused_identity() -> None:
    state = {"owner": {"id": "c1", "name": "Ada"}}
    inp = {"account_id": "a9", "initial": Decimal("10.00")}
    ok = evaluate(model, "open_account", state=state, input=inp, data_version="1",
                  facts={"identities": [{"entity": "Account", "id": "a9", "used": False}]})
    assert ok.result == "ALLOW"
    assert ok.lifecycle == [{"op": "create", "entity": "Account", "id": "a9",
                             "value": {"id": "a9", "owner": "c1", "balance": "10.00"}}]
    assert replay(model, ok.record_json).matches
    used = evaluate(model, "open_account", state=state, input=inp, data_version="1",
                    facts={"identities": [{"entity": "Account", "id": "a9", "used": True}]})
    assert used.result == "ENTITY_ID_ALREADY_USED"
    unknown = evaluate(model, "open_account", state=state, input=inp, data_version="1")
    assert unknown.result == "ERROR"
    assert unknown.reasons[0]["code"] == "UNKNOWN_FACT"


def test_removal_and_references() -> None:
    customer = {"customer": {"id": "c1", "name": "Ada"}}
    edge = {"entity": "Account", "id": "a1", "field": "owner"}
    denied = evaluate(model, "remove_customer", state=customer, data_version="1",
                      facts={"references": [{"entity": "Customer", "id": "c1", "incoming": [edge]}]})
    assert denied.result == "DENY"
    free = evaluate(model, "remove_customer", state=customer, data_version="1",
                    facts={"references": [{"entity": "Customer", "id": "c1", "incoming": []}]})
    assert free.result == "ALLOW"
    assert free.lifecycle[0]["op"] == "remove"
    assert free.facts == {"references": [{"entity": "Customer", "id": "c1", "incoming": []}]}
    bad = evaluate(model, "remove_customer", state=customer, data_version="1",
                   facts={"existence": [{"entity": "Customer", "id": "c1", "exists": False}]})
    assert bad.result == "INVALID_INPUT"
    assert bad.reasons[0]["code"] == "INCONSISTENT_FACTS"


def test_the_dsl_model_matches_the_wire_fixture_semantics() -> None:
    # The fixture and the DSL model differ in names and actions, but share entity declarations.
    fixture = json.loads((FIXTURES / "wire/valid/accounts.json").read_text())
    dsl = json.loads(model.to_wire_json())
    names = lambda doc: {e["name"]: [(f["name"], f["type"]) for f in e["fields"]] for e in doc["entities"]}  # noqa: E731
    assert names(fixture) == names(dsl)


@entity
class Thing:
    a = field(int)
    b = field(int)


@entity
class Holder:
    n = field(int)


@entity
class Owner:
    n = field(int)


@entity
class Pet:
    owner = field(Ref[Owner])


@action
def make(h: Holder, *, tid: Input[Id[Thing]]):
    create(Thing, id=tid, a=h.n)


@action
def both(h: Holder):
    set_(h.n, 1)
    remove(h)


@action
def wrong(h: Holder, *, x: Input[int]):
    remove(x)


@action
def adopt(p: Pet, *, o: Input[Ref[Owner]]):
    requires(p.owner == o)


def test_incomplete_creation_is_refused_at_admission() -> None:
    r = admit(BehaviorModule(entities=[Thing, Holder], actions=[make]))
    assert not r.ok
    assert [e.code for e in r.errors] == ["CREATE_INCOMPLETE"]


def test_remove_and_update_conflict_and_remove_needs_an_entity() -> None:
    r = admit(BehaviorModule(entities=[Holder], actions=[both]))
    assert [e.code for e in r.errors] == ["LIFECYCLE_CONFLICT"]
    with pytest.raises(BehaviorDefinitionError):
        BehaviorModule(entities=[Holder], actions=[wrong])


def test_ref_is_only_a_field_type() -> None:
    with pytest.raises(BehaviorTypeError) as e:
        BehaviorModule(entities=[Owner, Pet], actions=[adopt])
    assert e.value.code == "TYPE_MISMATCH"
