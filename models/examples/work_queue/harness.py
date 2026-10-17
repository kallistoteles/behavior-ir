"""Shared helpers of the feature-507 suite: the store, the reference history and its execution.

The history and the expected outcomes are stated by hand (spec, Reference history); nothing here
derives an expectation from an implementation's output.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from behavior import BehaviorModule, InMemoryBackend, StateConflict, Store

from models.examples.work_queue import workflow

ROOT = Path(__file__).resolve().parents[3]
EXPECTED: dict[str, Any] = json.loads((Path(__file__).parent / "expected.json").read_text())
Q = "document_work"
W = "document_work"  # the subject parameter of every transition: snake_case(DocumentWork)


def core_fixtures() -> Path:
    version = json.loads((ROOT / "core-release.json").read_text())["version"]
    return ROOT / ".core" / version / "tests" / "fixtures"


def genesis(model: BehaviorModule, seed: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    """A v2 genesis (the module declares commands) with Core's evidence policy `none`."""
    policy = (core_fixtures() / "governance-v2" / "evidence-policy-none.json").read_text()
    return Store.genesis_v2_for(model, workflow.SEED if seed is None else seed, policy)


def fresh_store(model: BehaviorModule, seed: list[dict[str, Any]] | None = None) -> Store:
    return Store.create(InMemoryBackend(), model, genesis(model, seed))


def action(capability: str) -> str:
    return f"{Q}__{capability}"


@dataclass(frozen=True)
class Op:
    """One operation of the reference history: evaluate `action` and commit it when allowed."""

    step: str
    action: str
    bindings: dict[str, str] = field(default_factory=dict)
    input: dict[str, Any] = field(default_factory=dict)
    context: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class StepResult:
    step: str
    result: str
    conflict: str | None
    position: int
    #: The reference item `w1` after the step, when it exists.
    item: dict[str, Any] | None = None


def evaluate(store: Store, model: BehaviorModule, op: Op) -> Any:
    return store.evaluate(model, op.action, bindings=op.bindings, input=op.input,
                          context=op.context, commit_time=workflow.COMMIT_TIME)


def commit(store: Store, model: BehaviorModule, ev: Any) -> str | None:
    """Commits an allowed evaluation; the refusal code if the Store refuses it."""
    try:
        store.commit(model, ev.bundle)
    except StateConflict as e:
        return "STATE_CONFLICT" if "STATE_CONFLICT" in str(e) else str(e)
    return None


def do(store: Store, model: BehaviorModule, op: Op) -> StepResult:
    ev = evaluate(store, model, op)
    conflict = commit(store, model, ev) if ev.decision.result == "ALLOW" else None
    return StepResult(op.step, str(ev.decision.result), conflict, store.current().position)


def claim(step: str, item: str, worker: str, generation: int, now: int, until: int) -> Op:
    return Op(step, action("claim"), {W: item}, {"expected_generation": generation},
              {"worker": worker, "now": now, "lease_until": until})


def complete(step: str, item: str, worker: str, generation: int, now: int,
             doc: str = "d1") -> Op:
    return Op(step, action("complete"), {W: item, "doc": doc}, {"generation": generation},
              {"worker": worker, "now": now})


def enqueue(step: str, item: str, document: str, request: str, available_at: int,
            actor: str = "admin") -> Op:
    return Op(step, action("enqueue"), {},
              {"id": item, "document": document, "request": request,
               "available_at": available_at}, {"actor": actor})


#: The reference history (spec, Reference history), preceded by the RequestFlow steps that ask
#: for the work (US4 AS6: an emitted command, then a separate enqueue by an adapter).
HISTORY: list[Op] = [
    Op("0a", "document_request__open", {}, {"request_id": "r1", "document": "d1"},
       {"actor": "u1"}),
    Op("0b", "document_request__submit", {"document_request": "r1"}, {}, {"actor": "u1"}),
    enqueue("1", "w1", "d1", "r1", available_at=10),
    claim("3", "w1", "A", 0, now=10, until=20),
    Op("4", action("retry"), {W: "w1"}, {"generation": 1},
       {"worker": "A", "now": 12, "available_at": 15}),
    claim("5", "w1", "B", 1, now=15, until=25),
    Op("6", action("recover"), {W: "w1"}, {}, {"now": 30, "available_at": 30, "actor": "admin"}),
    claim("7", "w1", "A", 2, now=31, until=40),
    complete("8", "w1", "B", 2, now=32),
    complete("9", "w1", "A", 3, now=33),
]


def run_history(model: BehaviorModule, store: Store | None = None,
                stop_before: str | None = None, on_commit: Any = None) -> tuple[Store, list[StepResult]]:
    store = store if store is not None else fresh_store(model)
    results = []
    for op in HISTORY:
        if op.step == stop_before:
            break
        before = store.current().position
        r = do(store, model, op)
        try:
            r = StepResult(r.step, r.result, r.conflict, r.position, item(store))
        except Exception:  # noqa: BLE001 - w1 does not exist yet
            pass
        results.append(r)
        if on_commit is not None and r.position != before:
            on_commit(store)
    return store, results


def item(store: Store, item_id: str = "w1", at: Any = None) -> dict[str, Any]:
    value: dict[str, Any] = (store.load("DocumentWork", item_id, at) if at is not None
                             else store.load("DocumentWork", item_id))["value"]
    return value


def ready(store: Store, model: BehaviorModule, now: int) -> list[str]:
    return [r["id"] for r in store.read(model, f"{Q}__ready", context={"now": now}).value]


def expired(store: Store, model: BehaviorModule, now: int) -> list[str]:
    return [r["id"] for r in store.read(model, f"{Q}__expired", context={"now": now}).value]


_COMMAND_NAMES: dict[str, dict[str, str]] = {}


def command_names(model: BehaviorModule) -> dict[str, str]:
    key = model.behavior_version
    if key not in _COMMAND_NAMES:
        items: dict[str, str] = model.engine.admission()["items"]
        _COMMAND_NAMES[key] = {h: k.split(":", 1)[1] for k, h in items.items()
                               if k.startswith("command:")}
    return _COMMAND_NAMES[key]


def commands(store: Store, model: BehaviorModule) -> list[dict[str, Any]]:
    """The committed command stream, as {command, <payload fields>} in commit order."""
    names = command_names(model)
    request = {"format": "behavior.command_stream_request.v1",
               "after": store.history_at(0).as_dict(), "max_records": 256}
    return [{"command": names[i["intent"]["declaration"]], **i["intent"]["payload"]}
            for i in store.commands_since(request).items]


def valid_work(value: dict[str, Any], m: int = workflow.M) -> bool:
    """`ValidWork` stated independently of the lowering (spec Context), for the property checks
    after every commit."""
    a, s = value["attempts"], value["status"]
    leased = s == "LEASED"
    return (0 <= a <= m
            and leased == (value["owner"] is not None and value["lease_until"] is not None)
            and (s != "READY" or a < m)
            and (not leased or a >= 1))


def all_items(store: Store, model: BehaviorModule) -> list[dict[str, Any]]:
    """Every work item, read by id through the expired/ready-independent snapshot."""
    ids = sorted({k["id"] for k in store.read(model, f"{Q}__ready", context={"now": 10**9}).value}
                 | {k["id"] for k in store.read(model, f"{Q}__expired",
                                                context={"now": 10**9}).value})
    return [item(store, i) for i in ids]


def work(item_id: str, status: str, attempts: int, owner: str | None = None,
         lease_until: int | None = None, available_at: int = 0, document: str = "d1",
         request: str = "r1") -> dict[str, Any]:
    """A work item for a genesis seed: trusted imported state, valid under `ValidWork` when the
    arguments are (spec, Genesis and provenance)."""
    return {"entity": "DocumentWork", "value": {
        "id": item_id, "status": status, "attempts": attempts, "owner": owner,
        "lease_until": lease_until, "available_at": available_at, "document": document,
        "request": request}}


def seeded(model: BehaviorModule, *items: dict[str, Any]) -> Store:
    """A store whose genesis holds the reference seed plus `items`."""
    return fresh_store(model, workflow.SEED + list(items))


def run(store: Store, model: BehaviorModule, *ops: Op) -> list[StepResult]:
    return [do(store, model, op) for op in ops]


def wire(model: BehaviorModule) -> dict[str, Any]:
    return json.loads(model.to_wire_json())  # type: ignore[no-any-return]


def wire_action(model: BehaviorModule, capability: str) -> dict[str, Any]:
    return next(a for a in wire(model)["actions"] if a["name"] == action(capability))


def has_compare(expr: Any, op: str, field_: str, param: str) -> bool:
    """Whether `expr` contains `op(field(subject, field_), param(param))`."""
    if isinstance(expr, dict):
        args = expr.get("args") or []
        if expr.get("op") == op and len(args) == 2 \
                and args[0].get("op") == "field" and args[0].get("field") == field_ \
                and args[1].get("op") == "param" and args[1].get("param") == param:
            return True
        return any(has_compare(v, op, field_, param) for v in expr.values())
    if isinstance(expr, list):
        return any(has_compare(v, op, field_, param) for v in expr)
    return False


def invocation_document(model: BehaviorModule, op: Op) -> str:
    """The requested-invocation document of `op` (`behavior.invocation.v1`)."""
    action_ = next(a for a in wire(model)["actions"] if a["name"] == op.action)
    entities = {p["name"]: p["type"]["name"] for p in action_["params"]
                if p.get("role") == "state"}
    doc = {"format": "behavior.invocation.v1", "capability": op.action,
           "bindings": {n: {"entity": entities[n], "id": i} for n, i in op.bindings.items()},
           "input": op.input, "context": op.context}
    return json.dumps(doc, sort_keys=True)


def invocation_snapshot(store: Store, model: BehaviorModule, position: int | None = None) -> str:
    """The complete parent snapshot at `position` (default: the head), from the Store's own
    export (every declared entity type in the universe; nothing inferred)."""
    at = store.current().position if position is None else position
    seed = store.export_seed_at(model, store.history_at(at))["seed"]
    types = [e["name"] for e in wire(model)["entities"]]
    universe = [{"entity": t, "members": [e["value"] for e in seed if e["entity"] == t]}
                for t in types]
    return json.dumps({"format": "behavior.snapshot.v1", "data_version": store.data_version(),
                       "entities": seed, "facts": {"universe": universe}}, sort_keys=True)
