#!/usr/bin/env bash
# Feature 507 (FR-037, SC-008): shared report infrastructure preserves historical 506
# outcomes and canonical bytes. Only the four selected Core-release/version metadata
# fields may differ after a pin update. Never replace the published baseline.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
python3 - "$here" <<'PY'
import difflib
import json
from pathlib import Path
import subprocess
import sys

import behavior

root = Path(sys.argv[1])
report = root / "target/conformance/506-report.json"
baseline = root / "specs/507-work-queue/baseline/506-report-quick.json"
pin = json.loads((root / "core-release.json").read_text())
versions = behavior.versions()
expected_core = {k: pin[k] for k in ("version", "commit")}
assert versions["core"] == expected_core and versions["engine"] == pin["version"], "build the declared Core first"


def fresh(doc):
    return (doc.get("mode") == "quick" and doc.get("core_release") == expected_core
            and doc.get("engine", {}).get("engine") == versions["engine"]
            and doc.get("engine", {}).get("verifier") == versions["verifier"])


def canonical(doc):
    return json.dumps(doc, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"


current = json.loads(report.read_text()) if report.exists() else {}
if not fresh(current):
    subprocess.run([str(root / "scripts/conformance.sh"), "506", "quick"], check=True, stdout=subprocess.DEVNULL)
    current = json.loads(report.read_text())
assert fresh(current), "the regenerated report must identify the current pinned Core and verifier"
old_bytes = baseline.read_text()
old = json.loads(old_bytes)
assert report.read_text() == canonical(current), "current report bytes are not canonical"
assert old_bytes == canonical(old), "historical report bytes are not canonical"
# Replace selected metadata leaves only. Check/Model identities, outcomes, typed
# values, nested fields and every supported format remain part of byte comparison.
for key in ("version", "commit"):
    current["core_release"][key] = old["core_release"][key]
for key in ("engine", "verifier"):
    current["engine"][key] = old["engine"][key]
if canonical(current) != old_bytes:
    difference = difflib.unified_diff(json.dumps(old, indent=2).splitlines(),
                                      json.dumps(current, indent=2).splitlines(), lineterm="")
    print("test_conformance_506_bytes: report content differs from the baseline", file=sys.stderr)
    print("\n".join(list(difference)[:20]), file=sys.stderr)
    sys.exit(1)
print("test_conformance_506_bytes: OK (selected release metadata normalized)")
PY
