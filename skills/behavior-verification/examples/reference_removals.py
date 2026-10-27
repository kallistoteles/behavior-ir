"""Supported Ref-target removal proofs plus independently allowed Core transitions."""
import behavior as b


@b.entity
class Target:
    value = b.field(int)


@b.entity
class Holder:
    target = b.field(b.Ref[Target])


@b.read
def target_value(target: Target):
    return target.value


@b.action
def remove_free(target: Target):
    b.requires(b.not_(b.referenced(target.id)))
    b.remove(target)


@b.action
def remove_joint(target: Target, holder: Holder):
    b.requires(holder.target == target.id)
    b.requires(b.count(b.select(Holder).where(lambda h: h.target == target.id)) == 1)
    b.remove(holder)
    b.remove(target)


@b.action
def retarget_remove(target: Target, other: Target, holder: Holder):
    b.requires(holder.target == target.id)
    b.requires(b.count(b.select(Holder).where(lambda h: h.target == target.id)) == 1)
    b.requires(target.id != other.id)
    b.set_(holder.target, other.id)
    b.remove(target)


for operation in [remove_free, remove_joint, retarget_remove]:
    model = b.BehaviorModule(entities=[Target, Holder], actions=[operation], reads=[target_value])
    report = b.verify(model, b.Profile(checks=["preservation"]))
    references = [c for c in report.checks if c["subject"]["kind"] == "reference_world"]
    assert references and all(c["outcome"] == "proven" for c in references), report.data
    assert report.verified
    seed = [{"entity": "Target", "value": {"id": key, "value": 0}} for key in ["t1", "t2"]]
    seed.append({"entity": "Holder", "value": {"id": "unrelated", "target": "t2"}})
    bindings = {"target": "t1"}
    if operation is not remove_free:
        seed.append({"entity": "Holder", "value": {"id": "h1", "target": "t1"}})
        bindings["holder"] = "h1"
    if operation is retarget_remove:
        bindings["other"] = "t2"
    store = b.Store.create(b.InMemoryBackend(), model, b.Store.genesis_for(model, seed))
    result = store.evaluate(model, operation.name, bindings=bindings,
                            commit_time="2026-10-27T00:00:00Z")
    assert result.decision.result == "ALLOW" and result.bundle is not None
    store.commit(model, result.bundle)
    assert store.current().position == 1
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
