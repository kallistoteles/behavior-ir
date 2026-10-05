"""Wrong release versions are tested exclusively in disposable repositories."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_wrong_version_stops_before_artifact_construction(tmp_path: Path) -> None:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in ("release.sh", "check-tag.sh"):
        shutil.copy2(ROOT / "scripts" / name, scripts / name)
    for name in ("release-build.sh", "release-check.sh"):
        stub = scripts / name
        stub.write_text("#!/usr/bin/env bash\ntouch artifacts-started\nexit 99\n")
        stub.chmod(0o755)
    (tmp_path / "Cargo.toml").write_text('[workspace.package]\nversion = "0.10.1"\n')
    for args in (["init", "-q", "-b", "main"], ["config", "user.name", "test"],
                 ["config", "user.email", "test@example.invalid"], ["add", "."],
                 ["commit", "-qm", "fixture"], ["tag", "-a", "v0.10.2", "-m", "wrong"]):
        subprocess.run(["git", *args], cwd=tmp_path, check=True, capture_output=True)
    result = subprocess.run(
        ["bash", "scripts/release.sh", "0.10.2"], cwd=tmp_path, text=True, capture_output=True,
        env={k: v for k, v in os.environ.items() if k not in ("GH_TOKEN", "GITHUB_TOKEN")},
    )
    assert result.returncode != 0
    assert "0.10.1" in result.stderr and "0.10.2" in result.stderr
    assert not (tmp_path / "artifacts-started").exists()
    assert subprocess.check_output(["git", "tag", "--list"], cwd=tmp_path, text=True) == "v0.10.2\n"
