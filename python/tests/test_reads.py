"""First-class reads in the Python DSL (feature 010): `@read`, plain evaluation with
`evaluate_read`, ad-hoc reads, and store reads that never write."""

from __future__ import annotations

import json
from typing import Any

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, InMemoryBackend, ReadRecord, ReadResult, Store,
    count, create, ensures, evaluate_read, project, read, remove, requires, select, set_,
)
from examples.lab_reads import model as lab
from examples.lab_reads.model import Culture, Customer, Order, model


def culture(id_: str, active: bool, measurements: int = 0, ph_total: int = 0) -> dict[str, Any]:
    return {"id": id_, "name": f"culture {id_}", "stage": None, "active": active,
            "measurements": measurements, "ph_total": ph_total}


def universe(entity_name: str, members: list[dict[str, Any]]) -> dict[str, Any]:
    return {"entity": entity_name, "members": members}


CULTURES = {"universe": [universe("Culture", [
    culture("c1", True, 2, 14), culture("c2", False), culture("c3", True, 1, 6)])]}


def test_reads_are_traced_into_the_module_at_wire_0_7() -> None:
    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == "0.7"
    names = sorted(r["name"] for r in wire["reads"])
    assert names == ["active_count", "active_cultures", "average_ph", "big_orders",
                     "culture_exists", "customer_summary", "open_total", "order_view",
                     "smallest_order"]
    open_total = next(r for r in wire["reads"] if r["name"] == "open_total")
    assert open_total["params"] == [
        {"name": "customer", "role": "state", "type": {"t": "entity", "name": "Customer"}}]


def test_a_declared_read_in_plain_mode() -> None:
    r = evaluate_read(model, "active_count", data_version="test:1", facts=CULTURES)
    assert isinstance(r, ReadResult)
    assert r.result == "VALUE"
    assert r.value == 2
    assert r.record_id.startswith("read:sha256:")
    assert isinstance(r.record, ReadRecord)
    assert r.record.as_dict()["read"] == {
        "name": "active_count", "hash": r.record.as_dict()["read"]["hash"], "declared": True}
    assert r.response.record_id == r.record_id
    # The function and its name are the same declared read.
    from examples.lab_reads.model import active_count
    again = evaluate_read(model, active_count, data_version="test:1", facts=CULTURES)
    assert again.record.to_json() == r.record.to_json()


def test_bound_entities_and_errors() -> None:
    orders = {"universe": [universe("Order", [
        {"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"},
        {"id": "o2", "customer": "k1", "amount": 4, "status": "CLOSED"}])]}
    r = evaluate_read(model, "open_total", state={"customer": {"id": "k1", "name": "Ada",
                                                               "credit_limit": 100}},
                      data_version="test:1", facts=orders)
    assert r.result == "VALUE" and r.value == 30
    missing = evaluate_read(model, "open_total", data_version="test:1")
    assert missing.result == "INVALID_INPUT"
    assert [x["code"] for x in missing.reasons] == ["MISSING_ARGUMENT"]
    zero = evaluate_read(model, "average_ph", data_version="test:1",
                         facts={"universe": [universe("Culture", [])]})
    assert zero.result == "EVALUATION_ERROR"
    assert zero.value is None


def test_an_undeclared_read_is_an_ad_hoc_read() -> None:
    @read
    def customers():
        return count(select(Customer))

    r = evaluate_read(model, customers, data_version="test:1",
                      facts={"universe": [universe("Customer", [
                          {"id": "k1", "name": "Ada", "credit_limit": 1}])]})
    assert r.result == "VALUE" and r.value == 1
    rec = r.record.as_dict()
    assert rec["read"]["declared"] is False
    assert rec["read"]["definition"]["read"]["name"] == "customers"
    # An ad-hoc read never enters the module.
    assert "customers" not in [x["name"] for x in json.loads(model.to_wire_json())["reads"]]


@pytest.mark.parametrize("statement", ["set_", "create", "remove", "requires", "ensures"])
def test_a_read_cannot_contain_effects_or_conditions(statement: str) -> None:
    def body(c: Culture) -> Any:
        if statement == "set_":
            set_(c.active, False)
        elif statement == "create":
            create(Culture, id=c.id, name="x", stage=None, active=True, measurements=0,
                   ph_total=0)
        elif statement == "remove":
            remove(c)
        elif statement == "requires":
            requires(c.active)
        else:
            ensures(c.active)
        return c.measurements

    body.__name__ = f"bad_{statement}"
    with pytest.raises(BehaviorDefinitionError, match="read"):
        BehaviorModule(entities=[Culture, Customer, Order], reads=[read(body)])


def _lab_store() -> Store:
    seed = [
        {"entity": "Culture", "value": culture("c1", True, 2, 14)},
        {"entity": "Culture", "value": culture("c2", False)},
        {"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}},
        {"entity": "Order", "value": {"id": "o1", "customer": "k1", "amount": 30,
                                      "status": "OPEN"}},
    ]
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed=seed))


def test_a_store_read_never_writes() -> None:
    store = _lab_store()
    head = store.current()
    history = list(store.history())
    r = store.read(model, "open_total", bindings={"customer": "k1"})
    assert r.result == "VALUE" and r.value == 30
    assert r.record.as_dict()["data_version"] == store.data_version()
    for _ in range(20):
        store.read(model, "active_count")
    assert store.current() == head
    assert list(store.history()) == history
    unknown = store.read(model, "open_total", bindings={"customer": "nobody"})
    assert unknown.result == "INVALID_BINDING"


def test_entities_are_not_read_parameters_without_a_module() -> None:
    # `@read` registers without running; an entity class used as a parameter is a bound entity.
    @read
    def cultures_of(c: Culture):
        return c.measurements

    assert cultures_of.name == "cultures_of"
    assert [p.role for p in cultures_of.params()] == ["state"]


# --- projections (US2) -------------------------------------------------------------------------

def _projection_model() -> BehaviorModule:
    @read
    def active_cultures():
        return project(select(lab.Culture).where(lambda c: c.active),
                       lambda c: [c.name, c.stage, lab.ph_avg(c)])

    @read
    def order_view(order: lab.Order):
        return project(order, lambda o: [o.status, o.amount])

    return BehaviorModule(
        entities=[lab.Culture, lab.Customer, lab.Order],
        enums=[lab.Stage, lab.OrderStatus],
        derived=[lab.ph_avg, lab.exposure, lab.standing],
        reads=[active_cultures, order_view],
    )


def test_a_query_projection_is_a_list_of_records() -> None:
    m = _projection_model()
    facts = {"universe": [universe("Culture", [
        culture("c3", True, 2, 12), culture("c1", True, 2, 14), culture("c2", False)])]}
    r = evaluate_read(m, "active_cultures", data_version="t", facts=facts)
    assert r.result == "VALUE", r.reasons
    assert [row["id"] for row in r.value] == ["c1", "c3"]
    assert set(r.value[0]) == {"id", "name", "stage", "ph_avg"}
    assert r.value[0]["stage"] is None


def test_an_entity_projection_is_one_record() -> None:
    m = _projection_model()
    r = evaluate_read(m, "order_view", data_version="t",
                      state={"order": {"id": "o1", "customer": "k1", "amount": 30,
                                       "status": "OPEN"}})
    assert r.value == {"id": "o1", "status": "OPEN", "amount": 30}


@pytest.mark.parametrize("bad", ["computed", "foreign_derived"])
def test_a_projection_item_is_a_field_or_a_derived_value_of_the_member(bad: str) -> None:
    def body(customer: lab.Customer) -> Any:
        if bad == "computed":
            return project(select(lab.Order), lambda o: [o.amount + 1])
        return project(select(lab.Order), lambda o: [lab.exposure(customer)])

    body.__name__ = f"bad_{bad}"
    with pytest.raises(BehaviorDefinitionError, match="UNKNOWN_PROJECTION_ITEM|projection"):
        BehaviorModule(entities=[lab.Culture, lab.Customer, lab.Order],
                       enums=[lab.Stage, lab.OrderStatus],
                       derived=[lab.ph_avg, lab.exposure, lab.standing], reads=[read(body)])
