#!/usr/bin/env python3
"""Model lowering compatibility gate (features 501, 502; lowering contract §7, spec FR-001a).

Version identifiers identify semantic functions, not source-code releases. For a fixed tuple
(model kind, model semantic version, lowerer version, Core Release) the mapping

    reference case → (ModelHash per model, BehaviorHash)

recorded in `models/fixtures/compat.json` is immutable:

1. every reference case, rebuilt now, must reproduce the current tuple's recorded hashes;
2. against the merge base with `main`, every established tuple must be byte-identical here, so
   editing an expected hash never passes; a changed lowering needs a version bump (a new
   tuple). Only a tuple whose Core Release is no longer pinned may be removed;
3. the current tuples together cover every reference case, each case under exactly one.

Cases are grouped by tuple (feature 502, research R10). A receipt's tuple is its (model kind,
model semantic version, lowerer version, Core Release), plus, when the model embeds others (a
RequestFlow's StateMachine), the embedded models' (kind, semantic version, lowerer version): a
StateMachine lowerer bump is then also a new RequestFlow tuple. Without embedded models the key
is exactly feature 501's, so its established entries stay byte-identical. A case whose
receipts differ in tuple (a flow beside a standalone machine) belongs to the embedding tuple
when that tuple's embedded versions cover the others.

A case with approvals (feature 503) belongs to the approval tuple. Its key adds `co_supplied`:
the (kind, semantic version, lowerer version) of every other model in the case and of the
models they embed, because the approval's subject may be a flow's entity and its hashes depend
on that lowering. A bump of any co-supplied lowerer is then also a new approval tuple. Its
entries add each approval's `generated` enum, entity and derived hashes.

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


KEY_FIELDS = ("model_kind", "model_semantic_version", "lowerer_version", "core_release")


def tuple_key(t: dict[str, Any]) -> str:
    fields = KEY_FIELDS + tuple(k for k in ("embedded", "co_supplied") if k in t)
    return canonical({k: t[k] for k in fields})


def versions(r: dict[str, Any]) -> list[dict[str, Any]]:
    """The version records of a receipt's model and of the models it embeds."""
    own = {"kind": r["model_kind"], "model_semantic_version": r["model_semantic_version"],
           "lowerer_version": r["lowerer_version"]}
    return [own] + [{"kind": e["kind"], "model_semantic_version": e["model_semantic_version"],
                     "lowerer_version": e["lowerer_version"]} for e in r.get("embedded", [])]


def receipt_tuple(r: dict[str, Any]) -> dict[str, Any]:
    t = {k: r[k] for k in KEY_FIELDS}
    if "embedded" in r:
        records = {canonical(v): v for v in (
            {"kind": e["kind"], "model_semantic_version": e["model_semantic_version"],
             "lowerer_version": e["lowerer_version"]} for e in r["embedded"])}
        t["embedded"] = [records[k] for k in sorted(records)]
    return t


def case_tuple(name: str, receipts: list[dict[str, Any]]) -> dict[str, Any]:
    """The one tuple a case belongs to (see the module documentation)."""
    approvals = [r for r in receipts if r["model_kind"] == "approval"]
    if approvals:
        own = {tuple_key(t): t for t in map(receipt_tuple, approvals)}
        if len(own) != 1:
            fail(f"case `{name}` has approvals under different version tuples: {sorted(own)}")
        t = dict(next(iter(own.values())))
        records = {canonical(v): v for r in receipts if r["model_kind"] != "approval"
                   for v in versions(r)}
        if records:
            t["co_supplied"] = [records[k] for k in sorted(records)]
        return t
    tuples = {tuple_key(t): t for t in map(receipt_tuple, receipts)}
    if len(tuples) == 1:
        return next(iter(tuples.values()))
    embedding = [t for t in tuples.values() if "embedded" in t]
    if len(embedding) == 1:
        e = embedding[0]
        covered = {canonical(x) for x in e["embedded"]}
        if all(t["core_release"] == e["core_release"] and canonical(
                {"kind": t["model_kind"], "model_semantic_version": t["model_semantic_version"],
                 "lowerer_version": t["lowerer_version"]}) in covered
               for t in tuples.values() if "embedded" not in t):
            return e
    fail(f"case `{name}` has no single version tuple: {sorted(tuples)}")
    return {}


def model_id(model: dict[str, Any]) -> str:
    return str(model.get("model_id", model.get("flow_id", model.get("approval_id"))))


def case_entry(module: Any, receipts: list[dict[str, Any]]) -> dict[str, Any]:
    entry: dict[str, Any] = {
        "behavior_hash": module.behavior_version,
        "models": {model_id(r["model"]): r["model_hash"] for r in receipts},
    }
    flows = [r for r in receipts if "embedded" in r]
    if flows:
        entry["embedded"] = {e["id"]: e["model_hash"] for r in flows for e in r["embedded"]}
        entry["entities"] = {
            model_id(r["model"]): next(x["core"]["name"] for x in r["source_map"]["entries"]
                                       if x["model"] == f"{model_id(r['model'])}/request_entity")
            for r in flows
        }
    approvals = [r for r in receipts if r["model_kind"] == "approval"]
    if approvals:
        entry["generated"] = {model_id(r["model"]): r["generated"] for r in approvals}
    return entry


def compute(root: Path) -> tuple[dict[str, dict[str, Any]], list[Any]]:
    """Builds every reference case: ({tuple key: {tuple, cases}}, receipts)."""
    sys.path.insert(0, str(root))
    from models.fixtures.reference_cases import CASES  # noqa: PLC0415

    groups: dict[str, dict[str, Any]] = {}
    all_receipts: list[Any] = []
    for name, build in sorted(CASES.items()):
        module = build()
        receipts = [r.to_dict() for r in module.model_receipts]
        t = case_tuple(name, receipts)
        groups.setdefault(tuple_key(t), {"tuple": t, "cases": {}})["cases"][name] = \
            case_entry(module, receipts)
        all_receipts += receipts
    return groups, all_receipts


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
        listed = doc["groups"] if "groups" in doc else [doc]
        groups = {tuple_key(g["tuple"]): {"tuple": g["tuple"], "cases": g["cases"]}
                  for g in listed}
        receipts: list[Any] = []
    else:
        groups, receipts = compute(root)
    if a.print_:
        print(canonical(receipts))
        return

    path = root / MANIFEST
    manifest = load(path.read_text() if path.exists() else None, str(MANIFEST))
    by_key = {tuple_key(t): t for t in manifest["tuples"]}

    if a.write:
        for key, g in sorted(groups.items()):
            if key in by_key:
                if by_key[key]["cases"] != g["cases"]:
                    fail(f"tuple {key} is established with other hashes; bump a version")
                continue
            manifest["tuples"].append({**g["tuple"], "cases": g["cases"]})
            print(f"check-models-compat: recorded tuple {key}")
        manifest["tuples"].sort(key=tuple_key)
        path.write_text(json.dumps(manifest, indent=1, sort_keys=True) + "\n")
        return

    # 1 and 3: each current tuple reproduces and covers exactly its cases.
    for key, g in sorted(groups.items()):
        recorded = by_key.get(key)
        if recorded is None:
            fail(f"no manifest entry for the current tuple {key}; record it with --write")
        cases = g["cases"]
        if sorted(recorded["cases"]) != sorted(cases):
            fail(f"tuple {key} does not cover exactly the reference cases {sorted(cases)}")
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
    summary = ", ".join(f"{len(g['cases'])} cases under {k}" for k, g in sorted(groups.items()))
    print(f"check-models-compat: OK ({summary})")


if __name__ == "__main__":
    main()
