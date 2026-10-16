"""The conformance report of feature 506 (contract `conformance-report.md`).

Every scenario test carries `@pytest.mark.scenario(id, level=n, check=...)`. Its outcome becomes
one report entry with exactly one status:

- `passed`: the test returned normally;
- `failed`: an assertion failed, any other exception, or a plain `pytest.skip`;
- `inconclusive`, `unverifiable`, `unsupported`, `blocked`: the test raised the matching
  `Outcome` below, with a reason and an owning layer, or the scenario is listed under an open
  readiness entry.

Only `passed` counts as passed. A declared scenario with no check in the run is `failed`
("not run"). The report is canonical JSON without timestamps, so equal outcomes give equal bytes.

Test seams (used by `scripts/tests/test_conformance_report.sh`): `BEHAVIOR_CONFORMANCE_DECLARED`
(comma-separated scenario ids) and `BEHAVIOR_CONFORMANCE_READINESS` (JSON list) replace the
declared scenarios and the readiness registry.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[3]

STATUSES = ("passed", "failed", "inconclusive", "unverifiable", "unsupported", "blocked")

#: The scenarios of contracts/reference-workflow.md §3; each must have at least one check.
DECLARED = (
    "C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11", "C12",
    "C13a", "C13b", "C14", "C15", "C16a", "C16b", "C17", "C18", "C19",
    "VO-001", "VO-005", "FR-004", "FR-009", "FR-018",
)

#: Open upstream issues and the scenarios they block (research R10). An empty `affects` is the
#: analysed result, recorded so that the report shows it. An entry with `"run": true` does not
#: stop the check from running: the check itself raises `Blocked` with its exact result when the
#: result is the documented limitation, and any other result keeps its own status.
READINESS: list[dict[str, Any]] = [
    {"issue": "behavior-ir-core#35", "affects": ["C19"], "run": True,
     "reason": "the verifier cannot discharge Approval's inductive preservation argument: the "
               "uniqueness postcondition of the decision actions is inconclusive "
               "(counterexample_not_reproduced)"},
    {"issue": "behavior-ir-core#29", "affects": [],
     "reason": "506 runs run_conformance, not Core's binding conformance fixtures"},
    {"issue": "behavior-ir-core#31", "affects": [],
     "reason": "506 compares identities and records, which carry no source locations"},
]

#: Facts the suite adds to the report (model versions, profiles), set by the suite's conftest.
REPORT_EXTRA: dict[str, Any] = {}


class Outcome(Exception):
    """A check that ends without a pass or a failure: its status, reason and owning layer."""

    status = "failed"

    def __init__(self, reason: str, owner: str) -> None:
        super().__init__(reason)
        self.reason = reason
        self.owner = owner


class Inconclusive(Outcome):
    status = "inconclusive"


class Unverifiable(Outcome):
    status = "unverifiable"


class Unsupported(Outcome):
    status = "unsupported"


class Blocked(Outcome):
    status = "blocked"


def full_mode() -> bool:
    return os.environ.get("BEHAVIOR_FULL_PROPERTIES") == "1"


def _declared() -> tuple[str, ...]:
    raw = os.environ.get("BEHAVIOR_CONFORMANCE_DECLARED")
    return tuple(x for x in raw.split(",") if x) if raw is not None else DECLARED


def _readiness() -> list[dict[str, Any]]:
    raw = os.environ.get("BEHAVIOR_CONFORMANCE_READINESS")
    return json.loads(raw) if raw is not None else READINESS


def _blocking(scenario: str) -> dict[str, Any] | None:
    return next((r for r in _readiness() if scenario in r["affects"]), None)


def readiness_entry(scenario: str) -> dict[str, Any] | None:
    """The open readiness entry affecting `scenario`, if any (for checks that run anyway)."""
    return _blocking(scenario)


def report_dir() -> Path:
    return Path(os.environ.get("BEHAVIOR_CONFORMANCE_DIR", ROOT / "target" / "conformance"))


_ENTRIES: dict[str, dict[str, Any]] = {}
_EVIDENCE: dict[str, Any] = {}


def record_evidence(check: str, evidence: Any) -> None:
    """Attaches deterministic evidence to check `check`'s report entry, whatever its status."""
    _EVIDENCE[check] = evidence


def pytest_configure(config: pytest.Config) -> None:
    config.addinivalue_line(
        "markers", "scenario(id, level, check=None): a feature-506 conformance check")
    _ENTRIES.clear()
    _EVIDENCE.clear()


def pytest_runtest_setup(item: pytest.Item) -> None:
    marker = item.get_closest_marker("scenario")
    if marker is not None:
        entry = _blocking(marker.args[0])
        if entry is not None and not entry.get("run"):
            raise Blocked(entry["reason"], entry["issue"])


def _entry(marker: Any, status: str, reason: str | None, owner: str | None) -> dict[str, Any]:
    scenario = marker.args[0]
    if status not in ("passed", "failed") and not owner:
        status, reason, owner = "failed", f"status {status} without an owner: {reason}", None
    check = marker.kwargs.get("check") or scenario
    return {"id": check, "scenario": scenario, "level": marker.kwargs["level"],
            "status": status, "reason": reason, "owner": owner,
            "evidence": _EVIDENCE.get(check)}


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item: pytest.Item, call: pytest.CallInfo[None]) -> Any:
    outcome = yield
    rep = outcome.get_result()
    marker = item.get_closest_marker("scenario")
    if marker is None:
        return
    exc = call.excinfo.value if call.excinfo is not None else None
    if rep.when == "setup" and not isinstance(exc, Outcome) and not rep.failed \
            and not rep.skipped:
        return
    if rep.when == "teardown" and rep.passed:
        return
    if isinstance(exc, Outcome):
        e = _entry(marker, exc.status, exc.reason, exc.owner)
        # Not a pytest failure: the report decides (scripts/conformance.sh).
        rep.outcome = "skipped"
        rep.longrepr = (str(item.path), item.location[1] or 0, f"{e['status']}: {exc.reason}")
    elif rep.skipped:
        e = _entry(marker, "failed", "skipped with pytest.skip; use an Outcome", None)
        rep.outcome = "failed"
    elif rep.failed:
        e = _entry(marker, "failed", str(exc) if exc is not None else "failed", None)
    else:
        e = _entry(marker, "passed", None, None)
    _ENTRIES[e["id"]] = e


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _engine_info() -> Any:
    """`engine-info` of the pinned bundled CLI (never the BEHAVIOR_CLI_BINARY override)."""
    try:
        out = subprocess.run([str(ROOT / "python" / "behavior" / "_bin" / "behavior"),
                              "engine-info"], capture_output=True, text=True, check=True)
        return json.loads(out.stdout)
    except Exception as e:  # noqa: BLE001 - recorded, never hidden
        return {"error": str(e)}


def build_report() -> dict[str, Any]:
    checks = dict(_ENTRIES)
    seen = {e["scenario"] for e in checks.values()}
    for scenario in _declared():
        if scenario not in seen:
            checks[scenario] = {"id": scenario, "scenario": scenario, "level": None,
                                "status": "failed", "reason": "not run", "owner": None,
                                "evidence": None}
    entries = [checks[k] for k in sorted(checks)]
    summary = {s: sum(1 for e in entries if e["status"] == s) for s in STATUSES}
    core = json.loads((ROOT / "core-release.json").read_text())
    return {
        "format": "behavior.conformance_report.v1",
        # Full conformance: every check passed, with nothing blocked or otherwise open.
        "complete": all(e["status"] == "passed" for e in entries),
        "mode": "full" if full_mode() else "quick",
        "core_release": {"version": core["version"], "commit": core["commit"]},
        "engine": _engine_info(),
        "readiness": _readiness(),
        "checks": entries,
        "summary": summary,
        **REPORT_EXTRA,
    }


def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    d = report_dir()
    d.mkdir(parents=True, exist_ok=True)
    (d / "506-report.json").write_text(canonical(build_report()) + "\n")


def gate_failures(report: dict[str, Any], mode: str) -> list[str]:
    """The checks that fail the gate (contract §4): `quick` fails on any `failed`; `full` (the
    integration gate CI runs) on any status other than `passed` or `blocked`; `complete` (full
    conformance) on any status other than `passed`."""
    allowed = {"quick": ("passed", "inconclusive", "unverifiable", "unsupported", "blocked"),
               "full": ("passed", "blocked"),
               "complete": ("passed",)}[mode]
    return [f"{e['id']}: {e['status']} ({e['reason']})" for e in report["checks"]
            if e["status"] not in allowed]


if __name__ == "__main__":  # python -m models.examples.purchase_approval.report REPORT MODE
    import sys

    doc = json.loads(Path(sys.argv[1]).read_text())
    bad = gate_failures(doc, sys.argv[2])
    print(f"conformance {sys.argv[2]}: " + ", ".join(f"{k} {v}" for k, v in doc["summary"].items()))
    print(f"full conformance: {'complete' if doc.get('complete') else 'NOT complete'}")
    for e in doc["checks"]:
        if e["status"] == "blocked":
            print(f"  blocked: {e['id']} by {e.get('owner')}")
    for line in bad:
        print(f"  {line}")
    sys.exit(1 if bad else 0)
