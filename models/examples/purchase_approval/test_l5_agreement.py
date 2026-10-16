"""Determinism and cross-path agreement: C02 (repeated lowering), C15 (repeated execution), C16a
(authoring conformance: Python and the Rust model authority), C16b (execution conformance: the
Python engine interface and the released Core CLI in a separate process) and C18 (a referenced
declaration changes exactly the identities that reference it). Contract comparison.md."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from behavior import evaluate, invoke

from models.examples.purchase_approval import harness, workflow
from models.examples.purchase_approval.report import Unsupported, canonical, report_dir


@pytest.mark.scenario("C02", level=1)
def test_c02_repeated_lowering_gives_identical_identities() -> None:
    first = harness.authoring_identity(workflow.module())
    second = harness.authoring_identity(workflow.module())
    assert canonical(first) == canonical(second)


@pytest.mark.scenario("C16a", level=1)
def test_c16a_python_and_rust_authoring_give_one_module() -> None:
    d = report_dir()
    d.mkdir(parents=True, exist_ok=True)
    python = harness.authoring_identity(workflow.module())
    (d / "c16a-python.json").write_text(canonical(python) + "\n")
    failed = d / "c16a-rust.failed"
    if failed.exists():
        raise AssertionError(f"the Rust half failed:\n{failed.read_text()[-2000:]}")
    rust_path = d / "c16a-rust.json"
    if not rust_path.exists():
        raise Unsupported("the Rust half was not run (scripts/conformance.sh runs it)",
                          "ecosystem")
    rust = json.loads(rust_path.read_text())
    differing = sorted(k for k in set(python["items"]) | set(rust["items"])
                       if python["items"].get(k) != rust["items"].get(k))
    assert not differing, differing
    assert canonical(python) == canonical(rust)


@pytest.mark.scenario("C18", level=1)
def test_c18_a_referenced_declaration_change_moves_exactly_its_dependents() -> None:
    before = harness.authoring_identity(workflow.module())["models"]
    after = harness.authoring_identity(harness.variant_module(*harness.ROLE_CHANGE))["models"]
    want = harness.EXPECTED["scenarios"]["C18"]
    changed = sorted(m for m in before if before[m]["model_hash"] != after[m]["model_hash"])
    assert changed == sorted(want["changed"])
    assert all(before[m]["model_hash"] == after[m]["model_hash"] for m in want["unchanged"])


# --- C16b: execution conformance --------------------------------------------------------------

def _cli(*args: str) -> tuple[int, str, str]:
    """The pinned Core Release's CLI in a separate process (checked by `pinned_engine_info`)."""
    p = subprocess.run([str(harness.pinned_cli()), *args], capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr


@pytest.fixture(scope="module", autouse=True)
def _pinned_cli() -> None:
    harness.pinned_engine_info()


def _normalize(stdout: str, stderr: str) -> Any:
    """`N_E`: the canonical record, or the refusal's error code (comparison.md §2)."""
    try:
        return canonical(json.loads(stdout))
    except json.JSONDecodeError:
        return {"error": stderr.strip().split(":", 1)[0]}


def _files(tmp: Path, **docs: str) -> list[str]:
    paths = []
    for name, text in docs.items():
        p = tmp / f"{name}.json"
        p.write_text(text)
        paths.append(str(p))
    return paths


@pytest.mark.scenario("C16b", level=2, check="C16b.invoke")
def test_c16b_recorded_invocations_agree_with_the_cli(history, tmp_path: Path) -> None:  # type: ignore[no-untyped-def]
    """Steps 4 (ALLOW), 6 (DENY) and 10 (ALLOW with an intent): the same wire, invocation
    document and complete snapshot through Python `invoke` and `behavior invoke`."""
    (wire,) = _files(tmp_path, wire=history.model.to_wire_json())
    for step, entry in sorted(history.records.items()):
        doc, snap = _files(tmp_path, invocation=entry["document"], snapshot=entry["snapshot"])
        python = invoke(history.model, entry["document"], entry["snapshot"])
        code, out, err = _cli("invoke", wire, doc, snap)
        assert _normalize(out, err) == canonical(json.loads(python.json)), (step, code, err)


def _request(store: Any, model: Any, op: harness.Op, drop: str | None = None) -> dict[str, Any]:
    """The evaluation request of `op` against the store's complete parent snapshot."""
    snap = json.loads(harness.invocation_snapshot(store, model, op))
    facts = snap["facts"]
    if drop is not None:
        facts["universe"] = [u for u in facts["universe"] if u["entity"] != drop]
    state = {name: store.load(harness._param_entities(model, op.capability)[name], ident)["value"]
             for name, ident in op.bindings.items()}
    return {"action": op.capability, "state": state, "input": op.input, "context": op.context,
            "data_version": snap["data_version"], "facts": facts}


@pytest.mark.scenario("C16b", level=2, check="C16b.eval")
def test_c16b_evaluations_agree_with_the_cli(prefix: Any, tmp_path: Path) -> None:
    """Step 4 (ALLOW), step 5 (DENY: a second decision by one approver), step 6 (DENY: not
    eligible), an incomplete snapshot (C09) and a missing input (C09.input), each through
    Python `evaluate` and `behavior eval`."""
    model, store = prefix("4")
    (wire,) = _files(tmp_path, wire=model.to_wire_json())
    o4 = harness.op("4")
    cases = {
        "allow": _request(store, model, o4),
        "incomplete_snapshot": _request(store, model, o4, drop="User"),
        "missing_input": {**_request(store, model, o4), "input": {}},
    }
    for step, name in (("5", "deny_duplicate"), ("6", "deny_not_eligible")):
        _, at = prefix(step)
        cases[name] = _request(at, model, harness.op(step))
    for name, req in cases.items():
        python = evaluate(model, req["action"], state=req["state"], input=req["input"],
                          context=req["context"], data_version=req["data_version"],
                          facts=req["facts"])
        # The same bytes the binding sends: canonical JSON (a refused request is recorded
        # verbatim, so whitespace would differ otherwise).
        (path,) = _files(tmp_path, request=canonical(req))
        code, out, err = _cli("eval", wire, path)
        assert _normalize(out, err) == canonical(json.loads(python.record_json)), (name, code, err)


@pytest.mark.scenario("C15", level=4)
def test_c15_repeated_execution_gives_identical_results(history) -> None:  # type: ignore[no-untyped-def]
    """The session's walk of H and a fresh one give the same results, positions, state
    identities, history records and command stream."""
    def fingerprint(model: Any, store: Any, results: list[Any]) -> tuple[Any, ...]:
        p = store.current().position
        return (store.store_id,
                [(r.step, r.result, r.conflict, r.position) for r in results],
                [store.state_at(i).state for i in range(p + 1)],
                [store.history_at(i).record for i in range(p + 1)],
                harness.commands(store, model))

    model = workflow.module()
    store, results = harness.run_history(model)
    assert fingerprint(model, store, results) == \
        fingerprint(history.model, history.store, history.results)
