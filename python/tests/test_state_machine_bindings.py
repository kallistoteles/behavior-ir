"""Transition state bindings (feature 507, D1(a); research R3, R13): a transition may change
further entities atomically with the status, and a binding never grants authority over an
owned field."""

from __future__ import annotations

from enum import Enum

import pytest

from behavior import (
    Approval, BehaviorModule, Id, InMemoryBackend, ModelBuildError, StateMachine, Store, entity,
    field, set_,
)

CT = "2026-10-17T00:00:00Z"


class Phase(Enum):
    OPEN = "Open"
    DONE = "Done"


@entity
class Folder:
    label = field(str)


@entity
class Task:
    phase = field(Phase)
    title = field(str)


def machine(**extra: object) -> StateMachine:
    m = StateMachine("task", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    m.transition("finish", from_={Phase.OPEN}, to=Phase.DONE, state={"folder": Folder},
                 effects=[lambda t, folder: set_(folder.label, "has-done")], **extra)
    return m


def module(m: StateMachine) -> BehaviorModule:
    return BehaviorModule(entities=[Folder, Task], enums=[Phase], models=[m])


def store(model: BehaviorModule) -> Store:
    seed = [{"entity": "Folder", "value": {"id": "f1", "label": "empty"}},
            {"entity": "Task", "value": {"id": "t1", "phase": "Open", "title": "x"}}]
    return Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))


def test_a_bound_entity_changes_atomically_with_the_status() -> None:
    model = module(machine())
    receipt = model.model_receipts[0]
    assert receipt.model_semantic_version == 3
    st = store(model)
    ev = st.evaluate(model, "task__finish", bindings={"task": "t1", "folder": "f1"},
                     commit_time=CT)
    assert ev.decision.result == "ALLOW"
    st.commit(model, ev.bundle)
    assert st.load("Task", "t1")["value"]["phase"] == "Done"
    assert st.load("Folder", "f1")["value"]["label"] == "has-done"


def test_a_machine_without_bindings_stays_level_2() -> None:
    m = StateMachine("task", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    m.transition("finish", from_={Phase.OPEN}, to=Phase.DONE)
    assert module(m).model_receipts[0].model_semantic_version == 2


def test_aliasing_two_bindings_to_one_id_is_core_behavior() -> None:
    # Two bindings naming the same id: Core's binding rules decide (it refuses aliased state
    # bindings with INVALID_BINDING); the model adds no rule of its own (requirement 5).
    m = StateMachine("task", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    m.transition("finish", from_={Phase.OPEN}, to=Phase.DONE, state={"a": Folder, "b": Folder},
                 effects=[lambda t, a, b: set_(a.label, "a")])
    model = module(m)
    st = store(model)
    ev = st.evaluate(model, "task__finish", bindings={"task": "t1", "a": "f1", "b": "f1"},
                     commit_time=CT)
    assert ev.decision.result == "INVALID_BINDING"


def test_a_binding_cannot_write_a_field_another_model_owns() -> None:
    m = StateMachine("task", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    ap = Approval("task_review", subject=Task, actor=Folder, approve_threshold=1,
                  open_when=lambda t: t.phase == Phase.OPEN, eligible=lambda t, who: True)
    ap.approve(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[Folder]})
    ap.reject(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[Folder]})
    decision = ap.decision_entity
    m.transition("finish", from_={Phase.OPEN}, to=Phase.DONE, state={"d": decision},
                 effects=[lambda t, d: set_(d.subject, t.id)])
    with pytest.raises(ModelBuildError) as e:
        BehaviorModule(entities=[Folder, Task], enums=[Phase], models=[m, ap])
    assert [x["code"] for x in e.value.errors] == ["UNAUTHORIZED_DECISION_WRITE"]


def test_a_case_mixing_language_levels_has_no_single_version_tuple() -> None:
    # Research R13: a reference case maps to one tuple, so a case with a standalone level-2 and
    # a standalone level-3 machine is rejected by the compat gate.
    import importlib.machinery
    import importlib.util
    from pathlib import Path

    path = Path(__file__).resolve().parents[2] / "scripts" / "check-models-compat.py"
    loader = importlib.machinery.SourceFileLoader("check_models_compat", str(path))
    spec = importlib.util.spec_from_loader("check_models_compat", loader)
    assert spec is not None
    gate = importlib.util.module_from_spec(spec)
    loader.exec_module(gate)
    m2 = StateMachine("a", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    m2.transition("finish", from_={Phase.OPEN}, to=Phase.DONE)
    from dataclasses import asdict
    def plain(r: object) -> dict[str, object]:
        return {k: v for k, v in asdict(r).items() if v is not None}  # type: ignore[call-overload]

    receipts = [plain(r) for r in module(machine()).model_receipts]
    receipts += [plain(r) for r in module(m2).model_receipts]
    assert {r["model_semantic_version"] for r in receipts} == {2, 3}
    with pytest.raises(SystemExit):
        gate.case_tuple("mixed", receipts)
