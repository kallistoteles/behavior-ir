"""First-class reads (feature 010): evaluating declared and ad-hoc reads, their records and
their capability responses.

A read observes one exact state and changes nothing. Every read returns a `ReadResult`: the
value, the full `ReadRecord` (evidence for the trusted host) and the `ReadResponse` (what an
untrusted caller may see: the result and the record's identity, nothing else).
"""

from __future__ import annotations

import copy
from dataclasses import dataclass
from typing import Any

from . import _engine
from .decl import ReadFn
from .errors import BehaviorDefinitionError
from .module import BehaviorModule, CompileSession, _SESSION, trace_read
from .results import ReplayResult


class _ReadSession(CompileSession):
    """The builder session of an ad-hoc read: typed against an admitted module's declarations
    and derived values, which are not traced again."""

    def __init__(self, model: BehaviorModule) -> None:
        super().__init__()
        self.builder = _engine.Builder.for_module(model.engine)

    def declare(self, t: Any) -> None:
        """Types come from the module."""

    def trace_derived(self, fn: Any) -> None:
        """Derived values come from the module, already typed."""


@dataclass(frozen=True)
class ReadResponse:
    """What a capability caller sees of a read: the declared result and the record identity.
    It carries no trace and no observations."""

    result: str
    value: Any
    reasons: list[dict[str, Any]] | None
    record_id: str
    _data: dict[str, Any]
    _json: str

    def as_dict(self) -> dict[str, Any]:
        return copy.deepcopy(self._data)

    def to_json(self) -> str:
        """Canonical JSON."""
        return self._json


@dataclass(frozen=True)
class ReadRecord:
    """The evidence of one read: canonical, content-addressed, replayable. A store never keeps
    it; a host may."""

    _data: dict[str, Any]
    _json: str

    @property
    def record_id(self) -> str:
        return str(self._data["record_id"])

    @property
    def result(self) -> str:
        return str(self._data["result"])

    def as_dict(self) -> dict[str, Any]:
        return copy.deepcopy(self._data)

    def to_json(self) -> str:
        """Canonical JSON."""
        return self._json


@dataclass(frozen=True)
class ReadResult:
    """The outcome of a read for the trusted host."""

    result: str
    value: Any
    reasons: list[dict[str, Any]] | None
    record_id: str
    record: ReadRecord
    response: ReadResponse


def _result(x: Any) -> ReadResult:
    rec = x.record
    resp = x.response
    response = ReadResponse(resp["result"], resp.get("value"), resp.get("reasons"),
                            resp["record_id"], resp, x.response_json)
    record = ReadRecord(rec, x.record_json)
    return ReadResult(rec["result"], rec.get("value"), rec.get("reasons"), rec["record_id"],
                      record, response)


def read_source(model: BehaviorModule, r: str | ReadFn) -> Any:
    """The engine source of a read: a declared read's name, or an ad-hoc read admitted against
    the module."""
    if isinstance(r, str):
        return r
    if not isinstance(r, ReadFn):
        raise BehaviorDefinitionError(f"{r!r} is not a read; decorate it with @read")
    if any(x is r for x in model.reads):
        return r.name
    session = _ReadSession(model)
    token = _SESSION.set(session)
    try:
        return trace_read(session, r, declared=False)
    finally:
        _SESSION.reset(token)


def evaluate_read(
    model: BehaviorModule,
    r: str | ReadFn,
    *,
    state: dict[str, Any] | None = None,
    input: dict[str, Any] | None = None,  # noqa: A002
    context: dict[str, Any] | None = None,
    data_version: str,
    facts: dict[str, Any] | None = None,
) -> ReadResult:
    """Evaluates a read in plain mode against supplied state values and facts. `r` is a declared
    read (its name or function) or an undeclared `@read` function (an ad-hoc read)."""
    x = model.engine.evaluate_read(
        read_source(model, r), state or {}, input or {}, context or {}, data_version, facts
    )
    return _result(x)


@dataclass(frozen=True)
class ReadExecution:
    """A read intent's execution: the `response` for the caller, the `record` for the host."""

    response: ReadResponse
    record: ReadRecord


def _execution(x: Any) -> ReadExecution:
    r = _result(x)
    return ReadExecution(r.response, r.record)


def read_intent(model: BehaviorModule, intent: dict[str, Any], *,
                host: dict[str, Any]) -> ReadExecution:
    """A read intent in plain mode (feature 010): an untrusted caller's `{capability, targets,
    input}` naming a declared read, with the trusted host's `{data_version, state, context,
    facts}`. Raises IntentRejected listing every problem; forward only `response`."""
    from .errors import IntentRejected

    try:
        x = model.engine.evaluate_read_intent(intent, host)
    except _engine.EngineIntentRejected as e:
        raise IntentRejected(e.args[0]) from None
    return _execution(x)


def record_json(record: ReadRecord | str) -> str:
    """The canonical JSON text of a read record."""
    return record.to_json() if isinstance(record, ReadRecord) else record


def replay_read(model: BehaviorModule, record: ReadRecord | str) -> ReplayResult:
    """Replays a read record from its own facts: the read is evaluated again with the recorded
    state, parameters and facts, and compared byte for byte (its identity included)."""
    matches, diff = model.engine.replay_read(record_json(record))
    return ReplayResult(matches, diff)


__all__ = [
    "ReadExecution", "ReadRecord", "ReadResponse", "ReadResult", "evaluate_read", "read_intent",
    "replay_read",
]
