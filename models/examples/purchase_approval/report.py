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

#: Open upstream issues and the scenarios they block (research R10). An entry with `"run": true`
#: does not stop the check from running: the check raises `Blocked` with its exact result, and
#: any other result keeps its own status. Empty since Core 0.15.0 (feature 508): #29 and #31 were
#: fixed in 0.14.0 and #35 (C19) in 0.15.0.
READINESS: list[dict[str, Any]] = []

configure(report="506-report.json", declared=DECLARED, readiness=READINESS)


if __name__ == "__main__":  # python -m models.examples.purchase_approval.report REPORT MODE
    sys.exit(main(sys.argv))
