"""Source locations (behavior-ir#19): a missing location never fails a build, a class is located
at its definition, and code without a file carries a marker, never a path under the working
directory.

Locations are outside semantics, so no module hash depends on them. Core relativizes a
module's locations (without an explicit `root`) and an ad-hoc read's locations against the
common directory of all their files, markers included. When markers and real files are mixed,
the real paths keep checkout-dependent directories. That limitation belongs to Core
(behavior-ir-core#31); the tests for it are strict expected failures, so they turn into
failures to remove once the pinned Core release fixes it.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import sys
import types
from pathlib import Path
from typing import Any

import pytest

from behavior import InMemoryBackend, Store

EXAMPLE = Path(__file__).resolve().parents[2] / "models" / "examples" / "state_machine" / "machine.py"

CORE_31 = pytest.mark.xfail(
    strict=True,
    reason="behavior-ir-core#31: Core's common directory counts markers next to real files",
)


def files(wire: str) -> set[str]:
    """Every `loc.file` in a module's wire JSON."""
    out: set[str] = set()

    def walk(x: Any) -> None:
        if isinstance(x, dict):
            if isinstance(x.get("file"), str):
                out.add(x["file"])
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)

    walk(json.loads(wire))
    return out


def load(path: Path, name: str, monkeypatch: pytest.MonkeyPatch | None = None) -> Any:
    """Loads `path` as module `name`; registered in `sys.modules` only with `monkeypatch`."""
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    if monkeypatch is not None:
        monkeypatch.setitem(sys.modules, name, mod)
    spec.loader.exec_module(mod)
    return mod


def checkout(root: Path, sources: dict[str, str]) -> Path:
    """Writes `sources` (relative path → text) under `root` and returns it."""
    for rel, text in sources.items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(text)
    return root


CHECKOUTS = ("one/checkout", "ci/build/elsewhere")


# --- markers, never the working directory -------------------------------------------------

@pytest.mark.parametrize("main_file", [None, "<stdin>"])
def test_an_enum_in_main_without_a_real_file_builds(
        main_file: str | None, monkeypatch: pytest.MonkeyPatch) -> None:
    """A notebook cell or REPL (`__main__` without `__file__`) used to raise `OSError`; with
    `python - < machine.py` the file is `<stdin>`. The build succeeds and the enum carries the
    marker."""
    main = types.ModuleType("__main__")
    if main_file is not None:
        main.__file__ = main_file
    monkeypatch.setitem(sys.modules, "__main__", main)
    exec(compile(EXAMPLE.read_text(), "<cell>", "exec"), main.__dict__)
    wire = main.__dict__["module"]().to_wire_json()
    enum = next(e for e in json.loads(wire)["enums"] if e["name"] == "OrderStatus")
    assert enum["loc"]["file"] == (main_file or "<__main__>")


def test_exec_code_is_located_by_markers_from_any_working_directory(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Code run through `exec` is `<string>`, not `<cwd>/<string>`; with no real file left, the
    module is the same from every working directory."""
    wires = []
    for where in ("a", "b/c"):
        (tmp_path / where).mkdir(parents=True)
        monkeypatch.chdir(tmp_path / where)
        namespace: dict[str, Any] = {"__name__": "exec_model"}
        exec(compile(EXAMPLE.read_text(), "<string>", "exec"), namespace)
        wires.append(namespace["module"]().to_wire_json())
    assert wires[0] == wires[1]
    assert files(wires[0]) == {"<string>", "<exec_model>"}


ENUM = """
from enum import Enum


class Colour(Enum):
    RED = "RED"
    BLUE = "BLUE"
"""

USES_ENUM = """
from behavior import BehaviorModule, entity, field

from colours import Colour


@entity
class Paint:
    colour = field(Colour)


def module():
    return BehaviorModule(entities=[Paint], enums=[Colour])
"""


def test_an_enum_is_located_at_its_definition_not_its_first_use(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """The enum is first used in another file; its file and line both come from its class."""
    root = checkout(tmp_path, {"colours.py": ENUM, "paint.py": USES_ENUM})
    colours = load(root / "colours.py", "colours", monkeypatch)
    paint = load(root / "paint.py", "paint", monkeypatch)
    wire = json.loads(paint.module().to_wire_json())
    [enum] = wire["enums"]
    assert enum["loc"] == {"file": "colours.py", "line": colours.Colour.__firstlineno__}


# --- real files: Core's root over the final IR ----------------------------------------------

SHARED_HELPER = """
def items(order):
    return [order.status]
"""

GUARD = """
def always(order):
    return True
"""

MACHINE = """
from enum import Enum

import shared_helper
import sibling_guard
from behavior import BehaviorModule, StateMachine, entity, field, project, read, select


class OrderStatus(Enum):
    DRAFT = "DRAFT"
    SUBMITTED = "SUBMITTED"


@entity
class Order:
    status = field(OrderStatus)


@read
def statuses():
    return project(select(Order), shared_helper.items)


def module():
    machine = StateMachine(
        "order", subject=Order, state_field="status", entry_states={OrderStatus.DRAFT}
    )
    machine.transition("submit", from_={OrderStatus.DRAFT}, to=OrderStatus.SUBMITTED,
                       requires=[sibling_guard.always])
    return BehaviorModule(entities=[Order], enums=[OrderStatus], models=[machine],
                          reads=[statuses])
"""


def test_real_files_from_two_checkouts_build_the_same_module(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Every location is a real file: a guard in a sibling file that returns a constant, and a
    projection whose item helper lives outside the checkout (its field nodes only name the
    items and are not in the final IR). The module is the same from both checkouts."""
    load(checkout(tmp_path / "outside", {"shared_helper.py": SHARED_HELPER})
         / "shared_helper.py", "shared_helper", monkeypatch)
    wires = []
    for where in CHECKOUTS:
        root = checkout(tmp_path / where, {"models/machine.py": MACHINE,
                                           "shared/sibling_guard.py": GUARD})
        load(root / "shared" / "sibling_guard.py", "sibling_guard", monkeypatch)
        wires.append(load(root / "models" / "machine.py", "machine", monkeypatch)
                     .module().to_wire_json())
    assert wires[0] == wires[1]
    assert files(wires[0]) >= {"models/machine.py", "shared/sibling_guard.py"}
    assert not any("outside" in f or str(tmp_path).lstrip("/") in f for f in files(wires[0]))


HELPER = """
from behavior import count, select


def total(entity):
    return count(select(entity))
"""

READ = """
import total_helper
from behavior import read

from machine import Order


@read
def orders():
    return total_helper.total(Order)
"""


def read_record_ids(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, read_src: str,
                    read_file: str | None) -> list[str]:
    """The record identity of ad-hoc read `orders` from two checkouts. The read is loaded from
    a real file, or run through `exec` when `read_file` is None; its helper is a real file."""
    machine = load(EXAMPLE, "machine", monkeypatch)
    m = machine.module()
    ids = []
    for where in CHECKOUTS:
        root = checkout(tmp_path / where, {"lib/total_helper.py": HELPER, "reads.py": read_src})
        load(root / "lib" / "total_helper.py", "total_helper", monkeypatch)
        if read_file is None:
            namespace: dict[str, Any] = {"__name__": "adhoc"}
            exec(compile(read_src, "<string>", "exec"), namespace)
            orders = namespace["orders"]
        else:
            orders = load(root / read_file, "reads", monkeypatch).orders
        store = Store.create(InMemoryBackend(), m, Store.genesis_for(m, []))
        ids.append(store.read(m, orders).record_id)
    return ids


def test_an_ad_hoc_read_from_real_files_has_one_record_identity(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    ids = read_record_ids(tmp_path, monkeypatch, READ, "reads.py")
    assert ids[0] == ids[1]


# --- receipts: markers excluded from the base (behavior-models) --------------------------

TRANSITIONS = """
from behavior import BehaviorModule


def add(machine, entities, enums, draft, submitted):
    machine.transition("submit", from_={draft}, to=submitted)
    return BehaviorModule(entities=entities, enums=enums, models=[machine])
"""

EXEC_MACHINE = """
from enum import Enum

from behavior import StateMachine, entity, field


class OrderStatus(Enum):
    DRAFT = "DRAFT"
    SUBMITTED = "SUBMITTED"


@entity
class Order:
    status = field(OrderStatus)


machine = StateMachine(
    "order", subject=Order, state_field="status", entry_states={OrderStatus.DRAFT}
)
built = transitions.add(machine, [Order], [OrderStatus], OrderStatus.DRAFT,
                        OrderStatus.SUBMITTED)
"""


def exec_machines(tmp_path: Path) -> list[Any]:
    """A StateMachine constructed through `exec` (`<string>`) whose transition is added from a
    real file, built without a root, from two checkouts."""
    built = []
    for where in CHECKOUTS:
        root = checkout(tmp_path / where, {"lib/transitions.py": TRANSITIONS})
        namespace: dict[str, Any] = {
            "__name__": "exec_machine",
            "transitions": load(root / "lib" / "transitions.py", "transitions"),
        }
        exec(compile(EXEC_MACHINE, "<string>", "exec"), namespace)
        built.append(namespace["built"])
    return built


def test_receipts_of_mixed_sources_are_checkout_independent(tmp_path: Path) -> None:
    receipts = [json.dumps(m.model_receipts[0].source_map, sort_keys=True)
                for m in exec_machines(tmp_path)]
    assert receipts[0] == receipts[1]
    assert str(tmp_path).lstrip("/") not in receipts[0], receipts[0]


# --- mixed markers and real files: the Core limitation ------------------------------------

def test_mixed_locations_never_change_the_module_hash(tmp_path: Path) -> None:
    """Locations are outside semantics: the hashes agree even where the wire does not."""
    first, second = exec_machines(tmp_path)
    assert first.behavior_version == second.behavior_version


def test_an_explicit_root_makes_mixed_modules_checkout_independent(tmp_path: Path) -> None:
    """The documented way around the Core limitation for modules: `root=`."""
    wires = []
    for where in CHECKOUTS:
        shutil.copytree(EXAMPLE.parent, tmp_path / where / "example")
        mod = load(tmp_path / where / "example" / "machine.py", "machine")
        m = mod.BehaviorModule(entities=[mod.Order], enums=[mod.OrderStatus],
                               models=[mod.order_machine()], root=str(tmp_path / where))
        wires.append(m.to_wire_json())
    assert wires[0] == wires[1]
    assert files(wires[0]) == {"example/machine.py", "<machine>"}


@CORE_31
def test_mixed_module_locations_are_checkout_independent(tmp_path: Path) -> None:
    """An `exec`'d machine (`<string>`) next to a real file, without a root."""
    wires = [m.to_wire_json() for m in exec_machines(tmp_path)]
    assert wires[0] == wires[1]


@CORE_31
def test_a_mixed_ad_hoc_read_has_one_record_identity(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """An ad-hoc read run through `exec` (`<string>`) that calls a helper in a real file."""
    ids = read_record_ids(tmp_path, monkeypatch, READ, None)
    assert ids[0] == ids[1]


EXEC_QUERY = """
from behavior import select


def all_orders(entity):
    return select(entity)
"""

PROJECTING_READ = """
import query_helper
from behavior import project, read

from machine import Order


@read
def projected():
    return project(query_helper.all_orders(Order), lambda o: [o.status])
"""


@CORE_31
def test_a_real_read_projecting_an_exec_query_is_checkout_independent(
        tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A real-file read whose projected query comes from an `exec`-defined helper."""
    helper = types.ModuleType("query_helper")
    exec(compile(EXEC_QUERY, "<string>", "exec"), helper.__dict__)
    monkeypatch.setitem(sys.modules, "query_helper", helper)
    machine = load(EXAMPLE, "machine", monkeypatch)
    m = machine.module()
    ids = []
    for where in CHECKOUTS:
        root = checkout(tmp_path / where, {"reads.py": PROJECTING_READ})
        projected = load(root / "reads.py", "reads", monkeypatch).projected
        store = Store.create(InMemoryBackend(), m, Store.genesis_for(m, []))
        ids.append(store.read(m, projected).record_id)
    assert ids[0] == ids[1]
