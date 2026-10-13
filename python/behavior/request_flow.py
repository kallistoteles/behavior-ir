"""RequestFlow: Python authoring syntax for request flows (feature 502).

`RequestFlow` only *records* what the author wrote, like `StateMachine`. The Rust model
authority (`behavior-models`) derives the request entity, resolves and checks the flow, lowers
its creations and lowers its transitions through the unchanged StateMachine lowerer.

`request_entity` is a typed handle to the generated entity, usable in annotations and in
`Id[...]` before the module is built. Its name comes from Rust (`EncodeEntityName`); its fields
are a view that the build compares with the schema Rust declares, raising on any difference
(research R11). Python never decides the schema.

Creation and transition callables are bound *by position* in the order Rust supplies:

    creation    request_id, inputs, state bindings, context   (each group sorted by name)
    transition  request, inputs, context                      (as for StateMachine)
"""

from __future__ import annotations

from enum import Enum
from typing import Any, Callable, Iterable, Mapping

from . import _engine
from .decl import EntityDecl, EntityVar, entity_decl
from .errors import BehaviorDefinitionError
from .expr import lift, param_ref
from .location import caller_loc
from .models import Emission, Fragment, StateMachine, _case, _fn_loc, _params
from .statements import CURRENT, Frame
from .types import BType, EnumT, IdT, OptionT, enum_type

#: `values(*params) -> {field: value}`: the creation's initializer `V_c`.
Initializer = Callable[..., Mapping[str, Any]]


class RequestFlow:
    """A request flow (contract `request-flow-contract.md`): one generated request entity, its
    creation capabilities and the StateMachine over its `status` field.

    The flow declares no entry states: they are the creations' target states."""

    def __init__(
        self,
        flow_id: str,
        *,
        actor: type,
        status: type[Enum] | EnumT,
        payload: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        assignee: bool = False,
        display_name: str | None = None,
        description: str | None = None,
    ) -> None:
        loc = caller_loc()
        actor_decl = entity_decl(actor)
        if actor_decl is None:
            raise BehaviorDefinitionError(f"{actor!r} is not an @entity class", *loc)
        try:
            name = _engine.encode_entity_name(flow_id)
        except ValueError as e:
            raise BehaviorDefinitionError(str(e), *loc) from None
        self.flow_id = flow_id
        self._actor = actor_decl
        self._status = status if isinstance(status, EnumT) else enum_type(status)
        self._payload = _params(payload)
        self.assignee = assignee
        self.display_name = display_name
        self.description = description
        self.loc = loc
        view: list[tuple[str, BType, tuple[str, int]]] = [
            ("requester", IdT(actor_decl.name), loc),
            ("status", self._status, loc),
        ]
        if assignee:
            view.append(("assignee", OptionT(IdT(actor_decl.name)), loc))
        view += [(n, t, loc) for n, t in self._payload]
        #: The generated request entity (a typed handle; the schema is Rust's).
        self.request_entity: type = type(name, (), {
            "__behavior_entity__": EntityDecl(name, view, loc),
            "__doc__": f"The request entity of RequestFlow `{flow_id}` (generated).",
        })
        self._creations: list[dict[str, Any]] = []
        # Transitions are recorded and traced exactly as a StateMachine's; Rust derives the
        # embedded machine itself, so this recorder's own model input is never used.
        self._machine = StateMachine(
            flow_id, subject=self.request_entity, state_field="status", entry_states=(),
        )

    @property
    def _decl(self) -> EntityDecl:
        return self.request_entity.__behavior_entity__  # type: ignore[attr-defined,no-any-return]

    def create(
        self,
        creation_id: str,
        *,
        to: Any,
        values: Initializer,
        inputs: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        state: Mapping[str, type] | Iterable[tuple[str, type]] | None = None,
        context: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        requires: Iterable[Fragment] = (),
        effects: Iterable[Fragment] = (),
        ensures: Iterable[Fragment] = (),
        emits: Iterable[Fragment] = (),
        display_name: str | None = None,
        description: str | None = None,
    ) -> RequestFlow:
        """Records a creation capability: a new request in state `to`. `values` returns the
        initializer for every field except `status`; `requires`, `effects`, `ensures` and
        `emits` are as for transitions. Each callable receives `request_id`, then the inputs,
        the state bindings and the context parameters, positionally."""
        loc = caller_loc()
        bindings = []
        items = state.items() if isinstance(state, Mapping) else (state or [])
        for n, cls in items:
            d = entity_decl(cls)
            if d is None:
                raise BehaviorDefinitionError(f"{cls!r} is not an @entity class", *loc)
            bindings.append((str(n), d))
        self._creations.append({
            "creation_id": creation_id,
            "to_state": _case(to),
            "inputs": _params(inputs),
            "state": bindings,
            "context": _params(context),
            "values": values,
            "requires": list(requires),
            "effects": list(effects),
            "ensures": list(ensures),
            "emits": list(emits),
            "display_name": display_name,
            "description": description,
            "loc": loc,
        })
        return self

    def transition(self, transition_id: str, **kwargs: Any) -> RequestFlow:
        """Records a status transition, exactly as `StateMachine.transition`."""
        self._machine.transition(transition_id, **kwargs)
        return self

    def declaration_types(self) -> list[BType]:
        """The types the flow's request entity names, for the module to declare (P1)."""
        return [self._status] + [t for _, t in self._payload]

    def parameter_types(self) -> list[Any]:
        """Every creation and transition parameter type."""
        own = [t for c in self._creations for _, t in c["inputs"] + c["context"]]
        return own + self._machine.parameter_types()

    def bind_schema(self, schema: Mapping[str, Any]) -> None:
        """Checks the handle against the schema Rust declared in P2 (research R11)."""
        decl = self._decl
        declared = [(n, t) for n, t in schema["fields"]]
        view = sorted((n, t.engine()) for n, t, _ in decl.fields)
        if schema["entity"] != decl.name or declared != view:
            raise RuntimeError(
                f"request entity of `{self.flow_id}`: the Python view {decl.name} {view} differs "
                f"from the declared {schema['entity']} {declared}: the binding is out of step "
                "with the model authority"
            )

    def engine_input(self) -> dict[str, Any]:
        """The flow input passed to the model authority, unchanged, plus the tracers."""
        file, line = self.loc
        machine = self._machine.engine_input()
        return {
            "kind": "request_flow",
            "flow_id": self.flow_id,
            "actor_entity": self._actor.name,
            "status_enum": self._status.name,
            "assignee": self.assignee,
            "payload": [(n, t.engine()) for n, t in self._payload],
            "creations": [
                {
                    "creation_id": c["creation_id"],
                    "to_state": c["to_state"],
                    "inputs": [(n, t.engine()) for n, t in c["inputs"]],
                    "state": [(n, d.name) for n, d in c["state"]],
                    "context": [(n, t.engine()) for n, t in c["context"]],
                    "display_name": c["display_name"],
                    "description": c["description"],
                    "file": c["loc"][0],
                    "line": c["loc"][1],
                }
                for c in self._creations
            ],
            "transitions": machine["transitions"],
            "display_name": self.display_name,
            "description": self.description,
            "file": file,
            "line": line,
            "tracer": machine["tracer"],
            "creation_tracer": self._trace_creation,
        }

    def _trace_creation(self, creation_id: str, params: list[tuple[str, str]]) -> dict[str, Any]:
        """Called by the model authority inside the creation's scope."""
        c = next(c for c in self._creations if c["creation_id"] == creation_id)
        state = dict(c["state"])
        symbols = [
            EntityVar(name, state[name], role) if role == "state" else param_ref(name)
            for name, role in params
        ]
        types = {n: t.engine() for n, t, _ in self._decl.fields}

        def conditions(fns: list[Fragment]) -> list[tuple[Any, str, int]]:
            return [(lift(fn(*symbols)).node, *_fn_loc(fn)) for fn in fns]

        values = c["values"](*symbols)
        if not isinstance(values, Mapping):
            raise BehaviorDefinitionError(
                f"creation `{creation_id}`: values must return a mapping of field to value",
                *_fn_loc(c["values"]),
            )
        # A field outside the schema is rejected by name (`INITIALIZER_UNKNOWN_FIELD`), so its
        # value is never used; it may not even be typeable (`None` for a disabled assignee).
        initializer = [
            (str(f), lift(v, types[str(f)]).node if str(f) in types else lift(False).node)
            for f, v in values.items()
        ]
        frame = Frame("action")
        token = CURRENT.set(frame)
        try:
            for fn in c["effects"]:
                fn(*symbols)
        finally:
            CURRENT.reset(token)
        if frame.preconditions or frame.postconditions or frame.lifecycle:
            raise BehaviorDefinitionError(
                f"creation `{creation_id}`: effects may only call set_()", *c["loc"]
            )
        commands = []
        for fn in c["emits"]:
            e = fn(*symbols)
            if not isinstance(e, Emission):
                raise BehaviorDefinitionError(
                    f"creation `{creation_id}`: an emits callable must return emit(...)",
                    *_fn_loc(fn),
                )
            commands.append((e.command, e.when, list(e.payload.items()), *e.loc))
        return {
            "initializer": initializer,
            "commands": commands,
            "guards": conditions(c["requires"]),
            "effects": [(e.param, e.field, e.value.node, *e.loc) for e in frame.effects],
            "postconditions": conditions(c["ensures"]),
        }
