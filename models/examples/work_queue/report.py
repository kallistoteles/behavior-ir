"""The conformance report of feature 507 (research R9, R10): its declared scenarios, readiness
registry and verification obligations, on the report machinery shared with feature 506
(`models.conformance.report`). Feature 506's scenarios and completion never depend on these."""

from __future__ import annotations

import sys
from typing import Any

from models.conformance.report import *  # noqa: F401,F403 - the report hooks and outcomes
from models.conformance.report import configure, main

#: The acceptance scenarios of spec.md (W23 split into its recovery and re-evaluation cases).
DECLARED = tuple(f"W{n:02d}" for n in range(1, 23)) + ("W23a", "W23b", "W24", "W25")

#: Open upstream issues affecting 507 scenarios. None is known; a Core limitation found by the
#: verifier is added here with `"run": True` and its exact result (FR-035).
READINESS: list[dict[str, Any]] = []

TIME = "now is the trusted evaluation time; its freshness at commit and clock monotonicity are host assumptions"
ORDINARY = "ordinary transitions only: migrations follow the host migration contract (rules 1-7)"
BACKEND = "a conforming Store backend"
IMPORT = "an item imported at genesis is trusted state with no recorded claim"
SMT_MODEL = ("SMT results hold within the pinned verifier's modeled fragment of Core's validity and "
             "evaluation semantics; they are local to each action")


def _deps(*pairs: tuple[str, str]) -> list[dict[str, str]]:
    return [{"class": c, "check": k} for c, k in pairs]


#: VO-001..VO-012 with every evidence dependency (spec, Verification obligations; research
#: R10). Check ids are the scenario checks and the named checks of test_obligations.py and
#: test_negative_controls.py.
OBLIGATIONS: dict[str, Any] = {
    "VO-001": {"dependencies": _deps(("contract", "SA.genesis"), ("smt", "SMT.enqueue.preservation")),
               "assumptions": [IMPORT, SMT_MODEL]},
    "VO-002": {"dependencies": _deps(("smt", "SMT.preservation")), "assumptions": [SMT_MODEL]},
    "VO-003": {"dependencies": _deps(("structural", "SA-MONO"), ("smt", "SMT.postcondition"),
                                     ("test", "PROP.mono"), ("test", "NC-writer")),
               "assumptions": [ORDINARY, SMT_MODEL]},
    "VO-004": {"dependencies": _deps(("smt", "SMT.preservation"), ("smt", "SMT.claim.evaluation_error")),
               "assumptions": [SMT_MODEL]},
    "VO-005": {"dependencies": _deps(("structural", "SA-FENCE"), ("smt", "SMT.postcondition"),
                                     ("test", "W07"), ("test", "NC-generation"),
                                     ("test", "NC-writer"), ("test", "NC-expiry")),
               "assumptions": [ORDINARY, TIME, SMT_MODEL]},
    "VO-006": {"dependencies": _deps(("smt", "SMT.preservation")), "assumptions": [SMT_MODEL]},
    "VO-007": {"dependencies": _deps(("structural", "SA-TERM"), ("structural", "SA-OWN"),
                                     ("test", "W15.terminal"), ("test", "NC-writer")),
               "assumptions": [ORDINARY]},
    "VO-008": {"dependencies": _deps(("contract", "SA-EXCL"), ("contract", "BACKEND"), ("test", "W04")),
               "assumptions": [BACKEND]},
    # Atomic publication is the Store's contract (a refused commit publishes nothing); the
    # evaluation-without-execution check is the scenario test.
    "VO-009": {"dependencies": _deps(("contract", "W24.no_partial"), ("test", "W24"))},
    "VO-010": {"dependencies": _deps(("contract", "W17"), ("test", "W18"))},
    "VO-011": {"dependencies": _deps(("structural", "SA-OWN"), ("test", "NC-writer"))},
    "VO-012": {"dependencies": _deps(("test", "ASSUMPTIONS")),
               "assumptions": [TIME, ORDINARY, BACKEND, IMPORT, SMT_MODEL]},
}

configure(report="507-report.json", declared=DECLARED, readiness=READINESS,
          obligations=OBLIGATIONS)


if __name__ == "__main__":  # python -m models.examples.work_queue.report REPORT MODE
    sys.exit(main(sys.argv))
