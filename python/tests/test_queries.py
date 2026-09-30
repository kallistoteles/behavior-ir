"""Relational queries in the Python DSL (feature 007): select/where/set algebra, the relational
operators, module invariants, query facts, and the store."""

from __future__ import annotations

import json
from decimal import Decimal

import pytest

from behavior import (
    BehaviorDefinitionError, BehaviorModule, BehaviorTypeError, CommitRefused, InMemoryBackend,
    Store, admit, constraint, count, entity, evaluate, field, invariant, min_, replay, requires,
    action, select, unique,
)
from examples.orders.behavior import Customer, Order, model

CUSTOMER = {"id": "c1", "name": "Ada", "credit_limit": "100.00", "region": "north"}


def order(id_: str, amount: str, status: str = "open", customer: str = "c1") -> dict[str, str]:
    return {"id": id_, "customer": customer, "amount": amount, "status": status, "region": "north"}


def universe(orders: list[dict[str, str]]) -> dict[str, object]:
    return {"universe": [{"entity": "Order", "members": orders}]}


def test_the_orders_model_is_a_0_6_module_with_a_module_invariant() -> None:
    assert admit(model).ok
    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == "0.6"
    inv = next(i for i in wire["invariants"] if i["name"] == "personnel_numbers_unique")
    assert "entity" not in inv and "param" not in inv
    assert inv["body"]["op"] == "unique"


def test_decisions_over_sets_use_and_record_query_facts() -> None:
    state = {"customer": CUSTOMER}
    closed = evaluate(model, "close_customer", state=state, data_version="1",
                      facts=universe([order("o1", "10.00", "closed")]))
    assert closed.result == "ALLOW"
    assert [q["members"] for q in closed.facts["queries"]] == [[]]
    assert replay(model, closed.record_json).matches
    blocked = evaluate(model, "close_customer", state=state, data_version="1",
                       facts=universe([order("o1", "10.00", "closed"), order("o2", "5.00")]))
    assert blocked.result == "DENY"
    assert [q["members"] for q in blocked.facts["queries"]] == [[{"id": "o2"}]]
    # The observed facts alone decide the same way (no universe needed).
    again = evaluate(model, "close_customer", state=state, data_version="1", facts=blocked.facts)
    assert again.record_json == blocked.record_json


def test_a_missing_query_fact_is_unknown_never_guessed() -> None:
    d = evaluate(model, "close_customer", state={"customer": CUSTOMER}, data_version="1")
    assert d.result == "ERROR"
    assert d.reasons[0]["code"] == "UNKNOWN_FACT"


def test_exact_sums_and_resulting_state_queries() -> None:
    state = {"customer": CUSTOMER}
    facts = universe([order("o1", "60.00")])
    facts["identities"] = [{"entity": "Order", "id": "o9", "used": False}]
    ok = evaluate(model, "place_order", state=state, input={"order_id": "o9", "amount": Decimal("40.00")},
                  data_version="1", facts=facts)
    assert ok.result == "ALLOW"
    post = next(s for s in ok.trace if s.phase == "postcondition")
    assert "100.00" in post.reads.values()
    over = evaluate(model, "place_order", state=state,
                    input={"order_id": "o9", "amount": Decimal("40.01")}, data_version="1", facts=facts)
    assert over.result == "DENY"


def test_the_module_invariant_refuses_a_duplicate_on_the_resulting_state() -> None:
    d = evaluate(model, "renumber", state={"employee": {"id": "e1", "personnel_number": "N1"}},
                 input={"number": "N2"}, data_version="1",
                 facts={"universe": [{"entity": "Employee", "members": [
                     {"id": "e1", "personnel_number": "N1"}, {"id": "e2", "personnel_number": "N2"}]}]})
    assert d.result == "DENY"
    assert d.reasons[0]["code"] == "INVARIANT_VIOLATED"


def test_queries_are_not_python_collections() -> None:
    @action
    def bad(customer: Customer):  # noqa: ANN202
        orders = select(Order)
        for _ in orders:
            pass

    with pytest.raises(BehaviorDefinitionError, match="not a Python collection"):
        BehaviorModule(entities=[Customer, Order], actions=[bad])

    @action
    def bad_len(customer: Customer):  # noqa: ANN202
        requires(len(select(Order)) == 0)  # type: ignore[arg-type]

    with pytest.raises(BehaviorDefinitionError):
        BehaviorModule(entities=[Customer, Order], actions=[bad_len])


def test_set_algebra_combines_one_entity_type() -> None:
    @action
    def mixed(customer: Customer):  # noqa: ANN202
        requires(count(select(Order).union(select(Customer))) == 0)

    with pytest.raises(BehaviorTypeError) as e:
        BehaviorModule(entities=[Customer, Order], actions=[mixed])
    assert e.value.code == "TYPE_MISMATCH"


def test_extrema_are_optional_and_lambdas_take_one_candidate() -> None:
    @action
    def cheapest(customer: Customer):  # noqa: ANN202
        requires(min_(select(Order), lambda o: o.amount).is_none())

    m = BehaviorModule(entities=[Customer, Order], actions=[cheapest])
    d = evaluate(m, "cheapest", state={"customer": CUSTOMER}, data_version="1", facts=universe([]))
    assert d.result == "ALLOW"

    @action
    def two(customer: Customer):  # noqa: ANN202
        requires(min_(select(Order), lambda o, p: o.amount).is_none())  # type: ignore[misc]

    with pytest.raises(BehaviorDefinitionError, match="exactly one parameter"):
        BehaviorModule(entities=[Customer, Order], actions=[two])


def test_queries_belong_in_module_invariants_and_action_conditions() -> None:
    @constraint
    def few_orders(o: Order):  # noqa: ANN202
        return count(select(Order)) >= 0

    m = BehaviorModule(entities=[Customer, Order], constraints=[few_orders])
    assert [e.code for e in admit(m).errors] == ["QUERY_NOT_ALLOWED"]

    @invariant
    def limit_is_one_customer():  # noqa: ANN202
        return count(select(Customer)) <= 1

    ok = BehaviorModule(entities=[Customer], invariants=[limit_is_one_customer])
    assert admit(ok).ok


def test_the_store_answers_queries_and_checks_module_invariants_at_genesis() -> None:
    seed = [{"entity": "Customer", "value": CUSTOMER},
            {"entity": "Order", "value": order("o1", "10.00", "closed")},
            {"entity": "Employee", "value": {"id": "e1", "personnel_number": "N1"}}]
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
    ev = store.evaluate(model, "close_customer", bindings={"customer": "c1"}, input={},
                        commit_time="2026-09-27T12:00:00Z")
    assert ev.decision.result == "ALLOW"
    bad = seed + [{"entity": "Employee", "value": {"id": "e2", "personnel_number": "N1"}}]
    with pytest.raises(CommitRefused) as e:
        Store.create(InMemoryBackend(), model, Store.genesis_for(model, bad))
    assert e.value.code == "GENESIS_INVALID"


def test_a_module_invariant_over_a_new_entity_type() -> None:
    @entity
    class Tag:
        label = field(str)

    @invariant
    def labels_unique():  # noqa: ANN202
        return unique(select(Tag), by=lambda t: t.label)

    m = BehaviorModule(entities=[Tag], invariants=[labels_unique])
    assert admit(m).ok
    wire = json.loads(m.to_wire_json())
    assert wire["invariants"][0]["body"]["op"] == "unique"
