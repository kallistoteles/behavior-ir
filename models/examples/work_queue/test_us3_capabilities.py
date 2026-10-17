"""US3: retry, renew, recover, exhaust, fail and cancel (W09–W15; FR-013; M = 1)."""

from __future__ import annotations

from typing import Any

import pytest

from models.examples.work_queue import harness, workflow
from models.examples.work_queue.harness import Op, W, action, claim, complete, work

ADMIN = {"actor": "admin"}


def op(cap: str, ctx: dict[str, Any], inp: dict[str, Any] | None = None,
       extra: dict[str, str] | None = None) -> Op:
    return Op(cap, action(cap), {W: "w1", **(extra or {})}, inp or {}, ctx)


def one(model: Any, item: dict[str, Any], o: Op) -> tuple[str, dict[str, Any]]:
    store = harness.seeded(model, item)
    r = harness.do(store, model, o)
    return r.result, harness.item(store)


LEASED = work("w1", "LEASED", 1, "A", 20)


@pytest.mark.scenario("W09", level=2)
def test_renew_moves_a_live_lease_later(model: Any) -> None:
    r, it = one(model, LEASED, op("renew", {"worker": "A", "now": 10, "lease_until": 30},
                                   {"generation": 1}))
    assert (r, it["lease_until"], it["attempts"], it["status"]) == ("ALLOW", 30, 1, "LEASED")
    r, _ = one(model, LEASED, op("renew", {"worker": "A", "now": 10, "lease_until": 20},
                                  {"generation": 1}))
    assert r == "DENY"  # the same deadline is not later


@pytest.mark.scenario("W10", level=2)
def test_an_expired_lease_cannot_be_renewed(model: Any) -> None:
    r, _ = one(model, LEASED, op("renew", {"worker": "A", "now": 20, "lease_until": 40},
                                  {"generation": 1}))
    assert r == "DENY"


@pytest.mark.scenario("W11", level=2)
def test_retry_returns_work_at_an_explicit_time(model: Any) -> None:
    r, it = one(model, LEASED, op("retry", {"worker": "A", "now": 10, "available_at": 15},
                                   {"generation": 1}))
    assert (r, it["status"], it["attempts"], it["available_at"], it["owner"]) == \
        ("ALLOW", "READY", 1, 15, None)
    r, _ = one(model, LEASED, op("retry", {"worker": "A", "now": 10, "available_at": 9},
                                  {"generation": 1}))
    assert r == "DENY"  # not earlier than now


@pytest.mark.scenario("W12", level=2)
def test_retry_at_the_maximum_is_denied(model: Any) -> None:
    r, _ = one(model, work("w1", "LEASED", workflow.M, "A", 20),
               op("retry", {"worker": "A", "now": 10, "available_at": 15},
                  {"generation": workflow.M}))
    assert r == "DENY"


@pytest.mark.scenario("W13", level=2)
def test_recover_returns_an_expired_lease_with_attempts_left(model: Any) -> None:
    r, it = one(model, LEASED, op("recover", {"now": 20, "available_at": 22, **ADMIN}))
    assert (r, it["status"], it["attempts"], it["available_at"]) == ("ALLOW", "READY", 1, 22)
    r, _ = one(model, LEASED, op("recover", {"now": 19, "available_at": 22, **ADMIN}))
    assert r == "DENY"  # not expired yet
    r, _ = one(model, LEASED, op("recover", {"now": 20, "available_at": 22, "actor": "u1"}))
    assert r == "DENY"  # not authorized


@pytest.mark.scenario("W14", level=2)
def test_exhaust_fails_an_expired_lease_without_attempts_left(model: Any) -> None:
    full = work("w1", "LEASED", workflow.M, "A", 20)
    r, it = one(model, full, op("exhaust", {"now": 20, **ADMIN}))
    assert (r, it["status"], it["owner"], it["lease_until"]) == ("ALLOW", "FAILED", None, None)
    r, _ = one(model, LEASED, op("exhaust", {"now": 20, **ADMIN}))
    assert r == "DENY"  # attempts left: recover applies
    r, _ = one(model, full, op("recover", {"now": 20, "available_at": 20, **ADMIN}))
    assert r == "DENY"  # no attempts left: exhaust applies


@pytest.mark.scenario("W15", level=2)
def test_cancel_needs_authority_and_fences_the_former_owner(model: Any) -> None:
    for item in (work("w1", "READY", 0), LEASED):
        r, it = one(model, item, op("cancel", ADMIN))
        assert (r, it["status"], it["owner"]) == ("ALLOW", "CANCELLED", None)
        r, _ = one(model, item, op("cancel", {"actor": "u1"}))
        assert r == "DENY"
    store = harness.seeded(model, LEASED)
    harness.do(store, model, op("cancel", ADMIN))
    assert harness.do(store, model, complete("x", "w1", "A", 1, now=5)).result == "DENY"


@pytest.mark.scenario("W15", level=2, check="W15.terminal")
def test_terminal_work_accepts_no_capability(model: Any) -> None:
    attempts = []
    for status in ("SUCCEEDED", "FAILED", "CANCELLED"):
        # Every capability is denied, so one store per status serves all of them.
        store = harness.seeded(model, work("w1", status, 1))
        for o in (claim("c", "w1", "A", 1, now=1, until=9),
                  op("renew", {"worker": "A", "now": 1, "lease_until": 9}, {"generation": 1}),
                  complete("c", "w1", "A", 1, now=1),
                  op("retry", {"worker": "A", "now": 1, "available_at": 1}, {"generation": 1}),
                  op("fail", {"worker": "A", "now": 1}, {"generation": 1}),
                  op("recover", {"now": 1, "available_at": 1, **ADMIN}),
                  op("exhaust", {"now": 1, **ADMIN}),
                  op("cancel", ADMIN)):
            r = harness.do(store, model, o)
            attempts.append((status, o.step, r.result, harness.item(store)["status"]))
        assert store.current().position == 0
    assert all(r == "DENY" and s == st for st, _, r, s in attempts), attempts


@pytest.mark.scenario("W14", level=2, check="W14.fail")
def test_the_owner_can_fail_a_live_lease(model: Any) -> None:
    r, it = one(model, LEASED, op("fail", {"worker": "A", "now": 10}, {"generation": 1}))
    assert (r, it["status"], it["attempts"]) == ("ALLOW", "FAILED", 1)


@pytest.mark.scenario("W12", level=2, check="W12.m1")
def test_with_m_1_a_claim_exhausts_the_attempts(model: Any) -> None:
    m1 = workflow.module(m=1)
    store = harness.seeded(m1, work("w1", "READY", 0))
    steps = harness.run(store, m1, claim("c", "w1", "A", 0, now=1, until=9),
                        op("retry", {"worker": "A", "now": 2, "available_at": 3},
                           {"generation": 1}))
    assert [s.result for s in steps] == ["ALLOW", "DENY"]
    exhaust = harness.do(store, m1, op("exhaust", {"now": 9, **ADMIN}))
    assert exhaust.result == "ALLOW" and harness.item(store)["status"] == "FAILED"


@pytest.mark.scenario("W13", level=2, check="W13.expired_read")
def test_the_expired_read_lists_exactly_the_expired_leases(model: Any) -> None:
    store = harness.seeded(model, LEASED, work("w2", "LEASED", 1, "B", 30),
                           work("w3", "READY", 0))
    assert harness.expired(store, model, 25) == ["w1"]
    assert harness.expired(store, model, 30) == ["w1", "w2"]
    assert harness.ready(store, model, 25) == ["w3"]
