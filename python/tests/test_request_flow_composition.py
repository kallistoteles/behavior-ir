"""Hand-written behavior sees the generated request entity (feature 502, US5: T046; law D5).

The request entity is declared in P2, before any hand-written item is traced, so actions,
authored entities, reads, queries and guards can all name it. Everything here is admitted and
runs as ordinary Core behavior."""

from __future__ import annotations

from typing import Any

from behavior import (
    BehaviorModule, Id, Input, Ref, action, count, create, entity, evaluate, evaluate_read,
    field, read, requires, select, set_,
)

from models.fixtures.rf_purchase_request import S, User, flow

F = flow()
PR = F.request_entity


@entity
class Comment:
    on = field(Id[PR])
    text = field(str)


@entity
class Attachment:
    of = field(Ref[PR])
    name = field(str)


@action
def retitle(request: PR, *, title: Input[str], actor: Input[Id[User]]):
    # A guard reading request fields, authored like any other.
    requires(request.status == S.DRAFT)
    requires(request.requester == actor)
    set_(request.title, title)


@action
def comment(*, id: Input[Id[Comment]], on: Input[Id[PR]], text: Input[str]):
    create(Comment, id=id, on=on, text=text)


@read
def open_requests():
    return count(select(PR).where(lambda r: r.status == S.DRAFT))


@read
def title_of(request: PR):
    return request.title


M = BehaviorModule(entities=[User, Comment, Attachment], enums=[S], models=[F],
                   actions=[retitle, comment], reads=[open_requests, title_of])


def request(status: str = "Draft", **over: Any) -> dict[str, Any]:
    return {"id": "q1", "requester": "alice", "assignee": None, "status": status,
            "title": "Laptop", "amount": 900, **over}


def snapshot(*requests: dict[str, Any], **facts: Any) -> dict[str, Any]:
    """The complete parent snapshot (Core 017): every declared entity type, the given requests
    and two users; `facts` adds further facts (e.g. `identities`)."""
    users = [{"id": u, "name": u} for u in ("alice", "bob")]
    return {"universe": [{"entity": "Attachment", "members": []},
                         {"entity": "Comment", "members": []},
                         {"entity": "PurchaseRequest", "members": list(requests)},
                         {"entity": "User", "members": users}], **facts}


def test_everything_is_admitted_as_ordinary_items() -> None:
    assert M.model_receipts[0].model_kind == "request_flow"


def test_a_hand_written_action_over_the_request_entity() -> None:
    ok = evaluate(M, "retitle", state={"request": request()}, context={},
                  input={"title": "Desk", "actor": "alice"}, data_version="1",
                  facts=snapshot(request()))
    assert ok.result == "ALLOW"
    assert [(c.field, c.new) for c in ok.changes] == [("title", "Desk")]
    for r, actor in ((request("Submitted"), "alice"), (request(), "bob")):
        denied = evaluate(M, "retitle", state={"request": r}, context={},
                          input={"title": "Desk", "actor": actor}, data_version="1",
                          facts=snapshot(r))
        assert denied.result == "DENY"


def test_an_input_identity_of_the_request_entity() -> None:
    d = evaluate(M, "comment", state={}, context={},
                 input={"id": "c1", "on": "q1", "text": "ok?"}, data_version="1",
                 facts=snapshot(request(),
                                identities=[{"entity": "Comment", "id": "c1", "used": False}]))
    assert d.result == "ALLOW", d.reasons
    assert d.lifecycle[0]["value"]["on"] == "q1"


def test_reads_and_queries_over_the_request_entity() -> None:
    universe = {"universe": [{"entity": "PurchaseRequest",
                              "members": [request(), request("Approved", id="q2")]}]}
    r = evaluate_read(M, open_requests, data_version="1", facts=universe)
    assert r.value == 1
    r = evaluate_read(M, title_of, data_version="1", state={"request": request()})
    assert r.value == "Laptop"
