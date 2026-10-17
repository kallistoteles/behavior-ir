"""WorkQueue: Python authoring syntax for work queues (feature 507).

`WorkQueue` only *records* what the author wrote, like `StateMachine`, `RequestFlow` and
`Approval`. The Rust model authority (`behavior-models`) derives the status enum and the work
entity, generates the invariant `ValidWork`, the derived value `terminal`, the reads `ready(now)`
and `expired(now)`, lowers the nine capabilities and enforces work ownership. Python decides
nothing semantic: no guards, no names, no ordering, no claim-generation logic.

A queue defines which changes of work state are valid. It never schedules, executes or orders
work; every decision is Core's evaluation of the lowered behavior.

Callables receive the subject (the work item; for `enqueue` there is none) positionally, then
every other parameter by **keyword**: the generated ones (`worker`, `now`, `lease_until`,
`available_at`, `generation`, `expected_generation`, enqueue's `id` and payload fields) and the
author's own inputs, state bindings and context. A callable receives only the keywords it
declares, unless it takes `**kwargs`.

`work_entity` is a typed handle to the generated entity, usable in annotations, `Id[...]` and
queries before the module is built; its name comes from Rust.
"""

from __future__ import annotations

import inspect
from typing import Any, Callable, Iterable, Mapping

from . import _engine
from .decl import EntityDecl, EntityVar, entity_decl
from .errors import BehaviorDefinitionError, BehaviorTypeError
from .expr import DerivedCall, call, lift, param_ref
from .location import caller_loc
from .models import Emission, Fragment, _fn_loc, _params
from .statements import CURRENT, Frame
from .types import BType, EnumT, IdT, NominalT, OptionT, PrimT, to_type

#: The nine capabilities and whether each takes an authorization guard (spec FR-021).
CAPABILITIES = {
    "enqueue": True, "claim": True, "renew": False, "complete": False, "retry": False,
    "fail": False, "recover": True, "exhaust": True, "cancel": True,
}
STATUSES = ("READY", "LEASED", "SUCCEEDED", "FAILED", "CANCELLED")

Params = Mapping[str, Any] | Iterable[tuple[str, Any]] | None


def _call(fn: Callable[..., Any], subject: Any, named: dict[str, Any]) -> Any:
    """Calls `fn` with the subject (if any) positionally and the keywords it declares."""
    sig = inspect.signature(fn)
    takes_all = any(p.kind is inspect.Parameter.VAR_KEYWORD for p in sig.parameters.values())
    kwargs = named if takes_all else {k: v for k, v in named.items() if k in sig.parameters}
    return fn(**kwargs) if subject is None else fn(subject, **kwargs)


class WorkQueue:
    """A work queue (contract `work-queue-contract.md`): persistent, claimable units of work and
    the valid changes of their state."""

    def __init__(
        self,
        queue_id: str,
        *,
        worker: type,
        time: NominalT,
        max_attempts: int,
        payload: Params = None,
        display_name: str | None = None,
        description: str | None = None,
    ) -> None:
        loc = caller_loc()
        worker_decl = entity_decl(worker)
        if worker_decl is None:
            raise BehaviorDefinitionError(f"{worker!r} is not an @entity class", *loc)
        try:
            status_name, entity_name = _engine.work_names(queue_id)
        except ValueError as e:
            raise BehaviorDefinitionError(str(e), *loc) from None
        self.queue_id = queue_id
        self._worker = worker_decl
        self.time = to_type(time)
        self.max_attempts = max_attempts
        self._payload = _params(payload)
        self.display_name = display_name
        self.description = description
        self.loc = loc
        #: The generated status enum (a typed handle; its cases are Rust's).
        self.status = EnumT(status_name, STATUSES, loc)
        view: list[tuple[str, BType, tuple[str, int]]] = [
            ("attempts", PrimT("int"), loc),
            ("available_at", self.time, loc),
            ("lease_until", OptionT(self.time), loc),
            ("owner", OptionT(IdT(worker_decl.name)), loc),
            ("status", self.status, loc),
        ]
        view += [(n, t, loc) for n, t in self._payload]
        #: The generated work entity (a typed handle; the schema is Rust's).
        self.work_entity: type = type(entity_name, (), {
            "__behavior_entity__": EntityDecl(entity_name, view, loc),
            "__doc__": f"The work entity of WorkQueue `{queue_id}` (generated).",
        })
        self._capabilities: dict[str, dict[str, Any]] = {}
        self._definitions: dict[str, str] = {}

    @property
    def _decl(self) -> EntityDecl:
        return self.work_entity.__behavior_entity__  # type: ignore[attr-defined,no-any-return]

    # --- capabilities ---------------------------------------------------------------------------

    def _capability(
        self,
        capability_id: str,
        authorization: Fragment | None,
        inputs: Params,
        context: Params,
        state: Mapping[str, type] | Iterable[tuple[str, type]] | None,
        requires: Iterable[Fragment],
        effects: Iterable[Fragment],
        ensures: Iterable[Fragment],
        emits: Iterable[Fragment],
        display_name: str | None,
        description: str | None,
    ) -> WorkQueue:
        loc = caller_loc()
        bindings = []
        items = state.items() if isinstance(state, Mapping) else (state or [])
        for n, cls in items:
            d = entity_decl(cls)
            if d is None:
                raise BehaviorDefinitionError(f"{cls!r} is not an @entity class", *loc)
            bindings.append((str(n), d))
        self._capabilities[capability_id] = {
            "capability_id": capability_id,
            "authorization": authorization,
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
        }
        return self

    def enqueue(self, *, authorized: Fragment | None = None, inputs: Params = None,
                context: Params = None, state: Any = None, requires: Iterable[Fragment] = (),
                effects: Iterable[Fragment] = (), ensures: Iterable[Fragment] = (),
                emits: Iterable[Fragment] = (), display_name: str | None = None,
                description: str | None = None) -> WorkQueue:
        """Records `enqueue`: creates one READY item with an explicit `id`, the payload and
        `available_at`. `authorized` is mandatory (use `lambda **_: True` to allow everyone)."""
        return self._capability("enqueue", authorized, inputs, context, state, requires, effects,
                                ensures, emits, display_name, description)

    def claim(self, *, eligible: Fragment | None = None, inputs: Params = None,
              context: Params = None, state: Any = None, requires: Iterable[Fragment] = (),
              effects: Iterable[Fragment] = (), ensures: Iterable[Fragment] = (),
              emits: Iterable[Fragment] = (), display_name: str | None = None,
              description: str | None = None) -> WorkQueue:
        """Records `claim`. `eligible(work, worker=..., ...)` is the mandatory authorization."""
        return self._capability("claim", eligible, inputs, context, state, requires, effects,
                                ensures, emits, display_name, description)

    def _plain(self, capability_id: str, *, authorized: Fragment | None = None,
               inputs: Params = None, context: Params = None, state: Any = None,
               requires: Iterable[Fragment] = (), effects: Iterable[Fragment] = (),
               ensures: Iterable[Fragment] = (), emits: Iterable[Fragment] = (),
               display_name: str | None = None, description: str | None = None) -> WorkQueue:
        return self._capability(
            capability_id, authorized, inputs, context, state, requires, effects, ensures,
            emits, display_name, description)

    def renew(self, **kw: Any) -> WorkQueue:
        """Records `renew`: the current owner moves its live lease later."""
        return self._plain("renew", **kw)

    def complete(self, **kw: Any) -> WorkQueue:
        """Records `complete`: the current owner finishes the work (SUCCEEDED)."""
        return self._plain("complete", **kw)

    def retry(self, **kw: Any) -> WorkQueue:
        """Records `retry`: the current owner gives the work back, READY at `available_at`."""
        return self._plain("retry", **kw)

    def fail(self, **kw: Any) -> WorkQueue:
        """Records `fail`: the current owner fails the work permanently (FAILED)."""
        return self._plain("fail", **kw)

    def recover(self, **kw: Any) -> WorkQueue:
        """Records `recover` (authorized): an expired lease with attempts left becomes READY."""
        return self._plain("recover", **kw)

    def exhaust(self, **kw: Any) -> WorkQueue:
        """Records `exhaust` (authorized): an expired lease with no attempts left is FAILED."""
        return self._plain("exhaust", **kw)

    def cancel(self, **kw: Any) -> WorkQueue:
        """Records `cancel` (authorized): READY or LEASED work becomes CANCELLED."""
        return self._plain("cancel", **kw)

    # --- handles --------------------------------------------------------------------------------

    def terminal(self, work: Any) -> DerivedCall:
        """Whether `work` is SUCCEEDED, FAILED or CANCELLED."""
        loc = caller_loc()
        if not isinstance(work, EntityVar) or work._decl.name != self._decl.name:
            raise BehaviorTypeError(
                f"`{self.queue_id}.terminal` takes a bound `{self._decl.name}` entity, got "
                f"{work!r}", "TYPE_MISMATCH", *loc)
        name = self._definitions.get("terminal")
        if name is None:
            raise BehaviorDefinitionError(
                f"`{self.queue_id}.terminal` is used before the queue is declared", *loc)
        args = [work._name]
        return DerivedCall(call("derived_ref", name, args, loc=loc), loc, name, args)

    def read_name(self, local: str) -> str:
        """The Core name of a generated read: `ready` or `expired`."""
        if local not in ("ready", "expired"):
            raise ValueError(f"no generated read `{local}`")
        return f"{self.queue_id}__{local}"

    def action_name(self, capability: str) -> str:
        """The Core name of a capability's action."""
        if capability not in CAPABILITIES:
            raise ValueError(f"no capability `{capability}`")
        return f"{self.queue_id}__{capability}"

    # --- build protocol -------------------------------------------------------------------------

    def declaration_types(self) -> list[BType]:
        """The authored types the work entity names, for the module to declare (P1)."""
        return [self.time] + [t for _, t in self._payload]

    def parameter_types(self) -> list[Any]:
        """Every authored capability parameter type, and the time type."""
        return [self.time] + [t for c in self._capabilities.values()
                              for _, t in c["inputs"] + c["context"]]

    def bind_schema(self, schema: Mapping[str, Any]) -> None:
        """Checks the handles against what Rust declared in P2."""
        if schema["kind"] == "enum":
            ok = schema["name"] == self.status.name and tuple(schema["cases"]) == self.status.values
            what = f"status enum {schema['name']} {schema['cases']}"
        else:
            declared = [(n, t) for n, t in schema["fields"]]
            view = sorted((n, t.engine()) for n, t, _ in self._decl.fields)
            ok = schema["entity"] == self._decl.name and declared == view
            what = f"work entity {schema['entity']} {declared}"
        if not ok:
            raise RuntimeError(
                f"`{self.queue_id}`: the Python view differs from the declared {what}: the "
                "binding is out of step with the model authority")

    def bind_definition(self, signature: Mapping[str, Any]) -> None:
        """Records a derived definition Rust added in P2, for the typed handles."""
        self._definitions[str(signature["local"])] = str(signature["name"])

    def engine_input(self) -> dict[str, Any]:
        """The queue input passed to the model authority, unchanged, plus the tracer."""
        file, line = self.loc
        return {
            "kind": "work_queue",
            "queue_id": self.queue_id,
            "worker_entity": self._worker.name,
            "time": self.time.engine(),
            "max_attempts": self.max_attempts,
            "payload": [(n, t.engine()) for n, t in self._payload],
            "capabilities": [
                {
                    "capability_id": c["capability_id"],
                    "inputs": [(n, t.engine()) for n, t in c["inputs"]],
                    "context": [(n, t.engine()) for n, t in c["context"]],
                    "state": [(n, d.name) for n, d in c["state"]],
                    "display_name": c["display_name"],
                    "description": c["description"],
                    "file": c["loc"][0],
                    "line": c["loc"][1],
                }
                for c in self._capabilities.values()
            ],
            "display_name": self.display_name,
            "description": self.description,
            "file": file,
            "line": line,
            "capability_tracer": self._trace,
        }

    def _trace(self, capability_id: str, params: list[tuple[str, str]]) -> dict[str, Any]:
        """Called by the model authority inside the capability's scope."""
        c = self._capabilities[capability_id]
        bound = dict(c["state"])
        subject: Any = None
        named: dict[str, Any] = {}
        for name, role in params:
            if role == "state" and name not in bound and subject is None:
                subject = EntityVar(name, self._decl, role)
            elif role == "state":
                named[name] = EntityVar(name, bound[name], role)
            else:
                named[name] = param_ref(name)

        def located(fn: Fragment) -> tuple[Any, str, int]:
            return (lift(_call(fn, subject, named)).node, *_fn_loc(fn))

        frame = Frame("action")
        token = CURRENT.set(frame)
        try:
            for fn in c["effects"]:
                _call(fn, subject, named)
        finally:
            CURRENT.reset(token)
        if frame.preconditions or frame.postconditions or frame.lifecycle:
            raise BehaviorDefinitionError(
                f"capability `{capability_id}`: effects may only call set_()", *c["loc"])
        commands = []
        for fn in c["emits"]:
            e = _call(fn, subject, named)
            if not isinstance(e, Emission):
                raise BehaviorDefinitionError(
                    f"capability `{capability_id}`: an emits callable must return emit(...)",
                    *_fn_loc(fn))
            commands.append((e.command, e.when, list(e.payload.items()), *e.loc))
        authorization = c["authorization"]
        return {
            "authorization": None if authorization is None else located(authorization),
            "guards": [located(fn) for fn in c["requires"]],
            "effects": [(e.param, e.field, e.value.node, *e.loc) for e in frame.effects],
            "postconditions": [located(fn) for fn in c["ensures"]],
            "commands": commands,
        }
