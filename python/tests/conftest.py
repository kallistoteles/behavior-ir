"""Shared pytest configuration."""

from __future__ import annotations

import json
import os
import pathlib

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
CORE_VERSION: str = json.loads((REPO_ROOT / "core-release.json").read_text())["version"]
# The pinned Core Release's schemas and conformance fixtures (feature 011, FR-024), installed by
# scripts/fetch-core.sh. They are never copied into this repository.
CORE_DIR = pathlib.Path(os.environ.get("BEHAVIOR_CORE_DIR", REPO_ROOT / ".core" / CORE_VERSION))
if not (CORE_DIR / "tests" / "fixtures").is_dir():
    raise RuntimeError(f"the core's conformance fixtures are missing at {CORE_DIR}; "
                       "run scripts/fetch-core.sh")
FIXTURES = CORE_DIR / "tests" / "fixtures"
