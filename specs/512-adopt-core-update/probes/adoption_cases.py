#!/usr/bin/env python3
"""Neutral public-package removal/proof controls; no dependency override or Core source copy."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

import behavior as b

ROOT = Path(__file__).resolve().parents[3]
NOW = "2026-10-27T00:00:00Z"


@b.entity
class Target:
    value = b.field(int)


@b.entity
class Holder:
    target = b.field(b.Ref[Target])


@b.command
class Audit:
    value = b.field(int)


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
def remove_retarget(target: Target, other: Target, holder: Holder):
    b.requires(holder.target == target.id)
    b.requires(b.count(b.select(Holder).where(lambda h: h.target == target.id)) == 1)
    b.requires(target.id != other.id)
    b.set_(holder.target, other.id)
    b.remove(target)


@b.action
def unguarded(target: Target):
    b.remove(target)


@b.action
def create_dangling(target: Target, *, new_id: b.Input[b.Id[Holder]]):
    b.create(Holder, id=new_id, target=target.id)
    b.remove(target)


ACTIONS = {"free": remove_free, "joint": remove_joint, "retarget": remove_retarget}


def model_for(action, profile):
    return b.BehaviorModule(entities=[Target, Holder], actions=[action], reads=[target_value],
                            commands=[Audit] if profile == "0.8" else [])


def store_for(model, seed):
    pin = json.loads((ROOT / "core-release.json").read_text())
    policy = (ROOT / ".core" / pin["version"] / "tests/fixtures/governance-v2/evidence-policy-none.json").read_text()
    genesis = b.Store.genesis_v2_for(model, seed, policy)
    return b.Store.create(b.InMemoryBackend(), model, genesis)


def positive(pattern, profile):
    action = ACTIONS[pattern]
    model = model_for(action, profile)
    wire = json.loads(model.to_wire_json())
    assert wire["ir_version"] == profile
    # Unrelated references remain allowed; no whole-population cap hides missing coverage.
    seed = [{"entity": "Target", "value": {"id": t, "value": 0}} for t in ["t1", "t2"]]
    seed.append({"entity": "Holder", "value": {"id": "unrelated", "target": "t2"}})
    bindings = {"target": "t1"}
    if pattern != "free":
        seed.append({"entity": "Holder", "value": {"id": "h1", "target": "t1"}})
        bindings["holder"] = "h1"
    if pattern == "retarget":
        bindings["other"] = "t2"
    store = store_for(model, seed)
    ev = store.evaluate(model, action.name, bindings=bindings, commit_time=NOW)
    assert ev.decision.result == "ALLOW" and ev.bundle is not None, ev.decision.record_json
    store.commit(model, ev.bundle)
    assert store.current().position == 1
    assert store.load("Holder", "unrelated")["value"]["target"] == "t2"
    if pattern == "retarget":
        assert store.load("Holder", "h1")["value"]["target"] == "t2"
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
    # The extra incoming row must refuse the same guarded transition without advancing head.
    occupied = store_for(model, seed + [{"entity": "Holder", "value": {"id": "extra", "target": "t1"}}])
    denied = occupied.evaluate(model, action.name, bindings=bindings, commit_time=NOW)
    assert denied.decision.result == "DENY" and denied.bundle is None
    assert occupied.current().position == 0
    report = b.verify(model, b.Profile(checks=["preservation"]))
    references = [c for c in report.checks if c["subject"]["kind"] == "reference_world"]
    supported = bool(references) and report.verified and all(c["outcome"] == "proven" for c in report.checks)
    return {"pattern": pattern, "profile": profile, "behavior_hash": model.behavior_version,
            "wire": wire, "report": report.data, "supported": supported,
            "runtime": {"allowed_position": 1, "occupied_refused_position": 0,
                        "data_replay": True, "behavior_replay": True}}


def negative(action, profile):
    model = model_for(action, profile)
    seed = [{"entity": "Target", "value": {"id": "t1", "value": 0}}]
    if action is unguarded:
        seed.append({"entity": "Holder", "value": {"id": "h1", "target": "t1"}})
    store = store_for(model, seed)
    ev = store.evaluate(model, action.name, bindings={"target": "t1"},
                        input={"new_id": "created"} if action is create_dangling else {}, commit_time=NOW)
    assert ev.decision.result == "DENY" and ev.bundle is None
    assert store.current().position == 0
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
    report = b.verify(model, b.Profile(checks=["preservation"]))
    assert not report.verified
    return {"action": action.name, "profile": profile, "report": report.data,
            "runtime_result": ev.decision.result, "position": 0}



@b.entity
class OtherHolder:
    target = b.field(b.Ref[Target])
    second = b.field(b.Ref[Target])


@b.action
def retarget_with_extra_bound(target: Target, other: Target, holder: Holder, other_holder: Holder):
    b.requires(holder.target == target.id)
    b.requires(b.count(b.select(Holder).where(lambda h: h.target == target.id)) == 1)
    b.requires(target.id != other.id)
    b.requires(other_holder.target == other.id)
    b.set_(holder.target, other.id)
    b.remove(target)


def extra_incoming_class(profile):
    model = b.BehaviorModule(entities=[Target, Holder, OtherHolder], actions=[remove_joint],
                            reads=[target_value], commands=[Audit] if profile == "0.8" else [])
    seed = [{"entity": "Target", "value": {"id": t, "value": 0}} for t in ["t1", "t2"]]
    seed += [{"entity": "Holder", "value": {"id": "h1", "target": "t1"}},
             {"entity": "OtherHolder", "value": {"id": "o1", "target": "t1", "second": "t2"}}]
    store = store_for(model, seed)
    ev = store.evaluate(model, remove_joint.name, bindings={"target": "t1", "holder": "h1"}, commit_time=NOW)
    assert ev.decision.result == "DENY" and ev.bundle is None
    assert store.current().position == 0
    report = b.verify(model, b.Profile(checks=["preservation"]))
    assert not report.verified
    return report


def unsupported_bound_order(profile):
    model = model_for(retarget_with_extra_bound, profile)
    report = b.verify(model, b.Profile(checks=["preservation"]))
    seed = [{"entity": "Target", "value": {"id": t, "value": 0}} for t in ["t1", "t2"]]
    seed += [{"entity": "Holder", "value": {"id": "h1", "target": "t1"}},
             {"entity": "Holder", "value": {"id": "h2", "target": "t2"}}]
    store = store_for(model, seed)
    ev = store.evaluate(model, retarget_with_extra_bound.name,
                        bindings={"target": "t1", "other": "t2", "holder": "h1", "other_holder": "h2"}, commit_time=NOW)
    assert ev.decision.result == "ALLOW" and ev.bundle is not None
    store.commit(model, ev.bundle)
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--require-supported", action="store_true")
    args = parser.parse_args()
    positives = [positive(pattern, profile) for profile in ["0.7", "0.8"] for pattern in ACTIONS]
    negatives = [negative(action, profile) for profile in ["0.7", "0.8"]
                 for action in [unguarded, create_dangling]]
    result = {"versions": b.versions(), "positives": positives, "negatives": negatives}
    if args.output:
        args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    failed = [f"{r['pattern']}/{r['profile']}" for r in positives if not r["supported"]]
    print(json.dumps({"positive_cases": len(positives), "negative_cases": len(negatives),
                      "runtime_controls": "passed", "missing_reference_proofs": failed}))
    if args.require_supported and failed:
        raise SystemExit("Required nonempty reference proofs not established: " + ", ".join(failed))


if __name__ == "__main__":
    main()


@b.entity
class CacheItem:
    value = b.field(int)


@b.action
def set_cache_value(item: CacheItem, *, value: b.Input[int]):
    b.set_(item.value, value)
    b.ensures(item.value == value)


def cache_model():
    return b.BehaviorModule(entities=[CacheItem], actions=[set_cache_value])
