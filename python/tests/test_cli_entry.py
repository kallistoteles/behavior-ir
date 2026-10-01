"""Feature 008: the `behavior` console script is the engine's own CLI, and a missing solver is
reported by name without affecting anything but verification (research R5, R6)."""

from __future__ import annotations

import os
import subprocess
import sys
from decimal import Decimal
from pathlib import Path

import pytest

from behavior import BehaviorError, evaluate, verify
from examples.orders.behavior import model

ROOT = Path(__file__).resolve().parents[2]
ORDERS = str(ROOT / "tests" / "fixtures" / "wire" / "valid" / "orders.json")
PREREQUISITE = "verification needs the Z3 SMT solver (supported: 4.16.0)"


def _run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, env=env)


def test_the_console_script_is_the_engine_cli() -> None:
    py = _run([sys.executable, "-m", "behavior._cli", "admit", ORDERS])
    rust = _run([str(ROOT / "target" / "debug" / "behavior"), "admit", ORDERS])
    assert py.returncode == rust.returncode == 0
    assert py.stdout == rust.stdout
    bad = _run([sys.executable, "-m", "behavior._cli", "no-such-command"])
    assert bad.returncode == 64


def test_a_missing_solver_is_named_and_only_verification_needs_it(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("BEHAVIOR_Z3", "/nonexistent/z3")
    with pytest.raises(BehaviorError, match=r"verification needs the Z3 SMT solver"):
        verify(model)
    d = evaluate(model, "check_orders",
                 state={"customer": {"id": "c1", "name": "Ada", "credit_limit": Decimal("100.00"),
                                     "region": "north"}},
                 data_version="1", facts={"universe": [{"entity": "Order", "members": []}]})
    assert d.result == "ALLOW"
    env = dict(os.environ, BEHAVIOR_Z3="/nonexistent/z3")
    cli = _run([sys.executable, "-m", "behavior._cli", "verify", ORDERS], env=env)
    assert cli.returncode == 3
    assert PREREQUISITE in cli.stderr
