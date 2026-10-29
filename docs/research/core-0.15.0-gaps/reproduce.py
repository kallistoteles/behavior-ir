#!/usr/bin/env python3
"""Public API investigation fixtures. No Core changes or safety certificates.

--require-supported is a red-capable regression signal for Gap 009.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import behavior as b

NOW = "2026-10-24T00:00:00Z"


@b.entity
class Target:
    value = b.field(int)


@b.entity
class Holder:
    target = b.field(b.Ref[Target])


@b.entity
class IdHolder:
    target = b.field(b.Id[Target])


@b.action
def remove_free(target: Target):
    b.requires(b.not_(b.referenced(target.id)))
    b.remove(target)


@b.action
def remove_without_guard(target: Target):
    b.remove(target)


@b.action
def remove_and_create_reference(target: Target, *, new_id: b.Input[b.Id[Holder]]):
    b.create(Holder, id=new_id, target=target.id)
    b.remove(target)


@b.action
def remove_both(target: Target, holder: Holder):
    b.requires(holder.target == target.id)
    b.remove(holder)
    b.remove(target)


@b.action
def retarget_then_remove(target: Target, other: Target, holder: Holder):
    b.requires(holder.target == target.id)
    b.requires(other.id != target.id)
    b.set_(holder.target, other.id)
    b.remove(target)


@b.action
def update(target: Target):
    b.set_(target.value, 1)


@b.constraint
def nonnegative(target: Target):
    return target.value >= 0


@b.action
def add(*, new_id: b.Input[b.Id[Target]]):
    b.create(Target, id=new_id, value=0)


@b.invariant
def global_true():
    return True


@b.invariant
def global_query():
    return b.count(b.select(Target)) >= 0


def save(path: Path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--require-supported", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    versions = b.versions()
    assert versions["binding"]["python"] == "0.14.0", versions
    assert versions["core"]["version"] == "0.15.0", versions
    assert versions["verifier"] == "0.10.0", versions
    rows = []
    for name, entities, action in [("ref", [Target, Holder], remove_free),
                                    ("ref_unguarded", [Target, Holder], remove_without_guard),
                                    ("id", [Target, IdHolder], remove_free),
                                    ("no_holder", [Target], remove_free)]:
        model = b.BehaviorModule(entities=entities, actions=[action])
        wire = model.to_wire_json()
        (args.output / f"009-{name}.json").write_text(wire)
        attestation = b.verify(model, b.Profile(checks=["preservation"]))
        save(args.output / f"009-{name}-attestation.json", attestation.data)
        runtime = []
        if name.startswith("ref"):
            assert len(attestation.checks) == 1, attestation.data
        for occupied in ([False, True] if not name.startswith("no_holder") else [False]):
            seed = [{"entity": "Target", "value": {"id": "t1", "value": 0}}]
            if occupied:
                seed.append({"entity": "Holder" if name.startswith("ref") else "IdHolder",
                             "value": {"id": "h1", "target": "t1"}})
            for action_name in [action.name]:
                store = b.Store.create(b.InMemoryBackend(), model, b.Store.genesis_for(model, seed))
                before = store.current()
                ev = store.evaluate(model, action_name, bindings={"target": "t1"}, commit_time=NOW)
                committed = False
                if ev.bundle is not None:
                    store.commit(model, ev.bundle)
                    committed = True
                assert b.replay_data(store).ok
                replay = b.replay_behavior(store, [model])
                assert replay.ok
                row = {"occupied": occupied, "action": action_name,
                       "result": ev.decision.result, "committed": committed,
                       "position_before": before.position, "position_after": store.current().position,
                       "behavior_replay_checked": replay.checked, "data_replay_ok": True,
                       "behavior_replay_ok": replay.ok}
                runtime.append(row)
                if name.startswith("ref") and occupied:
                    assert not committed and store.current().position == 0, row
                else:
                    assert committed and store.current().position == 1, row
        rows.append({"case": name, "behavior_hash": model.behavior_version,
                     "wire_sha256": hashlib.sha256(wire.encode()).hexdigest(),
                     "verification": attestation.result, "checks": attestation.checks,
                     "findings": attestation.findings, "runtime": runtime})
    positive = b.BehaviorModule(entities=[Target], actions=[update], constraints=[nonnegative])
    positive_attestation = b.verify(positive, b.Profile(checks=["preservation"]))
    assert positive_attestation.result == "verified" and positive_attestation.checks
    assert all(c["outcome"] == "proven" for c in positive_attestation.checks)
    save(args.output / "009-positive-attestation.json", positive_attestation.data)
    mixed = []
    for action, occupied, allowed in [(remove_and_create_reference, False, False),
                                      (remove_both, True, True),
                                      (retarget_then_remove, True, True)]:
        model = b.BehaviorModule(entities=[Target, Holder], actions=[action])
        (args.output / f"009-{action.name}.json").write_text(model.to_wire_json())
        seed = [{"entity": "Target", "value": {"id": "t1", "value": 0}},
                {"entity": "Target", "value": {"id": "t2", "value": 0}}]
        bindings = {"target": "t1"}
        if occupied:
            seed.append({"entity": "Holder", "value": {"id": "h1", "target": "t1"}})
            bindings["holder"] = "h1"
        if action is retarget_then_remove:
            bindings["other"] = "t2"
        store = b.Store.create(b.InMemoryBackend(), model, b.Store.genesis_for(model, seed))
        ev = store.evaluate(model, action.name, bindings=bindings,
                            input={"new_id": "new"} if not occupied else {}, commit_time=NOW)
        assert (ev.bundle is not None) == allowed, ev.decision.record_json
        if ev.bundle is not None:
            store.commit(model, ev.bundle)
        attestation = b.verify(model, b.Profile(checks=["preservation"]))
        data_replay = b.replay_data(store)
        behavior_replay = b.replay_behavior(store, [model])
        assert data_replay.ok and behavior_replay.ok
        if action is retarget_then_remove:
            assert store.load("Holder", "h1")["value"]["target"] == "t2"
        save(args.output / f"009-{action.name}-attestation.json", attestation.data)
        mixed.append({"action": action.name, "behavior_hash": model.behavior_version,
                      "runtime_result": ev.decision.result, "committed": allowed,
                      "position": store.current().position,
                      "data_replay_ok": data_replay.ok, "behavior_replay_ok": behavior_replay.ok,
                      "verification": attestation.result, "checks": attestation.checks,
                      "findings": attestation.findings})
    for name, invariants in [("local", []), ("global_true", [global_true]),
                              ("global_query", [global_query])]:
        model = b.BehaviorModule(entities=[Target], actions=[update, add], invariants=invariants)
        (args.output / f"008-{name}.json").write_text(model.to_wire_json())
    save(args.output / "009-results.json", {"versions": versions, "cases": rows, "mixed": mixed,
                                            "positive_smt": positive_attestation.data})
    print(json.dumps({"009": [{"case": r["case"], "verification": r["verification"],
                                "runtime": r["runtime"]} for r in rows]}, sort_keys=True))
    if args.require_supported:
        assert rows[0]["verification"] == "verified", "Gap009: guarded Ref-target removal is not verified"


if __name__ == "__main__":
    main()
