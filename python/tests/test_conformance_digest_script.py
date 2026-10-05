"""Behavioral coverage for the existing digest; not historical test-first evidence."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def _tree(tmp_path: Path, *, failing_cli: bool = False) -> Path:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    shutil.copy2(ROOT / "scripts/conformance-digest.sh", scripts)
    (tmp_path / "core-release.json").write_text('{"version":"test"}')
    core = tmp_path / ".core/test"
    (core / "tests/fixtures").mkdir(parents=True)
    (core / "schema").mkdir()
    (core / "tests/fixtures/z.json").write_bytes(b"z\n")
    (core / "schema/a.json").write_bytes(b"a\n")
    check = scripts / "determinism-check.sh"
    check.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + (
        "echo 'CLI failed: probe' >&2\nexit 19\n" if failing_cli else
        "printf 'ecosystem\\tprobe\\tabc\\n' >\"$BEHAVIOR_DIGEST_DIR/outputs.tsv\"\n"
    ))
    check.chmod(0o755)
    return scripts / "conformance-digest.sh"


def test_digest_is_canonical_and_changes_with_input(tmp_path: Path) -> None:
    script = _tree(tmp_path)
    first = subprocess.check_output(["bash", str(script)])
    expected = {
        "file:schema/a.json": hashlib.sha256(b"a\n").hexdigest(),
        "file:tests/fixtures/z.json": hashlib.sha256(b"z\n").hexdigest(),
        "output:ecosystem:probe": "abc",
    }
    assert first == (json.dumps(expected, sort_keys=True, separators=(",", ":")) + "\n").encode()
    assert subprocess.check_output(["bash", str(script)]) == first
    (tmp_path / ".core/test/tests/fixtures/z.json").write_bytes(b"changed\n")
    changed = json.loads(subprocess.check_output(["bash", str(script)]))
    assert changed["file:tests/fixtures/z.json"] == hashlib.sha256(b"changed\n").hexdigest()
    assert changed != expected


def test_digest_propagates_cli_failure_without_emitting_a_digest(tmp_path: Path) -> None:
    script = _tree(tmp_path, failing_cli=True)
    result = subprocess.run(["bash", str(script)], text=True, capture_output=True)
    assert result.returncode != 0
    assert "CLI failed: probe" in result.stderr
    assert result.stdout == ""
