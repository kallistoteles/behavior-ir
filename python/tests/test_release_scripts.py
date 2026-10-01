"""Feature 008: the release scripts refuse what they must and name the step that failed.

They run on a throwaway repository built from the current working tree, so the real tree and
its tags are never touched. This is slow (it builds a wheel and installs it into fresh
environments), so it runs only with BEHAVIOR_RELEASE_TESTS=1. `scripts/release-check.sh` sets
that for its gates step.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

pytestmark = pytest.mark.skipif(
    os.environ.get("BEHAVIOR_RELEASE_TESTS") != "1", reason="slow; set BEHAVIOR_RELEASE_TESTS=1"
)


def _git(cwd: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True,
                          text=True).stdout


@pytest.fixture(scope="module")
def repo(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """A committed copy of the working tree (tracked and untracked, non-ignored files)."""
    dst = tmp_path_factory.mktemp("release-repo")
    files = _git(ROOT, "ls-files", "--cached", "--others", "--exclude-standard").splitlines()
    for f in files:
        src = ROOT / f
        if src.is_file():
            (dst / f).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst / f)
    _git(dst, "init", "-q")
    _git(dst, "add", "-A")
    _git(dst, "-c", "user.name=test", "-c", "user.email=test@example.invalid", "commit", "-qm",
         "snapshot")
    return dst


def _run(repo: Path, *cmd: str) -> subprocess.CompletedProcess[str]:
    # Share the build cache with the real tree so the wheel builds incrementally.
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"))
    return subprocess.run(list(cmd), cwd=repo, capture_output=True, text=True, env=env)


def test_a_dirty_tree_is_refused_and_not_tagged(repo: Path) -> None:
    stray = repo / "stray.txt"
    stray.write_text("uncommitted\n")
    try:
        r = _run(repo, "scripts/release.sh", "0.8.0")
    finally:
        stray.unlink()
    assert r.returncode != 0
    assert "clean" in r.stderr, r.stderr
    assert _git(repo, "tag", "-l") == ""


def test_a_version_other_than_the_workspace_version_is_refused(repo: Path) -> None:
    r = _run(repo, "scripts/release.sh", "0.9.0")
    assert r.returncode != 0
    assert "0.9.0" in r.stderr and "0.8.0" in r.stderr, r.stderr
    assert _git(repo, "tag", "-l") == ""


@pytest.fixture(scope="module")
def dist(repo: Path) -> Path:
    r = _run(repo, "scripts/release.sh", "--build-only", "0.8.0")
    assert r.returncode == 0, r.stderr
    d = repo / "dist" / "v0.8.0"
    assert (d / "SHA256SUMS").is_file() and (d / "release-manifest.json").is_file()
    return d


def test_a_tampered_checksum_fails_the_install_step(repo: Path, dist: Path, tmp_path: Path) -> None:
    bad = tmp_path / "dist"
    shutil.copytree(dist, bad)
    sums = (bad / "SHA256SUMS").read_text()
    first = sums[0]
    (bad / "SHA256SUMS").write_text(("0" if first != "0" else "1") + sums[1:])
    r = _run(repo, "scripts/release-check.sh", "--skip-gates", str(bad))
    assert r.returncode != 0
    assert "FAILED" in r.stderr and "install" in r.stderr, r.stderr


def test_a_broken_skill_example_fails_its_step_by_name(repo: Path, dist: Path) -> None:
    broken = repo / "skills" / "behavior-authoring" / "examples" / "zz_broken.py"
    broken.parent.mkdir(parents=True, exist_ok=True)
    broken.write_text("import behavior\nassert False, 'deliberately broken'\n")
    try:
        r = _run(repo, "scripts/release-check.sh", "--skip-gates", str(dist))
    finally:
        broken.unlink()
    assert r.returncode != 0
    assert "FAILED" in r.stderr and "skill examples" in r.stderr, r.stderr
    assert "zz_broken.py" in r.stderr, r.stderr
