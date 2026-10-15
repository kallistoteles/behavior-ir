"""The RequestFlow lifecycle theorem as a property (feature 502, US2: T028; SC-003, research R12).

Random invocations of the reference flow, many of them invalid, run through the public engine.
Committed outcomes are applied to an in-test state; every request's status trace must lie in
`Q_entry · R_T*`: it starts in a creation target and changes only along transition edges."""

from __future__ import annotations

import random
from typing import Any

from behavior import evaluate

from models.fixtures.rf_purchase_request import EDGES, ENTRY, module

M = module()
USERS = ["alice", "bob", "carol"]
TRANSITIONS = ["submit", "start_review", "approve", "reject", "return_for_changes", "cancel"]
STATUSES = ["Draft", "Submitted", "InReview", "Approved", "Rejected", "NeedsChanges",
            "Cancelled"]


def snapshot(requests: dict[str, dict[str, Any]], **facts: Any) -> dict[str, Any]:
    """The complete parent snapshot (Core 017): every declared entity type, the in-test requests
    (without their traces) and the users; `facts` adds further facts (e.g. `identities`)."""
    members = [{k: v for k, v in r.items() if k != "trace"} for r in requests.values()]
    return {"universe": [{"entity": "PurchaseRequest", "members": members},
                         {"entity": "User", "members": [{"id": u, "name": u} for u in USERS]}],
            **facts}


def step(rng: random.Random, requests: dict[str, dict[str, Any]], n: int) -> None:
    actor = rng.choice(USERS)
    if not requests or rng.random() < 0.25:
        creation = rng.choice(sorted(ENTRY))
        rid = f"q{n}"
        d = evaluate(M, f"purchase_request__{creation}", state={}, context={"actor": actor},
                     input={"request_id": rid, "title": "t", "amount": rng.choice([0, 5])},
                     data_version="1",
                     facts=snapshot(requests, identities=[{"entity": "PurchaseRequest",
                                                           "id": rid, "used": False}]))
        if d.result == "ALLOW":
            value = d.lifecycle[0]["value"]
            assert value["status"] in ENTRY.values()
            assert value["status"] == ENTRY[creation]
            requests[rid] = dict(value, id=rid, trace=[value["status"]])
        return
    rid = rng.choice(sorted(requests))
    r = requests[rid]
    transition = rng.choice(TRANSITIONS)
    inputs = {"reviewer": rng.choice(USERS)} if transition == "start_review" else {}
    context = {} if transition in ("start_review", "reject", "return_for_changes") \
        else {"actor": actor}
    state = {k: v for k, v in r.items() if k != "trace"}
    d = evaluate(M, f"purchase_request__{transition}", state={"purchase_request": state},
                 context=context, input=inputs, data_version="1", facts=snapshot(requests))
    if d.result != "ALLOW":
        return
    for c in d.changes:
        if c.field == "status":
            assert (r["status"], c.new) in EDGES, (transition, r["status"], c.new)
            r["trace"].append(c.new)
        r[c.field] = c.new


def test_every_status_trace_is_in_entry_then_transition_edges() -> None:
    rng = random.Random(502)
    statuses_seen: set[str] = set()
    for trace in range(1000):
        requests: dict[str, dict[str, Any]] = {}
        for n in range(rng.randint(1, 8)):
            step(rng, requests, n)
        for r in requests.values():
            t = r["trace"]
            assert t[0] in ENTRY.values()
            assert all((a, b) in EDGES for a, b in zip(t, t[1:])), t
            statuses_seen.update(t)
    # The generator reaches far into the graph, not just the entry states.
    assert {"Approved", "Cancelled", "NeedsChanges"} <= statuses_seen, statuses_seen
