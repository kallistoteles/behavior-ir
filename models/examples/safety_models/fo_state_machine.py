"""FieldOwnership intersects StateMachine's existing lifecycle ownership."""
from __future__ import annotations

from enum import Enum
from behavior import (
    BehaviorModule, FieldOwnership, GeneratedAction, InMemoryBackend, StateMachine,
    Store, action, entity, field, set_,
)


class Phase(Enum):
    OPEN = "Open"
    DONE = "Done"


@entity
class Task:
    phase = field(Phase)
    note = field(str)


@action
def extra(task: Task):
    set_(task.phase, task.phase)


def build(*, safety: bool = True, extra_writer: bool = False) -> BehaviorModule:
    machine = StateMachine("task", subject=Task, state_field="phase", entry_states={Phase.OPEN})
    machine.transition("finish", from_={Phase.OPEN}, to=Phase.DONE)
    policy = FieldOwnership(id="task_control", entity=Task,
                            writers={"phase": [GeneratedAction("task", "finish")]})
    return BehaviorModule(entities=[Task], enums=[Phase], models=[machine],
                          actions=[extra] if extra_writer else [], safety=[policy] if safety else [])


def exercise() -> dict[str, object]:
    model = build()
    seed = [{"entity": "Task", "value": {"id": "t1", "phase": "Open", "note": "unchanged"}}]
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
    result = store.evaluate(model, "task__finish", bindings={"task": "t1"},
                            commit_time="2026-10-18T00:00:00Z")
    assert result.decision.result == "ALLOW"
    assert result.bundle is not None
    store.commit(model, result.bundle)
    assert store.load("Task", "t1")["value"] == {"id": "t1", "phase": "Done", "note": "unchanged"}
    return {"passed": True, "committed": 1, "boundary": "Core transitions; unchanged Model receipt"}
