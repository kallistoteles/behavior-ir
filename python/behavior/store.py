"""The persistence contract (feature 005): a store over a backend, evaluated against one
consistent snapshot, with guarded commits and replayable history.

The engine owns every rule; a backend only stores documents. `InMemoryBackend()` is the
reference backend; any object with the seven backend methods (`genesis`, `head`, `create`,
`version_at`, `version`, `record`, `commit`, dicts in and out) is a backend too.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Iterator, Sequence

from . import _engine
from .errors import CommitRefused, StateConflict
from .module import BehaviorModule
from .results import Decision

InMemoryBackend = _engine.InMemoryBackend


@dataclass(frozen=True)
class StateRef:
    """A state (content identity) at a history position."""

    state: str
    position: int

    def as_dict(self) -> dict[str, Any]:
        return {"state": self.state, "position": self.position}

    @staticmethod
    def of(d: dict[str, Any]) -> StateRef:
        return StateRef(d["state"], d["position"])


@dataclass(frozen=True)
class Evaluation:
    decision: Decision
    bundle: dict[str, Any] | None  # a commit bundle, only for allowed decisions


@dataclass(frozen=True)
class CommitResult:
    record_id: str
    result_state: StateRef
    already: bool  # the same transition was already committed
    evidence_trust: str | None  # "structural" when an authorization was bound


@dataclass(frozen=True)
class ReplayReport:
    ok: bool
    kind: str
    checked: int
    divergence: dict[str, Any] | None


@dataclass(frozen=True)
class ConformanceReport:
    cases: list[tuple[str, bool, str]]

    @property
    def ok(self) -> bool:
        return all(ok for _, ok, _ in self.cases)

    def failed(self) -> list[tuple[str, bool, str]]:
        return [c for c in self.cases if not c[1]]


class _Translate:
    def __enter__(self) -> None:
        return None

    def __exit__(self, kind: Any, e: Any, tb: Any) -> None:
        if isinstance(e, _engine.EngineStoreConflict):
            raise StateConflict(e.args[0], e.args[1]) from None
        if isinstance(e, _engine.EngineStoreRefused):
            raise CommitRefused(e.args[0], e.args[1]) from None


def _ref(r: StateRef | dict[str, Any]) -> dict[str, Any]:
    return r.as_dict() if isinstance(r, StateRef) else r


class Store:
    """Engine-owned store semantics over a backend."""

    def __init__(self, inner: _engine.Store) -> None:
        self._inner = inner

    @staticmethod
    def genesis_for(
        model: BehaviorModule, seed: Sequence[dict[str, Any]], policy: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """A genesis for the model's entities: `seed` lists {"entity", "value"} dicts; `policy`
        is an evidence policy (default: no governance evidence required)."""
        return _engine.Store.genesis_for(model.engine, list(seed), policy)

    @staticmethod
    def create(backend: Any, model: BehaviorModule, genesis: dict[str, Any]) -> Store:
        with _Translate():
            return Store(_engine.Store.create(backend, model.engine, genesis))

    @staticmethod
    def open(backend: Any) -> Store:
        with _Translate():
            return Store(_engine.Store.open(backend))

    @property
    def store_id(self) -> str:
        with _Translate():
            return self._inner.store_id()

    def current(self) -> StateRef:
        with _Translate():
            return StateRef.of(self._inner.current())

    def state_at(self, position: int) -> StateRef:
        with _Translate():
            return StateRef.of(self._inner.state_at(position))

    def load(self, entity: str, id: str, at: StateRef | None = None) -> dict[str, Any]:
        with _Translate():
            return self._inner.load(entity, id, _ref(at or self.current()))

    def evaluate(
        self,
        model: BehaviorModule,
        action: str,
        *,
        bindings: dict[str, str],
        input: dict[str, Any] | None = None,
        context: dict[str, Any] | None = None,
        commit_time: str,
        evidence: dict[str, Any] | None = None,
    ) -> Evaluation:
        """Evaluates `action` against the store's current state (one consistent snapshot);
        `bindings` names the entity id of each state parameter."""
        with _Translate():
            record, bundle = self._inner.evaluate(
                model.engine, action, bindings, input or {}, context or {}, commit_time, evidence
            )
        return Evaluation(Decision.from_record(record), bundle)

    def commit(
        self, model: BehaviorModule, bundle: dict[str, Any], expected_parent: StateRef | None = None
    ) -> CommitResult:
        """Commits `bundle` on the state it was evaluated against (or `expected_parent`); raises
        StateConflict when the store moved on, CommitRefused for any other refusal."""
        parent = _ref(expected_parent) if expected_parent else bundle["evaluated_state"]
        with _Translate():
            r = self._inner.commit(model.engine, parent, bundle)
        return CommitResult(
            r["record_id"], StateRef.of(r["result_state"]), r["already"], r["evidence_trust"]
        )

    def transitions(self, from_: StateRef, to: StateRef) -> list[dict[str, Any]]:
        with _Translate():
            return self._inner.transitions(from_.as_dict(), to.as_dict())

    def history(self) -> Iterator[dict[str, Any]]:
        """Every transition record from the genesis to the current state."""
        yield from self.transitions(self.state_at(0), self.current())


def _report(d: dict[str, Any]) -> ReplayReport:
    return ReplayReport(d["ok"], d["kind"], d["checked"], d["divergence"])


def replay_data(store: Store, from_: StateRef | None = None, to: StateRef | None = None) -> ReplayReport:
    """Re-applies every recorded change and recomputes every state identity."""
    return _report(store._inner.replay_data(
        _ref(from_ or store.state_at(0)), _ref(to or store.current())
    ))


def replay_behavior(
    store: Store,
    models: Sequence[BehaviorModule],
    from_: StateRef | None = None,
    to: StateRef | None = None,
) -> ReplayReport:
    """Re-evaluates every transition under its recorded behavior version."""
    return _report(store._inner.replay_behavior(
        [m.engine for m in models], _ref(from_ or store.state_at(0)), _ref(to or store.current())
    ))


def run_conformance(factory: Callable[[], Any]) -> ConformanceReport:
    """Runs the engine's conformance suite against backends created by `factory`."""
    return ConformanceReport(_engine.run_conformance(factory))
