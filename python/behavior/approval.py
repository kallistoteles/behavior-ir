"""Approval: Python authoring syntax for approvals (feature 503).

`Approval` only *records* what the author wrote, like `StateMachine` and `RequestFlow`. The Rust
model authority (`behavior-models`) derives the choice enum and the decision entity, checks the
thresholds, generates the five derived definitions, lowers the two decision capabilities and
enforces decision ownership. Python decides nothing semantic: no thresholds, no counts, no names.

Terminology: an attempt that is not permitted is DENIED; REJECT is a recorded choice; REJECTED is
the derived outcome.

`decision_entity` is a typed handle to the generated decision entity, usable in annotations and
in `Id[...]` before the module is built; its name comes from Rust and its fields are a view that
the build compares with the schema Rust declares. `approve_count`, `reject_count`, `approved`,
`rejected` and `pending` return references to the derived definitions Rust adds in P2; they take
a bound entity of the subject type.

Decision callables are bound *by position* in the order Rust supplies (data model §4):

    actor, values, requires, emits   decision_id, inputs (sorted), subject, context (sorted)
    open_when                        subject, then the others in that order
    eligible                         subject, actor, then the others in that order

`open_when` and `eligible` receive only as many arguments as they take.
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
from .types import BType, EnumT, IdT

#: `values(*params) -> {field: value}`: the payload initializer of a decision.
Initializer = Callable[..., Mapping[str, Any]]

_CAPABILITIES = ("approve", "reject")


def _leading(fn: Callable[..., Any], args: list[Any]) -> list[Any]:
    """The first arguments `fn` takes (all of them for `*args`)."""
    params = inspect.signature(fn).parameters.values()
    if any(p.kind is inspect.Parameter.VAR_POSITIONAL for p in params):
        return args
    return args[: len(list(params))]


class Approval:
    """An approval (contract `approval-contract.md`): immutable decisions about an existing
    subject, and the outcome derived from them. It never changes the subject."""

    def __init__(
        self,
        approval_id: str,
        *,
        subject: type,
        actor: type,
        approve_threshold: int,
        reject_threshold: int | None = None,
        payload: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        open_when: Fragment | None = None,
        eligible: Fragment | None = None,
        display_name: str | None = None,
        description: str | None = None,
    ) -> None:
        loc = caller_loc()
        subject_decl = entity_decl(subject)
        actor_decl = entity_decl(actor)
        for spec, decl in ((subject, subject_decl), (actor, actor_decl)):
            if decl is None:
                raise BehaviorDefinitionError(f"{spec!r} is not an @entity class", *loc)
        assert subject_decl is not None and actor_decl is not None
        try:
            choice_name, decision_name = _engine.decision_names(approval_id)
        except ValueError as e:
            raise BehaviorDefinitionError(str(e), *loc) from None
        self.approval_id = approval_id
        self._subject = subject_decl
        self._actor = actor_decl
        self.approve_threshold = approve_threshold
        self.reject_threshold = reject_threshold
        self._payload = _params(payload)
        self._open = open_when
        self._eligible = eligible
        self.display_name = display_name
        self.description = description
        self.loc = loc
        #: The generated choice enum (a typed handle; its cases are Rust's).
        self.choice = EnumT(choice_name, ("APPROVE", "REJECT"), loc)
        view: list[tuple[str, BType, tuple[str, int]]] = [
            ("actor", IdT(actor_decl.name), loc),
            ("choice", self.choice, loc),
            ("subject", IdT(subject_decl.name), loc),
        ]
        view += [(n, t, loc) for n, t in self._payload]
        #: The generated decision entity (a typed handle; the schema is Rust's).
        self.decision_entity: type = type(decision_name, (), {
            "__behavior_entity__": EntityDecl(decision_name, view, loc),
            "__doc__": f"The decision entity of Approval `{approval_id}` (generated).",
        })
        self._capabilities: dict[str, dict[str, Any]] = {}
        #: Local definition id → Core name, from P2 (the signatures Rust declared).
        self._definitions: dict[str, str] = {}

    @property
    def _decl(self) -> EntityDecl:
        return self.decision_entity.__behavior_entity__  # type: ignore[attr-defined,no-any-return]

    # --- capabilities ---------------------------------------------------------------------------

    def _capability(
        self,
        capability_id: str,
        actor: Fragment,
        values: Initializer | None,
        inputs: Mapping[str, Any] | Iterable[tuple[str, Any]] | None,
        context: Mapping[str, Any] | Iterable[tuple[str, Any]] | None,
        requires: Iterable[Fragment],
        emits: Iterable[Fragment],
        display_name: str | None,
        description: str | None,
    ) -> Approval:
        if not callable(actor):
            raise BehaviorDefinitionError(
                f"capability `{capability_id}`: `actor` must be a callable returning the actor's "
                "identity", *caller_loc()
            )
        self._capabilities[capability_id] = {
            "capability_id": capability_id,
            "actor": actor,
            "values": values,
            "inputs": _params(inputs),
            "context": _params(context),
            "requires": list(requires),
            "emits": list(emits),
            "display_name": display_name,
            "description": description,
            "loc": caller_loc(),
        }
        return self

    def approve(
        self,
        *,
        actor: Fragment,
        values: Initializer | None = None,
        inputs: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        context: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        requires: Iterable[Fragment] = (),
        emits: Iterable[Fragment] = (),
        display_name: str | None = None,
        description: str | None = None,
    ) -> Approval:
        """Records the `approve` capability: it records APPROVE. `actor` returns the deciding
        actor's identity, `values` the payload, `requires` the guards `G_A`, `emits` command
        intents."""
        return self._capability("approve", actor, values, inputs, context, requires, emits,
                                display_name, description)

    def reject(
        self,
        *,
        actor: Fragment,
        values: Initializer | None = None,
        inputs: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        context: Mapping[str, Any] | Iterable[tuple[str, Any]] | None = None,
        requires: Iterable[Fragment] = (),
        emits: Iterable[Fragment] = (),
        display_name: str | None = None,
        description: str | None = None,
    ) -> Approval:
        """Records the `reject` capability: it records REJECT (same shape as `approve`)."""
        return self._capability("reject", actor, values, inputs, context, requires, emits,
                                display_name, description)

    # --- derived handles ------------------------------------------------------------------------

    def _derived(self, local: str, subject: Any) -> DerivedCall:
        loc = caller_loc()
        if not isinstance(subject, EntityVar) or subject._decl.name != self._subject.name:
            raise BehaviorTypeError(
                f"`{self.approval_id}.{local}` takes a bound `{self._subject.name}` entity, got "
                f"{subject!r}", "TYPE_MISMATCH", *loc
            )
        name = self._definitions.get(local)
        if name is None:
            raise BehaviorDefinitionError(
                f"`{self.approval_id}.{local}` is used before the approval is declared (list the "
                "approval in the module's models)", *loc
            )
        args = [subject._name]
        return DerivedCall(call("derived_ref", name, args, loc=loc), loc, name, args)

    def approve_count(self, subject: Any) -> DerivedCall:
        """The number of APPROVE decisions recorded for `subject`."""
        return self._derived("approve_count", subject)

    def reject_count(self, subject: Any) -> DerivedCall:
        """The number of REJECT decisions recorded for `subject`."""
        return self._derived("reject_count", subject)

    def approved(self, subject: Any) -> DerivedCall:
        """Whether the outcome for `subject` is APPROVED."""
        return self._derived("approved", subject)

    def rejected(self, subject: Any) -> DerivedCall:
        """Whether the outcome for `subject` is REJECTED."""
        return self._derived("rejected", subject)

    def pending(self, subject: Any) -> DerivedCall:
        """Whether the outcome for `subject` is still PENDING."""
        return self._derived("pending", subject)

    # --- build protocol -------------------------------------------------------------------------

    def declaration_types(self) -> list[BType]:
        """The authored types the decision entity names, for the module to declare (P1)."""
        return [t for _, t in self._payload]

    def parameter_types(self) -> list[Any]:
        """Every capability parameter type."""
        return [t for c in self._capabilities.values() for _, t in c["inputs"] + c["context"]]

    def bind_schema(self, schema: Mapping[str, Any]) -> None:
        """Checks the handles against what Rust declared in P2."""
        if schema["kind"] == "enum":
            ok = schema["name"] == self.choice.name and tuple(schema["cases"]) == self.choice.values
            what = f"choice enum {schema['name']} {schema['cases']}"
        else:
            decl = self._decl
            declared = [(n, t) for n, t in schema["fields"]]
            view = sorted((n, t.engine()) for n, t, _ in decl.fields)
            ok = schema["entity"] == decl.name and declared == view
            what = f"decision entity {schema['entity']} {declared}"
        if not ok:
            raise RuntimeError(
                f"`{self.approval_id}`: the Python view differs from the declared {what}: the "
                "binding is out of step with the model authority"
            )

    def bind_definition(self, signature: Mapping[str, Any]) -> None:
        """Records a derived definition Rust added in P2, for the typed handles."""
        self._definitions[str(signature["local"])] = str(signature["name"])

    def engine_input(self) -> dict[str, Any]:
        """The approval input passed to the model authority, unchanged, plus the tracer."""
        file, line = self.loc
        return {
            "kind": "approval",
            "approval_id": self.approval_id,
            "subject_entity": self._subject.name,
            "actor_entity": self._actor.name,
            "approve_threshold": self.approve_threshold,
            "reject_threshold": self.reject_threshold,
            "payload": [(n, t.engine()) for n, t in self._payload],
            "capabilities": [
                {
                    "capability_id": c["capability_id"],
                    "inputs": [(n, t.engine()) for n, t in c["inputs"]],
                    "context": [(n, t.engine()) for n, t in c["context"]],
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
            "decision_tracer": self._trace_decision,
        }

    def _trace_decision(self, capability_id: str, params: list[tuple[str, str]]) -> dict[str, Any]:
        """Called by the model authority inside the capability's scope."""
        c = self._capabilities[capability_id]
        symbols = [
            EntityVar(name, self._subject, role) if role == "state" else param_ref(name)
            for name, role in params
        ]
        subject = next(s for s, (_, role) in zip(symbols, params) if role == "state")
        others = [s for s, (_, role) in zip(symbols, params) if role != "state"]

        def located(fn: Fragment, args: list[Any]) -> tuple[Any, str, int]:
            return (lift(fn(*args)).node, *_fn_loc(fn))

        actor = lift(c["actor"](*symbols))
        open_ = None if self._open is None else located(
            self._open, _leading(self._open, [subject, *others]))
        eligible = None if self._eligible is None else located(
            self._eligible, _leading(self._eligible, [subject, actor, *others]))
        types = {n: t.engine() for n, t, _ in self._decl.fields}
        values = {} if c["values"] is None else c["values"](*symbols)
        if not isinstance(values, Mapping):
            raise BehaviorDefinitionError(
                f"capability `{capability_id}`: values must return a mapping of field to value",
                *_fn_loc(c["values"]),
            )
        # A field outside the payload is rejected by name (`INITIALIZER_UNKNOWN_FIELD`), so its
        # value is never used.
        payload = [
            (str(f), lift(v, types[str(f)]).node if str(f) in types else lift(False).node)
            for f, v in values.items()
        ]
        commands = []
        for fn in c["emits"]:
            e = fn(*symbols)
            if not isinstance(e, Emission):
                raise BehaviorDefinitionError(
                    f"capability `{capability_id}`: an emits callable must return emit(...)",
                    *_fn_loc(fn),
                )
            commands.append((e.command, e.when, list(e.payload.items()), *e.loc))
        return {
            "actor": (actor.node, *_fn_loc(c["actor"])),
            "open": open_,
            "eligible": eligible,
            "guards": [located(fn, symbols) for fn in c["requires"]],
            "payload": payload,
            "commands": commands,
            "effects": [],
            "postconditions": [],
        }
