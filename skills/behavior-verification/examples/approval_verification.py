"""Local Approval postconditions, not a universal proof of reachable global safety."""
from behavior import (
    Approval, BehaviorModule, Id, Profile, entity, field, verify, versions,
)


@entity
class User:
    name = field(str)


@entity
class Document:
    title = field(str)


review = Approval("review", subject=Document, actor=User, approve_threshold=2, reject_threshold=1)
review.approve(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})
review.reject(actor=lambda decision_id, subject, actor: actor, context={"actor": Id[User]})
model = BehaviorModule(entities=[User, Document], models=[review])
assert versions()["verifier"] == "0.10.0"
local = verify(model, Profile(checks=["postcondition"]))
assert local.checks and all(c["outcome"] == "proven" for c in local.checks), local.checks
assert {c["action"]["name"] for c in local.checks} == {"review__approve", "review__reject"}
assert local.verified
# Other profiles and larger modules need their own results. Global safety additionally needs
# valid genesis, complete ownership and preservation across every committed action/migration.
