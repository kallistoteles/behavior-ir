"""Reference host protocol, not a Core allowlist or Safety runtime registry.

Every writer and migration must use the same coordinator. Multi-process hosts need
an equivalent distributed/transactional boundary. A lock held only by activation,
or an unlocked compare followed by activation, does not meet this contract.
"""
from __future__ import annotations
from threading import RLock
from typing import Any
from behavior import SafetyCertificate, validate_safety_snapshot


class CoordinatedHost:
    """Ephemeral host authorization/configuration; all business state remains in Core.

    The host must own exclusive access to its Store and backend. The lock cannot
    constrain code holding another direct handle; production enforcement is external.
    Certificates must already have been produced by trusted explicit certification
    or fresh bundle verification. This example does not persist deployment evidence.
    """
    def __init__(self, store: Any) -> None:
        self._store = store
        self._lock = RLock()
        self._segment: tuple[Any, str, str] | None = None

    def activate(self, module: Any, certificate: Any) -> Any:
        if not isinstance(certificate, SafetyCertificate):
            raise TypeError("A checked SafetyCertificate is required, not JSON")
        proof = certificate.to_dict()
        analysis = module.safety_analysis
        if analysis is None or proof["behavior_hash"] != module.behavior_version or proof["policy_set_hash"] != analysis.policy_set_hash:
            raise ValueError("Certificate does not cover incoming module and complete policies")
        with self._lock:
            anchor = self._store.current_history()
            validation = validate_safety_snapshot(self._store, module, at=anchor)
            if not validation.matches_anchor(module, self._store.current_history()):
                raise ValueError("Incoming state is invalid, unknown, or no longer the exact anchor")
            self._segment = (module, proof["behavior_hash"], analysis.policy_set_hash)
            return validation  # Guarantee begins here, without a retroactive history claim.

    def invoke(self, action: str, *, bindings: Any = None, inputs: Any = None,
               commit_time: str = "2026-10-18T00:00:00Z") -> Any:
        with self._lock:
            if self._segment is None:
                raise ValueError("No certified deployment segment is active")
            module, behavior_hash, policy_hash = self._segment
            if module.behavior_version != behavior_hash or module.safety_analysis.policy_set_hash != policy_hash:
                raise ValueError("Policy change requires fresh activation")
            result = self._store.evaluate(module, action, bindings=bindings or {},
                                         input=inputs or {}, commit_time=commit_time)
            if result.bundle is not None:
                self._store.commit(module, result.bundle)
            return result

    def migrate(self, migration: Any, *, commit_time: str) -> Any:
        with self._lock:
            # Migration is a separate boundary. Successful Core validity does not
            # certify the incoming relational property. No writes until reactivation.
            self._segment = None
            return self._store.migrate(migration, commit_time=commit_time)


def anchor_pitfalls(before: Any, after: Any) -> dict[str, bool]:
    """A command-only event demonstrates why StateId alone is insufficient.

    Even full equality is only a necessary check: all writers must remain excluded
    until activation. An unlocked check-then-switch still races the next commit.
    """
    return {"same_state": before.state == after.state,
            "same_full_history": before == after,
            "unlocked_comparison_enforces_continuity": False}
