"""The quick runtime requirement must fail the runner after preserving its report."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.mark.parametrize("elapsed, expected", ((34.99, 0), (35.01, 1)))
def test_work_queue_quick_budget_is_enforced(tmp_path: Path, elapsed: float, expected: int) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    shutil.copy2(ROOT / "scripts/conformance.sh", scripts / "conformance.sh")
    binaries = tmp_path / "bin"
    binaries.mkdir()
    cargo = binaries / "cargo"
    cargo.write_text("#!/bin/sh\nexit 0\n")
    cargo.chmod(0o755)
    python = binaries / "python"
    python.write_text(f"#!{sys.executable}\n" + '''\
import os, pathlib, sys, time
args = sys.argv[1:]
if args[0] == "-c":
    time.monotonic = lambda: 100.0
    exec(args[1])
elif args[:2] == ["-m", "pytest"]:
    out = pathlib.Path(os.environ["BEHAVIOR_CONFORMANCE_DIR"])
    (out / "507-report.json").write_text("{}")
elif args[0] == "-":
    time.monotonic = lambda: 100.0 + float(os.environ["TEST_ELAPSED"])
    sys.argv = args
    exec(sys.stdin.read())
''')
    python.chmod(0o755)
    result = subprocess.run(
        ["bash", str(scripts / "conformance.sh"), "507", "quick"],
        text=True, capture_output=True,
        env={**os.environ, "PATH": f"{binaries}:{os.environ['PATH']}",
             "BEHAVIOR_CLI_BINARY": "", "TEST_ELAPSED": str(elapsed)},
    )
    assert (tmp_path / "target/conformance/507-report.json").exists()
    assert result.returncode == expected, result.stdout + result.stderr
    assert "runtime" in result.stdout + result.stderr
