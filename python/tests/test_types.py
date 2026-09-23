"""The DSL's typing rules agree with the engine's (tests/fixtures/typing_cases.json)."""

from __future__ import annotations

import json
from decimal import Decimal
from typing import Any

import pytest

from behavior import types as T
from behavior.errors import BehaviorTypeError

from .conftest import FIXTURES

DOC = json.loads((FIXTURES / "typing_cases.json").read_text())
PRIMS = {"int": int, "decimal": Decimal, "string": str, "bool": bool}


def build_env(env: dict[str, Any]) -> dict[str, T.BType]:
    out: dict[str, T.BType] = {}
    for n in env["nominals"]:
        out[n["name"]] = T.nominal(n["name"], PRIMS[n["underlying"]["t"]], ops=set(n["ops"]))
    for e in env["enums"]:
        out[e["name"]] = T.EnumT(e["name"], tuple(e["values"]))
    return out


ENV = build_env(DOC["env"])


def from_wire(w: dict[str, Any]) -> T.BType:
    t = w["t"]
    if t in PRIMS:
        return {"int": T.INT, "decimal": T.DECIMAL, "string": T.STRING, "bool": T.BOOL}[t]
    if t == "option":
        return T.OptionT(from_wire(w["of"]))
    if t in ("nominal", "enum"):
        return ENV[w["name"]]
    if t == "id":
        return T.IdT(w["entity"])
    raise AssertionError(t)


@pytest.mark.parametrize("case", DOC["cases"], ids=lambda c: f"{c['op']}-{len(c['operands'])}")
def test_typing_case(case: dict[str, Any]) -> None:
    operands = [from_wire(o) for o in case["operands"]]
    try:
        if case["op"] == "check_type":
            T.check_type(operands[0])
            got: Any = operands[0].wire()
        else:
            target = ENV.get(case.get("target", ""))
            result, _convs = T.type_of_op(
                case["op"], operands, count=case.get("values"), target=target
            )
            got = result.wire()
    except BehaviorTypeError as e:
        got = {"error": e.code}
    assert got == case["expect"], case
