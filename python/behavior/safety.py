"""Safety authoring records. Rust owns resolution, permissions, identity and proofs.

Lowering produces an uncertified analysis. Explicit certification uses the locked
Lean toolchain at build time; ordinary Core execution does not invoke Lean.
"""
from __future__ import annotations

from dataclasses import dataclass
from os import PathLike
from typing import Any, Mapping, Sequence

from . import _engine
from .decl import ActionFn, entity_decl
from .errors import ModelBuildError

SafetyCertificate = _engine.SafetyCertificate


@dataclass(frozen=True)
class GeneratedAction:
    model_id: str
    local_action_id: str


def _selector(selector: Any) -> Any:
    if isinstance(selector, GeneratedAction):
        return {"model_id": selector.model_id, "local_action_id": selector.local_action_id}
    if isinstance(selector, ActionFn):
        return selector.name
    return selector


def _selectors(selectors: Any) -> Any:
    # Record supported containers only. Preserve malformed JSON-shaped values so
    # Rust rejects them; dict keys and string characters must not become grants.
    if isinstance(selectors, (list, tuple, set, frozenset)):
        return [_selector(s) for s in selectors]
    return selectors


class FieldOwnership:
    """Structural write, create and remove permission over an existing entity.

    Each listed field is protected; an empty list forbids its assignments.
    Omitted creator/remover lists are empty. Creation initialization uses creator
    permission. Overlapping policies intersect. No guard or action is generated.
    """
    def __init__(self, *, id: str, entity: Any,
                 writers: Mapping[str, Sequence[Any]] | None = None,
                 creators: Sequence[Any] = (), removers: Sequence[Any] = ()) -> None:
        self.id, self.entity = id, entity
        self.writers = {} if writers is None else writers
        self.creators, self.removers = creators, removers

    def engine_input(self) -> dict[str, Any]:
        declaration = entity_decl(self.entity) if isinstance(self.entity, type) else None
        writers = ({field: _selectors(selectors) for field, selectors in self.writers.items()}
                   if isinstance(self.writers, Mapping) else self.writers)
        return {"id": self.id, "entity": declaration.name if declaration else self.entity,
                "writers": writers,
                "creators": _selectors(self.creators), "removers": _selectors(self.removers)}



class LifecycleDependency:
    """Record the restricted immutable Ref parent/child safety pattern.

    Validates existing Core actions. It creates no runtime data or new guards.
    Noncanonical action shapes remain executable but cannot certify in v1.
    """
    def __init__(self, *, id: str, parent: Any, child: Any, owner_field: str,
                 parent_state: str, child_state: str, prohibited: Sequence[Any],
                 terminal: Sequence[Any], ownership: Sequence[str]) -> None:
        self.id, self.parent, self.child = id, parent, child
        self.owner_field, self.parent_state, self.child_state = owner_field, parent_state, child_state
        self.prohibited, self.terminal, self.ownership = prohibited, terminal, ownership

    def engine_input(self) -> dict[str, Any]:
        from enum import Enum
        def name(entity: Any) -> Any:
            declaration = entity_decl(entity) if isinstance(entity, type) else None
            return declaration.name if declaration else entity
        def members(values: Any) -> Any:
            return [v.value if isinstance(v, Enum) else v for v in values] if isinstance(values, (list, tuple, set, frozenset)) else values
        return {"id": self.id, "parent": name(self.parent), "child": name(self.child),
                "owner_field": self.owner_field, "parent_state": self.parent_state,
                "child_state": self.child_state, "prohibited": members(self.prohibited),
                "terminal": members(self.terminal), "ownership": _selectors(self.ownership)}

def _analyze(module: Any, policies: Sequence[Any]) -> Any:
    analysis, errors = _engine.analyze_safety(module.engine, [p.engine_input() for p in policies])
    if errors:
        raise ModelBuildError(list(errors))
    return analysis


def certify_safety(module: Any, *, output: str | PathLike[str],
                   toolchain: str | None = None, limits: Mapping[str, int] | None = None) -> Any:
    """Check generic/concrete proofs, axiom audit and fresh replay for this module.

    A certificate is conditional on its explicit assumptions. It does not validate
    genesis, authorize a deployment, preserve migrations or guarantee host execution.
    Missing tools and noncertifying assessments never produce a certificate.
    """
    analysis = module.safety_analysis
    if analysis is None:
        analysis = _analyze(module, ())
    return _engine.certify_safety(module.engine, analysis, str(output), toolchain, dict(limits) if limits is not None else None)


def load_safety_bundle(bundle: Any) -> Any:
    """Load fixed-schema unverified data. Loading never establishes proof validity."""
    if isinstance(bundle, _engine.UnverifiedSafetyBundle):
        return bundle
    if isinstance(bundle, Mapping):
        return _engine.import_safety_bundle(bundle)
    return _engine.load_safety_bundle(str(bundle))


def verify_safety_evidence(module: Any, bundle: Any, *, output: str | PathLike[str],
                           toolchain: str | None = None,
                           limits: Mapping[str, int] | None = None) -> Any:
    """Re-admit, re-extract and freshly check a bundle against expected module policies.

    The supplied module records the expected complete Safety Policy set. Imported
    flags, supplied proof code and cached compiled objects confer no authority.
    This operation does not authorize a Store deployment or establish genesis validity.
    """
    analysis = module.safety_analysis
    if analysis is None:
        analysis = _analyze(module, ())
    return _engine.verify_safety_evidence(module.engine, analysis, load_safety_bundle(bundle),
        str(output), toolchain, dict(limits) if limits is not None else None)


class SafetyStateValidation:
    """Readonly exact-history record; separate from a transition certificate.

    matches_anchor is a boundary check, not a lock. The host must prevent commits
    between validation and activation and authorize only the segment's module.
    """
    __slots__ = ("_inner",)
    _inner: _engine.SafetyStateValidation

    def __init__(self, inner: Any) -> None:
        if not isinstance(inner, _engine.SafetyStateValidation):
            raise TypeError("Expected an acquired Core Store validation record")
        object.__setattr__(self, "_inner", inner)

    def __setattr__(self, name: str, value: Any) -> None:
        raise AttributeError("SafetyStateValidation is readonly")

    @property
    def status(self) -> str:
        return str(self._inner.status)

    def to_dict(self) -> dict[str, Any]:
        return dict(self._inner.to_dict())

    def matches_anchor(self, module: Any, history: Any) -> bool:
        analysis = module.safety_analysis
        if analysis is None:
            analysis = _analyze(module, ())
        anchor = history.as_dict() if hasattr(history, "as_dict") else history
        return bool(self._inner.matches_anchor(module.engine, analysis, anchor))


def validate_safety_snapshot(store: Any, module: Any, *, at: Any = None) -> SafetyStateValidation:
    """Acquire Core's complete export and validate R/P at one exact history.

    No Lean call, implicit activation, relationship registry or persistent Safety
    state is involved. Backend completeness and Rust correspondence remain trusted.
    Detached JSON cannot establish Store provenance.
    """
    from .store import Store
    if not isinstance(store, Store):
        raise TypeError("Expected a Core Store, not detached snapshot data")
    analysis = module.safety_analysis
    if analysis is None:
        analysis = _analyze(module, ())
    anchor = at.as_dict() if hasattr(at, "as_dict") else at
    return SafetyStateValidation(_engine.validate_safety_snapshot(
        store._inner, module.engine, analysis, anchor))
