"""Try-out: purchase approval against a project budget, end to end.

Run inside the dev shell:  python -m examples.tryout.demo
"""

from __future__ import annotations

import json
from decimal import Decimal

from behavior import (
    BehaviorModule, BehaviorTypeError, IntentRejected, action, admit, evaluate, evaluate_intent,
    replay, set_,
)

from examples.tryout.model import Project, Status, model


def title(text: str) -> None:
    print(f"\n=== {text}")


def show(decision) -> None:  # type: ignore[no-untyped-def]
    print(f"result: {decision.result}")
    for r in decision.reasons:
        print(f"  reason: {r['code']}: {r['message']}")
    for c in decision.changes:
        print(f"  change: {c.param}.{c.field}: {c.old!r} -> {c.new!r}")


# --- 2. Admission: the engine type-checks and hashes it --------------------------------------

title("Admission")
result = admit(model)
print(f"admitted: {result.ok}")
print(f"behavior version: {result.behavior_version}")
print(f"evaluation order of derived values: {result.evaluation_order}")

# --- 3. Evaluation ---------------------------------------------------------------------------

anna = {"id": "anna", "role": "manager", "approval_limit": Decimal("50000")}
project = {"id": "p7", "budget": Decimal("100000"), "spent": Decimal("80000")}


def purchase(amount: str) -> dict[str, object]:
    return {"id": "po-1", "amount": Decimal(amount), "status": Status.PENDING, "approved_by": None}


def run(amount: str, actor: dict[str, object] = anna, proj: dict[str, object] = project):  # type: ignore[no-untyped-def]
    return evaluate(model, "approve", state={"purchase": purchase(amount), "project": proj},
                    context={"actor": actor}, data_version="42")


title("Approve 15 000 (within limit and budget)")
ok = run("15000")
show(ok)

title("Approve 30 000 (within Anna's limit, but the project only has 20 000 left)")
show(run("30000"))
print("  -> the invariant `within_budget` on the proposed state refused it")

title("Approve 60 000 (above Anna's approval limit)")
show(run("60000"))

title("A clerk tries to approve")
show(run("1000", actor={**anna, "role": "clerk"}))

title("Starting state already broken (spent > budget)")
show(run("1000", proj={**project, "spent": Decimal("120000")}))

# --- 4. The decision record: audit and replay ------------------------------------------------

title("Decision record of the first approval")
record = json.loads(ok.record_json)
for step in record["trace"]:
    print(f"  {step['phase']:<15} {step['expr_text']:<50} {step['outcome']}")
print(f"replay matches: {replay(model, ok.record_json).matches}")
tampered = dict(record, result="DENY")
print(f"tampered record replays: {replay(model, json.dumps(tampered)).matches} "
      f"({replay(model, json.dumps(tampered)).diff})")

# --- 5. An AI acting through the capability boundary -----------------------------------------

title("AI intent: approve po-1 (host supplies state and the logged-in user)")
host = {"state": {"purchase": purchase("15000"), "project": project}, "context": {"actor": anna}}
d = evaluate_intent(model, {"capability": "approve",
                            "targets": {"purchase": "po-1", "project": "p7"}, "input": {}},
                    data_version="42", **host)
print(f"result: {d.result}")

title("AI intent that claims to be a manager with a huge limit")
try:
    evaluate_intent(model, {"capability": "approve", "targets": {"purchase": "po-1", "project": "p7"},
                            "input": {},
                            "context": {"actor": {"id": "ai", "role": "manager",
                                                  "approval_limit": "99999999"}}},
                    data_version="42", **host)
except IntentRejected as e:
    for err in e.errors:
        print(f"rejected: {err['code']} at {err['path']}: {err['message']}")

title("AI intent that targets a different purchase than the host loaded")
try:
    evaluate_intent(model, {"capability": "approve",
                            "targets": {"purchase": "po-999", "project": "p7"}, "input": {}},
                    data_version="42", **host)
except IntentRejected as e:
    for err in e.errors:
        print(f"rejected: {err['code']} at {err['path']}: {err['message']}")

# --- 6. Type errors fail where they are written ----------------------------------------------

title("An author writes budget + a plain Decimal")
try:
    @action
    def bad(project: Project):
        set_(project.spent, project.spent + Decimal("10"))

    BehaviorModule(entities=[Project], actions=[bad])
except BehaviorTypeError as e:
    print(f"BehaviorTypeError at line {e.line}: {e.message}")

# --- 7. Identity: behavior is content-addressed ----------------------------------------------

title("Canonical serialization round trip")
wire = model.to_wire_json()
print(f"canonical wire IR: {len(wire)} bytes, starts {wire[:60]}...")
from behavior import _engine  # noqa: E402

reloaded, report = _engine.Module.from_wire(wire)
print(f"re-admitted from JSON, same version: {report['behavior_version'] == result.behavior_version}")
