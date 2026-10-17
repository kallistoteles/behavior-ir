"""Behavior Models: Python authoring syntax for models (feature 501).

`StateMachine` only *records* what the author wrote. Every model decision is made by the Rust
model authority (`behavior-models`): resolution, defaults, set semantics, validation, generated
names, ownership, lowering and identity. This module never sorts, deduplicates, defaults,
renames or validates model semantics; collections are passed on exactly as given.

Transition conditions and effects are ordinary behavior expressions. They are traced only
inside the scope the model authority opens, with the parameter names it chooses, bound to the
author's callables *by position* (subject, then inputs, then context, in the order Rust
supplies). The author's Python parameter names never reach the model.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, fields
from enum import Enum
from typing import Any, Callable, Iterable, Mapping

from .decl import EntityVar, command_decl, entity_decl
from .errors import BehaviorDefinitionError
from .expr import lift, param_ref
from .location import caller_loc, code_loc
from .statements import CURRENT, Frame
from .types import to_type

Fragment = Callable[..., Any]


def _case(value: Any) -> str:
    """A state as the engine names it: an enum member's value, or a case name given as text."""
    if isinstance(value, Enum):
        return str(value.value)
    if isinstance(value, str):
        return value
    raise BehaviorDefinitionError(f"{value!r} is not a state (use an enum member)", *caller_loc())


def _fn_loc(fn: Callable[..., Any]) -> tuple[str, int]:
    code = getattr(fn, "__code__", None)
    if code is None:
        return caller_loc()
    return code_loc(code)


def _params(spec: Mapping[str, Any] | Iterable[tuple[str, Any]] | None) -> list[tuple[str, Any]]:
    """Parameter declarations as given (a mapping, in any order: the model sorts them)."""
    if spec is None:
        return []
    items = spec.items() if isinstance(spec, Mapping) else spec
    return [(str(name), to_type(t)) for name, t in items]


@dataclass(frozen=True)
class LoweringReceipt:
    """The provenance of one model in a built module (`behavior.model_receipt.v1`, identity
    contract §3). The Rust model authority produces the receipt and owns its structure; this
    type only exposes it, read-only. `model` is the canonical document that `model_hash`
    hashes, so the hash can be re-derived from the receipt alone."""

    format: str
    model_hash: str
    model_kind: str
    model_semantic_version: int
    lowerer_version: int
    core_release: dict[str, str]
    behavior_hash: str
    model: dict[str, Any]
    source_map: dict[str, Any]
    #: Identity records of embedded models (feature 502: a RequestFlow's StateMachine); absent
    #: from receipts of models that embed none.
    embedded: list[dict[str, Any]] | None = None
    #: The generated enum, entity and derived definitions with their Core hashes (feature 503,
    #: approval data model §9); absent from receipts of models that generate none.
    generated: dict[str, Any] | None = None

    _OPTIONAL = ("embedded", "generated")

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> LoweringReceipt:
        names = {f.name for f in fields(cls)}
        if not names - set(cls._OPTIONAL) <= set(d) <= names:
            raise RuntimeError(
                f"receipt fields {sorted(d)} differ from LoweringReceipt {sorted(names)}: "
                "the binding is out of step with the model authority"
            )
        return cls(**copy.deepcopy(dict(d)))

    def to_dict(self) -> dict[str, Any]:
        """The receipt as the model authority produced it."""
        return {f.name: copy.deepcopy(getattr(self, f.name)) for f in fields(self)
                if not (f.name in self._OPTIONAL and getattr(self, f.name) is None)}


class Emission:
    """A command intent a transition emits (`K_t`): built by `emit(...)` inside an `emits`
    callable. It is a value the model authority places into the generated action; it is not a
    statement and changes no state."""

    def __init__(self, command: str, when: Any, payload: dict[str, Any],
                 loc: tuple[str, int]) -> None:
        self.command = command
        self.when = when
        self.payload = payload
        self.loc = loc


def emit(command_cls: type, *, when: Any = None, **payload: Any) -> Emission:
    """The command intent `command_cls(**payload)`, emitted only if `when` holds (if given)."""
    loc = caller_loc()
    decl = command_decl(command_cls)
    if decl is None:
        raise BehaviorDefinitionError(f"{command_cls!r} is not a @command class", *loc)
    types = {n: t for n, t, _ in decl.fields}
    unknown = sorted(set(payload) - set(types))
    if unknown:
        raise BehaviorDefinitionError(f"`{decl.name}` has no field(s) {unknown}", *loc)
    lifted = {n: lift(v, types[n].engine()).node for n, v in payload.items()}
    return Emission(decl.name, None if when is None else lift(when).node, lifted, loc)


class StateMachine:
    """A state machine over one enum field of an entity (contract `state-machine-contract.md`).

    The enum's cases are the states; the machine declares its entry states and transitions.
    The machine governs every ordinary change of the field; it never creates or removes
    entities. Each transition becomes an ordinary action `<machine id>__<transition id>`.
    """

    def __init__(
        self,
        model_id: str,
        *,
        subject: type,
        state_field: str,
        entry_states: Iterable[Any],
        subject_param: str | None = None,
        display_name: str | None = None,
        description: str | None = None,
    ) -> None:
        decl = entity_decl(subject)
        if decl is None:
            raise BehaviorDefinitionError(f"{subject!r} is not an @entity class", *caller_loc())
        self.model_id = model_id
        self._subject = decl
        self.state_field = state_field
        self.entry_states = [_case(s) for s in entry_states]
        self.subject_param = subject_param
        self.display_name = display_name
        self.description = description
        self.loc = caller_loc()
        self._transitions: list[dict[str, Any]] = []

    def transition(
        self,
        transition_id: str,
        *,
        from_: Iterable[Any],
        to: Any,
        inputs: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        context: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        state: Mapping[str, type] | Iterable[tuple[str, type]] | None = None,
        requires: Iterable[Fragment] = (),
        effects: Iterable[Fragment] = (),
        ensures: Iterable[Fragment] = (),
        emits: Iterable[Fragment] = (),
        display_name: str | None = None,
        description: str | None = None,
    ) -> StateMachine:
        """Records a transition. `requires` and `ensures` are callables returning a condition;
        `effects` are callables that call `set_(...)`; `emits` are callables returning
        `emit(Command, ...)`. Each callable receives the subject, then the state bindings, the
        inputs and the context parameters, each group sorted by name, positionally.

        `state` binds further entities (feature 507) that the transition may change atomically
        with the status. A binding never grants authority over a field another model owns."""
        loc = caller_loc()
        bindings = []
        items = state.items() if isinstance(state, Mapping) else (state or [])
        for n, cls in items:
            d = entity_decl(cls)
            if d is None:
                raise BehaviorDefinitionError(f"{cls!r} is not an @entity class", *loc)
            bindings.append((str(n), d))
        self._transitions.append({
            "transition_id": transition_id,
            "from_states": [_case(s) for s in from_],
            "to_state": _case(to),
            "inputs": _params(inputs),
            "context": _params(context),
            "state": bindings,
            "requires": list(requires),
            "effects": list(effects),
            "ensures": list(ensures),
            "emits": list(emits),
            "display_name": display_name,
            "description": description,
            "loc": loc,
        })
        return self

    def parameter_types(self) -> list[Any]:
        """Every input and context type, for the module to declare their enums and nominals."""
        return [t for tr in self._transitions for _, t in tr["inputs"] + tr["context"]]

    def engine_input(self) -> dict[str, Any]:
        """The model input passed to the model authority, unchanged, plus the tracer."""
        file, line = self.loc
        return {
            "model_id": self.model_id,
            "subject_entity": self._subject.name,
            "state_field": self.state_field,
            "subject_param": self.subject_param,
            "entry_states": self.entry_states,
            "transitions": [
                {
                    "transition_id": t["transition_id"],
                    "from_states": t["from_states"],
                    "to_state": t["to_state"],
                    "inputs": [(n, ty.engine()) for n, ty in t["inputs"]],
                    "context": [(n, ty.engine()) for n, ty in t["context"]],
                    "state": [(n, d.name) for n, d in t["state"]],
                    "display_name": t["display_name"],
                    "description": t["description"],
                    "file": t["loc"][0],
                    "line": t["loc"][1],
                }
                for t in self._transitions
            ],
            "display_name": self.display_name,
            "description": self.description,
            "file": file,
            "line": line,
            "tracer": self._trace,
        }

    def _trace(self, transition_id: str, params: list[tuple[str, str]]) -> dict[str, Any]:
        """Called by the model authority inside the transition's scope."""
        t = next(t for t in self._transitions if t["transition_id"] == transition_id)
        bound = dict(t["state"])
        symbols = [
            EntityVar(name, bound.get(name, self._subject), role) if role == "state"
            else param_ref(name)
            for name, role in params
        ]

        def conditions(fns: list[Fragment]) -> list[tuple[Any, str, int]]:
            out = []
            for fn in fns:
                loc = _fn_loc(fn)
                out.append((lift(fn(*symbols)).node, *loc))
            return out

        frame = Frame("action")
        token = CURRENT.set(frame)
        try:
            for fn in t["effects"]:
                fn(*symbols)
        finally:
            CURRENT.reset(token)
        if frame.preconditions or frame.postconditions or frame.lifecycle:
            raise BehaviorDefinitionError(
                f"transition `{transition_id}`: effects may only call set_()", *t["loc"]
            )
        commands = []
        for fn in t["emits"]:
            e = fn(*symbols)
            if not isinstance(e, Emission):
                raise BehaviorDefinitionError(
                    f"transition `{transition_id}`: an emits callable must return emit(...)",
                    *_fn_loc(fn),
                )
            commands.append((e.command, e.when, list(e.payload.items()), *e.loc))
        return {
            "commands": commands,
            "guards": conditions(t["requires"]),
            "effects": [(e.param, e.field, e.value.node, *e.loc) for e in frame.effects],
            "postconditions": conditions(t["ensures"]),
        }
