"""Mechanical part of the authoring rubric: run it from the agent's scratch repository.

It checks that `model.py` is admitted, that `checks.py` passes, that only public names are
imported from `behavior`, and that every `SEMANTIC_GAPS.md` entry has the fixed fields. The
judgement parts of the rubric stay manual.
"""

from __future__ import annotations

import ast
import re
import runpy
import subprocess
import sys
from pathlib import Path

import behavior

FIELDS = ["Date", "Behavior release", "Requirement", "Why inexpressible",
          "What was done instead", "Severity", "Evidence"]
problems: list[str] = []

for source in [Path("model.py"), Path("checks.py")]:
    if not source.exists():
        problems.append(f"{source} is missing")
        continue
    tree = ast.parse(source.read_text())
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.module and node.module.startswith("behavior"):
            if node.module != "behavior":
                problems.append(f"{source}: imports from {node.module}")
            for alias in node.names:
                if alias.name not in behavior.__all__:
                    problems.append(f"{source}: {alias.name} is not public")

if Path("model.py").exists():
    model = runpy.run_path("model.py")["model"]
    result = behavior.admit(model)
    if not result.ok:
        problems.append(f"model.py not admitted: {result.errors}")

if Path("checks.py").exists() and subprocess.run([sys.executable, "checks.py"]).returncode != 0:
    problems.append("checks.py fails")

gaps = Path("SEMANTIC_GAPS.md")
entries = re.split(r"^## ", gaps.read_text(), flags=re.M)[1:] if gaps.exists() else []
if len(entries) < 4:
    problems.append(f"{len(entries)} gap entries; requirements 11-14 need one each")
for entry in entries:
    title = entry.splitlines()[0]
    for f in FIELDS:
        if f"**{f}**" not in entry:
            problems.append(f"gap `{title}` lacks {f}")

print("\n".join(problems) or "mechanical checks: OK")
sys.exit(1 if problems else 0)
