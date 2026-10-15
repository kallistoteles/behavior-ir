"""Approval test fixtures (feature 503): `doc_approval` over an authored `Document` with actor
`User`, built through the public Python API, and helpers that drive it through the public
`Store` and read its derived values back with ordinary ad-hoc reads."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Callable

from . import rf_purchase_request as rf

from behavior import (
    Approval, BehaviorModule, Id, InMemoryBackend, StateMachine, Store, command, emit, entity,
    field, project, read, select,
)

T0 = "2026-10-15T00:00:00Z"
USERS = ("u1", "u2", "u3", "u4", "u5", "u6")


@entity
class User:
    name = field(str)


@entity
class Document:
    title = field(str)


def actor(decision_id: Any, subject: Any, actor: Any) -> Any:
    """The decision's actor: the trusted context `actor` (data model §4 order)."""
    return actor


def doc_approval(k_a: int, k_r: int | None, **kwargs: Any) -> Approval:
    """`doc_approval` with both capabilities taking `actor: Id[User]` from the context. Extra
    keyword arguments go to `Approval`; `approve=` / `reject=` dicts to the capabilities."""
    approve = kwargs.pop("approve", {})
    reject = kwargs.pop("reject", {})
    ap = Approval("doc_approval", subject=Document, actor=User, approve_threshold=k_a,
                  reject_threshold=k_r, **kwargs)
    ap.approve(**{"actor": actor, "context": {"actor": Id[User]}, **approve})
    ap.reject(**{"actor": actor, "context": {"actor": Id[User]}, **reject})
    return ap


@dataclass
class Built:
    """A built module, its approval, and the reads over the approval's derived values."""

    module: BehaviorModule
    approval: Approval
    reads: dict[str, Callable[..., Any]]


def build(ap: Approval, **module: Any) -> Built:
    """The module with `ap` among its models, and ad-hoc reads of the five derived values and of
    every recorded decision (built after the module, when the handles are bound)."""
    m = BehaviorModule(**{"entities": [User, Document], **module,
                          "models": [*module.get("models", []), ap]})
    reads: dict[str, Callable[..., Any]] = {}
    for local in ("approve_count", "reject_count", "approved", "rejected", "pending"):
        reads[local] = _derived_read(ap, local)

    @read
    def decisions():  # type: ignore[no-untyped-def]
        return project(select(ap.decision_entity), lambda d: [d.subject, d.actor, d.choice])

    reads["decisions"] = decisions
    return Built(m, ap, reads)


def _derived_read(ap: Approval, local: str) -> Callable[..., Any]:
    handle = getattr(ap, local)

    @read
    def value(doc: Document):  # type: ignore[no-untyped-def]
        return handle(doc)

    return value


def seed(docs: tuple[str, ...] = ("d1",)) -> list[dict[str, Any]]:
    users = [{"entity": "User", "value": {"id": u, "name": u}} for u in USERS]
    return users + [{"entity": "Document", "value": {"id": d, "title": d}} for d in docs]


def store(b: Built, docs: tuple[str, ...] = ("d1",), v2: bool = False) -> Store:
    """A store over the built module (a v2 genesis with evidence policy `none` for modules
    with commands)."""
    if v2:
        policy = (core_fixtures() / "governance-v2" / "evidence-policy-none.json").read_text()
        genesis = Store.genesis_v2_for(b.module, seed(docs), policy)
    else:
        genesis = Store.genesis_for(b.module, seed(docs))
    return Store.create(InMemoryBackend(), b.module, genesis)


def core_fixtures() -> Path:
    """The pinned Core Release's conformance fixtures (installed by scripts/fetch-core.sh)."""
    root = Path(__file__).resolve().parents[2]
    version = json.loads((root / "core-release.json").read_text())["version"]
    core = Path(os.environ.get("BEHAVIOR_CORE_DIR", root / ".core" / version))
    return core / "tests" / "fixtures"


class Run:
    """Drives decisions through the public Store and reads the outcome back."""

    def __init__(self, b: Built, docs: tuple[str, ...] = ("d1",), v2: bool = False) -> None:
        self.b = b
        self.store = store(b, docs, v2)
        self._n = 0

    def evaluate(self, capability: str, who: str, doc: str = "d1",
                 decision_id: str | None = None, **context: Any) -> Any:
        if decision_id is None:
            self._n += 1
            decision_id = f"x{self._n}"
        return self.store.evaluate(
            self.b.module, f"doc_approval__{capability}", bindings={"subject": doc},
            input={"decision_id": decision_id}, context={"actor": who, **context},
            commit_time=T0)

    def decide(self, capability: str, who: str, doc: str = "d1", **context: Any) -> bool:
        """Evaluates and commits an allowed decision; whether it was allowed."""
        ev = self.evaluate(capability, who, doc, **context)
        if ev.bundle is None:
            return False
        self.store.commit(self.b.module, ev.bundle)
        return True

    def value(self, local: str, doc: str = "d1") -> Any:
        return self.store.read(self.b.module, self.b.reads[local], bindings={"doc": doc}).value

    def outcome(self, doc: str = "d1") -> tuple[int, int, str]:
        """`(approve_count, reject_count, outcome)`, asserting exactly one predicate holds."""
        flags = [name for name in ("approved", "rejected", "pending")
                 if self.value(name, doc) is True]
        assert len(flags) == 1, f"exactly one outcome must hold, got {flags}"
        return self.value("approve_count", doc), self.value("reject_count", doc), flags[0].upper()

    def decisions(self) -> list[tuple[str, str, str]]:
        """Every recorded decision as `(subject, actor, choice)`."""
        rows = self.store.read(self.b.module, self.b.reads["decisions"]).value
        return [(r["subject"], r["actor"], r["choice"]) for r in rows]


# --- the compatibility reference cases (approval contract §5, T048) ---------------------------

def _by_context(decision_id: Any, *rest: Any) -> Any:
    """The decision's actor: the context `actor`, the last argument of every capability here
    (data model §4 order: decision_id, inputs, subject, context)."""
    return rest[-1]


@command
class DocumentApproved:
    document = field(Id[Document])
    by = field(Id[User])


def ap_document_review() -> BehaviorModule:
    """An approval over an authored `Document` (no RequestFlow): `k_A = 2`, `k_R = ⊥`, payload
    `reason` from the capability input, and a command on approve."""
    ap = Approval("document_review", subject=Document, actor=User, approve_threshold=2,
                  payload={"reason": str})
    for capability in (ap.approve, ap.reject):
        emits = ([lambda decision_id, reason, subject, actor:
                  emit(DocumentApproved, document=subject.id, by=actor)]
                 if capability == ap.approve else [])
        capability(actor=_by_context, inputs={"reason": str}, context={"actor": Id[User]},
                   values=lambda decision_id, reason, subject, actor: {"reason": reason},
                   emits=emits)
    return BehaviorModule(entities=[User, Document], models=[ap], commands=[DocumentApproved])


def ap_purchase_request() -> BehaviorModule:
    """The reference RequestFlow plus an approval over its request entity: `k_A = 2`, `k_R = 1`
    (a veto), open only IN_REVIEW, the requester not eligible, and two flow transitions guarded
    by the derived predicates."""
    s = rf.S
    f = rf.flow()
    ap = Approval("purchase_request_approval", subject=f.request_entity, actor=rf.User,
                  approve_threshold=2, reject_threshold=1,
                  open_when=lambda r: r.status == s.IN_REVIEW,
                  eligible=lambda r, who: who != r.requester)
    ap.approve(actor=_by_context, context={"actor": Id[rf.User]})
    ap.reject(actor=_by_context, context={"actor": Id[rf.User]})
    f.transition("finalize_approved", from_={s.IN_REVIEW}, to=s.APPROVED,
                 requires=[lambda r: ap.approved(r)])
    f.transition("finalize_rejected", from_={s.IN_REVIEW}, to=s.REJECTED,
                 requires=[lambda r: ap.rejected(r)])
    return BehaviorModule(entities=[rf.User], enums=[s], models=[f, ap])


class TicketStatus(Enum):
    OPEN = "Open"
    CLOSED = "Closed"


@entity
class Ticket:
    status = field(TicketStatus)


def ap_two_approvals() -> BehaviorModule:
    """`finance_approval` and `legal_review` over one `Document`, both with approve and reject,
    beside a standalone StateMachine."""
    approvals = []
    for approval_id, k_r in (("finance_approval", 1), ("legal_review", None)):
        ap = Approval(approval_id, subject=Document, actor=User, approve_threshold=1,
                      reject_threshold=k_r)
        ap.approve(actor=_by_context, context={"actor": Id[User]})
        ap.reject(actor=_by_context, context={"actor": Id[User]})
        approvals.append(ap)
    machine = StateMachine("ticket", subject=Ticket, state_field="status",
                           entry_states={TicketStatus.OPEN})
    machine.transition("close", from_={TicketStatus.OPEN}, to=TicketStatus.CLOSED)
    return BehaviorModule(entities=[User, Document, Ticket], enums=[TicketStatus],
                          models=[*approvals, machine])
