"""Python authoring layer for the verifiable Behavior IR (contracts/python-api.md).

The DSL only constructs behavior; the Rust engine admits, hashes, and evaluates it.
"""

from __future__ import annotations

import json
from typing import Any

from .decl import action, derived, entity, field, invariant, rule
from .errors import (
    BehaviorDefinitionError, BehaviorError, BehaviorInvalid, BehaviorTypeError, IntentRejected,
)
from .expr import and_, none, not_, or_, underlying
from .module import BehaviorModule
from .results import AdmissionError, AdmissionResult, Change, Decision, ReplayResult, TraceStep
from .values import encode_request_values
from .statements import ensures, requires, set_
from .types import Context, Id, Input, Option, nominal


def admit(model: BehaviorModule) -> AdmissionResult:
    """The engine's admission result for the module (parse, resolve, type-check, hash)."""
    return AdmissionResult.from_json(model._admission_json)


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
    """Evaluates `action` as a transition over state, input, and context (engine-side)."""
    request: dict[str, Any] = {
        "action": action,
        "data_version": data_version,
        "state": encode_request_values(state),
        "input": encode_request_values(input or {}),
        "context": encode_request_values(context or {}),
    }
    if git_revision is not None:
        request["git_revision"] = git_revision
    return Decision.from_json(model.engine.evaluate(json.dumps(request)))


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
    host: dict[str, Any] = {
        "data_version": data_version,
        "state": encode_request_values(state),
        "context": encode_request_values(context or {}),
    }
    if git_revision is not None:
        host["git_revision"] = git_revision
    text = model.engine.evaluate_intent(json.dumps(intent), json.dumps(host))
    out = json.loads(text)
    if out.get("rejected"):
        raise IntentRejected(out["errors"])
    return Decision.from_json(text)


def replay(model: BehaviorModule, record_json: str) -> ReplayResult:
    """Re-evaluates the request stored in a decision record and compares the outcome."""
    return ReplayResult.from_json(model.engine.replay(record_json))


__all__ = [
    "AdmissionError", "AdmissionResult", "Change", "Decision", "ReplayResult", "TraceStep",
    "evaluate", "evaluate_intent", "replay", "BehaviorDefinitionError", "BehaviorError",
    "BehaviorInvalid", "BehaviorModule", "BehaviorTypeError", "Context", "Id", "Input",
    "IntentRejected", "Option", "action", "admit", "and_", "derived", "ensures", "entity",
    "field", "invariant", "nominal", "none", "not_", "or_", "requires", "rule", "set_",
    "underlying",
]
