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
    """Asks the engine to admit the module: parse, resolve, type-check, and hash."""
    from . import _engine

    return AdmissionResult.from_json(_engine.admit(model.to_wire_json()))


def _admitted_wire(model: BehaviorModule) -> str:
    result = admit(model)
    if not result.ok:
        raise BehaviorInvalid(result)
    return model.to_wire_json()


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
    from . import _engine

    wire = _admitted_wire(model)
    request: dict[str, Any] = {
        "action": action,
        "data_version": data_version,
        "state": encode_request_values(state),
        "input": encode_request_values(input or {}),
        "context": encode_request_values(context or {}),
    }
    if git_revision is not None:
        request["git_revision"] = git_revision
    return Decision.from_json(_engine.evaluate(wire, json.dumps(request)))


def replay(model: BehaviorModule, record_json: str) -> ReplayResult:
    """Re-evaluates the request stored in a decision record and compares the outcome."""
    from . import _engine

    return ReplayResult.from_json(_engine.replay(_admitted_wire(model), record_json))


__all__ = [
    "AdmissionError", "AdmissionResult", "Change", "Decision", "ReplayResult", "TraceStep",
    "evaluate", "replay", "BehaviorDefinitionError", "BehaviorError",
    "BehaviorInvalid", "BehaviorModule", "BehaviorTypeError", "Context", "Id", "Input",
    "IntentRejected", "Option", "action", "admit", "and_", "derived", "ensures", "entity",
    "field", "invariant", "nominal", "none", "not_", "or_", "requires", "rule", "set_",
    "underlying",
]
