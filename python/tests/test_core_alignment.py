"""Feature 500: release metadata and the installed core describe the same current contract."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

import behavior

ROOT = Path(__file__).resolve().parents[2]
CURRENT_CORE = "0.12.0"


def test_the_declared_core_release_is_current() -> None:
    pin = json.loads((ROOT / "core-release.json").read_text())
    assert pin["format"] == "behavior.core_pin.v1"
    assert pin["version"] == CURRENT_CORE
    assert pin["tag"] == f"v{CURRENT_CORE}"
    assert pin["repository"] == "https://github.com/kallistoteles/behavior-ir-core"
    assert re.fullmatch(r"[0-9a-f]{40}", pin["commit"])
    expected_assets = {
        "cli": f"behavior-{CURRENT_CORE}-x86_64-linux-musl",
        "conformance": f"behavior-conformance-{CURRENT_CORE}.tar.gz",
    }
    for kind, filename in expected_assets.items():
        assert pin["assets"][kind]["file"] == filename
        assert re.fullmatch(r"[0-9a-f]{64}", pin["assets"][kind]["sha256"])


def test_the_extension_and_dependencies_name_the_exact_declared_core_revision() -> None:
    pin = json.loads((ROOT / "core-release.json").read_text())
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    dependency = manifest["workspace"]["dependencies"]["behavior-engine"]
    assert dependency == {"git": pin["repository"], "rev": pin["commit"]}
    source = f"git+{pin['repository']}?rev={pin['commit']}#{pin['commit']}"
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    core = [p for p in lock["package"] if p["name"] in {
        "behavior-core", "behavior-engine", "behavior-store", "behavior-verify",
    }]
    assert {p["name"] for p in core} == {
        "behavior-core", "behavior-engine", "behavior-store", "behavior-verify",
    }
    for package in core:
        assert package["version"] == CURRENT_CORE, package["name"]
        assert package["source"] == source, package["name"]
    assert behavior.versions()["core"] == {
        "version": CURRENT_CORE, "commit": pin["commit"],
    }


@pytest.mark.parametrize(("key", "expected"), [
    ("engine", CURRENT_CORE),
    ("accepted_wire_ir", ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8"]),
    ("records", ["0.4", "0.5", "0.6", "0.7"]),
    ("read_records", ["behavior.read_record.v1", "behavior.read_record.v2"]),
    ("verifier", "0.8.0"),
    ("command_stream", "behavior.command_stream_request.v1"),
    ("command_occurrence_domain", "behavior.command_occurrence.v1"),
])
def test_binding_and_bundled_cli_report_current_core_formats(key: str, expected: object) -> None:
    output = subprocess.run(
        [sys.executable, "-m", "behavior._cli", "engine-info"],
        check=True, capture_output=True, text=True,
    )
    cli = json.loads(output.stdout)
    assert behavior.versions().get(key) == expected, key
    assert cli.get(key) == expected, key


def test_legacy_authoring_stays_on_its_existing_profile() -> None:
    from examples.ledger.behavior import model

    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == "0.4"
    assert "commands" not in wire
    assert behavior.versions()["wire_ir"] == [
        "0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7",
    ]
