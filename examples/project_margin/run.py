"""Admits the project margin example and evaluates the guarded and unguarded actions."""

from behavior import admit, evaluate
from examples.project_margin.behavior import model

result = admit(model)
print("admitted:", result.ok, result.behavior_version)
print("evaluation_order:", result.evaluation_order)

for action in ("flag_project", "flag_project_unguarded"):
    for revenue in ("100", "0"):
        project = {"id": "p1", "revenue": revenue, "cost": "98", "flagged": False}
        # The complete parent snapshot: every declared entity type (here only Project).
        snapshot = {"universe": [{"entity": "Project", "members": [project]}]}
        d = evaluate(model, action, state={"project": project}, data_version="7", facts=snapshot)
        # Messages are diagnostics, outside the canonical record (decision record 0.8).
        reasons = d.diagnostics["reasons"]
        detail = reasons[0]["message"] if reasons else ""
        print(f"{action} revenue={revenue}: {d.result} {detail}")
