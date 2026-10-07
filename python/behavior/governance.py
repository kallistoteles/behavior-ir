"""Waivers, signed attestations, and commit authorization
(specs/002-smt-verification/contracts/governance.md).

A waiver never changes a verification result; the execution policy decides whether a proposed
transition may be committed, and a waiver counts only with a valid signature by a key the policy
trusts. All decisions are made by the engine.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from . import _engine
from .errors import BehaviorError
from .module import BehaviorModule
from .results import Decision
from .verify import Attestation


@dataclass(frozen=True)
class Authorization:
    """A commit authorization; `json` is the canonical artifact."""

    decision: str
    data: dict[str, Any]
    json: str

    @property
    def allowed(self) -> bool:
        return self.decision == "allow"

    @property
    def reasons(self) -> list[dict[str, Any]]:
        return list(self.data["reasons"])

    @property
    def waivers_used(self) -> list[dict[str, Any]]:
        return list(self.data["waivers_used"])


def _invalid(e: Exception) -> BehaviorError:
    return BehaviorError(str(e.args[1]))


def waiver_hash(waiver: dict[str, Any]) -> str:
    """The content hash a waiver's signatures are made over."""
    try:
        return _engine.waiver_hash(waiver)
    except _engine.EngineError as e:
        raise _invalid(e) from None


def sign_waiver(waiver: dict[str, Any], *, seed: str) -> dict[str, Any]:
    """A detached Ed25519 signed attestation over the waiver's hash (seed: 32 bytes as hex)."""
    try:
        return _engine.sign_waiver(waiver, seed)
    except _engine.EngineError as e:
        raise _invalid(e) from None


def authorize(
    model: BehaviorModule,
    decision: Decision,
    *,
    policy: dict[str, Any],
    attestation: Attestation | None = None,
    waivers: list[dict[str, Any]] | None = None,
    signatures: list[dict[str, Any]] | None = None,
    now: str,
) -> Authorization:
    """Decides whether `decision`'s transition may be committed under `policy` at `now`
    (RFC 3339 UTC, supplied by the caller)."""
    try:
        a = model.engine.authorize(
            policy,
            decision.record_json,
            attestation.json if attestation else None,
            waivers or [],
            signatures or [],
            now,
        )
    except _engine.EngineError as e:
        raise _invalid(e) from None
    return Authorization(a.decision, a.data, a.json)


def authorize_migration(
    migration: Any,
    store: Any,
    *,
    policy: dict[str, Any],
    attestation: Attestation | None = None,
    waivers: list[dict[str, Any]] | None = None,
    signatures: list[dict[str, Any]] | None = None,
    now: str,
) -> Authorization:
    """Decides whether `migration` may be applied to `store` in its current state under the
    execution `policy` at `now` (feature 009). This legacy structural authorization binds
    the migration and store state. Fresh required-governance writes now require trusted v2
    governance; this result cannot supply its independent authorization context. Core's
    public Rust prepared-migration APIs provide the trusted migration commit path."""
    try:
        a = migration.engine.authorize(
            policy,
            store.data_version(),
            attestation.json if attestation else None,
            waivers or [],
            signatures or [],
            now,
        )
    except _engine.EngineError as e:
        raise _invalid(e) from None
    return Authorization(a.decision, a.data, a.json)
