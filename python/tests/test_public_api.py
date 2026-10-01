"""Feature 008: the public API manifest equals the package, the CLI and the backend contract
(contracts/public-api.md, research R7)."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

import behavior

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = json.loads((ROOT / "api" / "public-api.json").read_text())


def test_the_manifest_is_for_this_release() -> None:
    assert MANIFEST["format"] == "behavior.public_api.v1"
    assert MANIFEST["release"] == behavior.__version__


def test_python_names_equal_the_package_exports() -> None:
    assert set(behavior.__all__) == set(MANIFEST["python"]["names"])


def _cli_help(*args: str) -> str:
    return subprocess.run(
        [sys.executable, "-m", "behavior._cli", *args, "--help"], capture_output=True, text=True,
        check=True,
    ).stdout


def test_cli_commands_and_flags_equal_the_manifest() -> None:
    top = _cli_help()
    commands = re.findall(r"^  ([a-z][a-z-]*)\s", top.split("Commands:")[1].split("Options:")[0],
                          re.M)
    commands = [c for c in commands if c != "help"]
    assert set(commands) == set(MANIFEST["cli"])
    for command, flags in MANIFEST["cli"].items():
        text = _cli_help(command)
        found = set(re.findall(r"(--[a-z][a-z-]*)", text)) - {"--help"}
        assert found == {f for f in flags if f.startswith("--")}, command


def test_backend_methods_equal_the_manifest() -> None:
    py = (ROOT / "crates" / "behavior-py" / "src" / "lib.rs").read_text()
    called = set(re.findall(r'self\.call(?:::<[^>]*>)?\(\s*"([a-z_]+)"', py))
    guarded = set(re.findall(r'hasattr\("([a-z_]+)"\)', py))
    trait = (ROOT / "crates" / "behavior-store" / "src" / "lib.rs").read_text()
    body = trait.split("pub trait Backend")[1].split("\n}\n")[0]
    declared = set(re.findall(r"fn ([a-z_]+)\(", body))
    assert called | guarded == declared
    assert set(MANIFEST["backend"]["required"]) == called - guarded
    assert set(MANIFEST["backend"]["optional"]) == guarded
