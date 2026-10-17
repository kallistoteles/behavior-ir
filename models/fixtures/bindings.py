"""Reference cases for transition state bindings (feature 507, D1(a); research R13). Each case
uses a single language level: a StateMachine at level 3, and a RequestFlow at level 3 with its
embedded machine at level 3."""

from __future__ import annotations

from enum import Enum

from behavior import BehaviorModule, Id, RequestFlow, StateMachine, entity, field, set_


class TaskPhase(Enum):
    OPEN = "Open"
    DONE = "Done"


@entity
class Board:
    open_tasks = field(int)


@entity
class Task:
    phase = field(TaskPhase)
    title = field(str)


def sm_bind_task_board() -> BehaviorModule:
    """`finish` decrements the bound board's open count atomically with the phase change."""
    m = StateMachine("task", subject=Task, state_field="phase", entry_states={TaskPhase.OPEN})
    m.transition("finish", from_={TaskPhase.OPEN}, to=TaskPhase.DONE, state={"board": Board},
                 requires=[lambda t, board: board.open_tasks > 0],
                 effects=[lambda t, board: set_(board.open_tasks, board.open_tasks - 1)])
    return BehaviorModule(entities=[Board, Task], enums=[TaskPhase], models=[m])


class TicketStatus(Enum):
    NEW = "New"
    CLOSED = "Closed"


@entity
class Agent:
    closed = field(int)


def rf_bind_ticket() -> BehaviorModule:
    """A flow whose `close` transition credits the bound agent."""
    f = RequestFlow("ticket", actor=Agent, status=TicketStatus, payload={"title": str})
    f.create("open", to=TicketStatus.NEW, inputs={"title": str}, context={"actor": Id[Agent]},
             values=lambda request_id, title, actor: {"requester": actor, "title": title})
    f.transition("close", from_={TicketStatus.NEW}, to=TicketStatus.CLOSED,
                 state={"agent": Agent},
                 effects=[lambda r, agent: set_(agent.closed, agent.closed + 1)])
    return BehaviorModule(entities=[Agent], enums=[TicketStatus], models=[f])
