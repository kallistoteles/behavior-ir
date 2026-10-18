"""An ordinary field with separate assignment, creation and removal permissions."""
from __future__ import annotations

from behavior import (
    BehaviorModule, FieldOwnership, Id, Input, InMemoryBackend, Store, action,
    create, entity, field, remove, set_,
)


@entity
class Note:
    text = field(str)
    label = field(str)


@action
def edit(note: Note, *, text: Input[str]):
    set_(note.text, text)


@action
def open_note(*, note_id: Input[Id[Note]], text: Input[str]):
    create(Note, id=note_id, text=text, label="unchanged")


@action
def discard(note: Note):
    remove(note)


@action
def extra(note: Note):
    set_(note.text, note.text)  # Even a no-op requires assignment permission.


def build(*, safety: bool = True, extra_writer: bool = False) -> BehaviorModule:
    policy = FieldOwnership(id="note_control", entity=Note, writers={"text": [edit]},
                            creators=[open_note], removers=[discard])
    return BehaviorModule(entities=[Note], actions=[edit, open_note, discard] + ([extra] if extra_writer else []),
                          safety=[policy] if safety else [])


def exercise() -> dict[str, object]:
    model = build()
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, []))
    for name, bindings, inputs in [
        ("open_note", {}, {"note_id": "n1", "text": "first"}),
        ("edit", {"note": "n1"}, {"text": "second"}),
        ("discard", {"note": "n1"}, {}),
    ]:
        result = store.evaluate(model, name, bindings=bindings, input=inputs,
                                commit_time="2026-10-18T00:00:00Z")
        assert result.decision.result == "ALLOW", result.decision
        assert result.bundle is not None
        store.commit(model, result.bundle)
        if name != "discard":
            assert store.load("Note", "n1")["value"] == {
                "id": "n1", "text": "first" if name == "open_note" else "second", "label": "unchanged"}
    assert store.current().position == 3
    return {"passed": True, "committed": 3, "boundary": "Core transitions; no genesis safety claim"}
