"""Driving the reference workflow through the public Store (feature 506).

`HISTORY` is reference history H of contracts/reference-workflow.md §1 as data. `run_history`
walks it, and after every committed step checks the invariant oracles. The oracles are the
spec's definitions (VO-003, VO-004) applied to rows read from Core; they decide nothing that
Core decides. Expectations live in expected.json, never here.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from behavior import BehaviorModule, InMemoryBackend, Store, invoke
from behavior.errors import StateConflict

from . import workflow

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
EXPECTED: dict[str, Any] = json.loads((HERE / "expected.json").read_text())
K_A, K_R = 2, 1


def pinned_cli() -> Path:
    """The CLI of the pinned Core Release, bundled by scripts/fetch-core.sh, which checks its
    digest. `BEHAVIOR_CLI_BINARY` is a development override and is never used here."""
    return ROOT / "python" / "behavior" / "_bin" / "behavior"


def pinned_engine_info() -> dict[str, Any]:
    """`engine-info` of the pinned CLI; fails unless it reports the pinned Core release."""
    import subprocess

    out = subprocess.run([str(pinned_cli()), "engine-info"], capture_output=True, text=True,
                         check=True).stdout
    info: dict[str, Any] = json.loads(out)
    version = json.loads((ROOT / "core-release.json").read_text())["version"]
    if info.get("engine") != version:
        raise AssertionError(f"the bundled CLI reports engine {info.get('engine')}, "
                             f"not the pinned Core release {version}")
    return info


def core_fixtures() -> Path:
    version = json.loads((ROOT / "core-release.json").read_text())["version"]
    return ROOT / ".core" / version / "tests" / "fixtures"


def genesis(model: BehaviorModule, seed: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    """A v2 genesis (the module declares a command) with Core's evidence policy `none`."""
    policy = (core_fixtures() / "governance-v2" / "evidence-policy-none.json").read_text()
    return Store.genesis_v2_for(model, workflow.SEED if seed is None else seed, policy)


def fresh_store(model: BehaviorModule) -> Store:
    return Store.create(InMemoryBackend(), model, genesis(model))


@dataclass(frozen=True)
class Op:
    """One operation of H. `kind`: `do` (evaluate, commit if allowed), `eval` (evaluate and keep
    the candidate under `key`), `commit` (commit the kept candidate `key`)."""

    step: str
    kind: str
    capability: str = ""
    bindings: dict[str, str] = field(default_factory=dict)
    input: dict[str, Any] = field(default_factory=dict)
    context: dict[str, Any] = field(default_factory=dict)
    key: str = ""


def _create(step: str, rid: str, title: str, amount: int) -> Op:
    return Op(step, "do", "purchase_request__create", {},
              {"request_id": rid, "title": title, "amount": amount}, {"actor": "alice"})


def _decide(step: str, kind: str, choice: str, rid: str, who: str, xid: str, key: str = "") -> Op:
    return Op(step, kind, f"purchase_request_approval__{choice}", {"subject": rid},
              {"decision_id": xid}, {"actor": who}, key)


def _flow(step: str, name: str, rid: str, input: dict[str, Any] | None = None,
          context: dict[str, Any] | None = None) -> Op:
    return Op(step, "do", f"purchase_request__{name}", {"purchase_request": rid}, input or {},
              context or {})


#: Reference history H (contracts/reference-workflow.md §1).
HISTORY: list[Op] = [
    _create("1", "r1", "Laptop", 900),
    _flow("2", "submit", "r1", context={"actor": "alice"}),
    _flow("3", "start_review", "r1", input={"reviewer": "dave"}),
    _decide("4", "do", "approve", "r1", "bob", "x1"),
    _decide("5", "do", "approve", "r1", "bob", "x2"),
    _decide("6", "do", "approve", "r1", "alice", "x3"),
    _decide("7", "eval", "approve", "r1", "carol", "x4", key="x4"),
    _decide("7", "eval", "approve", "r1", "dave", "x5", key="x5"),
    Op("7", "commit", key="x4"),
    Op("8", "commit", key="x5"),
    _decide("9", "do", "approve", "r1", "dave", "x5"),
    _flow("10", "finalize_approved", "r1", context={"actor": "alice"}),
    _create("11a", "r2", "Monitor", 300),
    _flow("11b", "submit", "r2", context={"actor": "alice"}),
    _flow("11c", "start_review", "r2", input={"reviewer": "bob"}),
    _decide("12", "eval", "approve", "r2", "dave", "x6", key="x6"),
    Op("13", "do", "set_role", {"user": "dave"}, {"role": "Requester"}),
    Op("14", "commit", key="x6"),
    _decide("15", "do", "approve", "r2", "dave", "x6"),
    _decide("16", "do", "reject", "r2", "carol", "x7"),
    _decide("17", "do", "approve", "r2", "bob", "x8"),
    _flow("18", "finalize_approved", "r2", context={"actor": "alice"}),
    _flow("19", "finalize_rejected", "r2"),
    Op("20", "do", "budget__close", {"budget": "b1"}),
]


#: The steps whose decision is also recorded as a portable `InvocationRecord` (T010): an
#: ALLOW, a DENY (not eligible) and an ALLOW with a command intent.
RECORDED_STEPS = ("4", "6", "10")

#: The fresh identities an operation creates, as (entity, input field).
_FRESH = {"purchase_request__create": ("PurchaseRequest", "request_id"),
          "purchase_request_approval__approve": ("PurchaseRequestApprovalDecision", "decision_id"),
          "purchase_request_approval__reject": ("PurchaseRequestApprovalDecision", "decision_id")}


_WIRE: dict[str, dict[str, Any]] = {}


def wire(model: BehaviorModule) -> dict[str, Any]:
    """The module's wire IR as data, parsed once per behavior version."""
    key = model.behavior_version
    if key not in _WIRE:
        _WIRE[key] = json.loads(model.to_wire_json())
    return _WIRE[key]


def _param_entities(model: BehaviorModule, capability: str) -> dict[str, str]:
    wire_ = wire(model)
    action = next(a for a in wire_["actions"] if a["name"] == capability)
    return {p["name"]: p["type"]["name"] for p in action["params"]
            if p.get("role") == "state"}


def invocation_document(model: BehaviorModule, op: Op) -> str:
    """The requested-invocation document of `op` (`behavior.invocation.v1`)."""
    entities = _param_entities(model, op.capability)
    doc = {"format": "behavior.invocation.v1", "capability": op.capability,
           "bindings": {n: {"entity": entities[n], "id": i} for n, i in op.bindings.items()},
           "input": op.input, "context": op.context}
    return json.dumps(doc, sort_keys=True)


def invocation_snapshot(store: Store, model: BehaviorModule, op: Op) -> str:
    """The complete parent snapshot at the store's current position, from the Store's own
    export (P6: the Store supplies it; nothing is inferred): every declared entity type is in
    the universe, an empty type with no members, and the fresh identity `op` creates is
    declared unused."""
    head = store.current()
    seed = store.export_seed_at(model, store.history_at(head.position))["seed"]
    types = [e["name"] for e in wire(model)["entities"]]
    universe = [{"entity": t, "members": [e["value"] for e in seed if e["entity"] == t]}
                for t in types]
    facts: dict[str, Any] = {"universe": universe}
    if op.capability in _FRESH:
        entity, key = _FRESH[op.capability]
        facts["identities"] = [{"entity": entity, "id": op.input[key], "used": False}]
    return json.dumps({"format": "behavior.snapshot.v1", "data_version": store.data_version(),
                       "entities": seed,
                       "facts": facts}, sort_keys=True)


@dataclass
class StepResult:
    step: str
    result: str | None  # ALLOW / DENY, or None for a commit-only operation
    conflict: str | None  # the commit error code, if the commit was refused
    position: int
    decision: Any = None
    #: The operation's kind: `do`, `eval` (a candidate kept for a later commit) or `commit`.
    kind: str = "do"
    #: For an operation that committed nothing: whether the exact head (position, state
    #: identity, record) was unchanged by it (FR-013; commands are published only with a commit
    #: record); None for committed operations and kept candidates.
    unchanged: bool | None = None


def evaluate(store: Store, model: BehaviorModule, op: Op) -> Any:
    return store.evaluate(model, op.capability, bindings=op.bindings, input=op.input,
                          context=op.context, commit_time=workflow.COMMIT_TIME)


def commit(store: Store, model: BehaviorModule, ev: Any) -> str | None:
    """Commits an allowed evaluation; the refusal code if the Store refuses it."""
    try:
        store.commit(model, ev.bundle)
    except StateConflict as e:
        return "STATE_CONFLICT" if "STATE_CONFLICT" in str(e) else str(e)
    return None


def decision_rows(store: Store, model: BehaviorModule) -> list[tuple[str, str, str]]:
    rows = store.read(model, "decisions").value
    return sorted((r["subject"], r["actor"], r["choice"]) for r in rows)


def unique_per_actor(rows: list[tuple[str, str, str]]) -> bool:
    """VO-003: at most one decision per (subject, actor)."""
    pairs = [(s, a) for s, a, _ in rows]
    return len(pairs) == len(set(pairs))


def thresholds_exclusive(rows: list[tuple[str, str, str]], k_a: int = K_A, k_r: int = K_R) -> bool:
    """VO-004: no subject has both thresholds reached."""
    for subject in {s for s, _, _ in rows}:
        a = sum(1 for s, _, c in rows if s == subject and c == "APPROVE")
        r = sum(1 for s, _, c in rows if s == subject and c == "REJECT")
        if a >= k_a and r >= k_r:
            return False
    return True


def outcome(store: Store, model: BehaviorModule, rid: str, at: Any = None) -> str:
    """The approval outcome of request `rid`, read through Core's derived predicates: exactly
    one of approved, rejected and pending holds."""
    row = store.read(model, "request_outcome", bindings={"request": rid}, at=at).value
    flags = {k: v for k, v in row.items() if k.startswith("purchase_request_approval__")}
    held = [k.rsplit("__", 1)[1].upper() for k, v in flags.items() if v is True]
    assert len(held) == 1, row
    return held[0]


def status(store: Store, rid: str) -> str:
    value: str = store.load("PurchaseRequest", rid)["value"]["status"]
    return value


_COMMAND_NAMES: dict[str, dict[str, str]] = {}


def command_names(model: BehaviorModule) -> dict[str, str]:
    """Command declaration hash → name, from the admission report's item table (cached per
    behavior version)."""
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


def head(store: Store) -> dict[str, Any]:
    """The exact head: store, position, state identity and record."""
    return store.history_at(store.current().position).as_dict()


def snapshot(store: Store, model: BehaviorModule) -> tuple[Any, ...]:
    """What a failed commit must leave unchanged: the head (position, state identity, record)
    and the command stream after it, which must stay empty: no command is published without a
    commit (FR-013)."""
    h = head(store)
    request = {"format": "behavior.command_stream_request.v1", "after": h, "max_records": 1}
    return (h, [i["command_occurrence_id"] for i in store.commands_since(request).items])


def run_history(model: BehaviorModule, store: Store | None = None, on_commit: Any = None,
                records: dict[str, Any] | None = None, stop_before: str | None = None,
                track_unchanged: bool = False) -> tuple[Store, list[StepResult]]:
    """Walks H, up to `stop_before` (a step id) if given. After every committed step,
    `on_commit(store)` (if given) runs. For the
    `RECORDED_STEPS`, the decision is also recorded through `invoke` against the same parent
    snapshot, into `records` (with its document and snapshot)."""
    store = store or fresh_store(model)
    kept: dict[str, Any] = {}
    results: list[StepResult] = []
    for op in HISTORY:
        if op.step == stop_before:
            break
        if records is not None and op.kind == "do" and op.step in RECORDED_STEPS:
            document = invocation_document(model, op)
            snapshot_doc = invocation_snapshot(store, model, op)
            records[op.step] = {"record": invoke(model, document, snapshot_doc),
                                "document": document, "snapshot": snapshot_doc}
        if op.kind == "eval":
            ev = evaluate(store, model, op)
            kept[op.key] = ev
            results.append(StepResult(op.step, ev.decision.result, None,
                                      store.current().position, ev.decision, kind="eval"))
            continue
        if op.kind == "commit":
            ev = kept.pop(op.key)
            before = head(store) if track_unchanged else None
            refused = commit(store, model, ev)
            results.append(StepResult(
                op.step, None, refused, store.current().position, kind="commit",
                unchanged=(head(store) == before) if refused and before else None))
            if refused is None and on_commit is not None:
                on_commit(store)
            continue
        before = head(store) if track_unchanged else None
        ev = evaluate(store, model, op)
        refused = commit(store, model, ev) if ev.bundle is not None else None
        committed = ev.bundle is not None and refused is None
        results.append(StepResult(op.step, ev.decision.result, refused,
                                  store.current().position, ev.decision,
                                  unchanged=None if committed or before is None
                                  else head(store) == before))
        if ev.bundle is not None and refused is None and on_commit is not None:
            on_commit(store)
    return store, results


def op(step: str, index: int = 0) -> Op:
    """The `index`-th operation of step `step` in H."""
    return [o for o in HISTORY if o.step == step][index]


def variant_module(old: str, new: str) -> BehaviorModule:
    """The reference module built from a copy of workflow.py with one textual change to an
    authored declaration (C14.wrong_module, C18). The copy runs as its own module, so nothing
    is shared with `workflow`."""
    import sys
    import types

    path = HERE / "workflow.py"
    source = path.read_text()
    assert old in source, old
    mod = types.ModuleType("purchase_approval_variant")
    mod.__file__ = str(path)
    sys.modules[mod.__name__] = mod
    try:
        exec(compile(source.replace(old, new, 1), str(path), "exec"), mod.__dict__)
        built: BehaviorModule = mod.module()
    finally:
        del sys.modules[mod.__name__]
    return built


#: C18's declaration change: `Role` gains a case. `User` references `Role`, and the flow (actor)
#: and the approval (actor) reference `User`; the `budget` machine references neither.
ROLE_CHANGE = ('    APPROVER = "Approver"\n', '    APPROVER = "Approver"\n    AUDITOR = "Auditor"\n')


def authoring_identity(model: BehaviorModule) -> dict[str, Any]:
    """The authoring identity `N_A` (contract comparison.md §1): BehaviorHash, every admitted
    item's hash, each model's versions and hash, and Approval's generated hashes. Source
    locations are not part of it."""
    admission = model.engine.admission()
    models: dict[str, Any] = {}
    generated: dict[str, Any] = {}
    for r in model.model_receipts:
        d = r.to_dict()
        mid = next(d["model"][k] for k in ("model_id", "flow_id", "approval_id")
                   if k in d["model"])
        models[mid] = {"kind": d["model_kind"],
                       "model_semantic_version": d["model_semantic_version"],
                       "lowerer_version": d["lowerer_version"], "model_hash": d["model_hash"]}
        if "generated" in d:
            generated[mid] = d["generated"]
    return {"behavior_hash": model.behavior_version, "items": admission["items"],
            "models": models, "generated": generated}
