"""US5: Python and Rust authoring give the same admitted module and identities (W20)."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

import pytest

from models.examples.work_queue import workflow
from models.examples.work_queue.report import report_dir


def python_identity(model: Any) -> dict[str, Any]:
    """The same authoring identity the Rust half writes (no source locations)."""
    admission = model.engine.admission()
    models, generated = {}, {}
    for r in model.model_receipts:
        key = next(r.model[k] for k in ("model_id", "flow_id", "approval_id", "queue_id")
                   if k in r.model)
        models[key] = {"kind": r.model_kind, "model_semantic_version": r.model_semantic_version,
                       "lowerer_version": r.lowerer_version, "model_hash": r.model_hash}
        if r.generated is not None:
            generated[key] = r.generated
    return {"behavior_hash": model.behavior_version, "items": admission["items"],
            "models": models, "generated": generated}


@pytest.mark.scenario("W20", level=1)
def test_python_and_rust_author_the_same_module() -> None:
    path = Path(os.environ.get("BEHAVIOR_CONFORMANCE_DIR", report_dir())) / "w20-rust.json"
    failed = path.with_name("w20-rust.failed")
    assert not failed.exists(), failed.read_text()[-2000:]
    if not path.exists():
        pytest.fail(f"{path} is missing: run scripts/conformance.sh 507")
    rust = json.loads(path.read_text())
    python = python_identity(workflow.module())
    assert python["models"] == rust["models"]
    assert python["generated"] == rust["generated"]
    assert python["items"] == rust["items"]
    assert python["behavior_hash"] == rust["behavior_hash"]
