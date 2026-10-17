"""The executable reference application of feature 507 (spec, Reference history).

    python -m models.examples.work_queue.run

It authors and admits the queue, creates a store from a checked initial state, runs the
reference history and prints each step with its expected and actual outcome. It then prints the
final state and the committed command stream, replays the history twice (data replay and
behavior replay), and exits non-zero on any difference.
"""

from __future__ import annotations

import sys

from behavior import replay_behavior, replay_data

from models.examples.work_queue import harness, workflow


def main() -> int:
    model = workflow.module()
    print(f"admitted: {model.behavior_version}")
    problems: list[str] = []

    def check(store: object) -> None:
        try:
            value = harness.item(store)  # type: ignore[arg-type]
        except Exception:  # noqa: BLE001 - no item before enqueue
            return
        if not harness.valid_work(value):
            problems.append(f"ValidWork violated: {value}")

    store, results = harness.run_history(model, on_commit=check)
    for r in results:
        want = harness.EXPECTED["steps"][r.step]
        ok = r.result == want["result"] and r.position == want["position"] and not r.conflict
        if "item" in want:
            ok = ok and r.item is not None and all(r.item[k] == v for k, v in want["item"].items())
        print(f"step {r.step:>3}: {r.result:<6} position {r.position:>2}  "
              f"{'ok' if ok else 'EXPECTED ' + str(want)}")
        if not ok:
            problems.append(f"step {r.step}")

    final = harness.EXPECTED["final"]
    actual = {
        "position": store.current().position,
        "item": {k: harness.item(store)[k] for k in final["item"]},
        "documents": {d: store.load("Document", d)["value"]["processed"]
                      for d in final["documents"]},
        "request_status": store.load("DocumentRequest", "r1")["value"]["status"],
        "commands": harness.commands(store, model),
    }
    for key, value in actual.items():
        ok = value == final[key]
        print(f"final {key:<14}: {value}  {'ok' if ok else 'EXPECTED ' + str(final[key])}")
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
