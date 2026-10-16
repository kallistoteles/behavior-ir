"""The executable reference application of feature 506 (FR-001, FR-020).

    python -m models.examples.purchase_approval.run

It authors and admits the workflow, creates a store from a checked initial state, runs
reference history H (contracts/reference-workflow.md §1), and prints each step with its expected
and actual outcome. It then prints the final state and the committed command stream, replays the
history twice (data replay and behavior replay), and exits non-zero on any difference.
"""

from __future__ import annotations

import sys

from behavior import replay_behavior, replay_data

from models.examples.purchase_approval import harness, workflow


def main() -> int:
    model = workflow.module()
    print(f"admitted: {model.behavior_version}")
    problems: list[str] = []

    def check(store: object) -> None:
        rows = harness.decision_rows(store, model)  # type: ignore[arg-type]
        if not (harness.unique_per_actor(rows) and harness.thresholds_exclusive(rows)):
            problems.append(f"invariant violated: {rows}")

    store, results = harness.run_history(model, on_commit=check)
    for r in results:
        want = harness.EXPECTED["steps"][r.step]
        if r.kind == "commit":  # the commit of a kept candidate: its refusal and position
            actual = f"commit {'refused ' + r.conflict if r.conflict else 'ok'}"
            ok = r.conflict == want.get("conflict") and r.position == want["position"]
        elif r.kind == "eval":  # a kept candidate: its decision; its commit is checked above
            actual = f"{r.result} (kept)"
            ok = r.result == want["result"]
        else:  # evaluated and, when allowed, committed: result, commit outcome and position
            actual = f"{r.result}{' commit refused ' + r.conflict if r.conflict else ''}"
            ok = (r.result == want["result"] and r.conflict == want.get("conflict")
                  and r.position == want["position"])
        print(f"step {r.step:>3}: {actual:<26} position {r.position:>2}  "
              f"{'ok' if ok else 'EXPECTED ' + str(want)}")
        if not ok:
            problems.append(f"step {r.step}")

    final = harness.EXPECTED["final"]
    actual = {
        "position": store.current().position,
        "status": {rid: harness.status(store, rid) for rid in final["status"]},
        "decisions": [list(d) for d in harness.decision_rows(store, model)],
        "roles": {u: store.load("User", u)["value"]["role"] for u in final["roles"]},
        "budget": {b: store.load("Budget", b)["value"]["status"] for b in final["budget"]},
        "commands": harness.commands(store, model),
    }
    for key, value in actual.items():
        ok = value == final[key]
        print(f"final {key:<9}: {value}  {'ok' if ok else 'EXPECTED ' + str(final[key])}")
        if not ok:
            problems.append(f"final {key}")

    data = replay_data(store)
    behavior = replay_behavior(store, [model])
    print(f"data replay: {'ok' if data.ok else data.divergence}; "
          f"behavior replay: {'ok' if behavior.ok else behavior.divergence}")
    if not (data.ok and behavior.ok):
        problems.append("replay")

    for p in problems:
        print(f"MISMATCH: {p}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
