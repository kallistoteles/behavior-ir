"""Python authoring layer for the verifiable Behavior IR (contracts/python-api.md).

The DSL only constructs behavior; the Rust engine admits, hashes, and evaluates it.
"""

from __future__ import annotations

from typing import Any

from .decl import action, constraint, derived, entity, field, invariant, rule
from .errors import (
    BehaviorDefinitionError, BehaviorError, BehaviorInvalid, BehaviorTypeError, IntentRejected,
)
from .expr import and_, none, not_, or_, underlying
from .module import BehaviorModule
from .results import AdmissionError, AdmissionResult, Change, Decision, ReplayResult, TraceStep
from .statements import ensures, requires, set_
from .types import Context, Id, Input, Option, nominal
from .governance import Authorization, authorize, sign_waiver, waiver_hash
from .verify import Attestation, Profile, verify


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
) -> Decision:
    """Evaluates `action` as a transition over state, input, and context (engine-side).

    Values are passed as Python objects (`Decimal`, `Enum` members, `None`, ...); floats raise
    `TypeError` at the engine boundary.
    """
    record = model.engine.evaluate(
        action, state, input or {}, context or {}, data_version, git_revision
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
    "Attestation", "Authorization", "Profile", "authorize", "sign_waiver", "verify", "waiver_hash",
    "AdmissionError", "AdmissionResult", "Change", "Decision", "ReplayResult", "TraceStep",
    "evaluate", "evaluate_intent", "replay", "BehaviorDefinitionError", "BehaviorError",
    "BehaviorInvalid", "BehaviorModule", "BehaviorTypeError", "Context", "Id", "Input",
    "IntentRejected", "Option", "action", "admit", "constraint", "and_", "derived", "ensures", "entity",
    "field", "invariant", "nominal", "none", "not_", "or_", "requires", "rule", "set_",
    "underlying",
]
