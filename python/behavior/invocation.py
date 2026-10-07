"""Checked Core invocation documents and their immutable canonical outcome records."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from . import _engine
from .errors import BehaviorError
from .module import BehaviorModule
from .results import ReplayResult

Document = str | dict[str, Any]


@dataclass(frozen=True)
class InvocationRecord:
    """Core's evidence for one invocation; dictionary access returns detached data."""

    _inner: _engine.InvocationRecord

    @property
    def data(self) -> dict[str, Any]:
        return self._inner.data

    @property
    def json(self) -> str:
        """The engine's canonical record bytes, excluding source diagnostics."""
        return self._inner.json

    @property
    def record_id(self) -> str:
        return self._inner.record_id

    @property
    def outcome_kind(self) -> str:
        return self._inner.outcome_kind

    @property
    def refusal_stage(self) -> str | None:
        return self._inner.refusal_stage

    @property
    def inner_record(self) -> dict[str, Any] | None:
        return self._inner.inner_record

    @property
    def diagnostics(self) -> dict[str, Any] | None:
        return self._inner.diagnostics

    def as_dict(self) -> dict[str, Any]:
        return self.data

    def to_json(self) -> str:
        return self.json


@dataclass(frozen=True)
class Invocation:
    """A read-only store invocation and an optional uncommitted action candidate."""

    record: InvocationRecord
    bundle: dict[str, Any] | None


def record_json(record: InvocationRecord | str) -> str:
    return record.json if isinstance(record, InvocationRecord) else record


def invoke(
    model: BehaviorModule,
    document: Document,
    snapshot: Document,
    *,
    context: dict[str, Any] | None = None,
) -> InvocationRecord:
    """Invokes a declared action or read against an explicit snapshot.

    With explicit host `context`, `document` is a capability intent. Otherwise it is a
    requested invocation. Semantic refusals return records; invalid transport raises
    BehaviorError. Raw document strings reach Core unchanged.
    """
    try:
        record = model.engine.invoke(document, snapshot, context)
    except _engine.EngineError as error:
        raise BehaviorError(str(error.args[1])) from None
    return InvocationRecord(record)


def replay_invocation(model: BehaviorModule, record: InvocationRecord | str) -> ReplayResult:
    """Asks Core to re-evaluate and compare the recorded request, facts and outcome."""
    matches, diff = model.engine.replay_invocation(record_json(record))
    return ReplayResult(matches, diff)
