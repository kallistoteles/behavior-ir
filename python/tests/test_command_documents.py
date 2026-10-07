"""Feature 500: current command documents retain Core identity and committed history."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

import behavior as bh

from .conftest import FIXTURES

T0 = "2026-10-05T12:00:00Z"


def fixture(name: str) -> str:
    return (FIXTURES / name).read_text()


def receipt_model() -> Any:
    assert callable(getattr(bh.BehaviorModule, "from_wire_json", None)), "wire import is public"
    return bh.BehaviorModule.from_wire_json(fixture("commands/modules/receipt.json"))


def v2_store(module: Any) -> bh.Store:
    assert callable(getattr(bh.Store, "genesis_v2_for", None)), "v2 genesis is explicit"
    policy = fixture("governance-v2/evidence-policy-none.json")
    genesis = bh.Store.genesis_v2_for(module, [], policy)
    assert genesis["format"] == "behavior.store_genesis.v2"
    return bh.Store.create(bh.InMemoryBackend(), module, genesis)


def stream(store: bh.Store, after: Any, through: Any | None = None,
           max_records: int = 256) -> Any:
    request = {"format": "behavior.command_stream_request.v1", "after": after.as_dict(),
               "max_records": max_records}
    if through is not None:
        request["through"] = through.as_dict()
    return store.commands_since(request)


def test_command_fixture_keeps_frozen_record_and_identities() -> None:
    module = receipt_model()
    record = bh.invoke(module, fixture("commands/invocations/receipt.json"),
                       fixture("commands/snapshots/receipt.json"))
    golden = json.loads(fixture("commands/records/receipt.golden.json"))
    assert module.behavior_version == golden["behavior_version"]
    assert record.json == fixture("commands/records/receipt.invocation.json")
    assert record.record_id == golden["invocation_record_id"]
    assert record.inner_record == json.loads(fixture("commands/records/receipt.json"))
    assert [x["intent_hash"] for x in record.inner_record["commands"]["intents"]] == (
        golden["intent_hashes"])
    assert bh.replay_invocation(module, record).matches


def test_decision_07_exposes_commands_and_keeps_diagnostics_detached() -> None:
    module = receipt_model()
    decision = bh.evaluate(module, "receipt", state={}, input={"recipient": "customer-1"},
                           data_version="test:commands:empty")
    expected = json.loads(fixture("commands/records/receipt.json"))
    assert json.loads(decision.record_json) == expected
    assert decision.commands == expected["commands"]["intents"]
    assert decision.diagnostics and decision.diagnostics["trace"]
    assert all("loc" not in step and "expr_text" not in step for step in expected["trace"])
    assert any("loc" in step for step in decision.diagnostics["trace"])
    assert "diagnostics" not in json.loads(decision.record_json)
    assert len(decision.trace) == len(expected["trace"])
    assert bh.replay(module, decision.record_json).matches


def test_current_wire_admission_rejects_duplicates_and_invalid_command_payloads() -> None:
    module = receipt_model()
    text = module.to_wire_json()
    duplicate = '{"ir_version":"0.7",' + text[1:]
    with pytest.raises(bh.BehaviorInvalid):
        bh.BehaviorModule.from_wire_json(duplicate)
    invalid = json.loads(text)
    invalid["actions"][0]["command_effects"][0]["payload"] = {}
    with pytest.raises(bh.BehaviorInvalid):
        bh.BehaviorModule.from_wire_json(json.dumps(invalid))


def test_candidates_are_invisible_until_commit_and_recovery_keeps_occurrence_identity() -> None:
    module = receipt_model()
    store = v2_store(module)
    start = store.current_history()
    assert start.as_dict()["format"] == "behavior.history_ref.v1"
    assert start.record == start.store == store.store_id
    assert store.history_at(0) == start
    result = store.invoke(module, fixture("commands/invocations/receipt.json"), commit_time=T0)
    assert result.bundle is not None
    assert stream(store, start).items == []
    assert store.current_history() == start
    committed = store.commit(module, result.bundle)
    end = store.current_history()
    assert end.position == 1 and end.state == start.state and end.record != start.record
    assert store.history_at(1) == end
    page = stream(store, start)
    assert page.complete and page.next_after == page.observed_head == end
    assert len(page.items) == 1
    occurrence = page.items[0]
    assert occurrence["store"] == store.store_id
    assert occurrence["history_position"] == 1
    assert occurrence["commit_record_hash"] == committed.record_id
    assert occurrence["multiplicity_index"] == 0
    assert occurrence["intent"] == result.record.inner_record["commands"]["intents"][0]
    assert "command_occurrence_id" not in result.record.inner_record["commands"]["intents"][0]
    assert store.commit(module, result.bundle).already
    assert store.current_history() == end
    assert stream(store, start).items == page.items
    assert bh.replay_data(store).ok
    assert bh.replay_behavior(store, [module]).ok


def paged_model() -> Any:
    receipt_model()  # Missing API is asserted before a new release fixture is read.
    wire = json.loads(fixture("commands/modules/receipt.json"))
    emission = wire["actions"][0]["command_effects"][0]
    wire["actions"][0]["command_effects"] = [copy.deepcopy(emission) for _ in range(3)]
    quiet = copy.deepcopy(wire["actions"][0])
    quiet["name"] = "quiet"
    for command in quiet["command_effects"]:
        command["when"] = {"op": "lit", "type": {"t": "bool"}, "value": False,
                           "loc": {"file": "paging.beh", "line": 1}}
    wire["actions"].append(quiet)
    return bh.BehaviorModule.from_wire_json(json.dumps(wire))


def commit_receipt(store: bh.Store, module: Any, recipient: str,
                   capability: str = "receipt") -> Any:
    request = {"format": "behavior.invocation.v1", "capability": capability, "bindings": {},
               "input": {"recipient": recipient}, "context": {}}
    result = store.invoke(module, request, commit_time=T0)
    assert result.bundle is not None
    store.commit(module, result.bundle)
    return result


def test_paging_keeps_whole_events_duplicate_counts_and_empty_event_progress() -> None:
    module = paged_model()
    store = v2_store(module)
    start = store.current_history()
    commit_receipt(store, module, "first")
    commit_receipt(store, module, "quiet", "quiet")
    commit_receipt(store, module, "last")
    pinned = store.current_history()
    first = stream(store, start, max_records=1)
    assert len(first.items) == 3
    assert [item["multiplicity_index"] for item in first.items] == [0, 1, 2]
    assert len({item["command_occurrence_id"] for item in first.items}) == 3
    assert first.next_after.position == 1 and first.observed_head == pinned
    assert not first.complete
    # A later event cannot enter this already pinned interval.
    commit_receipt(store, module, "later")
    second = stream(store, first.next_after, pinned, max_records=1)
    assert second.items == []
    assert second.next_after.position == 2 and not second.complete
    third = stream(store, second.next_after, pinned, max_records=1)
    assert len(third.items) == 3 and third.complete and third.next_after == pinned
    assert [item["history_position"] for item in first.items + third.items] == [1, 1, 1, 3, 3, 3]
    assert {item["intent"]["payload"]["recipient"] for item in third.items} == {"last"}
    assert stream(store, first.next_after, pinned, max_records=1).items == []


@pytest.mark.parametrize("corruption", ["wrong_store", "future", "wrong_record", "limit_zero",
                                       "limit_large", "reversed", "unknown_key"])
def test_invalid_stream_requests_are_explicit_refusals_without_history_changes(
    corruption: str,
) -> None:
    module = receipt_model()
    store = v2_store(module)
    start = store.current_history()
    commit_receipt(store, module, "first")
    end = store.current_history()
    request = {"format": "behavior.command_stream_request.v1", "after": start.as_dict(),
               "through": end.as_dict(), "max_records": 1}
    if corruption == "wrong_store":
        request["after"]["store"] = "sha256:" + "f" * 64
        request["after"]["record"] = request["after"]["store"]
    elif corruption == "future":
        request["through"]["position"] = 99
    elif corruption == "wrong_record":
        request["through"]["record"] = "sha256:" + "f" * 64
    elif corruption == "limit_zero":
        request["max_records"] = 0
    elif corruption == "limit_large":
        request["max_records"] = 1025
    elif corruption == "reversed":
        request["after"], request["through"] = request["through"], request["after"]
    else:
        request["injected"] = True
    with pytest.raises(bh.CommitRefused):
        store.commands_since(request)
    assert store.current_history() == end


def test_raw_stream_duplicate_keys_are_not_reduced_by_the_binding() -> None:
    module = receipt_model()
    store = v2_store(module)
    start = store.current_history()
    request = json.dumps({"format": "behavior.command_stream_request.v1",
                          "after": start.as_dict(), "max_records": 1})
    duplicated = '{"max_records":256,' + request[1:]
    with pytest.raises(bh.CommitRefused, match="duplicate|Duplicate"):
        store.commands_since(duplicated)
    assert store.current_history() == start


def test_current_profile_cannot_write_into_legacy_history() -> None:
    module = receipt_model()
    store = bh.Store.create(bh.InMemoryBackend(), module, bh.Store.genesis_for(module, []))
    before = store.current()
    candidate = store.invoke(module, fixture("commands/invocations/receipt.json"), commit_time=T0)
    assert candidate.bundle is not None
    with pytest.raises(bh.CommitRefused) as error:
        store.commit(module, candidate.bundle)
    assert error.value.code == "HISTORY_FORMAT_UPGRADE_REQUIRED"
    assert store.current() == before


def test_candidate_and_export_documents_preserve_exact_history_and_refuse_forged_anchors() -> None:
    assert callable(getattr(bh, "governance_candidate", None)), "candidate inspection is native"
    module = receipt_model()
    store = v2_store(module)
    start = store.current_history()
    result = store.invoke(module, fixture("commands/invocations/receipt.json"), commit_time=T0)
    assert result.bundle is not None
    candidate = bh.governance_candidate(json.dumps(result.bundle))
    assert candidate["format"] == "behavior.governance_candidate.v2"
    assert candidate["transition_hash"] == result.bundle["transition_hash"]
    assert candidate["content"]["evaluated_history"] == start.as_dict()
    assert store.current_history() == start
    exported = store.export_seed_at(module, start)
    assert exported["source_history"] == start.as_dict()
    assert exported["state"] == start.state and exported["schema"] == module.schema_hash
    assert exported["seed"] == []
    invalid = start.as_dict()
    invalid["record"] = "sha256:" + "f" * 64
    with pytest.raises(bh.CommitRefused):
        store.export_seed_at(module, json.dumps(invalid))
    assert store.current_history() == start


def test_cli_evidence_requires_independent_context_and_attachment_preserves_candidate(
    tmp_path: Path,
) -> None:
    assert callable(getattr(bh, "with_trusted_evidence", None)), "evidence attachment is native"
    assert callable(getattr(bh.Store, "commit_with_context", None)), "live context is explicit"
    module = receipt_model()
    policy_path = FIXTURES / "governance-v2/policy-verifier-0.8.json"
    evidence_policy_path = FIXTURES / "governance-v2/evidence-policy-verifier-0.8.json"
    context_path = FIXTURES / "governance-v2/context.json"
    genesis = bh.Store.genesis_v2_for(module, [], evidence_policy_path.read_text())
    store = bh.Store.create(bh.InMemoryBackend(), module, genesis)
    start = store.current_history()
    result = store.invoke(module, fixture("commands/invocations/receipt.json"), commit_time=T0)
    assert result.bundle is not None
    candidate = bh.governance_candidate(result.bundle)
    candidate_path = tmp_path / "candidate.json"
    candidate_path.write_text(json.dumps(candidate))
    envelope_path, evidence_path = tmp_path / "envelope.json", tmp_path / "evidence.json"
    wire_path = FIXTURES / "commands/modules/receipt.json"
    cli = [sys.executable, "-m", "behavior._cli", "governance"]
    verified = subprocess.run([
        *cli, "verify", str(wire_path), "--profile", str(FIXTURES / "governance-v2/profile.json"),
        "--seed", str(FIXTURES / "governance-v2/verifier.seed"), "--out", str(envelope_path),
    ], capture_output=True, text=True)
    assert verified.returncode == 0, verified.stderr
    authorized = subprocess.run([
        *cli, "authorize", str(candidate_path), "--wire", str(wire_path), "--evidence-policy",
        str(evidence_policy_path), "--policy", str(policy_path), "--seed",
        str(FIXTURES / "governance-v2/authorizer.seed"), "--verification", str(envelope_path),
        "--context", str(context_path), "--now", T0, "--out", str(evidence_path),
    ], capture_output=True, text=True)
    assert authorized.returncode == 0, authorized.stderr
    attached = bh.with_trusted_evidence(json.dumps(result.bundle), evidence_path.read_text())
    assert bh.governance_candidate(attached) == candidate
    with pytest.raises(bh.CommitRefused) as missing:
        store.commit(module, attached)
    assert missing.value.code == "CONTEXT_REQUIRED"
    substituted = json.loads(context_path.read_text())
    substituted["policy_time"] = "2026-10-05T12:00:01Z"
    with pytest.raises(bh.CommitRefused) as mismatch:
        store.commit_with_context(module, attached, context=substituted,
                                  execution_policy=policy_path.read_text())
    assert mismatch.value.code == "CONTEXT_MISMATCH"
    assert store.current_history() == start
    committed = store.commit_with_context(module, json.dumps(attached),
                                          context=context_path.read_text(),
                                          execution_policy=policy_path.read_text())
    assert committed.evidence_trust == "authenticated"
    assert bh.replay_data(store).ok and bh.replay_behavior(store, [module]).ok
    assert store.commit(module, attached).already
    assert store.current_history().position == 1


def test_evidence_attachment_rejects_invalid_and_duplicate_documents_without_a_write() -> None:
    assert callable(getattr(bh, "with_trusted_evidence", None)), "evidence attachment is native"
    module = receipt_model()
    store = v2_store(module)
    result = store.invoke(module, fixture("commands/invocations/receipt.json"), commit_time=T0)
    assert result.bundle is not None
    before = store.current_history()
    for evidence in ["not JSON", '{"format":"behavior.evidence.v2"}',
                     '{"format":"x","format":"behavior.evidence.v2"}']:
        with pytest.raises(bh.CommitRefused):
            bh.with_trusted_evidence(result.bundle, evidence)
    assert store.current_history() == before
