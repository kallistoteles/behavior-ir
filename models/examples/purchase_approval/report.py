"""The conformance report of feature 506 (contract `conformance-report.md`): its declared
scenarios and readiness registry. The report machinery (statuses, outcomes, hooks, the canonical
writer and the gate) is shared with feature 507 in `models.conformance.report`."""

from __future__ import annotations

import sys
from typing import Any

from models.conformance.report import *  # noqa: F401,F403 - the report hooks and outcomes
from models.conformance.report import configure, main

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

configure(report="506-report.json", declared=DECLARED, readiness=READINESS)


if __name__ == "__main__":  # python -m models.examples.purchase_approval.report REPORT MODE
    sys.exit(main(sys.argv))
