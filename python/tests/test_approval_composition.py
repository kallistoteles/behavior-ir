"""Approval composes with RequestFlow, with other approvals and with hand-written behavior
(feature 503, US4: T039).

Approval predicates are ordinary derived rules: a RequestFlow transition can require them, and
hand-written actions, reads and guards can name the decision entity and the predicates (law D5).
Everything runs through the public Store. A refused attempt is DENIED.
"""

from __future__ import annotations

import json
from enum import Enum
from typing import Any

import pytest

from behavior import (
    Approval, BehaviorError, BehaviorModule, Id, InMemoryBackend, Input, StateMachine, Store,
    action, count, entity, field, read, requires, select,
)

from models.fixtures.rf_purchase_request import S, User, flow as base_flow

T = "2026-10-15T00:00:00Z"
USERS = ("u1", "u2", "u3")


def by_actor(decision_id: Any, subject: Any, actor: Any) -> Any:
    return actor


def approval(approval_id: str, subject: type, k_a: int, k_r: int | None) -> Approval:
    a = Approval(approval_id, subject=subject, actor=User, approve_threshold=k_a,
                 reject_threshold=k_r)
    a.approve(actor=by_actor, context={"actor": Id[User]})
    a.reject(actor=by_actor, context={"actor": Id[User]})
    return a


def store_for(m: BehaviorModule, rows: list[dict[str, Any]]) -> Store:
    seed = [{"entity": "User", "value": {"id": u, "name": u}} for u in USERS] + rows
    return Store.create(InMemoryBackend(), m, Store.genesis_for(m, seed))


def attempt(store: Store, m: BehaviorModule, capability: str, subject: str, actor: str,
            decision_id: str) -> str:
    """Evaluates and, if allowed, commits a decision; returns the result."""
    ev = store.evaluate(m, capability, bindings={"subject": subject},
                        input={"decision_id": decision_id}, context={"actor": actor},
                        commit_time=T)
    if ev.bundle is not None:
        store.commit(m, ev.bundle)
    return ev.decision.result


def request_row(status: S = S.IN_REVIEW) -> dict[str, Any]:
    return {"entity": "PurchaseRequest", "value": {
        "id": "r1", "status": status.value, "requester": "u1", "title": "Laptop", "amount": 5,
        "assignee": None}}


def status_of(store: Store, m: BehaviorModule) -> str:
    @read
    def request_status(request: PR):  # type: ignore[valid-type]
        return request.status

    r = store.read(m, request_status, bindings={"request": "r1"})
    value: str = r.response.value
    return value


PR = base_flow().request_entity


# --- RequestFlow guarded by an approval predicate --------------------------------------------

def guarded_module() -> tuple[BehaviorModule, Approval]:
    """`purchase_request_approval` over the flow's request entity (2 approvals or 1 veto), and
    two flow transitions guarded by its outcome."""
    a = approval("purchase_request_approval", PR, 2, 1)
    f = base_flow()
    f.transition("finalize", from_={S.IN_REVIEW}, to=S.APPROVED,
                 requires=[lambda r: a.approved(r)])
    f.transition("close_rejected", from_={S.IN_REVIEW}, to=S.REJECTED,
                 requires=[lambda r: a.rejected(r)])
    return BehaviorModule(entities=[User], enums=[S], models=[f, a]), a


def transition(store: Store, m: BehaviorModule, name: str) -> str:
    ev = store.evaluate(m, f"purchase_request__{name}", bindings={"purchase_request": "r1"},
                        input={}, context={}, commit_time=T)
    if ev.bundle is not None:
        store.commit(m, ev.bundle)
    return ev.decision.result


def test_finalize_approved_denied_until_approved_then_commits() -> None:
    m, _ = guarded_module()
    store = store_for(m, [request_row()])
    capability = "purchase_request_approval__approve"
    assert transition(store, m, "finalize") == "DENY"
    assert attempt(store, m, capability, "r1", "u1", "x1") == "ALLOW"
    assert transition(store, m, "finalize") == "DENY"
    assert attempt(store, m, capability, "r1", "u2", "x2") == "ALLOW"
    assert transition(store, m, "finalize") == "ALLOW"
    assert status_of(store, m) == "Approved"


def test_finalize_rejected_after_veto() -> None:
    m, _ = guarded_module()
    store = store_for(m, [request_row()])
    assert attempt(store, m, "purchase_request_approval__reject", "r1", "u3", "x1") == "ALLOW"
    assert transition(store, m, "finalize") == "DENY"
    assert transition(store, m, "close_rejected") == "ALLOW"
    assert status_of(store, m) == "Rejected"


def test_status_stays_in_review_while_outcome_approved() -> None:
    """The approval never changes its subject: an APPROVED outcome leaves the flow's status
    where it was until a flow transition moves it (spec §30)."""
    m, _ = guarded_module()
    store = store_for(m, [request_row()])
    for i, who in enumerate(("u1", "u2")):
        assert attempt(store, m, "purchase_request_approval__approve", "r1", who, f"x{i}") \
            == "ALLOW"
    assert status_of(store, m) == "InReview"


# --- two approvals on one subject --------------------------------------------------------------

def test_two_approvals_on_one_subject_are_independent() -> None:
    finance = approval("finance_approval", PR, 1, 1)
    legal = approval("legal_review", PR, 1, 1)
    m = BehaviorModule(entities=[User], enums=[S], models=[base_flow(), finance, legal])
    actions = {a["name"] for a in json.loads(m.to_wire_json())["actions"]}
    assert {"finance_approval__approve", "finance_approval__reject",
            "legal_review__approve", "legal_review__reject"} <= actions
    store = store_for(m, [request_row()])
    # One actor may decide in both approvals: they are separate decision entities.
    assert attempt(store, m, "finance_approval__approve", "r1", "u1", "f1") == "ALLOW"
    assert attempt(store, m, "legal_review__reject", "r1", "u1", "l1") == "ALLOW"
    # Each outcome is terminal in its own approval only.
    assert attempt(store, m, "finance_approval__reject", "r1", "u2", "f2") == "DENY"
    assert attempt(store, m, "legal_review__approve", "r1", "u2", "l2") == "DENY"


# --- an approval without a RequestFlow --------------------------------------------------------

@entity
class Document:
    title = field(str)


def test_approval_on_document_without_request_flow() -> None:
    a = approval("doc_approval", Document, 1, None)
    m = BehaviorModule(entities=[User, Document], models=[a])
    store = store_for(m, [{"entity": "Document", "value": {"id": "d1", "title": "Spec"}}])
    assert attempt(store, m, "doc_approval__approve", "d1", "u1", "x1") == "ALLOW"
    assert attempt(store, m, "doc_approval__approve", "d1", "u2", "x2") == "DENY"


# --- hand-written behavior over the decision entity and predicates (law D5) -------------------

DOC = approval("doc_review", Document, 1, None)
DECISION = DOC.decision_entity


@entity
class Publication:
    document = field(Id[Document])
    decision = field(Id[DECISION])


@action
def publish(document: Document, *, publication_id: Input[Id[Publication]],
            decision: Input[Id[DECISION]]):
    from behavior import create
    requires(DOC.approved(document))
    create(Publication, id=publication_id, document=document.id, decision=decision)


@read
def decisions_total():
    return count(select(DECISION))


def test_handwritten_behavior_uses_decision_entity_and_predicates() -> None:
    m = BehaviorModule(entities=[User, Document, Publication], models=[DOC],
                       actions=[publish], reads=[decisions_total])
    store = store_for(m, [{"entity": "Document", "value": {"id": "d1", "title": "Spec"}}])

    def publish_now(pid: str) -> str:
        ev = store.evaluate(m, "publish", bindings={"document": "d1"},
                            input={"publication_id": pid, "decision": "x1"}, context={},
                            commit_time=T)
        if ev.bundle is not None:
            store.commit(m, ev.bundle)
        return ev.decision.result

    assert publish_now("p0") == "DENY"
    assert attempt(store, m, "doc_review__approve", "d1", "u1", "x1") == "ALLOW"
    assert publish_now("p1") == "ALLOW"
    assert store.read(m, decisions_total, bindings={}).response.value == 1


def test_predicate_handle_rejects_non_subject_argument() -> None:
    a = approval("doc_check", Document, 1, None)

    @action
    def wrong(user: User):
        requires(a.approved(user))

    with pytest.raises(BehaviorError):
        BehaviorModule(entities=[User, Document], models=[a], actions=[wrong])


def test_reference_document_review_runs_without_request_flow() -> None:
    """Quickstart 10: the reference case `ap_document_review` (no RequestFlow; `k_A = 2`, payload
    `reason` from the input, a command on approve) runs through the public Store."""
    from models.fixtures import approval as fixtures

    m = fixtures.ap_document_review()
    policy = (fixtures.core_fixtures() / "governance-v2" / "evidence-policy-none.json").read_text()
    seed = [{"entity": "User", "value": {"id": u, "name": u}} for u in USERS]
    seed.append({"entity": "Document", "value": {"id": "d1", "title": "Spec"}})
    store = Store.create(InMemoryBackend(), m, Store.genesis_v2_for(m, seed, policy))

    def review(capability: str, who: str, decision_id: str) -> str:
        ev = store.evaluate(m, f"document_review__{capability}", bindings={"subject": "d1"},
                            input={"decision_id": decision_id, "reason": "checked"},
                            context={"actor": who}, commit_time=T)
        if ev.bundle is not None:
            store.commit(m, ev.bundle)
        return ev.decision.result

    assert review("approve", "u1", "x1") == "ALLOW"
    assert review("reject", "u2", "x2") == "ALLOW"   # no reject threshold: counted, stays open
    assert review("approve", "u3", "x3") == "ALLOW"   # the second approval: APPROVED
    assert review("approve", "u2", "x4") == "DENY"    # terminal


# --- approval predicates as StateMachine and creation guards (FR-010) -------------------------

class ContractStatus(Enum):
    DRAFT = "Draft"
    SIGNED = "Signed"


@entity
class Contract:
    title = field(str)
    status = field(ContractStatus)


def test_state_machine_transition_guarded_by_approval() -> None:
    """A StateMachine transition requires `approved(subject)`: DENY before the threshold, ALLOW
    after it. The approval never moves the machine."""
    a = approval("contract_approval", Contract, 2, None)
    sm = StateMachine("contract", subject=Contract, state_field="status",
                      entry_states={ContractStatus.DRAFT})
    sm.transition("sign", from_={ContractStatus.DRAFT}, to=ContractStatus.SIGNED,
                  requires=[lambda c: a.approved(c)])
    m = BehaviorModule(entities=[User, Contract], enums=[ContractStatus], models=[sm, a])
    store = store_for(m, [{"entity": "Contract", "value": {
        "id": "c1", "title": "NDA", "status": "Draft"}}])
    sign_action = next(x for x in json.loads(m.to_wire_json())["actions"]
                       if x["name"] == "contract__sign")
    subject = sign_action["params"][0]["name"]

    def sign() -> str:
        ev = store.evaluate(m, "contract__sign", bindings={subject: "c1"}, input={},
                            context={}, commit_time=T)
        if ev.bundle is not None:
            store.commit(m, ev.bundle)
        return ev.decision.result

    assert sign() == "DENY"
    assert attempt(store, m, "contract_approval__approve", "c1", "u1", "x1") == "ALLOW"
    assert sign() == "DENY"
    assert attempt(store, m, "contract_approval__approve", "c1", "u2", "x2") == "ALLOW"
    assert sign() == "ALLOW"


def test_request_flow_creation_guarded_by_approval() -> None:
    """A RequestFlow creation with a state binding requires `approved(document)`: no request
    can be created from a document before its approval reaches the threshold."""
    a = approval("doc_approval", Document, 1, None)
    f = base_flow()
    f.create("from_document", to=S.DRAFT, inputs={"title": str, "amount": int},
             state={"document": Document}, context={"actor": Id[User]},
             values=lambda request_id, amount, title, document, actor: {
                 "requester": actor, "title": title, "amount": amount, "assignee": None},
             requires=[lambda request_id, amount, title, document, actor:
                       a.approved(document)])
    m = BehaviorModule(entities=[User, Document], enums=[S], models=[f, a])
    store = store_for(m, [{"entity": "Document", "value": {"id": "d1", "title": "Spec"}}])

    def create(request_id: str) -> str:
        ev = store.evaluate(m, "purchase_request__from_document", bindings={"document": "d1"},
                            input={"request_id": request_id, "title": "Laptop", "amount": 5},
                            context={"actor": "u1"}, commit_time=T)
        if ev.bundle is not None:
            store.commit(m, ev.bundle)
        return ev.decision.result

    assert create("r1") == "DENY"
    assert attempt(store, m, "doc_approval__approve", "d1", "u2", "x1") == "ALLOW"
    assert create("r2") == "ALLOW"
