"""The persistence contract (feature 005): a store over a backend, evaluated against one
consistent snapshot, with guarded commits and replayable history.

The engine owns every rule; a backend only stores documents. `InMemoryBackend()` is the
reference backend; any object with the seven backend methods (`genesis`, `head`, `create`,
`version_at`, `version`, `record`, `commit`, dicts in and out) is a backend too.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass
from typing import Any, Callable, Iterator, Sequence

from . import _engine
from .errors import BehaviorError, CommitRefused, StateConflict
from .invocation import Document, Invocation, InvocationRecord, record_json
from .module import BehaviorModule
from .results import Decision, ReplayResult

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
class HistoryRef:
    """An exact committed event: store, state, position and record identity.

    Core checks anchors returned by Store and validates every supplied anchor at its boundary.
    """

    store: str
    state: str
    position: int
    record: str
    format: str = "behavior.history_ref.v1"

    def as_dict(self) -> dict[str, Any]:
        return {"format": self.format, "store": self.store, "state": self.state,
                "position": self.position, "record": self.record}

    @staticmethod
    def of(data: dict[str, Any]) -> HistoryRef:
        return HistoryRef(data["store"], data["state"], data["position"], data["record"],
                          data["format"])


@dataclass(frozen=True)
class CommandStreamPage:
    """Core's validated committed occurrences and checkpoint for a whole-event page."""

    _data: dict[str, Any]

    @property
    def data(self) -> dict[str, Any]:
        return copy.deepcopy(self._data)

    @property
    def items(self) -> list[dict[str, Any]]:
        return copy.deepcopy(self._data["items"])

    @property
    def observed_head(self) -> HistoryRef:
        return HistoryRef.of(self._data["observed_head"])

    @property
    def next_after(self) -> HistoryRef:
        return HistoryRef.of(self._data["next_after"])

    @property
    def complete(self) -> bool:
        return bool(self._data["complete"])

    def as_dict(self) -> dict[str, Any]:
        return self.data


@dataclass(frozen=True)
class SchemaRef:
    """A store schema in force from a history position on (feature 009): its SchemaHash, entity
    declarations (name → declaration hash), the position it applies from, and the record that
    introduced it (the genesis hash for the genesis schema)."""

    hash: str
    declarations: dict[str, str]
    since: int
    migration_record: str

    @staticmethod
    def of(d: dict[str, Any]) -> SchemaRef:
        return SchemaRef(d["hash"], dict(d["declarations"]), d["since"], d["migration_record"])


@dataclass(frozen=True)
class Evaluation:
    decision: Decision
    bundle: dict[str, Any] | None  # a commit bundle, only for allowed decisions


@dataclass(frozen=True)
class CommitResult:
    record_id: str
    result_state: StateRef
    already: bool  # the same transition was already committed
    evidence_trust: str | None  # historical "structural", or v2 "authenticated"


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
            details = e.args[2] if len(e.args) > 2 else None
            raise CommitRefused(e.args[0], e.args[1], details) from None


def _ref(r: StateRef | dict[str, Any]) -> dict[str, Any]:
    return r.as_dict() if isinstance(r, StateRef) else r


def governance_candidate(bundle: Document) -> dict[str, Any]:
    """Inspects Core's exact candidate document without asserting live commitment."""
    with _Translate():
        return _engine.governance_candidate(bundle)


def with_trusted_evidence(bundle: Document, evidence: Document) -> dict[str, Any]:
    """Attaches checked v2 evidence while preserving the candidate's identity.

    A fresh governed commit still needs independently supplied authorization context.
    """
    with _Translate():
        return _engine.with_trusted_evidence(bundle, evidence)


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
    def genesis_v2_for(
        model: BehaviorModule, seed: Sequence[dict[str, Any]], policy: Document
    ) -> dict[str, Any]:
        """Begins a v2 lineage with an explicit immutable trusted evidence policy."""
        with _Translate():
            return _engine.Store.genesis_v2_for(model.engine, list(seed), policy)

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

    def current_history(self) -> HistoryRef:
        """The exact committed head; command-only events can retain the same state identity."""
        with _Translate():
            return HistoryRef.of(self._inner.current_history())

    def history_at(self, position: int) -> HistoryRef:
        with _Translate():
            return HistoryRef.of(self._inner.history_at(position))

    def commands_since(self, request: Document) -> CommandStreamPage:
        """Enumerates committed commands through checked anchors, preserving whole events."""
        with _Translate():
            return CommandStreamPage(self._inner.commands_since(request))

    def export_seed_at(
        self, model: BehaviorModule, history: HistoryRef | Document
    ) -> dict[str, Any]:
        """Exports the validated live state at an exact history anchor for a new lineage."""
        anchor = history.as_dict() if isinstance(history, HistoryRef) else history
        with _Translate():
            return self._inner.export_seed_at(model.engine, anchor)

    def data_version(self, at: StateRef | None = None) -> str:
        """`store:<id>;state:<state>;position:<n>` of state `at` (default: the current state):
        what decision records and migration authorizations are bound to."""
        with _Translate():
            return str(self._inner.data_version(_ref(at or self.current())))

    def schema_at(self, at: StateRef | None = None) -> SchemaRef:
        """The schema under which state `at` (default: the current state) is valid."""
        with _Translate():
            return SchemaRef.of(self._inner.schema_at(_ref(at or self.current())))

    def schema_history(self) -> list[SchemaRef]:
        """Every schema this store has had, oldest first, each with the position it applies
        from."""
        with _Translate():
            return [SchemaRef.of(d) for d in self._inner.schema_history()]

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

    def invoke(
        self,
        model: BehaviorModule,
        document: Document,
        *,
        commit_time: str,
        evidence: Document | None = None,
        at: StateRef | None = None,
    ) -> Invocation:
        """Invokes a checked requested document against one store snapshot.

        Resolution/evaluation refusals return records; invalid document shapes or transport
        raise BehaviorError with Core's ordered diagnostics. Use invoke_intent for capability
        intents and their decode-refusal records. Only an allowed action at the captured head
        has a candidate bundle.
        Reads, refusals and past-state invocations never commit anything.
        """
        try:
            with _Translate():
                record, bundle = self._inner.invoke(
                    model.engine, document, commit_time, evidence,
                    _ref(at) if at is not None else None,
                )
        except _engine.EngineError as error:
            raise BehaviorError(str(error.args[1])) from None
        return Invocation(InvocationRecord(record), bundle)

    def invoke_intent(
        self,
        model: BehaviorModule,
        intent: Document,
        *,
        context: dict[str, Any],
        commit_time: str,
        at: StateRef | None = None,
    ) -> Invocation:
        """Invokes a capability intent with independently supplied host context."""
        with _Translate():
            record, bundle = self._inner.invoke_intent(
                model.engine, intent, context, commit_time, _ref(at) if at is not None else None,
            )
        return Invocation(InvocationRecord(record), bundle)

    def replay_invocation(
        self, model: BehaviorModule, record: InvocationRecord | str
    ) -> ReplayResult:
        """Replays Core's outcome against its original position in this store."""
        with _Translate():
            matches, diff = self._inner.replay_invocation(model.engine, record_json(record))
        return ReplayResult(matches, diff)

    def commit(
        self, model: BehaviorModule, bundle: Document, expected_parent: StateRef | None = None
    ) -> CommitResult:
        """Commits `bundle` on the state it was evaluated against (or `expected_parent`); raises
        StateConflict when the store moved on, CommitRefused for any other refusal."""
        parent = _ref(expected_parent) if expected_parent is not None else None
        with _Translate():
            r = self._inner.commit(model.engine, parent, bundle)
        return CommitResult(
            r["record_id"], StateRef.of(r["result_state"]), r["already"], r["evidence_trust"]
        )

    def commit_with_context(
        self,
        model: BehaviorModule,
        bundle: Document,
        *,
        context: Document,
        execution_policy: Document,
        expected_parent: StateRef | None = None,
    ) -> CommitResult:
        """Commits with explicit host authorization context decoded against execution_policy.

        Core decodes the bundle, chooses its evaluated parent when none is supplied, and
        compares this context with the signed context before any fresh governed write.
        Core independently enforces the actual signed policy allowed by the store genesis.
        """
        with _Translate():
            result = self._inner.commit_with_context(
                model.engine, _ref(expected_parent) if expected_parent is not None else None,
                bundle, context, execution_policy,
            )
        return CommitResult(
            result["record_id"], StateRef.of(result["result_state"]), result["already"],
            result["evidence_trust"],
        )

    def migrate(
        self, migration: Any, *, commit_time: str, evidence: dict[str, Any] | None = None
    ) -> CommitResult:
        """Applies a migration (feature 009) as one atomic transition: the store's schema must be
        the migration's source; source validity, requirements, transforms and target validity
        are checked on the complete state. Fresh required-governance writes need explicit
        trusted governance; legacy authorization evidence cannot supply the independent v2
        context. Core returns its upgrade or context refusal before mutation. Other migration
        refusals raise CommitRefused; concurrent state changes raise StateConflict."""
        with _Translate():
            r = self._inner.migrate(
                migration.engine, migration.source.engine, migration.target.engine, commit_time,
                evidence,
            )
        return CommitResult(
            r["record_id"], StateRef.of(r["result_state"]), r["already"], r["evidence_trust"]
        )

    def read(
        self,
        model: BehaviorModule,
        r: Any,
        *,
        bindings: dict[str, str] | None = None,
        input: dict[str, Any] | None = None,  # noqa: A002
        context: dict[str, Any] | None = None,
        at: StateRef | None = None,
    ) -> Any:
        """Reads (feature 010) at `at` or the current state, read once; never writes. `r` is a
        declared read (name or function) or an ad-hoc `@read` function; `bindings` names the
        entity id of each bound entity. An id that does not exist there gives a result of
        `INVALID_BINDING`; a schema mismatch raises CommitRefused."""
        from .reads import _result, read_source

        source = read_source(model, r)
        with _Translate():
            x = self._inner.read(
                model.engine, source, bindings or {}, input or {}, context or {},
                _ref(at) if at is not None else None,
            )
        return _result(x)

    def read_intent(
        self,
        model: BehaviorModule,
        intent: dict[str, Any],
        *,
        context: dict[str, Any] | None = None,
        at: StateRef | None = None,
    ) -> Any:
        """A read intent (feature 010) at `at` or the current state: the capability boundary
        for untrusted callers. Raises IntentRejected listing every problem (including targets
        that do not exist there); forward only the execution's `response`."""
        from .errors import IntentRejected
        from .reads import _execution

        try:
            with _Translate():
                x = self._inner.read_intent(
                    model.engine, intent, context or {}, _ref(at) if at is not None else None
                )
        except _engine.EngineIntentRejected as e:
            raise IntentRejected(e.args[0]) from None
        return _execution(x)

    def replay_read(self, model: BehaviorModule, record: Any) -> ReplayResult:
        """Replays a read record against this store (feature 010): its state must be a state of
        this store; the read is evaluated again there and compared byte for byte."""
        from .reads import record_json

        with _Translate():
            matches, diff = self._inner.replay_read(model.engine, record_json(record))
        return ReplayResult(matches, diff)

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
    *,
    migrations: Sequence[Any] = (),
) -> ReplayReport:
    """Re-evaluates every transition under its recorded behavior version, and re-runs every
    migration (feature 009) with the given `Migration` objects."""
    return _report(store._inner.replay_behavior(
        [m.engine for m in models], _ref(from_ or store.state_at(0)), _ref(to or store.current()),
        [(m.engine, m.source.engine, m.target.engine) for m in migrations],
    ))


def run_conformance(factory: Callable[[], Any]) -> ConformanceReport:
    """Runs the engine's conformance suite against backends created by `factory`."""
    return ConformanceReport(_engine.run_conformance(factory))
