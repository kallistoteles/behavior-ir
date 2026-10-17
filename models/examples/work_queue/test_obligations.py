"""The evidence behind the verification obligations VO-001..VO-012 (spec, Verification
obligations; research R10; contracts/safety-arguments.md).

Each check here is one dependency of an obligation in report.OBLIGATIONS:

- `SMT.*`: the pinned verifier's per-action results, in full mode;
- `SA-*`: the structural arguments' premises, each tied to executable enforcement (build errors
  and the lowered IR); the negative controls are separate dependencies;
- `SA.genesis`, `SA-EXCL`, `BACKEND`: Core and Store contracts;
- `PROP.mono`: seeded random histories through the Store;
- `ASSUMPTIONS`: every obligation that relies on an assumption names it.

This file deliberately has no `from __future__ import annotations`: the writer premises define
actions inside functions, so their annotations must be evaluated eagerly."""

import json
import random
from typing import Any

import pytest

from behavior import (
    CommitRefused, InMemoryBackend, ModelBuildError, Profile, _engine, action, remove,
    run_conformance, set_, verify,
)
from models.examples.work_queue import harness, report, workflow
from models.examples.work_queue.harness import Op, W, action as act, claim, complete, work
from models.examples.work_queue.report import (
    Inconclusive, Unsupported, full_mode, readiness_entry, record_evidence,
)

TRANSITIONS = ("claim", "renew", "complete", "retry", "fail", "recover", "exhaust", "cancel")
TERMINAL = {"SUCCEEDED", "FAILED", "CANCELLED"}


# --- SMT (full mode) -----------------------------------------------------------------------

_ATTESTATION: dict[str, Any] = {}


def _attestation() -> Any:
    if not full_mode():
        raise Unsupported("static verification runs in full mode", "environment")
    notice = _engine.solver_notice()
    if notice is not None:
        raise Unsupported(f"no solver: {notice}", "environment")
    if "a" not in _ATTESTATION:
        model = workflow.module()
        _ATTESTATION["a"] = (model, verify(model, Profile()))
    return _ATTESTATION["a"]


def _outcomes(kind: str, actions: tuple[str, ...]) -> dict[str, list[Any]]:
    model, a = _attestation()
    assert a.data.get("behavior_version") == model.behavior_version
    out: dict[str, list[Any]] = {}
    for c in a.checks:
        name = (c.get("action") or {}).get("name")
        if c.get("kind") == kind and name in actions:
            out.setdefault(name, []).append(c.get("outcome"))
    return out


def _require_proven(check: str, kind: str, actions: tuple[str, ...]) -> None:
    got = _outcomes(kind, actions)
    record_evidence(check, {k: sorted(json.dumps(o) for o in v) for k, v in sorted(got.items())})
    missing = [a for a in actions if a not in got]
    assert not missing, f"no {kind} check for {missing}"
    open_ = {a: o for a, os in got.items() for o in os if o != "proven"}
    if open_ and all(o == "inconclusive" or (isinstance(o, dict) and "inconclusive" in json.dumps(o))
                     for o in open_.values()):
        entry = readiness_entry(check)
        raise Inconclusive(f"{kind} not proven: {open_}", entry["issue"] if entry else "core (verifier)")
    assert not open_, open_


@pytest.mark.scenario("VO-002", level=4, check="SMT.preservation")
def test_every_action_preserves_valid_work() -> None:
    _require_proven("SMT.preservation", "preservation",
                    tuple(act(c) for c in ("enqueue", *TRANSITIONS)))


@pytest.mark.scenario("VO-001", level=4, check="SMT.enqueue.preservation")
def test_enqueue_creates_only_valid_work() -> None:
    _require_proven("SMT.enqueue.preservation", "preservation", (act("enqueue"),))


@pytest.mark.scenario("VO-005", level=4, check="SMT.postcondition")
def test_the_generation_postconditions_are_proven() -> None:
    _require_proven("SMT.postcondition", "postcondition",
                    tuple(act(c) for c in ("claim", "renew", "complete", "retry", "fail")))


@pytest.mark.scenario("VO-004", level=4, check="SMT.claim.evaluation_error")
def test_the_claim_increment_cannot_overflow() -> None:
    _require_proven("SMT.claim.evaluation_error", "evaluation_error", (act("claim"),))


# --- structural premises (contracts/safety-arguments.md) -----------------------------------


def _wire(model: Any) -> dict[str, Any]:
    return {a["name"]: a for a in harness.wire(model)["actions"]}


def _build_codes(make: Any) -> list[str]:
    with pytest.raises(ModelBuildError) as e:
        workflow.module(extra_actions=make)
    return sorted({x["code"] for x in e.value.errors})


def _writer(field_name: str) -> Any:
    def make(q: Any) -> list[Any]:
        item_type = q.work_entity

        @action
        def tamper(item: item_type):  # type: ignore[valid-type,no-untyped-def]
            set_(getattr(item, field_name), getattr(item, field_name))

        return [tamper]
    return make


def _premise_p1(model: Any) -> None:
    """P1: only claim writes `attempts` (by + 1); enqueue creates it at 0; no other writer."""
    actions = _wire(model)
    writers = sorted(n for n, a in actions.items()
                     if any(e.get("target", {}).get("field") == "attempts" for e in a["effects"]))
    assert writers == [act("claim")], writers
    eff = next(e for e in actions[act("claim")]["effects"] if e.get("target", {}).get("field") == "attempts")
    assert eff["value"]["op"] == "add", eff
    assert _build_codes(_writer("attempts")) == ["UNAUTHORIZED_WORK_WRITE"]


def _premise_p2() -> None:
    """P2: no removal; creation only by enqueue."""
    def make(q: Any) -> list[Any]:
        item_type = q.work_entity

        @action
        def purge(item: item_type):  # type: ignore[valid-type,no-untyped-def]
            remove(item)

        return [purge]

    assert _build_codes(make) == ["UNAUTHORIZED_WORK_REMOVAL"]


def _premise_p3_p7(model: Any) -> None:
    """P3, P7: status changes only through the machine; no transition leaves a terminal state."""
    actions = _wire(model)
    for c in TRANSITIONS:
        sources = actions[act(c)]["preconditions"][0]["expr"]
        assert sources["op"] == "in" and not TERMINAL & set(sources["values"]), (c, sources)
    assert _build_codes(_writer("status")) == ["UNAUTHORIZED_OWNED_FIELD_WRITE"]


def _premise_p4(model: Any) -> None:
    """P4: the generation comparisons and postconditions are generated."""
    actions = _wire(model)
    for c in ("renew", "complete", "retry", "fail"):
        a = actions[act(c)]
        assert any(harness.has_compare(p, "eq", "attempts", "generation")
                   for p in a["preconditions"]), c
        assert any(harness.has_compare(p, "eq", "attempts", "generation")
                   for p in a["postconditions"]), c
    claim_ = actions[act("claim")]
    assert any(harness.has_compare(p, "eq", "attempts", "expected_generation")
               for p in claim_["preconditions"])


@pytest.mark.scenario("VO-003", level=4, check="SA-MONO")
def test_sa_mono_premises_hold(model: Any) -> None:
    _premise_p1(model)
    _premise_p2()


@pytest.mark.scenario("VO-005", level=4, check="SA-FENCE")
def test_sa_fence_premises_hold(model: Any) -> None:
    _premise_p1(model)
    _premise_p2()
    _premise_p4(model)


@pytest.mark.scenario("VO-007", level=4, check="SA-TERM")
def test_sa_term_premises_hold(model: Any) -> None:
    _premise_p2()
    _premise_p3_p7(model)


@pytest.mark.scenario("VO-011", level=4, check="SA-OWN")
def test_sa_own_every_owned_item_is_protected(model: Any) -> None:
    for name in ("attempts", "owner", "lease_until", "available_at", "document", "request"):
        assert _build_codes(_writer(name)) == ["UNAUTHORIZED_WORK_WRITE"], name
    _premise_p2()
    _premise_p3_p7(model)


@pytest.mark.scenario("VO-008", level=4, check="SA-EXCL")
def test_sa_excl_the_store_refuses_a_second_commit_from_one_head(model: Any) -> None:
    store = harness.seeded(model, work("w1", "READY", 0))
    a = harness.evaluate(store, model, claim("a", "w1", "A", 0, now=1, until=9))
    b = harness.evaluate(store, model, claim("b", "w1", "B", 0, now=1, until=9))
    assert [harness.commit(store, model, a), harness.commit(store, model, b)] == \
        [None, "STATE_CONFLICT"]


# --- Core and Store contracts --------------------------------------------------------------


@pytest.mark.scenario("VO-001", level=3, check="SA.genesis")
def test_genesis_is_checked_and_imported_leases_are_fenced_from_genesis(model: Any) -> None:
    with pytest.raises(CommitRefused) as e:
        harness.seeded(model, work("w1", "LEASED", 2, None, 30))  # no owner: not ValidWork
    assert e.value.code == "GENESIS_INVALID"
    # An imported LEASED item: trusted state, no recorded claim; its deadline is not assumed live.
    store = harness.seeded(model, work("w1", "LEASED", 2, "A", 30))
    assert store.current().position == 0  # no claim transition exists for it
    assert harness.expired(store, model, 40) == ["w1"]
    steps = harness.run(
        store, model,
        Op("r", act("recover"), {W: "w1"}, {}, {"now": 40, "available_at": 40,
                                                "actor": "admin"}),
        claim("c", "w1", "B", 2, now=41, until=50),
        complete("old", "w1", "A", 2, now=42))
    assert [s.result for s in steps] == ["ALLOW", "ALLOW", "DENY"]


@pytest.mark.scenario("VO-008", level=3, check="BACKEND")
def test_the_backend_passes_the_releases_conformance_suite() -> None:
    if not full_mode():
        raise Unsupported("the backend conformance suite runs in full mode", "environment")
    r = run_conformance(InMemoryBackend)
    assert r.ok, r.failed


# --- properties over random histories ------------------------------------------------------


def _random_history(model: Any, rnd: random.Random, steps: int) -> list[str]:
    """Problems found in one random history: SA-MONO, SA-FENCE and ValidWork violations."""
    store = harness.seeded(model, work("w1", "READY", 0))
    now, problems = 0, []
    for _ in range(steps):
        before = harness.item(store)
        now += rnd.randint(0, 6)
        worker = rnd.choice("ABZ")
        gen = max(0, before["attempts"] + rnd.choice((-1, 0, 0, 1)))
        cap = rnd.choice(TRANSITIONS)
        ctx: dict[str, Any] = {"now": now}
        inp: dict[str, Any] = {}
        if cap in ("claim", "renew", "complete", "retry", "fail"):
            ctx["worker"] = worker
            inp["expected_generation" if cap == "claim" else "generation"] = gen
        if cap in ("claim", "renew"):
            ctx["lease_until"] = now + rnd.randint(0, 8)
        if cap in ("retry", "recover"):
            ctx["available_at"] = now + rnd.randint(-1, 4)
        if cap in ("recover", "exhaust", "cancel"):
            ctx["actor"] = rnd.choice(("admin", "u1"))
        bindings = {W: "w1"}
        if cap == "complete":
            bindings["doc"] = "d1"
        r = harness.do(store, model, Op(cap, act(cap), bindings, inp, ctx))
        after = harness.item(store)
        committed = r.result == "ALLOW" and r.conflict is None
        delta = after["attempts"] - before["attempts"]
        if delta not in (0, 1) or (delta == 1 and not (committed and cap == "claim")):
            problems.append(f"SA-MONO: {cap} {before['attempts']}→{after['attempts']}")
        if committed and cap in ("renew", "complete", "retry", "fail") \
                and inp["generation"] != before["attempts"]:
            problems.append(f"SA-FENCE: {cap} with generation {gen} at {before['attempts']}")
        if before["status"] in TERMINAL and after != before:
            problems.append(f"SA-TERM: {before['status']} changed by {cap}")
        if not harness.valid_work(after):
            problems.append(f"ValidWork: {after}")
    return problems


@pytest.mark.scenario("VO-003", level=4, check="PROP.mono")
def test_random_histories_keep_the_safety_properties(model: Any) -> None:
    count = 200 if full_mode() else 8
    found = []
    for seed in range(count):
        found += [f"seed {seed}: {p}" for p in _random_history(model, random.Random(seed), 12)]
    record_evidence("PROP.mono", {"histories": count, "steps_each": 12})
    assert not found, found[:5]


# --- assumptions --------------------------------------------------------------------------


@pytest.mark.scenario("VO-012", level=4, check="ASSUMPTIONS")
def test_every_obligation_names_the_assumptions_it_relies_on() -> None:
    o = report.OBLIGATIONS
    need = {"VO-003": [report.ORDINARY], "VO-005": [report.ORDINARY, report.TIME],
            "VO-007": [report.ORDINARY], "VO-008": [report.BACKEND],
            "VO-001": [report.IMPORT],
            "VO-012": [report.TIME, report.ORDINARY, report.BACKEND, report.IMPORT,
                       report.SMT_MODEL]}
    # Every obligation with an SMT dependency names the verifier's scope.
    for vo, spec in o.items():
        if any(d["class"] == "smt" for d in spec["dependencies"]):
            need.setdefault(vo, []).append(report.SMT_MODEL)
    missing = {vo: [a for a in want if a not in o[vo].get("assumptions", [])]
               for vo, want in need.items()}
    assert not any(missing.values()), missing
    assert set(o) == {f"VO-{n:03d}" for n in range(1, 13)}
    # A dependency's class matches its check: one check never backs two classes of one VO.
    for vo, spec in o.items():
        checks = [d["check"] for d in spec["dependencies"]]
        assert len(checks) == len(set(checks)), (vo, checks)
