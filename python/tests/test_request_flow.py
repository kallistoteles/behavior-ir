"""RequestFlow through the Python binding (feature 502, US1: T016).

The flow is authored in Python; the Rust model authority declares its request entity, lowers
its creations and lowers its transitions through the StateMachine lowerer. Core runs the whole
lifecycle; every outcome here is an ordinary Core outcome."""

from __future__ import annotations

from typing import Any

import pytest

from behavior import ModelBuildError, evaluate
from behavior.types import STRING, IdT


from models.fixtures.rf_purchase_request import S, flow, module as build

M = build()


def request(status: str, assignee: str | None = None) -> dict[str, Any]:
    return {"id": "q1", "requester": "alice", "assignee": assignee, "status": status,
            "title": "Laptop", "amount": 900}


def test_the_generated_items_are_ordinary() -> None:
    receipt, = M.model_receipts
    assert receipt.model_kind == "request_flow"
    assert receipt.model["state_machine"]["document"]["entry_states"] == ["Draft", "Submitted"]


def test_creation_starts_in_the_target_state() -> None:
    d = evaluate(M, "purchase_request__create", state={},
                 context={"actor": "alice"},
                 input={"request_id": "q1", "title": "Laptop", "amount": 900},
                 data_version="1",
                 facts={"identities": [{"entity": "PurchaseRequest", "id": "q1",
                                        "used": False}]})
    assert d.result == "ALLOW", d
    value = d.lifecycle[0]["value"]
    assert value["status"] == "Draft"
    assert value["requester"] == "alice"


def test_a_creation_guard_is_an_ordinary_guard() -> None:
    d = evaluate(M, "purchase_request__create", state={},
                 context={"actor": "alice"},
                 input={"request_id": "q1", "title": "Laptop", "amount": 0},
                 data_version="1",
                 facts={"identities": [{"entity": "PurchaseRequest", "id": "q1",
                                        "used": False}]})
    assert d.result != "ALLOW"


def test_the_reference_lifecycle_runs_end_to_end() -> None:
    steps = [
        ("submit", {}, {"actor": "alice"}, "Submitted", None),
        ("start_review", {"reviewer": "bob"}, {}, "InReview", "bob"),
        ("return_for_changes", {}, {}, "NeedsChanges", "bob"),
        ("submit", {}, {"actor": "alice"}, "Submitted", "bob"),
        ("start_review", {"reviewer": "bob"}, {}, "InReview", "bob"),
        ("approve", {}, {"actor": "bob"}, "Approved", "bob"),
    ]
    status, assignee = "Draft", None
    for transition, inp, ctx, target, after in steps:
        d = evaluate(M, f"purchase_request__{transition}",
                     state={"purchase_request": request(status, assignee)},
                     context=ctx, input=inp, data_version="1")
        assert d.result == "ALLOW", (transition, d)
        assert ("status", target) in [(c.field, c.new) for c in d.changes]
        status, assignee = target, after


def test_authority_is_explicit() -> None:
    # Only the authored guards decide: the assignee approves, nobody else.
    d = evaluate(M, "purchase_request__approve",
                 state={"purchase_request": request("InReview", "bob")},
                 context={"actor": "alice"}, input={}, data_version="1")
    assert d.result != "ALLOW"
    # A transition outside its source states is refused by the StateMachine guard.
    d = evaluate(M, "purchase_request__approve",
                 state={"purchase_request": request("Draft", "bob")},
                 context={"actor": "bob"}, input={}, data_version="1")
    assert d.result != "ALLOW"


def test_the_request_entity_handle_is_typed() -> None:
    f = flow()
    assert f.request_entity.__name__ == "PurchaseRequest"
    build(f)  # the drift check passes against the schema Rust declared


def test_model_errors_surface_as_model_build_errors() -> None:
    f = flow()
    f.create("submit", to=S.DRAFT, values=lambda request_id: {})
    with pytest.raises(ModelBuildError) as e:
        build(f)
    assert [x["code"] for x in e.value.errors] == ["CAPABILITY_ID_COLLISION"]


#: The reference flow's source sets and the actor its guard admits (None: unguarded), written
#: out independently of the model so the test does not read its oracle from the build.
SOURCES = {
    "submit": ({"Draft", "NeedsChanges"}, "alice"),
    "start_review": ({"Submitted"}, None),
    "approve": ({"InReview"}, "bob"),
    "reject": ({"InReview"}, None),
    "return_for_changes": ({"InReview"}, None),
    "cancel": ({"Draft", "Submitted", "NeedsChanges"}, "alice"),
}


@pytest.mark.parametrize("transition", sorted(SOURCES))
@pytest.mark.parametrize("status", [s.value for s in S])
def test_a_transition_commits_iff_its_source_set_and_guards_allow(
    status: str, transition: str
) -> None:
    """SC-002: for every status × transition pair, commit ⇔ status ∈ F_t ∧ ⋀G_t."""
    sources, admitted = SOURCES[transition]
    inp = {"reviewer": "bob"} if transition == "start_review" else {}
    actors = [None] if admitted is None else [admitted, "carol"]
    for actor in actors:
        d = evaluate(M, f"purchase_request__{transition}",
                     state={"purchase_request": request(status, assignee="bob")},
                     context={} if actor is None else {"actor": actor},
                     input=inp, data_version="1")
        expected = status in sources and actor in (None, admitted)
        assert (d.result == "ALLOW") == expected, (status, transition, actor, d)
        if expected:
            _, target = next((a, b) for a, b in EDGES_BY[transition] if a == status)
            assert [(c.field, c.new) for c in d.changes if c.field == "status"] \
                == [("status", target)]
        else:
            assert not d.changes


#: Each transition's target (the edges of `R_T`, per transition).
EDGES_BY = {
    t: {(s, {"submit": "Submitted", "start_review": "InReview", "approve": "Approved",
             "reject": "Rejected", "return_for_changes": "NeedsChanges",
             "cancel": "Cancelled"}[t]) for s in sources}
    for t, (sources, _) in SOURCES.items()
}


@pytest.mark.parametrize("creation, target", [("create", "Draft"),
                                              ("import_submitted", "Submitted")])
def test_every_creation_starts_in_exactly_its_target(creation: str, target: str) -> None:
    """SC-002: a creation's new request has exactly its declared initial status."""
    d = evaluate(M, f"purchase_request__{creation}", state={},
                 context={"actor": "alice"},
                 input={"request_id": "q1", "title": "Laptop", "amount": 900},
                 data_version="1",
                 facts={"identities": [{"entity": "PurchaseRequest", "id": "q1",
                                        "used": False}]})
    assert d.result == "ALLOW", d
    created, = d.lifecycle
    assert created["value"]["status"] == target


def _drift(edit: Any) -> Any:
    """A reference flow whose request-entity handle is edited after construction."""
    from behavior.decl import EntityDecl

    f = flow()
    d = f.request_entity.__behavior_entity__
    name, fields = edit(d.name, list(d.fields))
    f.request_entity.__behavior_entity__ = EntityDecl(name, fields, d.loc)
    return f


@pytest.mark.parametrize("edit", [
    pytest.param(lambda n, fs: (n + "X", fs), id="entity name"),
    pytest.param(lambda n, fs: (n, [(fn, IdT("User") if fn == "amount" else t, loc)
                                    for fn, t, loc in fs]), id="field type"),
    pytest.param(lambda n, fs: (n, [x for x in fs if x[0] != "title"]), id="missing field"),
    pytest.param(lambda n, fs: (n, fs + [("colour", STRING, fs[0][2])]), id="extra field"),
])
def test_a_drifted_request_entity_handle_fails_the_build(edit: Any) -> None:
    """Research R11: the Python view of the request entity is checked against the schema Rust
    declares in P2; any difference fails the build instead of being silently used."""
    with pytest.raises(RuntimeError, match="out of step with the model authority"):
        build(_drift(edit))


def test_an_unchanged_handle_binds() -> None:
    f = flow()
    build(f)  # bind_schema accepts the view Rust declared
    d = f.request_entity.__behavior_entity__
    assert d.name == "PurchaseRequest"
    assert sorted(n for n, _, _ in d.fields) == ["amount", "assignee", "requester", "status",
                                                  "title"]
