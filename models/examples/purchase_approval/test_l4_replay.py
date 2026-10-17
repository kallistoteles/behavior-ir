"""Level 4, replay and verification, each a separate check: data replay (C13a), behavior replay
(C13b), commit-history consistency (C13.history), the integrity negative controls (C14.*) and
static verification (C19). A reproduced decision is never taken as proof that its commit was
valid; that is C13.history's own check."""

from __future__ import annotations

import copy
import json
from typing import Any

import pytest

from behavior import (
    Profile, Store, _engine, replay_behavior, replay_data, replay_invocation, verify,
)
from python.tests.test_store_conformance import DictBackend

from models.examples.purchase_approval import harness, workflow
from models.examples.purchase_approval.report import (
    Blocked, Inconclusive, Unsupported, Unverifiable, full_mode, readiness_entry,
    record_evidence,
)


@pytest.mark.scenario("C13a", level=4)
def test_c13a_data_replay_reconstructs_the_head(history) -> None:  # type: ignore[no-untyped-def]
    report = replay_data(history.store)
    assert report.ok, report.divergence
    assert report.checked == history.store.current().position


@pytest.mark.scenario("C13b", level=4)
def test_c13b_behavior_replay_reproduces_every_recorded_decision(history) -> None:  # type: ignore[no-untyped-def]
    report = replay_behavior(history.store, [history.model])
    assert report.ok, report.divergence
    assert set(history.records) == set(harness.RECORDED_STEPS)
    for step, entry in history.records.items():
        result = replay_invocation(history.model, entry["record"])
        if not result.matches and result.diff and "missing" in result.diff.lower():
            raise Unverifiable(f"step {step}: {result.diff}", "core")
        assert result.matches, (step, result.diff)


@pytest.mark.scenario("C13b", level=4, check="C13.records_agree")
def test_recorded_invocations_agree_with_the_store_and_the_expectations(history) -> None:  # type: ignore[no-untyped-def]
    """Each portable record (T010) has the result the Store decided at that step and the result
    expected.json states for it: replay and CLI agreement alone would only confirm a record's
    self-consistency."""
    assert set(history.records) == set(harness.RECORDED_STEPS)
    for step, entry in history.records.items():
        outcome = entry["record"].data["outcome"]
        assert outcome["kind"] == "evaluated", (step, outcome)
        recorded = outcome["record"]["result"]
        store_result = [r for r in history.step(step) if r.kind == "do"][-1].result
        expected = harness.EXPECTED["steps"][step]["result"]
        assert recorded == store_result == expected, (step, recorded, store_result, expected)


@pytest.mark.scenario("C13b", level=4, check="C13.history")
def test_commit_history_is_a_parent_linked_chain(history) -> None:  # type: ignore[no-untyped-def]
    """Historical commit validity, so far as the records allow: contiguous positions, each
    committed on the previous head and evaluated against it, linked to the previous record, and
    carrying the module's behavior version."""
    store = history.store
    entries = list(store.history())
    assert [e["position"] for e in entries] == list(range(1, store.current().position + 1))
    previous = store.state_at(0)
    for e in entries:
        p = e["position"]
        head = {"position": previous.position, "state": previous.state}
        assert e["committed_on"] == head, p
        assert e["evaluated_against"] == head, p
        assert e["previous_record"] == store.history_at(p - 1).record, p
        assert e["bundle"]["behavior_version"] == history.model.behavior_version, p
        assert e["result_state"] == store.state_at(p).as_dict(), p
        previous = store.state_at(p)


def _mutated(entry: dict[str, Any], change: Any) -> str:
    data = copy.deepcopy(entry["record"].data)
    change(data)
    return json.dumps(data, sort_keys=True)


def _detected(model: Any, record_json: str) -> bool:
    """Whether replaying `record_json` reports a mismatch or refuses it."""
    try:
        return not replay_invocation(model, record_json).matches
    except Exception:  # noqa: BLE001 - a refusal is detection
        return True


def _flip_role(d: dict[str, Any]) -> None:
    for u in d["outcome"]["record"]["facts"]["universe"]:
        if u["entity"] == "User":
            for m in u["members"]:
                if m["id"] == "bob":
                    m["role"] = "Requester"


C14_CONTROLS = {
    "C14.altered_facts": _flip_role,
    "C14.altered_input": lambda d: d["input"].__setitem__("decision_id", "x99"),
    "C14.altered_result": lambda d: d["outcome"]["record"].__setitem__("result", "DENY"),
    "C14.missing_context": lambda d: d["context"].pop("actor"),
}


@pytest.mark.parametrize("check", [
    pytest.param(c, marks=pytest.mark.scenario("C14", level=4, check=c))
    for c in sorted(C14_CONTROLS)])
def test_c14_a_tampered_record_is_detected(history, check: str) -> None:  # type: ignore[no-untyped-def]
    tampered = _mutated(history.records["4"], C14_CONTROLS[check])
    assert _detected(history.model, tampered), check


@pytest.mark.scenario("C14", level=4, check="C14.wrong_module")
def test_c14_a_record_replayed_against_another_module_is_detected(history) -> None:  # type: ignore[no-untyped-def]
    other = harness.variant_module(*harness.ROLE_CHANGE)
    assert other.behavior_version != history.model.behavior_version
    assert _detected(other, history.records["4"]["record"].json)


@pytest.mark.scenario("C14", level=4, check="C14.stored_transition")
def test_c14_a_tampered_stored_transition_fails_data_replay() -> None:
    model = workflow.module()
    backend = DictBackend()
    store = Store.create(backend, model, harness.genesis(model))
    harness.run_history(model, store=store, stop_before="5")
    assert replay_data(store).ok
    record = backend._records[2]  # the transition at position 3 (start_review)
    for version in record.get("new_versions", []):
        if version.get("entity") == "PurchaseRequest":
            version["value"]["assignee"] = "alice"
            break
    else:
        raise Unsupported("no stored version to alter in the reference backend", "ecosystem")
    assert not replay_data(store).ok


#: The original, unchanged Approval reference module (feature 506). Core 0.15.0 (verifier 0.10.0,
#: feature 028, fixing behavior-ir-core#35) proves exactly this module; C19 passes only for it
#: (feature 508, Q5).
REFERENCE_MODULE = "sha256:a04ba37ab89e051e448abb760bd754bbb0955125fd22ba8da927b8529a531779"
#: The uniqueness postcondition of the two decision actions, which verifier 0.9.0 left
#: inconclusive and verifier 0.10.0 proves.
_UNIQUENESS = ("count(where(select(PurchaseRequestApprovalDecision), d, d.subject == subject.id "
               "and d.actor == actor)) <= 1")


@pytest.mark.scenario("C19", level=4)
def test_c19_static_verification_of_the_reference_module() -> None:
    if not full_mode():
        raise Unsupported("static verification runs in full mode", "environment")
    notice = _engine.solver_notice()
    if notice is not None:
        raise Unsupported(f"no solver: {notice}", "environment")
    model = workflow.module()
    profile = Profile()
    attestation = verify(model, profile)
    record_evidence("C19", {
        "module": model.behavior_version,
        "attested_module": attestation.data.get("behavior_version"),
        "attestation": attestation.hash,
        "result": attestation.result,
        "verifier_version": attestation.data.get("verifier_version"),
        "solver_version": attestation.data.get("solver_version"),
        "profile": attestation.data.get("profile"),
        # Each check's outcome; `cached` is left out: it depends on the run, not the module.
        "checks": sorted(
            ({"kind": c.get("kind"), "subject": (c.get("subject") or {}).get("name"),
              "action": c.get("action"), "outcome": c.get("outcome")}
             for c in attestation.checks), key=lambda c: json.dumps(c, sort_keys=True)),
        "findings": [{"kind": f.get("kind"), "explanation": f.get("explanation")}
                     for f in attestation.findings],
    })
    assert attestation.data.get("behavior_version") == model.behavior_version
    meta = (f"attestation {attestation.hash}, verifier {attestation.data.get('verifier_version')}, "
            f"solver {attestation.data.get('solver_version')}, rlimit {profile.rlimit}")
    if attestation.verified:
        assert not attestation.findings, attestation.findings
        # The proof is of the unchanged module, and covers both uniqueness postconditions.
        assert model.behavior_version == REFERENCE_MODULE, model.behavior_version
        uniqueness = sorted(
            (c["action"]["name"], c.get("outcome")) for c in attestation.checks
            if c.get("kind") == "postcondition" and _UNIQUENESS in str(c.get("subject")))
        assert uniqueness == [("purchase_request_approval__approve", "proven"),
                              ("purchase_request_approval__reject", "proven")], uniqueness
        return
    kinds = {f.get("kind") for f in attestation.findings}
    if kinds == {"inconclusive"}:
        # The verifier found no confirmed violation and no proof: neither a pass nor a failure.
        # A Core limitation is reported as blocked only through an explicit readiness entry.
        cited = "; ".join(f["explanation"] for f in attestation.findings)
        entry = readiness_entry("C19")
        if entry is not None:
            raise Blocked(f"{entry['issue']}: {meta}: {cited}", entry["issue"])
        raise Inconclusive(f"{meta}: {cited}", "core (verifier)")
    raise AssertionError(f"{meta}: {attestation.result}: {attestation.findings}")
