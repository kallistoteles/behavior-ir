"""SMT verification of a behavior module (specs/002-smt-verification/contracts/engine-api.md).

The engine translates the admitted module to SMT, runs the pinned solver, and confirms every
counterexample by evaluation; Python only selects the profile and reads the attestation.
"""

from __future__ import annotations

import warnings
from dataclasses import dataclass
from typing import Any

from . import _engine
from .errors import BehaviorError
from .module import BehaviorModule


@dataclass(frozen=True)
class Profile:
    """Which checks run (default: all) and the deterministic solver budget."""

    checks: list[str] | None = None
    rlimit: int = 20_000_000
    wall_clock_guard_ms: int = 60_000


@dataclass(frozen=True)
class Attestation:
    """The verification attestation; `json` is the canonical artifact."""

    result: str
    hash: str
    data: dict[str, Any]
    json: str

    @property
    def verified(self) -> bool:
        return self.result == "verified"

    @property
    def checks(self) -> list[dict[str, Any]]:
        return list(self.data["checks"])

    @property
    def findings(self) -> list[dict[str, Any]]:
        return list(self.data["findings"])


def verify(
    model: BehaviorModule, profile: Profile | None = None, cache: str | None = None
) -> Attestation:
    """Verifies `model`; raises BehaviorError if the solver is unavailable."""
    p = profile or Profile()
    notice = _engine.solver_notice()
    if notice is not None:
        warnings.warn(notice, stacklevel=2)
    try:
        a = model.engine.verify(p.checks, p.rlimit, p.wall_clock_guard_ms, cache)
    except _engine.EngineError as e:
        raise BehaviorError(str(e.args[1])) from None
    return Attestation(a.result, a.hash, a.data, a.json)
