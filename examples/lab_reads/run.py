"""The lab's questions, answered by reads (feature 010): `python -m examples.lab_reads.run`.

Every question the lab asks is a read: no effect-free action, no entity that exists only to be
bound. Reads never change the store; their records are evidence that replays.
"""

from __future__ import annotations

from behavior import InMemoryBackend, IntentRejected, Store

from .model import model

T0 = "2026-10-03T08:00:00Z"


def act(store: Store, action: str, bindings: dict[str, str], **input: object) -> None:
    e = store.evaluate(model, action, bindings=bindings, input=dict(input), commit_time=T0)
    assert e.bundle is not None, e.decision.result
    store.commit(model, e.bundle)


def main() -> None:
    seed = [{"entity": "Customer", "value": {"id": "k1", "name": "Ada", "credit_limit": 100}}]
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed=seed))
    act(store, "start_culture", {}, culture_id="c1", name="Basil")
    act(store, "start_culture", {}, culture_id="c2", name="Mint")
    act(store, "measure", {"culture": "c1"}, ph=7)
    act(store, "measure", {"culture": "c2"}, ph=6)
    act(store, "place_order", {"customer": "k1"}, order_id="o1", amount=30)
    audit_point = store.current()
    act(store, "place_order", {"customer": "k1"}, order_id="o2", amount=90)
    act(store, "retire", {"culture": "c2"})
    head = store.current()

    questions = [
        ("How many cultures are active?", "active_count", {}, {}),
        ("What is Ada's open total?", "open_total", {"customer": "k1"}, {}),
        ("Does culture c3 exist?", "culture_exists", {}, {"culture_id": "c3"}),
        ("Show order o2.", "order_view", {"order": "o2"}, {}),
        ("Show the active cultures.", "active_cultures", {}, {}),
        ("Which orders are at least 50?", "big_orders", {}, {"threshold": 50}),
    ]
    for question, name, bindings, input_ in questions:
        r = store.read(model, name, bindings=bindings, input=input_)
        print(f"{question} {r.value}  [{r.record_id}]")
        assert store.replay_read(model, r.record).matches

    # An auditor's question about the past: the open total just before order o2.
    then = store.read(model, "open_total", bindings={"customer": "k1"}, at=audit_point)
    print(f"Ada's open total at position {audit_point.position}: {then.value}")

    # An agent asks through the capability boundary and sees only the declared result.
    x = store.read_intent(model, {"capability": "customer_summary",
                                  "targets": {"customer": "k1"}})
    print("agent sees:", x.response.to_json())
    try:
        store.read_intent(model, {"capability": "close_order", "targets": {"order": "o1"}})
    except IntentRejected as e:
        print("agent may not:", [err["code"] for err in e.errors])

    assert store.current() == head, "a read never changes the store"
    print("store unchanged at position", head.position)


if __name__ == "__main__":
    main()
