#!/usr/bin/env python3
"""Compares decision records while ignoring `record_version` (feature 004, research R8).

Usage: scripts/record-diff.py OLD NEW [OLD NEW ...]
       scripts/record-diff.py --git REF FILE...   (compare each FILE with its version at REF)

Exits 0 when every pair differs at most in `record_version`, 1 otherwise, printing each other
difference as `path: old -> new`.
"""

from __future__ import annotations

import json
import subprocess
import sys
from typing import Any


def diff(a: Any, b: Any, path: str, out: list[str]) -> None:
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if path == "" and k == "record_version":
                continue
            if k not in a or k not in b:
                out.append(f"{path}.{k}: {a.get(k, '<missing>')!r} -> {b.get(k, '<missing>')!r}")
            else:
                diff(a[k], b[k], f"{path}.{k}", out)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b)):
            diff(x, y, f"{path}[{i}]", out)
    elif a != b:
        out.append(f"{path}: {json.dumps(a)} -> {json.dumps(b)}")


def main(argv: list[str]) -> int:
    pairs: list[tuple[str, Any, Any]] = []
    if argv[:1] == ["--git"]:
        ref, files = argv[1], argv[2:]
        for f in files:
            old = subprocess.run(["git", "show", f"{ref}:{f}"], capture_output=True, text=True, check=True).stdout
            pairs.append((f, json.loads(old), json.load(open(f))))
    else:
        if len(argv) % 2:
            print(__doc__, file=sys.stderr)
            return 2
        for old, new in zip(argv[::2], argv[1::2]):
            pairs.append((new, json.load(open(old)), json.load(open(new))))
    failed = False
    for name, a, b in pairs:
        out: list[str] = []
        diff(a, b, "", out)
        if out:
            failed = True
            print(f"{name}:")
            for line in out:
                print(f"  {line}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
