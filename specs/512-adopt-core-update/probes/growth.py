#!/usr/bin/env python3
"""Bounded consumer-path measurements; never removes the real global rule or its evidence."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import resource
import time

import behavior as b

NOW = "2026-10-27T00:00:00Z"
POPULATIONS = [64, 256, 1024]
REPETITIONS = 3
CREATIONS = 8


@b.entity
class Item:
    value = b.field(int)


@b.action
def add_item(*, item_id: b.Input[b.Id[Item]]):
    b.create(Item, id=item_id, value=0)


@b.action
def update_item(item: Item):
    b.set_(item.value, 1)


def model_for(limit):
    @b.invariant
    def maximum_items():
        return b.count(b.select(Item)) <= limit
    return b.BehaviorModule(entities=[Item], actions=[add_item, update_item],
                            invariants=[maximum_items])


def store_for(model, population):
    seed = [{"entity": "Item", "value": {"id": f"i{i}", "value": 0}} for i in range(population)]
    return b.Store.create(b.InMemoryBackend(), model, b.Store.genesis_for(model, seed))


def boundary():
    model = model_for(1000)
    store = store_for(model, 999)
    allowed = store.evaluate(model, "add_item", bindings={}, input={"item_id": "item1000"}, commit_time=NOW)
    assert allowed.decision.result == "ALLOW" and allowed.bundle is not None
    store.commit(model, allowed.bundle)
    before = store.current()
    denied = store.evaluate(model, "add_item", bindings={}, input={"item_id": "item1001"}, commit_time=NOW)
    assert denied.decision.result == "DENY" and denied.bundle is None
    assert store.current() == before
    update = store.evaluate(model, "update_item", bindings={"item": "i0"}, commit_time=NOW)
    assert update.decision.result == "ALLOW" and update.bundle is not None
    store.commit(model, update.bundle)
    assert store.load("Item", "i0")["value"]["value"] == 1
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
    return {"limit": 1000, "creation_1000": "ALLOW", "creation_1001": "DENY",
            "refused_head_unchanged": True, "unaffected_update": "ALLOW",
            "data_replay": True, "behavior_replay": True}


def measure(model, population, repetition):
    setup_start = time.perf_counter_ns()
    store = store_for(model, population)
    setup_ns = time.perf_counter_ns() - setup_start
    observations = []
    window_start = time.perf_counter_ns()
    for i in range(CREATIONS):
        start = time.perf_counter_ns()
        ev = store.evaluate(model, "add_item", bindings={}, input={"item_id": f"new{i}"}, commit_time=NOW)
        evaluated = time.perf_counter_ns()
        assert ev.decision.result == "ALLOW" and ev.bundle is not None
        store.commit(model, ev.bundle)
        committed = time.perf_counter_ns()
        observations.append({"population_before": population + i,
            "evaluate_ns": evaluated - start, "commit_ns": committed - evaluated,
            "cycle_ns": committed - start,
            "record_bytes": len(ev.decision.record_json.encode()),
            "bundle_bytes": len(json.dumps(ev.bundle, sort_keys=True, separators=(",", ":")).encode())})
    window_ns = time.perf_counter_ns() - window_start
    assert store.current().position == CREATIONS
    replay_start = time.perf_counter_ns()
    assert b.replay_data(store).ok and b.replay_behavior(store, [model]).ok
    return {"initial_population": population, "final_population": population + CREATIONS,
            "repetition": repetition, "setup_ns": setup_ns, "window_wall_ns": window_ns,
            "transaction_wall_ns": sum(o["cycle_ns"] for o in observations),
            "replay_ns": time.perf_counter_ns() - replay_start,
            "data_replay": True, "behavior_replay": True, "cycles": observations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    # One fixed available CPU makes baseline/candidate scheduling conditions explicit.
    available = sorted(os.sched_getaffinity(0))
    os.sched_setaffinity(0, {available[0]})
    model = model_for(10000)
    rows = []
    boundary_result = boundary()
    for population in POPULATIONS:
        for repetition in range(REPETITIONS):
            rows.append(measure(model, population, repetition))
        print(json.dumps({"population": population, "repetitions_complete": REPETITIONS}), flush=True)
    result = {"versions": b.versions(), "platform": platform.platform(),
              "cpu_affinity": sorted(os.sched_getaffinity(0)), "serial": True,
              "measurement_limit": 10000, "populations": POPULATIONS,
              "repetitions": REPETITIONS, "creations_per_repetition": CREATIONS,
              "method": "Fresh Store for each repetition; retained through all eight actual creation cycles. Native in-memory backend; complete public evaluate/commit including Python return conversion. Metric serialization excluded from transaction_wall_ns but included in window_wall_ns. Setup/replay separate; installed package and warm OS file caches; no result cache or source override.",
              "wire": json.loads(model.to_wire_json()), "behavior_hash": model.behavior_version,
              "boundary": boundary_result, "observations": rows,
              "peak_process_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
