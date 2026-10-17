"""The conformance report shared by the end-to-end suites (features 506 and 507; 506 contract
`conformance-report.md`, 507 research R9, R10).

A suite's `report.py` calls `configure(...)` with its report file, declared scenarios, readiness
registry and (507) verification obligations, and its `conftest.py` star-imports that module so
the hooks below are active.

Every scenario test carries `@pytest.mark.scenario(id, level=n, check=...)`. Its outcome becomes
one report entry with exactly one status:

- `passed`: the test returned normally;
- `failed`: an assertion failed, any other exception, or a plain `pytest.skip`;
- `inconclusive`, `unverifiable`, `unsupported`, `blocked`: the test raised the matching
  `Outcome` below, with a reason and an owning layer, or the scenario is listed under an open
  readiness entry.

Only `passed` counts as passed. A declared scenario with no check in the run is `failed`
("not run"). The report is canonical JSON without timestamps, so equal outcomes give equal bytes.

Obligations (feature 507, research R10): each verification obligation lists its evidence
dependencies `{class, check}` (class `smt`, `structural`, `test` or `contract`) and its
assumptions. Its status is the weakest status of its dependencies, ordered `failed < blocked <
inconclusive < unverifiable < unsupported < passed`; a dependency with no check in the run is
`failed` ("not run"). Assumptions never contribute a status. A report without obligations has no
`obligations` key, so feature 506's report is unchanged.

Test seams (used by `scripts/tests/test_conformance_report.sh`): `BEHAVIOR_CONFORMANCE_DECLARED`
(comma-separated scenario ids), `BEHAVIOR_CONFORMANCE_READINESS` (JSON list) and
`BEHAVIOR_CONFORMANCE_OBLIGATIONS` (JSON object) replace the configured values.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]

STATUSES = ("passed", "failed", "inconclusive", "unverifiable", "unsupported", "blocked")
#: The obligation order, weakest first (research R10).
WEAKEST_FIRST = ("failed", "blocked", "inconclusive", "unverifiable", "unsupported", "passed")
EVIDENCE_CLASSES = ("smt", "structural", "test", "contract")

#: The suite's configuration (see `configure`).
CONFIG: dict[str, Any] = {
    "report": "report.json",
    "declared": (),
    "readiness": [],
    "obligations": {},
}

#: Facts the suite adds to the report (model versions, profiles), set by the suite's conftest.
REPORT_EXTRA: dict[str, Any] = {}


def configure(*, report: str, declared: tuple[str, ...], readiness: list[dict[str, Any]],
              obligations: dict[str, Any] | None = None) -> None:
    CONFIG.update(report=report, declared=declared, readiness=readiness,
                  obligations=obligations or {})


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
    return tuple(x for x in raw.split(",") if x) if raw is not None else tuple(CONFIG["declared"])


def _readiness() -> list[dict[str, Any]]:
    raw = os.environ.get("BEHAVIOR_CONFORMANCE_READINESS")
    return json.loads(raw) if raw is not None else list(CONFIG["readiness"])


def _obligations() -> dict[str, Any]:
    raw = os.environ.get("BEHAVIOR_CONFORMANCE_OBLIGATIONS")
    return json.loads(raw) if raw is not None else dict(CONFIG["obligations"])


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
        "markers", "scenario(id, level, check=None): an end-to-end conformance check")
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


def obligation_entries(checks: dict[str, dict[str, Any]]) -> dict[str, Any]:
    """Each obligation with its dependencies' statuses and its own (the weakest)."""
    out: dict[str, Any] = {}
    for vo, spec in sorted(_obligations().items()):
        deps = []
        for d in spec["dependencies"]:
            if d["class"] not in EVIDENCE_CLASSES:
                status, reason = "failed", f"unknown evidence class {d['class']}"
            else:
                e = checks.get(d["check"])
                status, reason = (("failed", "not run") if e is None
                                  else (e["status"], e["reason"]))
            deps.append({"class": d["class"], "check": d["check"], "status": status,
                         "reason": reason})
        weakest = min((x["status"] for x in deps), key=WEAKEST_FIRST.index, default="failed")
        out[vo] = {"status": weakest, "dependencies": deps,
                   "assumptions": list(spec.get("assumptions", []))}
    return out


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
    obligations = obligation_entries(checks) if _obligations() else None
    report: dict[str, Any] = {
        "format": "behavior.conformance_report.v1",
        # Full conformance: every check passed, with nothing blocked or otherwise open.
        "complete": all(e["status"] == "passed" for e in entries) and all(
            o["status"] == "passed" for o in (obligations or {}).values()),
        "mode": "full" if full_mode() else "quick",
        "core_release": {"version": core["version"], "commit": core["commit"]},
        "engine": _engine_info(),
        "readiness": _readiness(),
        "checks": entries,
        "summary": summary,
        **REPORT_EXTRA,
    }
    if obligations is not None:
        report["obligations"] = obligations
    return report


def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    d = report_dir()
    d.mkdir(parents=True, exist_ok=True)
    (d / CONFIG["report"]).write_text(canonical(build_report()) + "\n")


def gate_failures(report: dict[str, Any], mode: str) -> list[str]:
    """The checks and obligations that fail the gate (506 contract §4): `quick` fails on any
    `failed`; `full` (the integration gate CI runs) on any status other than `passed` or
    `blocked`; `complete` (full conformance) on any status other than `passed`."""
    allowed = {"quick": ("passed", "inconclusive", "unverifiable", "unsupported", "blocked"),
               "full": ("passed", "blocked"),
               "complete": ("passed",)}[mode]
    bad = [f"{e['id']}: {e['status']} ({e['reason']})" for e in report["checks"]
           if e["status"] not in allowed]
    bad += [f"{vo}: {o['status']}" for vo, o in sorted(report.get("obligations", {}).items())
            if o["status"] not in allowed]
    return bad


def main(argv: list[str]) -> int:
    """`python -m <suite report module> REPORT MODE`: prints the summary and gates."""
    doc = json.loads(Path(argv[1]).read_text())
    bad = gate_failures(doc, argv[2])
    print(f"conformance {argv[2]}: " + ", ".join(f"{k} {v}" for k, v in doc["summary"].items()))
    if "obligations" in doc:
        counts: dict[str, int] = {}
        for o in doc["obligations"].values():
            counts[o["status"]] = counts.get(o["status"], 0) + 1
        print("obligations: " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
        # Which kinds of evidence the obligations rest on: a passed VO is never only "proven".
        kinds: dict[str, int] = {}
        for o in doc["obligations"].values():
            for d in o.get("dependencies", []):
                kinds[d["class"]] = kinds.get(d["class"], 0) + 1
        print("evidence: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items()))
              + f"; assumptions named by {sum(1 for o in doc['obligations'].values() if o.get('assumptions'))}"
              f" of {len(doc['obligations'])} obligations")
    print(f"full conformance: {'complete' if doc.get('complete') else 'NOT complete'}")
    for e in doc["checks"]:
        if e["status"] == "blocked":
            print(f"  blocked: {e['id']} by {e.get('owner')}")
    for line in bad:
        print(f"  {line}")
    return 1 if bad else 0
