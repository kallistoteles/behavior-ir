"""Entities and field types: text, booleans, enums, fixed-scale money, optional values,
identities and references. Admission checks everything; the result is a content-addressed module."""

from decimal import Decimal
from enum import Enum

from behavior import (
    BehaviorModule, Context, Id, Option, Ref, action, admit, entity, evaluate, field, nominal,
    requires, set_,
)

# A fixed-scale decimal type: exactly two fractional digits, with the operations it allows.
Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


class InvoiceStatus(Enum):
    PENDING = "pending"
    APPROVED = "approved"


@entity
class User:
    role = field(str)
    approval_limit = field(Money)


@entity
class Invoice:
    amount = field(Money)
    status = field(InvoiceStatus)
    approved_by = field(Option[Id[User]])  # optional: None until approved
    owner = field(Ref[User])  # a reference: Id[User] that must point at an existing User


@action
def approve(invoice: Invoice, approver: User):
    requires(invoice.status == InvoiceStatus.PENDING)
    requires(invoice.amount <= approver.approval_limit)
    set_(invoice.status, InvoiceStatus.APPROVED)
    set_(invoice.approved_by, approver.id)


# Who is acting is context: the host passes the whole acting entity, which the action reads but
# never changes. Use a state parameter instead when the entity comes from the store and may change.
@action
def approve_as(invoice: Invoice, *, actor: Context[User]):
    requires(actor.role == "manager")
    requires(invoice.amount <= actor.approval_limit)
    set_(invoice.status, InvoiceStatus.APPROVED)
    set_(invoice.approved_by, actor.id)


model = BehaviorModule(entities=[User, Invoice], actions=[approve, approve_as])

result = admit(model)
assert result.ok, result.errors
assert model.behavior_version is not None and model.behavior_version.startswith("sha256:")

state = {
    "invoice": {"id": "i1", "amount": Decimal("250.00"), "status": InvoiceStatus.PENDING,
                "approved_by": None, "owner": "u1"},
    "approver": {"id": "u1", "role": "manager", "approval_limit": Decimal("500.00")},
}
facts = {"existence": [{"entity": "User", "id": "u1", "exists": True}]}
decision = evaluate(model, "approve", state=state, data_version="1", facts=facts)
assert decision.result == "ALLOW", decision.reasons
assert [(c.field, c.new) for c in decision.changes] == [("status", "approved"), ("approved_by", "u1")]

as_manager = evaluate(model, "approve_as", state={"invoice": state["invoice"]}, data_version="1",
                      context={"actor": {"id": "u2", "role": "manager",
                                         "approval_limit": Decimal("1000.00")}}, facts=facts)
assert as_manager.result == "ALLOW", as_manager.reasons

# Malformed data is not a decision: a value that does not fit its type is INVALID_INPUT (an
# incoming entity that breaks a constraint would be INVALID_STATE).
negative = dict(state["invoice"], amount=Decimal("250.001"))
assert evaluate(model, "approve", state=dict(state, invoice=negative), data_version="1",
                facts=facts).result == "INVALID_INPUT"  # three decimals do not fit Money

raise AssertionError("T079 deliberately broken skill example")
