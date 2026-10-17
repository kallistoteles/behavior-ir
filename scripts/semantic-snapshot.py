#!/usr/bin/env python3
"""The semantic snapshot of the ecosystem under the pinned Core (feature 508, spec Q8.1).

    scripts/semantic-snapshot.py OUT.json

It records, as canonical JSON, everything that must not change when the Core pin changes:

- every compat reference case: each receipt (ModelHash, BehaviorHash, canonical model document,
  source map, generated items) and the module's BehaviorHash;
- the 506 and 507 reference histories: per step the decision result, the position and the
  resulting state identity, and the final command stream.

Release metadata is not part of the snapshot: each receipt's `core_release` is removed (it names
the pin, which is what changes). Verification results are deliberately absent; they are the
separate verification comparison (spec Q8.2).

    scripts/semantic-snapshot.py --compare OLD.json NEW.json

prints every difference and exits 1 if there is any.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _receipt(r: Any) -> dict[str, Any]:
    d: dict[str, Any] = r.to_dict()
    d.pop("core_release", None)
    return d


def _cases() -> dict[str, Any]:
    from models.fixtures.reference_cases import CASES

    out = {}
    for name, build in sorted(CASES.items()):
        module = build()
        out[name] = {"behavior_hash": module.behavior_version,
                     "receipts": [_receipt(r) for r in module.model_receipts]}
    return out


def _history_506() -> dict[str, Any]:
    from models.examples.purchase_approval import harness, workflow

    model = workflow.module()
    store, results = harness.run_history(model)
    return {
        "behavior_hash": model.behavior_version,
        "steps": [{"step": r.step, "kind": r.kind, "result": r.result, "conflict": r.conflict,
                   "position": r.position} for r in results],
        "states": [store.state_at(p).as_dict() for p in range(store.current().position + 1)],
        "commands": harness.commands(store, model),
    }


def _history_507() -> dict[str, Any]:
    from models.examples.work_queue import harness, workflow

    model = workflow.module()
    store, results = harness.run_history(model)
    return {
        "behavior_hash": model.behavior_version,
        "steps": [{"step": r.step, "result": r.result, "conflict": r.conflict,
                   "position": r.position, "item": r.item} for r in results],
        "states": [store.state_at(p).as_dict() for p in range(store.current().position + 1)],
        "commands": harness.commands(store, model),
    }


def snapshot() -> dict[str, Any]:
    sys.path.insert(0, str(ROOT))
    return {"format": "behavior.semantic_snapshot.v1", "cases": _cases(),
            "history_506": _history_506(), "history_507": _history_507()}


def _diff(a: Any, b: Any, path: str, out: list[str]) -> None:
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out.append(f"{path}/{k}: only in {'new' if k not in a else 'old'}")
            else:
                _diff(a[k], b[k], f"{path}/{k}", out)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b)):
            _diff(x, y, f"{path}[{i}]", out)
    elif a != b:
        out.append(f"{path}: {canonical(a)[:160]} != {canonical(b)[:160]}")


def main(argv: list[str]) -> int:
    if argv[1:2] == ["--compare"]:
        old, new = (json.loads(Path(p).read_text()) for p in argv[2:4])
        diffs: list[str] = []
        _diff(old, new, "", diffs)
        for d in diffs:
            print(d)
        print(f"semantic-snapshot: {len(diffs)} difference(s)")
        return 1 if diffs else 0
    Path(argv[1]).write_text(canonical(snapshot()) + "\n")
    print(f"semantic-snapshot: wrote {argv[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
