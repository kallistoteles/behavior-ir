"""Typed results built from the engine's native results (contracts/engine-api.md)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class AdmissionError:
    code: str
    message: str
    loc: dict[str, Any] | None = None
    related_locs: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True)
class AdmissionResult:
    ok: bool
    behavior_version: str | None
    errors: list[AdmissionError]
    evaluation_order: list[str]
    items: dict[str, str]

    @staticmethod
    def from_dict(v: dict[str, Any]) -> AdmissionResult:
        return AdmissionResult(
            ok=v["ok"],
            behavior_version=v.get("behavior_version"),
            errors=[
                AdmissionError(e["code"], e["message"], e.get("loc"), e.get("related_locs", []))
                for e in v["errors"]
            ],
            evaluation_order=v["evaluation_order"],
            items=v["items"],
        )


@dataclass(frozen=True)
class TraceStep:
    phase: str
    expr_text: str
    hash: str
    reads: dict[str, Any]
    outcome: Any
    loc: dict[str, Any]
    name: str | None = None


@dataclass(frozen=True)
class Change:
    param: str
    field: str
    old: Any
    new: Any


@dataclass(frozen=True)
class Decision:
    result: str
    changes: list[Change]
    reasons: list[dict[str, Any]]
    trace: list[TraceStep]
    derived: list[dict[str, Any]]
    behavior_version: str
    record_json: str  # the canonical decision record, the audit artifact
    #: Creations and removals of an allowed decision (feature 006), in effect order.
    lifecycle: list[dict[str, Any]] = field(default_factory=list)
    #: The evaluation facts the decision observed (feature 006; query and field facts, feature
    #: 007), or an empty dict.
    facts: dict[str, Any] = field(default_factory=dict)

    @staticmethod
    def from_record(record: Any) -> Decision:
        v = record.data
        return Decision(
            result=v["result"],
            changes=[Change(c["param"], c["field"], c["old"], c["new"]) for c in v["changes"]],
            reasons=v["reasons"],
            trace=[
                TraceStep(s["phase"], s["expr_text"], s["hash"], s["reads"], s["outcome"],
                          s["loc"], s.get("name"))
                for s in v["trace"]
            ],
            derived=v["derived"],
            behavior_version=v["behavior_version"],
            record_json=record.json,
            lifecycle=v.get("lifecycle", []),
            facts=v.get("facts", {}),
        )


@dataclass(frozen=True)
class ReplayResult:
    matches: bool
    diff: str | None = None
