"""Release metadata may change; historical conformance outcomes and format may not."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import shutil
import subprocess

import behavior as b
import pytest

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "specs/507-work-queue/baseline/506-report-quick.json"


@pytest.mark.parametrize("change,accepted", [
    ("release_metadata", True), ("stale_report", True),
    ("status", False), ("missing_check", False), ("level_type", False),
    ("nested_metadata", False), ("formats", False), ("model", False),
    ("extra_core_metadata", False), ("wrong_engine", False),
    ("wrong_verifier", False), ("missing_core_version", False),
    ("noncanonical_bytes", False),
])
def test_conformance_baseline_allows_only_selected_release_metadata(
    tmp_path: Path, change: str, accepted: bool,
) -> None:
    scripts = tmp_path / "scripts"
    (scripts / "tests").mkdir(parents=True)
    baseline_dir = tmp_path / "specs/507-work-queue/baseline"
    baseline_dir.mkdir(parents=True)
    original = BASELINE.read_bytes()
    (baseline_dir / BASELINE.name).write_bytes(original)
    shutil.copyfile(ROOT / "scripts/tests/test_conformance_506_bytes.sh",
                    scripts / "tests/test_conformance_506_bytes.sh")
    pin = json.loads((ROOT / "core-release.json").read_text())
    (tmp_path / "core-release.json").write_text(json.dumps(pin))
    current = copy.deepcopy(json.loads(original))
    versions = b.versions()
    current["core_release"] = versions["core"]
    current["engine"]["engine"] = versions["engine"]
    current["engine"]["verifier"] = versions["verifier"]
    if change == "status": current["checks"][0]["status"] = "failed"
    elif change == "missing_check": current["checks"].pop()
    elif change == "level_type": current["checks"][0]["level"] = True
    elif change == "nested_metadata": current["checks"][0]["core_release"] = {"version": "changed"}
    elif change == "formats": current["engine"]["accepted_wire_ir"].pop()
    elif change == "model": current["models"]["unexpected"] = "changed"
    elif change == "extra_core_metadata": current["core_release"]["unexpected"] = "changed"
    elif change == "wrong_engine": current["engine"]["engine"] = "99.0.0"
    elif change == "wrong_verifier": current["engine"]["verifier"] = "99.0.0"
    elif change == "missing_core_version": current["core_release"].pop("version")
    encoded = json.dumps(current, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"
    if change == "noncanonical_bytes": encoded = json.dumps(current, indent=2) + "\n"
    (tmp_path / "next-report.json").write_text(encoded)
    report = tmp_path / "target/conformance/506-report.json"
    report.parent.mkdir(parents=True)
    report.write_bytes(original if change == "stale_report" else encoded.encode())
    # This unit stub is only a regeneration marker; it never supplies conformance proof.
    runner = scripts / "conformance.sh"
    runner.write_text('#!/usr/bin/env bash\nset -eu\ncd "$(dirname "$0")/.."\n'
                      'touch regenerated\ncp next-report.json target/conformance/506-report.json\n')
    runner.chmod(0o755)
    result = subprocess.run(["bash", str(scripts / "tests/test_conformance_506_bytes.sh")],
                            capture_output=True, text=True)
    assert (result.returncode == 0) == accepted, result.stdout + result.stderr
    if change == "stale_report": assert (tmp_path / "regenerated").exists()
    assert (baseline_dir / BASELINE.name).read_bytes() == original
