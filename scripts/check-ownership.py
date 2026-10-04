#!/usr/bin/env python3
"""Placement by semantic ownership (feature 011, FR-023): every file of the repository has
exactly one owner, decided by the ordered rule table in
`specs/011-core-ecosystem-split/contracts/ownership.md` (the first matching rule wins).

    scripts/check-ownership.py                 # every file matches a rule; exit 1 otherwise
    scripts/check-ownership.py --list core     # the files the core receives (core and both)
    scripts/check-ownership.py --list core-only  # the files that leave the ecosystem
    scripts/check-ownership.py --list ecosystem  # the files that stay (ecosystem and both)

Files are the tracked files plus untracked files that are not ignored.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "specs" / "011-core-ecosystem-split" / "contracts" / "ownership.md"
OWNERS = {"core", "ecosystem", "both"}


def glob_regex(glob: str) -> re.Pattern[str]:
    """A glob over repository paths: `**` spans directories, `*` and `?` stay in one segment,
    `[...]` is a character class."""
    out, i = [], 0
    while i < len(glob):
        c = glob[i]
        if glob.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif glob.startswith("**", i):
            out.append(".*")
            i += 2
        elif c == "*":
            out.append("[^/]*")
            i += 1
        elif c == "?":
            out.append("[^/]")
            i += 1
        elif c == "[":
            end = glob.index("]", i)
            out.append(glob[i : end + 1])
            i = end + 1
        else:
            out.append(re.escape(c))
            i += 1
    return re.compile("".join(out) + r"\Z")


def rules() -> list[tuple[int, list[re.Pattern[str]], str]]:
    table = []
    for line in MANIFEST.read_text().splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 3 or not cells[0].isdigit():
            continue
        owner = cells[2]
        if owner not in OWNERS:
            sys.exit(f"check-ownership: rule {cells[0]} has unknown owner {owner!r}")
        globs = re.findall(r"`([^`]+)`", cells[1])
        if not globs:
            sys.exit(f"check-ownership: rule {cells[0]} has no glob")
        table.append((int(cells[0]), [glob_regex(g) for g in globs], owner))
    if not table:
        sys.exit(f"check-ownership: no rules in {MANIFEST}")
    return table


def files() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return sorted(p for p in out.splitlines() if (ROOT / p).is_file())


def owners(table: list[tuple[int, list[re.Pattern[str]], str]], path: str) -> list[tuple[int, str]]:
    return [(n, owner) for n, globs, owner in table if any(g.match(path) for g in globs)]


def main(argv: list[str]) -> int:
    table = rules()
    placed: dict[str, tuple[int, str]] = {}
    unmatched = []
    overlaps: dict[tuple[int, int], int] = {}
    for path in files():
        matches = owners(table, path)
        if not matches:
            unmatched.append(path)
            continue
        placed[path] = matches[0]
        for n, owner in matches[1:]:
            if owner != matches[0][1]:
                key = (matches[0][0], n)
                overlaps[key] = overlaps.get(key, 0) + 1

    if len(argv) == 2 and argv[0] == "--list":
        wanted = {"core": {"core", "both"}, "core-only": {"core"},
                  "ecosystem": {"ecosystem", "both"}}.get(argv[1])
        if wanted is None:
            print(__doc__, file=sys.stderr)
            return 2
        if unmatched:
            print(f"check-ownership: {len(unmatched)} files match no rule; run without --list",
                  file=sys.stderr)
            return 1
        for path, (_, owner) in placed.items():
            if owner in wanted:
                print(path)
        return 0
    if argv:
        print(__doc__, file=sys.stderr)
        return 2

    for (first, later), count in sorted(overlaps.items()):
        print(f"check-ownership: note: {count} files placed by rule {first} also match rule "
              f"{later} (another owner); the first rule decides", file=sys.stderr)
    if unmatched:
        for path in unmatched:
            print(f"check-ownership: no rule places {path}", file=sys.stderr)
        return 1
    counts = {o: sum(1 for _, owner in placed.values() if owner == o) for o in sorted(OWNERS)}
    print("check-ownership: OK " + ", ".join(f"{o} {n}" for o, n in counts.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
