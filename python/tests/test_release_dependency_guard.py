"""Release entrypoints reject a noncanonical graph before doing artifact work."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.mark.parametrize(("script", "args"), [
    ("release-build.sh", ["0.12.0", "dist"]),
    ("release-check.sh", ["--skip-gates", "dist"]),
    ("release-verify.sh", ["v0.12.0"]),
])
def test_release_entrypoint_checks_pin_first(tmp_path: Path, script: str, args: list[str]) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    shutil.copy2(ROOT / "scripts" / script, scripts / script)
    guard = scripts / "check-core-pin.sh"
    guard.write_text("#!/usr/bin/env bash\necho 'core override active: test refusal' >&2\nexit 23\n")
    guard.chmod(0o755)
    binaries = tmp_path / "bin"
    binaries.mkdir()
    # Any external release operation before the guard is a test failure.
    for name in ("git", "gh", "maturin", "auditwheel", "python3", "mktemp"):
        stub = binaries / name
        stub.write_text("#!/usr/bin/env bash\necho premature >>\"$RELEASE_TRACE\"\nexit 99\n")
        stub.chmod(0o755)
    result = subprocess.run(
        ["bash", str(scripts / script), *args], text=True, capture_output=True,
        env={**os.environ, "PATH": f"{binaries}:{os.environ['PATH']}",
             "RELEASE_TRACE": str(tmp_path / "trace")},
    )
    assert result.returncode == 23, result.stderr
    assert "core override active" in result.stderr
    assert not (tmp_path / "trace").exists()
