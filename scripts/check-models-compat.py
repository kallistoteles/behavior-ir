#!/usr/bin/env python3
"""Model lowering compatibility gate (feature 501, lowering contract §7, spec FR-001a).

Version identifiers identify semantic functions, not source-code releases. For a fixed tuple
(model kind, model semantic version, lowerer version, Core Release) the mapping

    reference case → (ModelHash per model, BehaviorHash)

recorded in `models/fixtures/compat.json` is immutable:

1. every reference case, rebuilt now, must reproduce the current tuple's recorded hashes;
2. against the merge base with `main`, every established tuple must be byte-identical here, so
   editing an expected hash never passes; a changed lowering needs a version bump (a new
   tuple). Only a tuple whose Core Release is no longer pinned may be removed;
3. the current tuple must cover every reference case.

    scripts/check-models-compat.py                 # the gate
    scripts/check-models-compat.py --write         # record the current tuple if it is new
    scripts/check-models-compat.py --print         # print the canonical receipts (determinism)
    scripts/check-models-compat.py --computed F    # use precomputed cases (script self-test)
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = Path("models/fixtures/compat.json")
FORMAT = "behavior.model_compat.v1"


def fail(message: str) -> None:
    print(f"check-models-compat: FAILED: {message}", file=sys.stderr)
    sys.exit(1)


def canonical(v: Any) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def tuple_key(t: dict[str, Any]) -> str:
    return canonical({k: t[k] for k in
                      ("model_kind", "model_semantic_version", "lowerer_version", "core_release")})


def compute(root: Path) -> tuple[dict[str, Any], dict[str, Any], list[Any]]:
    """Builds every reference case: (current tuple, cases, receipts)."""
    sys.path.insert(0, str(root))
    from models.fixtures.reference_cases import CASES  # noqa: PLC0415

    tuples, cases, receipts = set(), {}, []
    for name, build in sorted(CASES.items()):
        module = build()
        models = {}
        for receipt in module.model_receipts:
            r = receipt.to_dict()
            tuples.add(tuple_key(r))
            models[r["model"]["model_id"]] = r["model_hash"]
            receipts.append(r)
        cases[name] = {"behavior_hash": module.behavior_version, "models": models}
    if len(tuples) != 1:
        fail(f"reference cases disagree on the version tuple: {sorted(tuples)}")
    return json.loads(next(iter(tuples))), cases, receipts


def load(text: str | None, what: str) -> dict[str, Any]:
    if text is None:
        return {"format": FORMAT, "tuples": []}
    doc = json.loads(text)
    if doc.get("format") != FORMAT:
        fail(f"{what}: not a {FORMAT} document")
    return doc


def git(root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)


def merge_base_manifest(root: Path) -> dict[str, Any] | None:
    """The manifest at the merge base with `main` (None if it did not exist there). Fails when
    the merge base is unavailable: the gate never passes silently without history."""
    for ref in ("main", "origin/main"):
        base = git(root, "merge-base", "HEAD", ref)
        if base.returncode == 0:
            shown = git(root, "show", f"{base.stdout.strip()}:{MANIFEST}")
            return load(shown.stdout, "merge-base manifest") if shown.returncode == 0 else None
    fail("no merge base with `main` or `origin/main` (a shallow clone? fetch full history)")
    return None


def pinned_core(root: Path) -> dict[str, str]:
    pin = json.loads((root / "core-release.json").read_text())
    return {"commit": pin["commit"], "version": pin["version"]}


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, default=ROOT)
    p.add_argument("--write", action="store_true")
    p.add_argument("--print", dest="print_", action="store_true")
    p.add_argument("--computed", type=Path)
    a = p.parse_args()
    root: Path = a.root

    if a.computed is not None:
        doc = json.loads(a.computed.read_text())
        current, cases, receipts = doc["tuple"], doc["cases"], []
    else:
        current, cases, receipts = compute(root)
    if a.print_:
        print(canonical(receipts))
        return

    path = root / MANIFEST
    manifest = load(path.read_text() if path.exists() else None, str(MANIFEST))
    by_key = {tuple_key(t): t for t in manifest["tuples"]}
    key = tuple_key(current)

    if a.write:
        if key in by_key:
            if by_key[key]["cases"] != cases:
                fail("the current tuple is established with other hashes; bump a version")
            print("check-models-compat: current tuple already recorded")
            return
        manifest["tuples"].append({**current, "cases": cases})
        manifest["tuples"].sort(key=tuple_key)
        path.write_text(json.dumps(manifest, indent=1, sort_keys=True) + "\n")
        print("check-models-compat: recorded the current tuple")
        return

    # 1 and 3: the current tuple reproduces and covers every case.
    recorded = by_key.get(key)
    if recorded is None:
        fail(f"no manifest entry for the current tuple {key}; record it with --write")
    if sorted(recorded["cases"]) != sorted(cases):
        fail(f"the current tuple does not cover exactly the reference cases {sorted(cases)}")
    for name, got in sorted(cases.items()):
        if recorded["cases"][name] != got:
            fail(f"case `{name}` lowers differently under an unchanged version tuple: "
                 f"recorded {canonical(recorded['cases'][name])}, now {canonical(got)}")

    # 2: established tuples are immutable.
    base = merge_base_manifest(root)
    if base is not None:
        pinned = pinned_core(root)
        for t in base["tuples"]:
            k = tuple_key(t)
            if k not in by_key:
                if t["core_release"] == pinned:
                    fail(f"established tuple {k} was removed")
                continue
            if canonical(by_key[k]["cases"]) != canonical(t["cases"]):
                fail(f"established tuple {k} was changed; a changed lowering needs a new "
                     "model semantic version or lowerer version")
    print(f"check-models-compat: OK ({len(cases)} cases, tuple {key})")


if __name__ == "__main__":
    main()
