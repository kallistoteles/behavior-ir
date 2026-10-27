"""Core adoption may change receipt release metadata, never semantic baseline fields."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import shutil
import subprocess

import pytest


@pytest.mark.parametrize("change,accepted", [
    ("unchanged", True),
    ("core_release", True),
    ("model_hash", False),
    ("behavior_hash", False),
    ("generated", False),
    ("source_map", False),
    ("nested_core_release", False),
    ("missing_duplicate", False),
    ("new_receipt", True),
])
def test_receipt_baseline_preserves_every_semantic_field(
    tmp_path: Path, change: str, accepted: bool,
) -> None:
    root = Path(__file__).resolve().parents[2]
    scripts = tmp_path / "scripts"
    (scripts / "tests").mkdir(parents=True)
    baseline_dir = tmp_path / "specs/507-work-queue/baseline"
    baseline_dir.mkdir(parents=True)
    script = scripts / "tests/test_pinned_identity_baseline.sh"
    shutil.copyfile(root / "scripts/tests/test_pinned_identity_baseline.sh", script)
    receipt = {
        "model_kind": "StateMachine", "model_hash": "model:original",
        "behavior_hash": "behavior:original", "core_release": {"version": "old"},
        "generated": {"step": "hash:original"}, "source_map": {"action": "line:1"},
        "canonical_model": {"domain": {"core_release": "a protected domain field"}},
    }
    baseline = [copy.deepcopy(receipt), copy.deepcopy(receipt)]
    current = copy.deepcopy(baseline)
    if change == "core_release":
        for r in current:
            r["core_release"] = {"version": "new", "commit": "new-commit"}
    elif change == "nested_core_release":
        current[0]["canonical_model"]["domain"]["core_release"] = "changed domain value"
    elif change == "missing_duplicate":
        current.pop()
    elif change == "new_receipt":
        extra = copy.deepcopy(receipt)
        extra["model_hash"] = "model:additional"
        current.append(extra)
    elif change != "unchanged":
        current[0][change] = "changed semantic value"
    baseline_path = baseline_dir / "compat-print.txt"
    baseline_bytes = json.dumps(baseline).encode()
    baseline_path.write_bytes(baseline_bytes)
    (scripts / "current.json").write_text(json.dumps(current))
    producer = scripts / "check-models-compat.py"
    producer.write_text(
        '#!/usr/bin/env bash\nset -eu\n'
        'if [ "${1:-}" = --print ]; then cat "$(dirname "$0")/current.json"; fi\n'
    )
    producer.chmod(0o755)
    result = subprocess.run(["bash", str(script)], capture_output=True, text=True)
    assert (result.returncode == 0) == accepted, result.stdout + result.stderr
    assert baseline_path.read_bytes() == baseline_bytes
