"""Consumer smoke scenario for a Behavior release (feature 008, US1).

It imports only the installed `behavior` package and uses no repository paths, so it runs
unchanged in a clean environment that has nothing but the release's wheel. It authors a module,
admits it, evaluates decisions, verifies, persists transitions in a store, replays them, and
drives the command-line tool. Each artifact is printed as one canonical JSON line. The release
check runs it inside and outside the repository and compares the two outputs byte for byte.

    python smoke.py                          # everything
    python smoke.py --expect-missing-solver  # BEHAVIOR_Z3 points nowhere: verification must say so
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from decimal import Decimal
from enum import Enum
from pathlib import Path
from typing import Any

from behavior import (
    BehaviorError, BehaviorModule, Id, InMemoryBackend, Input, Migration, Store, action, admit,
    count, create, entity, enum_map, evaluate, field, invariant, nominal, replay,
    replay_behavior, replay_data, requires, select, sum_, unique, verify,
)
from behavior import versions as behavior_versions

EXPECT_MISSING_SOLVER = "--expect-missing-solver" in sys.argv[1:]
PREREQUISITE = "verification needs the Z3 SMT solver"

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"}, scale=2)


class OrderStatus(Enum):
    OPEN = "open"
    CLOSED = "closed"


@entity
class Customer:
    name = field(str)
    credit_limit = field(Money)


@entity
class Order:
    customer = field(Id[Customer])
    amount = field(Money)
    status = field(OrderStatus)


@entity
class Employee:
    number = field(str)


@invariant
def numbers_unique():  # type: ignore[no-untyped-def]
    return unique(select(Employee), by=lambda e: e.number)


@action
def place_order(customer: Customer, *, order_id: Input[Id[Order]], amount: Input[Money]):  # type: ignore[no-untyped-def]
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(amount > Money(Decimal("0")))
    requires(sum_(orders, lambda o: o.amount) + amount <= customer.credit_limit)
    create(Order, id=order_id, customer=customer.id, amount=amount, status=OrderStatus.OPEN)


@action
def close_customer_check(customer: Customer):  # type: ignore[no-untyped-def]
    orders = select(Order).where(lambda o: o.customer == customer.id)
    requires(count(orders.where(lambda o: o.status == OrderStatus.OPEN)) == 0)


@action
def hire(*, employee_id: Input[Id[Employee]], number: Input[str]):  # type: ignore[no-untyped-def]
    create(Employee, id=employee_id, number=number)


model = BehaviorModule(
    entities=[Customer, Order, Employee],
    invariants=[numbers_unique],
    actions=[place_order, close_customer_check, hire],
)


def widened() -> tuple[BehaviorModule, Migration]:
    """The next schema (feature 009): orders may also be held; and the migration to it."""

    class OrderStatus(Enum):  # noqa: F811
        OPEN = "open"
        CLOSED = "closed"
        HELD = "held"

    @entity
    class Order:  # noqa: F811
        customer = field(Id[Customer])
        amount = field(Money)
        status = field(OrderStatus)

    target = BehaviorModule(entities=[Customer, Order, Employee], invariants=[numbers_unique])
    old_status = globals()["OrderStatus"]
    migration = Migration(source=model, target=target, transforms={
        globals()["Order"]: lambda old: {"status": enum_map(old.status, {
            old_status.OPEN: OrderStatus.OPEN, old_status.CLOSED: OrderStatus.CLOSED})},
    })
    return target, migration


def emit(label: str, value: Any) -> None:
    print(json.dumps({label: value}, sort_keys=True, separators=(",", ":"), ensure_ascii=False))


def check(cond: bool, message: str) -> None:
    if not cond:
        print(f"smoke: FAILED: {message}", file=sys.stderr)
        sys.exit(1)


CUSTOMER = {"id": "c1", "name": "Ada", "credit_limit": Decimal("100.00")}
ORDERS = {"universe": [{"entity": "Order", "members": [
    {"id": "o1", "customer": "c1", "amount": "60.00", "status": "open"}]}],
    "identities": [{"entity": "Order", "id": "o9", "used": False}]}


def main() -> None:
    # 1. Author and admit.
    result = admit(model)
    check(result.ok, f"admission: {result}")
    emit("behavior_version", model.behavior_version)

    # 2. Plain evaluation with supplied facts, and replay of the record alone.
    allowed = evaluate(model, "place_order", state={"customer": CUSTOMER},
                       input={"order_id": "o9", "amount": Decimal("40.00")}, data_version="1",
                       facts=ORDERS)
    denied = evaluate(model, "place_order", state={"customer": CUSTOMER},
                      input={"order_id": "o9", "amount": Decimal("40.01")}, data_version="1",
                      facts=ORDERS)
    check(allowed.result == "ALLOW" and denied.result == "DENY",
          f"decisions: {allowed.result} {denied.result}")
    check(replay(model, allowed.record_json).matches, "replay of a record")
    emit("record_allowed", json.loads(allowed.record_json))
    emit("record_denied", json.loads(denied.record_json))

    # 3. Verification (or, without a solver, the named prerequisite).
    if EXPECT_MISSING_SOLVER:
        try:
            verify(model)
        except BehaviorError as e:
            check(PREREQUISITE in str(e), f"missing solver message: {e}")
        else:
            check(False, "verification ran without a solver")
    else:
        emit("attestation", json.loads(verify(model).json))

    # 4. A store: two committed transitions, then both replays.
    seed = [{"entity": "Customer", "value": {"id": "c1", "name": "Ada", "credit_limit": "100.00"}}]
    store = Store.create(InMemoryBackend(), model, Store.genesis_for(model, seed))
    now = "2026-09-30T12:00:00Z"
    for order_id, amount in [("o1", "30.00"), ("o2", "50.00")]:
        ev = store.evaluate(model, "place_order", bindings={"customer": "c1"},
                            input={"order_id": order_id, "amount": Decimal(amount)}, commit_time=now)
        check(ev.bundle is not None, f"store decision: {ev.decision.result}")
        assert ev.bundle is not None
        store.commit(model, ev.bundle)
    over = store.evaluate(model, "place_order", bindings={"customer": "c1"},
                          input={"order_id": "o3", "amount": Decimal("20.01")}, commit_time=now)
    check(over.decision.result == "DENY", "the credit limit holds in the store")
    emit("state", store.current().as_dict())
    for r in store.history():
        emit("transition", r)
    check(replay_data(store).ok, "data replay")
    check(replay_behavior(store, [model]).ok, "behavior replay")

    # 4b. A schema change in place (feature 009): migrate, then replay across it.
    target, migration = widened()
    check(migration.admit().ok, f"migration admission: {migration.admit().errors}")
    emit("migration", {"hash": migration.hash, "summary": migration.summary()})
    store.migrate(migration, commit_time=now)
    emit("schema_history", [[s.since, s.hash] for s in store.schema_history()])
    check(replay_data(store).ok, "data replay across a migration")
    check(replay_behavior(store, [model, target], migrations=[migration]).ok,
          "behavior replay across a migration")

    # 5. The command-line tool, from this environment's console script.
    cli = str(Path(sys.executable).parent / "behavior")
    with tempfile.TemporaryDirectory() as tmp:
        wire = Path(tmp) / "module.json"
        wire.write_text(model.to_wire_json())
        request = Path(tmp) / "request.json"
        request.write_text(json.dumps({
            "action": "place_order", "data_version": "1", "state": {"customer": {
                "id": "c1", "name": "Ada", "credit_limit": "100.00"}},
            "input": {"order_id": "o9", "amount": "40.00"}, "context": {},
            "facts": json.loads(json.dumps(ORDERS))}))
        runs: list[tuple[str, list[str]]] = [
            ("admit", ["admit", str(wire)]),
            ("eval", ["eval", str(wire), str(request)]),
        ]
        results: dict[str, subprocess.CompletedProcess[str]] = {}
        for name, args in runs:
            results[name] = subprocess.run([cli, *args], capture_output=True, text=True)
        record = Path(tmp) / "record.json"
        record.write_text(results["eval"].stdout)
        results["replay"] = subprocess.run([cli, "replay", str(wire), str(record)],
                                           capture_output=True, text=True)
        results["verify"] = subprocess.run([cli, "verify", str(wire)], capture_output=True,
                                           text=True)
    for name in ["admit", "eval", "replay"]:
        check(results[name].returncode == 0, f"behavior {name}: {results[name].stderr}")
        emit(f"cli_{name}", json.loads(results[name].stdout))
    if EXPECT_MISSING_SOLVER:
        check(results["verify"].returncode == 3 and PREREQUISITE in results["verify"].stderr,
              f"behavior verify without a solver: {results['verify'].stderr}")
    else:
        check(results["verify"].returncode in (0, 1), f"behavior verify: {results['verify'].stderr}")
        emit("cli_verify", json.loads(results["verify"].stdout))
    # 6. One install (feature 011, US4): the package names the exact core it bundles, and its
    # command line is that core's own tool. Checked only, never emitted, so the output stays
    # comparable across releases.
    versions = behavior_versions()
    core = versions.get("core", {})
    check(set(core) == {"version", "commit"} and len(str(core.get("commit"))) == 40,
          f"the package names its core: {core}")
    check(versions["engine"] == core.get("version"), f"engine {versions['engine']} is the core")
    info = subprocess.run([cli, "engine-info"], capture_output=True, text=True)
    check(info.returncode == 0 and json.loads(info.stdout)["engine"] == core.get("version"),
          f"behavior engine-info reports the bundled core: {info.stdout or info.stderr}")
    print("smoke: OK", file=sys.stderr)


if __name__ == "__main__":
    main()
