"""Feature 500: the binding preserves the released Core invocation boundary."""

from __future__ import annotations

import copy
import json
from typing import Any

import pytest

import behavior as bh

from .conftest import FIXTURES

T0 = "2026-10-05T12:00:00Z"
REQUESTS = [
    "check_standing", "customer_count", "pair_total", "register", "settle3", "summary",
    "summary_unknown", "summary_wrongtype", "suspend", "suspend_unknown", "suspend_wrongtype",
    "transfer", "transfer_alias",
]
INTENTS = [
    "extra_binding", "missing_binding", "register_no_bindings", "summary_read", "suspend_one",
    "transfer_two", "unknown_capability", "unknown_identity", "with_context", "with_metadata",
    "with_state", "with_targets", "wrong_type",
]


def fixture(name: str) -> str:
    return (FIXTURES / "invocation" / name).read_text()


def model() -> Any:
    # Assert the missing feature before reading new-release assets during the initial red run.
    assert callable(getattr(bh.BehaviorModule, "from_wire_json", None)), "wire import is public"
    return bh.BehaviorModule.from_wire_json(fixture("modules/ledger.json"))


def store_for(module: Any) -> bh.Store:
    seed = json.loads(fixture("snapshots/s1.json"))["entities"]
    return bh.Store.create(bh.InMemoryBackend(), module, bh.Store.genesis_for(module, seed))


def test_wire_import_preserves_admission_and_canonical_roundtrip() -> None:
    module = model()
    admission = bh.admit(module)
    assert admission.ok
    again = bh.BehaviorModule.from_wire_json(module.to_wire_json())
    assert bh.admit(again).items == admission.items
    assert again.behavior_version == module.behavior_version
    invalid = json.loads(fixture("modules/ledger.json"))
    invalid["ir_version"] = "unsupported"
    with pytest.raises(bh.BehaviorInvalid):
        bh.BehaviorModule.from_wire_json(json.dumps(invalid))


@pytest.mark.parametrize("name", REQUESTS)
def test_plain_requests_equal_core_records_and_replay(name: str) -> None:
    module = model()
    record = bh.invoke(module, fixture(f"invocations/{name}.json"),
                       fixture("snapshots/s1.json"))
    assert record.json == fixture(f"records/{name}.expected.json")
    assert record.data == json.loads(record.json)
    assert record.record_id == record.data["record_id"]
    assert record.outcome_kind == record.data["outcome"]["kind"]
    assert bh.replay_invocation(module, record).matches
    assert bh.replay_invocation(module, record.json).matches


@pytest.mark.parametrize("name", INTENTS)
def test_capability_intents_equal_core_records_with_explicit_host_context(name: str) -> None:
    module = model()
    record = bh.invoke(module, fixture(f"intents/{name}.json"),
                       fixture("snapshots/s1.json"), context={})
    assert record.json == fixture(f"intent-records/{name}.expected.json")
    assert bh.replay_invocation(module, record).matches
    if record.outcome_kind == "pre_evaluation_refusal":
        assert record.inner_record is None
        assert record.refusal_stage is not None


def test_raw_duplicate_keys_and_unparseable_transport_are_rejected() -> None:
    module = model()
    request = fixture("invocations/transfer.json")
    snapshot = fixture("snapshots/s1.json")
    duplicated = '{"capability":"suspend_customer",' + request[1:]
    with pytest.raises(bh.BehaviorError, match="duplicate|Duplicate"):
        bh.invoke(module, duplicated, snapshot)
    with pytest.raises(bh.BehaviorError):
        bh.invoke(module, "not JSON", snapshot)
    duplicated_snapshot = '{"entities":[],' + snapshot[1:]
    with pytest.raises(bh.BehaviorError, match="duplicate|Duplicate"):
        bh.invoke(module, request, duplicated_snapshot)


def test_independent_decode_problems_keep_core_order_without_evaluation() -> None:
    module = model()
    intent = {
        "format": "wrong", "capability": "transfer",
        "bindings": {"from_": "a1", "to": {"entity": "Customer", "id": "c1"},
                     "extra": {"entity": "Customer", "id": "c2"}},
        "input": {"amount": 20}, "state": {}, "targets": {}, "context": {},
    }
    record = bh.invoke(module, intent, fixture("snapshots/s1.json"), context={})
    assert record.outcome_kind == "pre_evaluation_refusal"
    assert record.refusal_stage == "DECODE"
    assert record.inner_record is None
    assert [problem["code"] for problem in record.data["outcome"]["problems"]] == [
        "UNSUPPORTED_FORMAT", "INVALID_IDENTITY", "STATE_NOT_ALLOWED", "CONTEXT_FROM_HOST",
        "LEGACY_TARGETS", "EXTRA_BINDING", "INVALID_BINDING",
    ]
    assert bh.replay_invocation(module, record).matches


def test_inconsistent_snapshot_is_a_replayable_decode_refusal() -> None:
    module = model()
    snapshot = json.loads(fixture("snapshots/s1.json"))
    snapshot["entities"].append(copy.deepcopy(snapshot["entities"][0]))
    record = bh.invoke(module, fixture("invocations/suspend.json"), snapshot)
    assert record.refusal_stage == "DECODE"
    assert record.inner_record is None
    assert any(p["code"] == "INCONSISTENT_FACTS" for p in record.data["outcome"]["problems"])
    assert bh.replay_invocation(module, record).matches


def test_modified_record_fails_replay_with_a_difference() -> None:
    module = model()
    record = bh.invoke(module, fixture("invocations/transfer.json"),
                       fixture("snapshots/s1.json"))
    altered = record.data
    altered["outcome"]["record"]["changes"][0]["new"] += 1
    result = bh.replay_invocation(module, json.dumps(altered))
    assert not result.matches
    assert result.diff is not None
    # Inspecting a returned dictionary does not mutate the engine-owned artifact.
    assert record.json == fixture("records/transfer.expected.json")


@pytest.mark.parametrize("name", REQUESTS)
def test_store_requests_match_plain_snapshot_and_remain_read_only(name: str) -> None:
    module = model()
    store = store_for(module)
    before = store.current()
    snapshot = json.loads(fixture("snapshots/s1.json"))
    snapshot["data_version"] = store.data_version()
    request = fixture(f"invocations/{name}.json")
    plain = bh.invoke(module, request, snapshot)
    result = store.invoke(module, request, commit_time=T0)
    assert result.record.json == plain.json
    allowed_action = plain.data["kind"] == "action" and plain.inner_record is not None and (
        plain.inner_record["result"] == "ALLOW")
    assert (result.bundle is not None) == allowed_action
    assert store.replay_invocation(module, result.record).matches
    assert store.current() == before
    assert list(store.history()) == []


@pytest.mark.parametrize("name", INTENTS)
def test_store_intents_match_plain_snapshot_and_refusals_have_no_bundle(name: str) -> None:
    module = model()
    store = store_for(module)
    before = store.current()
    snapshot = json.loads(fixture("snapshots/s1.json"))
    snapshot["data_version"] = store.data_version()
    intent = fixture(f"intents/{name}.json")
    plain = bh.invoke(module, intent, snapshot, context={})
    result = store.invoke_intent(module, intent, context={}, commit_time=T0)
    assert result.record.json == plain.json
    assert store.replay_invocation(module, result.record).matches
    if plain.outcome_kind == "pre_evaluation_refusal" or plain.data["kind"] == "read":
        assert result.bundle is None
    assert store.current() == before


def test_past_actions_have_no_bundle_and_reads_preserve_history() -> None:
    module = model()
    store = store_for(module)
    start = store.current()
    request = fixture("invocations/suspend.json")
    candidate = store.invoke(module, request, commit_time=T0)
    assert candidate.bundle is not None
    store.commit(module, candidate.bundle)
    end = store.current()
    past = store.invoke(module, request, commit_time=T0, at=start)
    assert past.record.inner_record["result"] == "ALLOW"
    assert past.bundle is None
    assert store.replay_invocation(module, past.record).matches
    read = store.invoke(module, fixture("invocations/summary.json"), commit_time=T0, at=start)
    assert read.record.inner_record["value"]["status"] == "ACTIVE"
    assert read.bundle is None
    assert store.current() == end
    assert len(list(store.history())) == 1


def test_store_transport_errors_and_foreign_positions_are_not_successful_refusals() -> None:
    module = model()
    store = store_for(module)
    before = store.current()
    with pytest.raises(bh.BehaviorError):
        store.invoke_intent(module, "not JSON", context={}, commit_time=T0)
    foreign = bh.StateRef("sha256:" + "f" * 64, 0)
    with pytest.raises(bh.CommitRefused):
        store.invoke(module, fixture("invocations/suspend.json"), commit_time=T0, at=foreign)
    assert store.current() == before


def legacy_candidate() -> tuple[bh.Store, Any, dict[str, Any]]:
    """Existing legacy fixture setup isolates raw bundle handling from new invocation APIs."""
    from examples.ledger.behavior import model as ledger

    seed = [{"entity": "Account", "value": {"id": id_, "active": True, "balance": balance}}
            for id_, balance in [("a1", "100.00"), ("a2", "5.00")]]
    store = bh.Store.create(bh.InMemoryBackend(), ledger, bh.Store.genesis_for(ledger, seed))
    _, bundle = store._inner.evaluate(ledger.engine, "transfer", {"from_": "a1", "to": "a2"},
                                     {"amount": "1.00"}, {}, T0, None)
    assert bundle is not None
    return store, ledger, bundle


def test_raw_legacy_bundle_commit_uses_its_checked_evaluated_parent() -> None:
    store, module, bundle = legacy_candidate()
    committed = store.commit(module, json.dumps(bundle))
    assert committed.result_state.position == 1
    assert store.load("Account", "a1")["value"]["balance"] == "99.00"
    assert store.commit(module, json.dumps(bundle)).already


def test_duplicate_raw_legacy_bundle_is_refused_without_mutation() -> None:
    store, module, bundle = legacy_candidate()
    before = store.current()
    raw = '{"commit_time":"2026-10-05T12:00:01Z",' + json.dumps(bundle)[1:]
    with pytest.raises(bh.CommitRefused, match="duplicate|Duplicate"):
        store.commit(module, raw)
    assert store.current() == before


def test_malformed_store_request_preserves_native_diagnostics_without_evaluation() -> None:
    store, module, _ = legacy_candidate()
    before = store.current()
    malformed = {"format": "wrong", "capability": 17, "bindings": {"bad": "a1"},
                 "input": {}, "context": {}}
    with pytest.raises(bh.BehaviorError) as shape:
        store.invoke(module, malformed, commit_time=T0)
    text = str(shape.value)
    codes = ["UNSUPPORTED_FORMAT", "INVALID_CAPABILITY", "INVALID_IDENTITY"]
    offsets = [text.index(code) for code in codes]
    assert offsets == sorted(offsets)
    assert "bindings.bad" in text
    with pytest.raises(bh.BehaviorError, match="document is not canonicalizable"):
        store.invoke(module, "not JSON", commit_time=T0)
    assert store.current() == before
    assert list(store.history()) == []
