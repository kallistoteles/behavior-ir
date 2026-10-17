"""Three Models compose through public generated handles and ordinary Core expressions."""
from enum import Enum

from behavior import (
    Approval, BehaviorModule, Id, Input, ModelBuildError, RequestFlow, StateMachine, action,
    admit, entity, field, set_,
)


class Status(Enum):
    OPEN = "Open"
    DONE = "Done"


@entity
class User:
    name = field(str)


@entity
class Ticket:
    status = field(Status)


machine = StateMachine("ticket", subject=Ticket, state_field="status", entry_states={Status.OPEN})
machine.transition("close", from_={Status.OPEN}, to=Status.DONE)
flow = RequestFlow("request", actor=User, status=Status, payload={"title": str})
flow.create("open", to=Status.OPEN, inputs={"title": str}, context={"actor": Id[User]},
            values=lambda request_id, title, actor: {"requester": actor, "title": title})
Request = flow.request_entity
review = Approval("review", subject=Request, actor=User, approve_threshold=1, reject_threshold=1,
                  open_when=lambda r: r.status == Status.OPEN,
                  eligible=lambda r, who: who != r.requester)
review.approve(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})
review.reject(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})
Decision = review.decision_entity
flow.transition("finish", from_={Status.OPEN}, to=Status.DONE,
                requires=[lambda r: review.approved(r)])
model = BehaviorModule(entities=[User, Ticket], enums=[Status], models=[machine, flow, review])
assert admit(model).ok
assert Request.__name__ == "Request" and Decision.__name__ == "ReviewDecision"
assert {r.model_kind for r in model.model_receipts} == {"state_machine", "request_flow", "approval"}
assert all(r.behavior_hash == model.behavior_version for r in model.model_receipts)


@action
def rewrite_decision(d: Decision, *, who: Input[Id[User]]):
    set_(d.actor, who)


try:
    BehaviorModule(entities=[User, Ticket], enums=[Status], models=[machine, flow, review],
                   actions=[rewrite_decision])
except ModelBuildError as error:
    assert "UNAUTHORIZED_DECISION_WRITE" in {e["code"] for e in error.errors}, error
else:
    raise AssertionError("composed ownership must reject decision mutation")
