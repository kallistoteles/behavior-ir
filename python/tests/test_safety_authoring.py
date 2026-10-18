"""Safety authoring records delegate meaning to Rust; lowering is not certification."""
import pytest
import behavior as b


@b.entity
class Item:
    note = b.field(str)


@b.action
def edit(item: Item):
    b.set_(item.note, "changed")


def module(*policies):
    return b.BehaviorModule(entities=[Item], actions=[edit], safety=policies)


def test_structural_authority_is_checked_by_rust():
    p = b.FieldOwnership(id="notes", entity=Item, writers={"note": [edit]})
    m = module(p)
    assert m.safety_analysis.status == "Lowered"
    assert not m.safety_analysis.is_certified
    assert m.model_receipts == []
    assert m.behavior_version == module().behavior_version
    with pytest.raises(b.ModelBuildError, match="SAFETY_UNAUTHORIZED_WRITE"):
        module(b.FieldOwnership(id="notes", entity=Item, writers={"note": []}))


def test_defaults_and_malformed_inputs_cannot_authorize():
    with pytest.raises(b.ModelBuildError):
        module(b.FieldOwnership(id="notes", entity=Item, writers={"missing": [edit]}))
    with pytest.raises(b.ModelBuildError):
        module(b.FieldOwnership(id="notes", entity=Item, writers={"note": ["stale"]}))
    with pytest.raises(b.ModelBuildError):
        module(b.FieldOwnership(id="notes", entity=Item, writers={"note": [edit, edit]}))


@pytest.mark.parametrize("options", [{"writers": False}, {"writers": {"note": "edit"}},
                                     {"creators": True}, {"removers": {"edit": True}}])
def test_malformed_container_shapes_reach_rust_unchanged(options):
    with pytest.raises(b.ModelBuildError):
        module(b.FieldOwnership(id="notes", entity=Item, **options))


def test_ordinary_build_does_not_start_proof_tools(monkeypatch):
    import subprocess
    monkeypatch.setattr(subprocess, "Popen", lambda *a, **kw: pytest.fail("ordinary build launched tool"))
    m = module()
    assert m.safety_analysis is None
    assert m.model_receipts == []


def test_generated_action_is_only_a_recording_selector():
    selector = b.GeneratedAction("machine", "advance")
    assert selector.model_id == "machine"
    assert selector.local_action_id == "advance"


def test_recording_delegates_complete_policy_to_rust(monkeypatch):
    original = b._engine.analyze_safety
    calls = []
    def record(engine, inputs):
        calls.append(inputs)
        return original(engine, inputs)
    monkeypatch.setattr(b._engine, "analyze_safety", record)
    module(b.FieldOwnership(id="notes", entity=Item, writers={"note": [edit]}))
    assert calls == [[{"id": "notes", "entity": "Item", "writers": {"note": ["edit"]},
                       "creators": [], "removers": []}]]


@pytest.mark.parametrize("case", ["fo_field", "fo_state_machine"])
def test_reference_cases_execute_and_preserve_core_identity(case):
    import importlib
    reference = importlib.import_module("models.examples.safety_models." + case)
    plain, protected = reference.build(safety=False), reference.build()
    assert protected.to_wire_json() == plain.to_wire_json()
    assert protected.model_receipts == plain.model_receipts
    assert reference.exercise()["passed"]
    with pytest.raises(b.ModelBuildError):
        reference.build(extra_writer=True)


def test_analysis_and_certificates_cannot_be_built_from_flags():
    for cls in [b._engine.SafetyAnalysis, b._engine.SafetyCertificate, b._engine.SafetyAssessment]:
        with pytest.raises(TypeError):
            cls()


def test_reference_runner_distinguishes_lowering_from_failed_certification(tmp_path, monkeypatch):
    import json
    from types import SimpleNamespace
    from models.examples.safety_models import run
    assert run.main(["--case", "fo_field", "--output", str(tmp_path / "lowered")]) == 0
    report = json.loads((tmp_path / "lowered" / "summary.json").read_text())
    assert report["cases"]["fo_field"]["status"] == "Lowered"
    assert report["cases"]["fo_field"]["certified"] is False
    monkeypatch.setattr(run, "certify_safety", lambda *a, **kw: SimpleNamespace(
        certificate=None, to_dict=lambda: {"outcome": "Inconclusive"}))
    assert run.main(["--case", "fo_field", "--certify", "--output", str(tmp_path / "refused")]) == 1


def test_empty_policy_set_cannot_receive_a_certificate(tmp_path):
    result = b.certify_safety(module(), output=tmp_path)
    assert result.outcome == "Failed"
    assert result.certificate is None
