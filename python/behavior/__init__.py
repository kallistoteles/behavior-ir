"""Python authoring layer for the verifiable Behavior IR (contracts/python-api.md).

The DSL only constructs behavior; the Rust engine admits, hashes, and evaluates it.
"""

from __future__ import annotations

from importlib import metadata
from typing import Any

from . import _engine
from ._versions import python_version
from .decl import action, constraint, derived, entity, field, invariant, rule
from .errors import (
    BehaviorDefinitionError, BehaviorError, BehaviorInvalid, BehaviorTypeError, CommitRefused,
    IntentRejected, StateConflict,
)
from .expr import and_, exists, none, not_, or_, referenced, rescale, underlying
from .module import BehaviorModule
from .query import Query, all_, any_, count, max_, min_, select, sum_, unique
from .results import AdmissionError, AdmissionResult, Change, Decision, ReplayResult, TraceStep
from .statements import create, ensures, remove, requires, set_
from .types import Context, Exact, Id, Input, Option, Ref, Rounding, nominal
from .governance import Authorization, authorize, authorize_migration, sign_waiver, waiver_hash
from .verify import Attestation, Profile, verify
from .migration import (
    Migration, MigrationAdmission, apply_migration, enum_map, strict_enum_map, strict_unwrap,
    verify_migration,
)
from .store import (
    CommitResult, ConformanceReport, Evaluation, InMemoryBackend, ReplayReport, SchemaRef,
    StateRef, Store, replay_behavior, replay_data, run_conformance,
)


def _python_version(cargo: str) -> str:
    """The engine's Cargo version in Python's form (see `_versions.python_version`); a version
    without a Python equivalent is returned unchanged, so it can only mismatch."""
    try:
        return python_version(cargo)
    except ValueError:
        return cargo


def _check_versions(binding: str, engine: str) -> None:
    """A binding runs only on the engine of its own release (feature 008: exact match, compared
    in Python's version form)."""
    if binding != _python_version(engine):
        raise ImportError(
            f"behavior binding {binding} requires engine {binding}, but loaded engine {engine}"
        )


__version__: str = metadata.version("behavior")
_check_versions(__version__, _engine.ENGINE_VERSION)


def versions() -> dict[str, Any]:
    """Every version of this release: the engine, the wire IR, record and store document formats
    it reads and writes, the verifier, and this binding (feature 008)."""
    info: dict[str, Any] = _engine.engine_info()
    info["binding"] = {"python": __version__}
    return info


def admit(model: BehaviorModule) -> AdmissionResult:
    """The engine's admission result for the module (parse, resolve, type-check, hash)."""
    return AdmissionResult.from_dict(model._admission)


def evaluate(
    model: BehaviorModule,
    action: str,
    *,
    state: dict[str, Any],
    input: dict[str, Any] | None = None,
    context: dict[str, Any] | None = None,
    data_version: str,
    git_revision: str | None = None,
    facts: dict[str, Any] | None = None,
) -> Decision:
    """Evaluates `action` as a transition over state, input, and context (engine-side).

    Values are passed as Python objects (`Decimal`, `Enum` members, `None`, ...); floats raise
    `TypeError` at the engine boundary. `facts` supplies the evaluation facts a lifecycle
    decision needs (feature 006): `{"existence": [...], "identities": [...], "references":
    [...]}`, and for queries (feature 007) `"queries"` (memberships) and `"fields"` (member
    values), or `"universe"` sections listing every entity of a type. A needed fact that is missing gives the result `ERROR` with `UNKNOWN_FACT`; facts
    that cannot describe one valid state give `INVALID_INPUT` with `INCONSISTENT_FACTS`.
    """
    record = model.engine.evaluate(
        action, state, input or {}, context or {}, data_version, git_revision, facts
    )
    return Decision.from_record(record)


def evaluate_intent(
    model: BehaviorModule,
    intent: dict[str, Any],
    *,
    state: dict[str, Any],
    context: dict[str, Any] | None = None,
    data_version: str,
    git_revision: str | None = None,
) -> Decision:
    """Evaluates a structured intent (capability, targets, input) with host-supplied state and
    context. Raises IntentRejected listing every problem; nothing is evaluated then."""
    from . import _engine

    try:
        record = model.engine.evaluate_intent(
            intent, state, context or {}, data_version, git_revision
        )
    except _engine.EngineIntentRejected as e:
        raise IntentRejected(e.args[0]) from None
    return Decision.from_record(record)


def replay(model: BehaviorModule, record_json: str) -> ReplayResult:
    """Re-evaluates the request stored in a decision record and compares the outcome."""
    matches, diff = model.engine.replay(record_json)
    return ReplayResult(matches, diff)


__all__ = [
    "CommitRefused", "CommitResult", "ConformanceReport", "Evaluation", "InMemoryBackend",
    "ReplayReport", "SchemaRef", "StateConflict", "StateRef", "Store", "replay_behavior",
    "replay_data", "run_conformance",
    "Exact", "Rounding", "rescale",
    "Attestation", "Authorization", "Profile", "authorize", "sign_waiver", "verify", "waiver_hash",
    "AdmissionError", "AdmissionResult", "Change", "Decision", "ReplayResult", "TraceStep",
    "evaluate", "evaluate_intent", "replay", "BehaviorDefinitionError", "BehaviorError",
    "BehaviorInvalid", "BehaviorModule", "BehaviorTypeError", "Context", "Id", "Input",
    "IntentRejected", "Option", "action", "admit", "constraint", "and_", "derived", "ensures", "entity",
    "field", "invariant", "nominal", "none", "not_", "or_", "requires", "rule", "set_",
    "underlying", "Ref", "create", "remove", "exists", "referenced",
    "Query", "select", "count", "any_", "all_", "sum_", "min_", "max_", "unique",
    "versions", "__version__",
    "Migration", "MigrationAdmission", "apply_migration", "authorize_migration", "enum_map", "strict_enum_map",
    "strict_unwrap", "verify_migration",
]
