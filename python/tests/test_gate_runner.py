"""Exercise the shared runner in a disposable tree with controlled gate outcomes."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def _run(tmp_path: Path, *, fail: str = "", pytest_mode: str = "stub") -> subprocess.CompletedProcess[str]:
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    shutil.copy2(ROOT / "scripts/gates.sh", scripts / "gates.sh")
    (scripts / "tests").mkdir()
    (tmp_path / "bin").mkdir()
    wrapper = f"#!{sys.executable}\n" + '''\
import os, pathlib, subprocess, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
label = " ".join([name, *args])
with open(os.environ["GATE_TRACE"], "a") as trace:
    trace.write(label + "\\n")
if os.environ.get("FAIL_GATE") == name:
    sys.exit(17)
if name == "python" and args[:2] == ["-m", "pytest"]:
    mode = os.environ["PYTEST_MODE"]
    if mode != "stub":
        if mode == "collect":
            args.append("--collect-only")
        sys.exit(subprocess.run([sys.executable, *args]).returncode)
'''
    for name in ("check-core-pin.sh", "check-public-surface.sh", "fetch-core.sh",
                 "determinism-check.sh", "check-terms.sh", "check-workflows.sh"):
        path = scripts / name
        path.write_text(wrapper)
        path.chmod(0o755)
    for name in ("cargo", "maturin", "python", "python3", "mypy"):
        path = tmp_path / "bin" / name
        path.write_text(wrapper)
        path.chmod(0o755)
    test_script = scripts / "tests/test_stub.sh"
    test_script.write_text(wrapper)
    test_script.chmod(0o755)
    return subprocess.run(
        ["bash", str(scripts / "gates.sh")], cwd=tmp_path, text=True, capture_output=True,
        env={**os.environ, "PATH": f"{tmp_path / 'bin'}:{os.environ['PATH']}",
             "GATE_TRACE": str(tmp_path / "trace"), "FAIL_GATE": fail,
             "PYTEST_MODE": pytest_mode},
    )


def _model_tree(tmp_path: Path) -> None:
    shutil.copytree(ROOT / "models", tmp_path / "models", ignore=shutil.ignore_patterns("__pycache__"))
    tests = tmp_path / "python/tests"
    tests.mkdir(parents=True)
    (tests / "test_binding.py").write_text("def test_binding():\n    assert True\n")


def test_gate_order_is_deterministic(tmp_path: Path) -> None:
    result = _run(tmp_path)
    assert result.returncode == 0, result.stderr
    assert (tmp_path / "trace").read_text().splitlines() == [
        "check-core-pin.sh", "check-public-surface.sh", "fetch-core.sh",
        "cargo fmt --all --check", "cargo clippy -q --workspace --all-targets -- -D warnings",
        "maturin develop -q", "python -m pytest -q python/tests models", "mypy",
        "determinism-check.sh", "python3 scripts/check-ownership.py", "check-terms.sh",
        "check-workflows.sh", "test_stub.sh",
    ]


def test_a_failed_gate_names_the_failure_and_stops(tmp_path: Path) -> None:
    result = _run(tmp_path, fail="maturin")
    assert result.returncode != 0
    assert "gates: FAILED maturin develop" in result.stderr
    trace = (tmp_path / "trace").read_text()
    assert "python -m pytest" not in trace and "mypy" not in trace


def test_shared_gate_collects_the_actual_model_tests(tmp_path: Path) -> None:
    _model_tree(tmp_path)
    result = _run(tmp_path, pytest_mode="collect")
    assert result.returncode == 0, result.stdout + result.stderr
    for name in ("test_a_transition_lowers_to_requires_set_ensures",
                 "test_the_core_admits_it_without_knowing_the_model",
                 "test_lowering_is_deterministic"):
        assert name in result.stdout


def test_broken_lowering_fails_the_shared_gate(tmp_path: Path) -> None:
    _model_tree(tmp_path)
    lowering = tmp_path / "models/examples/state_machine/lower.py"
    lowering.write_text('def lower(machine):\n    raise AssertionError("broken lowering probe")\n')
    result = _run(tmp_path, pytest_mode="run")
    assert result.returncode != 0, result.stdout + result.stderr
    assert "broken lowering probe" in result.stdout
    assert "gates: FAILED pytest" in result.stderr
    assert "mypy" not in (tmp_path / "trace").read_text()
