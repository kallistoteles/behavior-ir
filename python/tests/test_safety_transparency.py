"""Core replay has no dependency on Safety policy/evidence/proof tools (FR-038)."""
import behavior as b
import pytest
from .test_store_conformance import DictBackend


@pytest.mark.parametrize("case", ["fo_field", "fo_state_machine"])
def test_core_replays_without_safety_policies_certificates_or_lean(case, tmp_path, monkeypatch):
    import importlib
    reference = importlib.import_module("models.examples.safety_models." + case)
    authored = reference.build()
    wire = authored.to_wire_json()
    # Admission/replay below gets only ordinary Core IR; discard all policy inputs.
    plain = b.BehaviorModule.from_wire_json(wire)
    assert plain.safety_analysis is None
    assert plain.behavior_version == authored.behavior_version
    assert plain.to_wire_json() == wire
    # Reuse the existing host backend fixture: InMemoryBackend is a factory marker
    # in this binding, so a second open(marker) would create an unrelated store.
    backend = DictBackend()
    seed = ([] if case == "fo_field" else
            [{"entity": "Task", "value": {"id": "t1", "phase": "Open", "note": "untouched"}}])
    store = b.Store.create(backend, authored, b.Store.genesis_for(authored, seed))
    operations = ([("open_note", {}, {"note_id": "n1", "text": "first"}),
                   ("edit", {"note": "n1"}, {"text": "second"})] if case == "fo_field" else
                  [("task__finish", {"task": "t1"}, {})])
    for action, bindings, inputs in operations:
        result = store.evaluate(authored, action, bindings=bindings, input=inputs,
                                commit_time="2026-10-18T00:00:00Z")
        assert result.decision.result == "ALLOW" and result.bundle is not None
        store.commit(authored, result.bundle)
    head = store.current()
    del authored
    def forbidden(*args, **kwargs):
        pytest.fail("Core replay accessed Safety analysis or certification")
    monkeypatch.setattr(b._engine, "analyze_safety", forbidden)
    monkeypatch.setattr(b._engine, "certify_safety", forbidden)
    # No executable is available through PATH and no certificate was created.
    monkeypatch.setenv("PATH", str(tmp_path))
    restored = b.Store.open(backend)
    assert restored.current() == head
    assert b.replay_data(restored).ok
    assert b.replay_behavior(restored, [plain]).ok
    if case == "fo_field":
        assert restored.load("Note", "n1")["value"] == {
            "id": "n1", "text": "second", "label": "unchanged"}
    else:
        assert restored.load("Task", "t1")["value"] == {
            "id": "t1", "phase": "Done", "note": "untouched"}


@pytest.mark.parametrize("case", ["ld_parent_child", "ld_shared_parent", "ld_vm_resource"])
def test_dependency_histories_replay_with_core_ir_alone(case, tmp_path, monkeypatch):
    import importlib
    reference=importlib.import_module("models.examples.safety_models."+case)
    model,store=reference.history()
    wire=model.to_wire_json()
    del model
    def forbidden(*args,**kwargs):
        pytest.fail("Core replay accessed a Safety policy, relationship registry or proof tool")
    monkeypatch.setattr(b._engine,"analyze_safety",forbidden)
    monkeypatch.setattr(b._engine,"certify_safety",forbidden)
    monkeypatch.setenv("PATH",str(tmp_path))
    plain=b.BehaviorModule.from_wire_json(wire)
    assert plain.safety_analysis is None
    assert b.replay_data(store).ok
    assert b.replay_behavior(store,[plain]).ok
